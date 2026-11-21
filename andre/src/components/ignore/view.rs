//! Drawing for the ignore editor: the file tabs, the list of lines and the detail of the
//! cursor's line. Everything here draws from the editor's state or builds from it. Nothing
//! reads the disk.

use std::borrow::Cow;

use andre_core::IgnoreFile;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::{IgnoreDraft, IgnoreEditComponent};
use crate::theme::ThemeColors;
use crate::ui::chips::{chip_line, Chip};
use crate::ui::detail::Detail;
use crate::ui::selection::{row, row_with_glyphs};
use crate::ui::text_field::field_spans;
use crate::ui::{elide, plural};

/// Most matched file names the detail lists. The rest are counted.
const FILES_SHOWN: usize = 8;
/// Widest a line may be in the detail's title. A longer line is cut with an ellipsis.
const TITLE_WIDTH: usize = 40;

/// The file tabs. Each shows how many patterns the file holds, or `(none)` when it does not
/// exist yet.
pub fn tab_chips(files: &[IgnoreDraft], active: usize) -> Vec<Chip> {
    files
        .iter()
        .enumerate()
        .map(|(index, draft)| Chip {
            label: chip_label(draft),
            active: index == active,
            key: KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE),
        })
        .collect()
}

pub fn render_chips(
    frame: &mut Frame,
    area: Rect,
    colors: &ThemeColors,
    files: &[IgnoreDraft],
    active: usize,
) {
    let chips = tab_chips(files, active);
    frame.render_widget(
        Paragraph::new(chip_line(&chips, Vec::new(), colors, area.width)),
        area,
    );
}

fn chip_label(draft: &IgnoreDraft) -> String {
    let name = draft.kind.file_name();
    match draft.lines {
        Some(_) => format!("{name} {}", pattern_count(draft.lines())),
        None => format!("{name} (none)"),
    }
}

/// The list's title: the file name, with `· unsaved ●` when the lines differ from the file.
pub fn list_title(draft: &IgnoreDraft) -> String {
    let name = draft.kind.file_name();
    if draft.is_dirty() {
        format!("{name} \u{b7} unsaved \u{25cf}")
    } else {
        name.to_string()
    }
}

/// The rows of the list: each line, the line being typed (when it is new), then the add row.
/// The cursor's row is drawn with the selection fill.
pub fn list_rows(
    editor: &IgnoreEditComponent,
    colors: &ThemeColors,
    width: u16,
) -> Vec<Line<'static>> {
    let cursor = editor.list.cursor;
    let draft = editor.current();
    let typing = editor.editing.as_ref();
    let mut rows = Vec::new();
    if draft.lines.is_none() {
        return rows;
    }
    for (index, line) in draft.lines().iter().enumerate() {
        let selected = index == cursor;
        match typing {
            Some(edit) if !edit.new && edit.row == index => {
                let content = field_spans(&edit.field, true, colors);
                rows.push(row_with_glyphs(
                    Vec::new(),
                    content,
                    selected,
                    colors,
                    width,
                ));
            }
            _ => rows.push(row(line_content(line, colors), selected, colors, width)),
        }
    }
    let count = draft.lines().len();
    if let Some(edit) = typing.filter(|edit| edit.new) {
        let content = field_spans(&edit.field, true, colors);
        rows.push(row_with_glyphs(
            Vec::new(),
            content,
            count == cursor,
            colors,
            width,
        ));
    }
    let add = rows.len();
    let dim = Style::default().fg(colors.text_dim);
    rows.push(row(
        vec![Span::styled("+ add a pattern", dim)],
        add == cursor,
        colors,
        width,
    ));
    rows
}

/// A line's text: comments in `text_dim`, patterns in `text`.
fn line_content(line: &str, colors: &ThemeColors) -> Vec<Span<'static>> {
    let style = if is_comment(line) {
        Style::default().fg(colors.text_dim)
    } else {
        Style::default().fg(colors.text)
    };
    vec![Span::styled(line.to_string(), style)]
}

/// Lines that match something: not blank, not a comment.
pub fn pattern_count(lines: &[String]) -> usize {
    lines.iter().filter(|line| is_pattern(line)).count()
}

fn is_pattern(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !is_comment(line)
}

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// The detail's title for the line `text` (already trimmed).
pub fn line_title(text: &str) -> String {
    if text.is_empty() {
        "blank line".to_string()
    } else if is_comment(text) {
        "comment".to_string()
    } else {
        elide(text, TITLE_WIDTH)
    }
}

/// The detail for a line of `kind`. `text` is trimmed. `matches` is the package files the
/// line matches, or the reason it does not compile. Every line's detail says what the file
/// does, so the `.stow-local-ignore` warning shows on any of its rows.
pub fn line_detail(
    colors: &ThemeColors,
    kind: IgnoreFile,
    text: &str,
    matches: Result<Vec<String>, String>,
) -> Detail {
    let mut headline = explain(colors, kind);
    if text.is_empty() || is_comment(text) {
        return Detail {
            headline,
            ..Detail::default()
        };
    }

    let mut detail = Detail::default();
    if let Err(reason) = &matches {
        let err = Style::default().fg(colors.err);
        detail
            .headline
            .push(Line::styled(format!("invalid pattern: {reason}"), err));
        detail.headline.push(Line::raw(""));
    }
    detail.headline.append(&mut headline);
    detail
        .facts
        .push((Cow::Borrowed("pattern"), text.to_string()));
    if let Ok(files) = matches {
        detail.facts.push((
            Cow::Borrowed("matches"),
            format!(
                "{} {} in this package",
                files.len(),
                plural(files.len(), "file", "files")
            ),
        ));
        detail.sections = matched_files(colors, &files);
    }
    detail
}

/// The detail for a file rather than a line: what the file does, and how many patterns it
/// holds. A file that does not exist says so and how `+` creates it.
pub fn file_detail(colors: &ThemeColors, draft: &IgnoreDraft) -> Detail {
    let mut detail = Detail {
        headline: explain(colors, draft.kind),
        ..Detail::default()
    };
    match &draft.lines {
        Some(_) => detail.facts.push((
            Cow::Borrowed("patterns"),
            pattern_count(draft.lines()).to_string(),
        )),
        None => {
            detail.headline.push(Line::raw(""));
            detail.headline.push(Line::styled(
                "no file yet. + creates it from the starter lines.",
                Style::default().fg(colors.text_dim),
            ));
        }
    }
    detail
}

/// What the file is for, in the words the detail shows on every row.
fn explain(colors: &ThemeColors, kind: IgnoreFile) -> Vec<Line<'static>> {
    let text = Style::default().fg(colors.text);
    let dim = Style::default().fg(colors.text_dim);
    match kind {
        IgnoreFile::GitIgnore => vec![
            Line::styled(
                "Git ignores matching files. andre never reads this file for linking.",
                text,
            ),
            Line::styled("The match is a glob preview; git decides for itself.", dim),
        ],
        IgnoreFile::StowLocal => vec![
            Line::styled(
                "Files that match are never linked. One Perl regex per line.",
                text,
            ),
            Line::raw(""),
            Line::styled(
                "! This file REPLACES the default ignore list for this package.",
                Style::default().fg(colors.warn),
            ),
        ],
    }
}

/// The matched files as section rows, with the rest counted.
fn matched_files(colors: &ThemeColors, files: &[String]) -> Vec<Line<'static>> {
    let text = Style::default().fg(colors.text);
    let mut rows: Vec<Line<'static>> = files
        .iter()
        .take(FILES_SHOWN)
        .map(|file| Line::styled(format!("\u{b7} {file}"), text))
        .collect();
    if files.len() > FILES_SHOWN {
        rows.push(Line::styled(
            format!("and {} more", files.len() - FILES_SHOWN),
            Style::default().fg(colors.text_dim),
        ));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_count_skips_blank_lines_and_comments() {
        let lines: Vec<String> = ["# head", "", "  ", "*.env", "  .env*  "]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(pattern_count(&lines), 2);
    }

    #[test]
    fn line_title_names_blank_and_comment_lines() {
        assert_eq!(line_title(""), "blank line");
        assert_eq!(line_title("# secrets"), "comment");
        assert_eq!(line_title("*.env"), "*.env");
    }
}
