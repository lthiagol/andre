//! The unlink screen's parts: the link as the one row of its list, and its detail. The
//! component reads the link target once, so paint never touches the disk.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::theme::ThemeColors;
use crate::ui::detail::{text_lines, Detail};
use crate::ui::display_path;
use crate::ui::selection::row_with_glyphs;

/// The symlink an unstow confirm asks about, with the path it points at.
pub struct UnstowLink {
    pub path: PathBuf,
    /// What the link points at, or `(unresolvable)`.
    pub target: String,
}

/// Where the unlink stands.
pub enum Outcome<'a> {
    /// Waiting for `y`.
    Pending,
    Removed,
    Failed(&'a str),
}

/// The link's file name, with the symlink mark `~`.
pub fn link_row(
    link: &UnstowLink,
    selected: bool,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let name = link.path.file_name().map_or_else(
        || link.path.display().to_string(),
        |n| n.to_string_lossy().into_owned(),
    );
    row_with_glyphs(
        vec![Span::styled("~", Style::default().fg(colors.text_dim))],
        vec![Span::styled(
            format!(" {name}"),
            Style::default().fg(colors.text),
        )],
        selected,
        colors,
        width,
    )
}

/// The link's detail: what is removed and what it points at, then the outcome.
pub fn link_detail(
    link: &UnstowLink,
    home: &Path,
    outcome: Outcome,
    colors: &ThemeColors,
) -> Detail {
    let path = display_path(&link.path, home);
    let target = display_path(Path::new(&link.target), home);
    let keep = Line::styled(
        "The source file stays in the package.",
        Style::default().fg(colors.text_dim),
    );
    match outcome {
        Outcome::Pending => Detail {
            headline: Vec::new(),
            facts: vec![(Cow::Borrowed("remove"), path)],
            sections: vec![
                Line::styled(
                    format!("points to {target}"),
                    Style::default().fg(colors.text),
                ),
                Line::raw(""),
                keep,
            ],
        },
        Outcome::Removed => Detail {
            headline: vec![Line::styled(
                "\u{2713} removed",
                Style::default().fg(colors.ok),
            )],
            facts: vec![(Cow::Borrowed("remove"), path)],
            sections: vec![keep],
        },
        Outcome::Failed(message) => Detail {
            headline: vec![Line::styled(
                "\u{2717} failed",
                Style::default().fg(colors.err),
            )],
            facts: vec![(Cow::Borrowed("remove"), path)],
            sections: text_lines(message, Style::default().fg(colors.text)),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A failed unlink keeps each line of its message on its own row.
    #[test]
    fn test_failed_message_is_one_section_row_per_line() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let link = UnstowLink {
            path: PathBuf::from("/home/u/.bashrc"),
            target: "(unresolvable)".into(),
        };
        let detail = link_detail(
            &link,
            Path::new("/home/u"),
            Outcome::Failed("a: x\nAll operations aborted."),
            &colors,
        );
        let rows: Vec<String> = detail
            .sections
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert_eq!(rows, ["a: x", "All operations aborted."]);
    }
}
