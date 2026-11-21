//! The run screen's parts: the progress chip, the result rows and the result detail. The
//! component builds the rows and the detail; these functions lay them out.

use std::borrow::Cow;

use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use crate::execute::ExecutionResult;
use crate::theme::ThemeColors;
use crate::ui::detail::{text_lines, Detail};
use crate::ui::selection::row_with_glyphs;

const SPINNER_CHARS: &[char] = &[
    '\u{28FB}', '\u{28FC}', '\u{28FD}', '\u{28FE}', '\u{28FF}', '\u{28F7}', '\u{28EF}', '\u{28DF}',
    '\u{28BF}', '\u{287F}',
];

/// The widest the progress bar gets. It shrinks before the counts do.
const BAR_MAX: usize = 40;

/// The spinner glyph for `frame`.
pub fn spinner(frame: usize) -> char {
    SPINNER_CHARS[frame % SPINNER_CHARS.len()]
}

/// A skip never ran: its message starts `Skipped`.
pub fn is_skipped(result: &ExecutionResult) -> bool {
    !result.success
        && result
            .error_message
            .as_deref()
            .is_some_and(|m| m.starts_with("Skipped"))
}

/// `  stow  ████░░░░  2/3  ⣼`: the action, a bar, the count and the spinner.
pub fn progress_line(
    action: &str,
    done: usize,
    total: usize,
    spinner: char,
    width: u16,
    colors: &ThemeColors,
) -> Line<'static> {
    let prefix = format!("  {action}  ");
    let suffix = format!("  {done}/{total}  {spinner}");
    let room = usize::from(width).saturating_sub(prefix.width() + suffix.width());
    let bar = room.min(BAR_MAX);
    let filled = (done.min(total) * bar).checked_div(total).unwrap_or(0);
    Line::from(vec![
        Span::styled(prefix, Style::default().fg(colors.text)),
        Span::styled(
            "\u{2588}".repeat(filled),
            Style::default().fg(colors.accent),
        ),
        Span::styled(
            "\u{2591}".repeat(bar - filled),
            Style::default().fg(colors.text_dim),
        ),
        Span::styled(suffix, Style::default().fg(colors.text)),
    ])
}

/// `  done  ✓ 2 succeeded  ✗ 1 failed`, with skips counted last when there are any.
pub fn summary_line(results: &[ExecutionResult], colors: &ThemeColors) -> Line<'static> {
    let skipped = results.iter().filter(|r| is_skipped(r)).count();
    let succeeded = results.iter().filter(|r| r.success).count();
    let failed = results.len() - succeeded - skipped;
    let mut spans = vec![
        Span::styled("  done  ", Style::default().fg(colors.text)),
        Span::styled(
            format!("\u{2713} {succeeded} succeeded"),
            Style::default().fg(colors.ok),
        ),
        Span::styled(
            format!("  \u{2717} {failed} failed"),
            Style::default().fg(colors.err),
        ),
    ];
    if skipped > 0 {
        spans.push(Span::styled(
            format!("  \u{2205} {skipped} skipped"),
            Style::default().fg(colors.text_dim),
        ));
    }
    Line::from(spans)
}

/// One finished result: its mark and `group/package`.
pub fn result_row(
    result: &ExecutionResult,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let (mark, color) = if result.success {
        ("\u{2713}", colors.ok)
    } else if is_skipped(result) {
        ("\u{2205}", colors.text_dim)
    } else {
        ("\u{2717}", colors.err)
    };
    row_with_glyphs(
        vec![Span::styled(mark, Style::default().fg(color))],
        vec![Span::styled(
            format!(" {}/{}", result.group, result.package),
            Style::default().fg(colors.text),
        )],
        selected,
        colors,
        width,
    )
}

/// The package that is running now, with the spinner in front of it.
pub fn running_row(
    spinner: char,
    label: &str,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    row_with_glyphs(
        vec![Span::styled(
            spinner.to_string(),
            Style::default().fg(colors.accent),
        )],
        vec![Span::styled(
            format!(" {label}"),
            Style::default().fg(colors.text),
        )],
        selected,
        colors,
        width,
    )
}

/// The detail of a finished result: its outcome, and for a failure or skip the message and
/// a fix when the message is one we know.
pub fn result_detail(result: &ExecutionResult, colors: &ThemeColors) -> Detail {
    let (headline, color) = if result.success {
        ("\u{2713} succeeded", colors.ok)
    } else if is_skipped(result) {
        ("\u{2205} skipped", colors.text_dim)
    } else {
        ("\u{2717} failed", colors.err)
    };
    let mut sections = Vec::new();
    if let Some(message) = result.error_message.as_deref().filter(|_| !result.success) {
        sections.extend(text_lines(message, Style::default().fg(colors.text)));
        if let Some(fix) = fix_hint(message) {
            sections.push(Line::raw(""));
            sections.push(Line::styled(fix, Style::default().fg(colors.text_dim)));
        }
    }
    Detail {
        headline: vec![Line::styled(headline, Style::default().fg(color))],
        facts: Vec::new(),
        sections,
    }
}

/// The fix for a real file where a link should go. Both engines say so, in their own words.
const REAL_FILE_HINT: &str =
    "A real file sits where the link goes. Move it into the package, or delete it.";

/// What to do about a failure we know, read from its message. Native and GNU stow word
/// their conflicts differently; both kinds get a hint.
pub fn fix_hint(message: &str) -> Option<&'static str> {
    if message.contains("is not a symlink")
        || message.contains("neither a link nor a directory")
        || message.contains("not owned by stow")
    {
        Some(REAL_FILE_HINT)
    } else if message.contains("foreign or dangling symlink") {
        Some("A link that is not this package's sits where the link goes. Remove it, then run again.")
    } else {
        None
    }
}

/// The detail of a run that has not finished a package yet.
pub fn running_detail(spinner: char, label: String, colors: &ThemeColors) -> Detail {
    Detail {
        headline: vec![Line::styled(
            format!("{spinner} running"),
            Style::default().fg(colors.accent),
        )],
        facts: vec![(Cow::Borrowed("package"), label)],
        sections: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn failed(message: &str) -> ExecutionResult {
        ExecutionResult {
            group: "home".into(),
            package: "shell".into(),
            success: false,
            error_message: Some(message.into()),
        }
    }

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn test_native_real_file_message_gets_the_real_file_hint() {
        let message = "target already exists and is not a symlink: /home/u/.bashrc";
        assert_eq!(fix_hint(message), Some(REAL_FILE_HINT));
    }

    #[test]
    fn test_native_foreign_symlink_message_gets_the_foreign_hint() {
        let message = "target already exists (foreign or dangling symlink): /home/u/.vimrc";
        assert!(fix_hint(message).unwrap().starts_with("A link that is not"));
    }

    #[test]
    fn test_stow_neither_a_link_nor_a_directory_gets_the_real_file_hint() {
        let message = "WARNING! stowing shell would cause conflicts:\n  * existing target is neither a link nor a directory: .bashrc\nAll operations aborted.";
        assert_eq!(fix_hint(message), Some(REAL_FILE_HINT));
    }

    #[test]
    fn test_stow_cannot_stow_over_a_real_file_gets_the_real_file_hint() {
        let message =
            "cannot stow shell over existing target .bashrc since neither a link nor a directory";
        assert_eq!(fix_hint(message), Some(REAL_FILE_HINT));
    }

    #[test]
    fn test_stow_not_owned_by_stow_gets_the_real_file_hint_the_mockup_shows() {
        let message = "existing target is not owned by stow: .config/kitty/kitty.conf";
        assert_eq!(fix_hint(message), Some(REAL_FILE_HINT));
    }

    #[test]
    fn test_unknown_failure_gets_no_hint() {
        assert_eq!(fix_hint("Operation timed out after 60s"), None);
    }

    /// Each line of an engine's message keeps its own row, so "All operations aborted."
    /// does not join the line before it.
    #[test]
    fn test_multi_line_message_is_one_section_row_per_line() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let detail = result_detail(&failed("a: x\nAll operations aborted."), &colors);
        let rows: Vec<String> = detail.sections.iter().map(text).collect();
        assert_eq!(rows, ["a: x", "All operations aborted."]);
    }
}
