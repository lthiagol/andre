//! The unlink screen's parts: the link as the one row of its list, and its detail. The
//! component reads the link target once, so paint never touches the disk.

use std::borrow::Cow;
use std::path::{Path, PathBuf};

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::theme::ThemeColors;
use crate::ui::detail::{text_lines, wrap_value, Detail, VALUE_WIDTH};
use crate::ui::display_path;
use crate::ui::selection::row_with_glyphs;

/// The symlink an unstow confirm asks about, with the path it points at.
pub struct UnstowLink {
    pub path: PathBuf,
    /// What the link points at, or `(unresolvable)`.
    pub target: String,
    /// The package the link points into, when it points into the packages folder.
    pub package: Option<String>,
    /// The target from the packages folder, such as `shell/.bashrc`, when it points into it.
    pub inside: Option<String>,
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
    // Restow puts the link back; it needs the package's name.
    let stays = match &link.package {
        Some(package) => {
            format!("The source file stays in the package. Restow {package} to put the link back.")
        }
        None => "The source file stays in the package.".to_string(),
    };
    let keep = Line::styled(stays, Style::default().fg(colors.text_dim));
    match outcome {
        Outcome::Pending => {
            // Inside the packages folder the target reads from there; elsewhere, as typed.
            let points_to = link.inside.clone().unwrap_or(target);
            let mut facts = vec![(Cow::Borrowed("remove"), path)];
            // The label sits on the first row; the rest of the target wraps under the value.
            for (i, row) in wrap_value(&points_to, VALUE_WIDTH).into_iter().enumerate() {
                let label = if i == 0 { "points to" } else { "" };
                facts.push((Cow::Borrowed(label), row));
            }
            Detail {
                headline: Vec::new(),
                facts,
                sections: vec![keep],
            }
        }
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

    /// The keep line names the package, so the user knows what to restow.
    #[test]
    fn test_keep_line_names_the_package_to_restow() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let link = UnstowLink {
            path: PathBuf::from("/home/u/.bashrc"),
            target: "/home/u/dotfiles/shell/.bashrc".into(),
            package: Some("shell".into()),
            inside: Some("shell/.bashrc".into()),
        };
        let detail = link_detail(&link, Path::new("/home/u"), Outcome::Pending, &colors);
        let keep: String = detail.sections[0]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert_eq!(
            keep,
            "The source file stays in the package. Restow shell to put the link back."
        );
    }

    /// Pending, `points to` is a fact under `remove`: the target from the packages folder
    /// when it points into one, else the target as typed.
    #[test]
    fn test_pending_points_to_is_a_fact_under_remove() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let inside = UnstowLink {
            path: PathBuf::from("/home/u/.bashrc"),
            target: "/home/u/dotfiles/test/.bashrc".into(),
            package: Some("test".into()),
            inside: Some("test/.bashrc".into()),
        };
        let detail = link_detail(&inside, Path::new("/home/u"), Outcome::Pending, &colors);
        let facts: Vec<(&str, &str)> = detail
            .facts
            .iter()
            .map(|(label, value)| (label.as_ref(), value.as_str()))
            .collect();
        assert_eq!(
            facts,
            [("remove", "~/.bashrc"), ("points to", "test/.bashrc")]
        );
        assert_eq!(
            detail.sections.len(),
            1,
            "the keep line is the only section"
        );

        let outside = UnstowLink {
            path: PathBuf::from("/home/u/.profile"),
            target: "/etc/profile".into(),
            package: None,
            inside: None,
        };
        let detail = link_detail(&outside, Path::new("/home/u"), Outcome::Pending, &colors);
        assert_eq!(detail.facts[1].1, "/etc/profile");
    }

    /// A `points to` longer than the value column wraps onto `""`-labelled rows under it.
    #[test]
    fn test_pending_points_to_wraps_under_its_value_column() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let link = UnstowLink {
            path: PathBuf::from("/home/u/.config/terminal-emulator/config.toml"),
            target: "/home/u/dotfiles/terminal-emulator/.config/terminal-emulator/config.toml"
                .into(),
            package: Some("terminal-emulator".into()),
            inside: Some("terminal-emulator/.config/terminal-emulator/config.toml".into()),
        };
        let detail = link_detail(&link, Path::new("/home/u"), Outcome::Pending, &colors);
        let facts: Vec<(&str, &str)> = detail
            .facts
            .iter()
            .map(|(label, value)| (label.as_ref(), value.as_str()))
            .collect();
        assert_eq!(
            facts,
            [
                ("remove", "~/.config/terminal-emulator/config.toml"),
                ("points to", "terminal-emulator/.config/"),
                ("", "terminal-emulator/"),
                ("", "config.toml"),
            ]
        );
    }

    /// A failed unlink keeps each line of its message on its own row.
    #[test]
    fn test_failed_message_is_one_section_row_per_line() {
        let colors = ThemeColors::from_theme(andre_core::Theme::Default);
        let link = UnstowLink {
            path: PathBuf::from("/home/u/.bashrc"),
            target: "(unresolvable)".into(),
            package: None,
            inside: None,
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
