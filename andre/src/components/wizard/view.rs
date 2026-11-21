//! Paint for the wizard. Every function reads the wizard's state and draws it; none changes
//! it. Details are built from that state and the config's paths, which need no disk reads.

use std::borrow::Cow;
use std::path::Path;

use andre_core::config::Config;
use ratatui::{
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::picker::{refusal, FilePicker};
use super::{NameRow, NewPackageWizard, Source, Step, SOURCES, STEPS};
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::icons::Icons;
use crate::ui::layout::{screen_rects, ScreenRects, Shape};
use crate::ui::list_panel::{self, render_focused_list_panel, render_list_panel, Empty, ListView};
use crate::ui::plural;
use crate::ui::selection::{row, row_with_glyphs};
use crate::ui::step_bar::{step_line, StepBar};
use crate::ui::text_field::{field_line, TextField};
use crate::ui::{display_path, elide};

/// Rows the name field takes at the top of the name step's list pane: border, field, border.
/// A refused name takes one more, for its error.
const FIELD_ROWS: u16 = 3;

/// The name step's two panels: the field on top, the matching rows below it.
pub fn name_panels(list: Rect, error: bool) -> (Rect, Rect) {
    let field = Rect {
        height: (FIELD_ROWS + u16::from(error)).min(list.height),
        ..list
    };
    let matches = Rect {
        y: list.y + field.height,
        height: list.height - field.height,
        ..list
    };
    (field, matches)
}

pub fn render(frame: &mut Frame, area: Rect, ctx: &AppContext, wizard: &NewPackageWizard) {
    let colors = ThemeColors::from_theme(ctx.core.theme);
    let icons = Icons::for_set(ctx.core.icons);
    let rects = screen_rects(area, Shape::Wizard);
    if let Some(bar) = rects.chip {
        let steps = StepBar {
            steps: STEPS,
            current: wizard.step.bar_index(),
        };
        frame.render_widget(Paragraph::new(step_line(&steps, icons, &colors)), bar);
    }
    match wizard.step {
        Step::Group => render_group(frame, rects, ctx, wizard, &colors),
        Step::Name => render_name(frame, rects, ctx, wizard, &colors),
        Step::Source => render_source(frame, rects, ctx, wizard, &colors, icons),
        Step::Files => render_files(frame, rects, ctx, wizard, &colors, icons),
    }
}

fn render_group(
    frame: &mut Frame,
    rects: ScreenRects,
    ctx: &AppContext,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
) {
    let groups = wizard.groups();
    let width = list_panel::inner(rects.list).width;
    let rows = groups
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let text = Style::default().fg(colors.text);
            let content = vec![Span::styled(name.to_string(), text)];
            row(content, i == wizard.group_list.cursor, colors, width)
        })
        .collect();
    let empty = Empty {
        message: "no groups in andre.yml",
        hint: None,
    };
    render_list_panel(
        frame,
        rects.list,
        colors,
        "group",
        rows,
        &wizard.group_list,
        Some(empty),
    );

    let (title, detail) = match groups.get(wizard.group_list.cursor) {
        Some(name) => {
            let packages = wizard
                .listing
                .get(*name)
                .map(Vec::as_slice)
                .unwrap_or_default();
            (name.to_string(), group_detail(ctx, colors, name, packages))
        }
        None => ("group".to_string(), no_group_detail(colors)),
    };
    render_detail(frame, rects.detail, colors, &title, &detail);
}

fn render_name(
    frame: &mut Frame,
    rects: ScreenRects,
    ctx: &AppContext,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
) {
    let (field, matches) = name_panels(rects.list, wizard.name.error.is_some());
    // The error gets a row of its own: at 32 columns it would not fit after the field.
    let plain = TextField {
        error: None,
        ..wizard.name.clone()
    };
    let mut lines = vec![gutter(field_line("name", &plain, true, colors))];
    if let Some(error) = &wizard.name.error {
        let err = Style::default().fg(colors.err);
        lines.push(gutter(Line::styled(error.clone(), err)));
    }
    render_focused_list_panel(
        frame,
        field,
        colors,
        true,
        "name",
        lines,
        &ListView::default(),
        None,
    );

    let names = wizard.name_rows();
    let width = list_panel::inner(matches).width;
    let rows = names
        .iter()
        .enumerate()
        .map(|(i, name_row)| {
            name_row_line(
                name_row,
                i == wizard.name_list.cursor,
                wizard,
                colors,
                width,
            )
        })
        .collect();
    let existing = names.len().saturating_sub(1);
    let title = format!("matches {existing}");
    render_list_panel(
        frame,
        matches,
        colors,
        &title,
        rows,
        &wizard.name_list,
        None,
    );

    let detail = name_detail(ctx, colors, wizard);
    render_detail(frame, rects.detail, colors, "new package", &detail);
}

/// A line with the panel's 1-column gutter in front, as the list rows have.
fn gutter(line: Line<'static>) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    spans.extend(line.spans);
    Line::from(spans)
}

/// One row of the name step: the create row, or an existing package the text matches.
fn name_row_line(
    name_row: &NameRow,
    selected: bool,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let text = Style::default().fg(colors.text);
    let dim = Style::default().fg(colors.text_dim);
    let value = Style::default().fg(colors.value);
    let typed = format!("`{}`", wizard.name.value);
    let content = match name_row {
        NameRow::Create if wizard.name_exists() => {
            vec![
                Span::styled("→ add files to ", text),
                Span::styled(typed, value),
            ]
        }
        NameRow::Create if wizard.name.value.is_empty() => vec![Span::styled("+ create", text)],
        NameRow::Create => vec![Span::styled("+ create ", text), Span::styled(typed, value)],
        NameRow::Existing(package) => vec![
            Span::styled("→ ", dim),
            Span::styled(package.clone(), text),
            Span::styled("  add files to it", dim),
        ],
    };
    row(content, selected, colors, width)
}

fn render_source(
    frame: &mut Frame,
    rects: ScreenRects,
    ctx: &AppContext,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    icons: &Icons,
) {
    let width = list_panel::inner(rects.list).width;
    let rows = SOURCES
        .iter()
        .enumerate()
        .map(|(i, source)| {
            // Enter takes the cursor's row, so the radio is the row the cursor is on.
            let on = i == wizard.source_list.cursor;
            let glyph = if on { icons.radio_on } else { icons.radio_off };
            let glyph_color = if on { colors.accent } else { colors.text_dim };
            row_with_glyphs(
                vec![Span::styled(
                    format!("{glyph} "),
                    Style::default().fg(glyph_color),
                )],
                vec![Span::styled(
                    source_label(*source),
                    Style::default().fg(colors.text),
                )],
                i == wizard.source_list.cursor,
                colors,
                width,
            )
        })
        .collect();
    render_list_panel(
        frame,
        rects.list,
        colors,
        "source",
        rows,
        &wizard.source_list,
        None,
    );

    let source = SOURCES
        .get(wizard.source_list.cursor)
        .copied()
        .unwrap_or(Source::Empty);
    let detail = source_detail(ctx, colors, wizard, source);
    render_detail(frame, rects.detail, colors, source_label(source), &detail);
}

fn render_files(
    frame: &mut Frame,
    rects: ScreenRects,
    ctx: &AppContext,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    icons: &Icons,
) {
    let Some(picker) = &wizard.picker else {
        return;
    };
    let width = list_panel::inner(rects.list).width;
    // Inside a marked folder every entry is marked with it: a dim box, not a mark of its own.
    let inherited = picker.is_covered();
    let dim = Style::default().fg(colors.text_dim);
    let rows = picker
        .entries()
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let marked = picker.is_marked(entry);
            let glyph_color = if marked {
                colors.accent
            } else {
                colors.text_dim
            };
            let name_color = if entry.is_selectable() {
                colors.text
            } else {
                colors.text_dim
            };
            let glyphs = vec![Span::styled(
                format!("{} ", icons.checkbox(marked || inherited)),
                Style::default().fg(glyph_color),
            )];
            let mut content = vec![
                Span::styled(format!("{} ", entry.kind_icon()), dim),
                Span::styled(entry.name.clone(), Style::default().fg(name_color)),
            ];
            if let Some(target) = picker.link_target(entry) {
                // The target takes what the row has left, elided at the end.
                let arrow = Span::styled("  \u{2192} ", dim);
                let used: usize = 1 + glyphs
                    .iter()
                    .chain(&content)
                    .chain(std::iter::once(&arrow))
                    .map(Span::width)
                    .sum::<usize>();
                let budget = usize::from(width).saturating_sub(used);
                content.push(arrow);
                let shown = elide(&display_path(target, &ctx.home_dir), budget);
                content.push(Span::styled(shown, dim));
            }
            row_with_glyphs(glyphs, content, i == picker.list.cursor, colors, width)
        })
        .collect();
    let title = display_path(&picker.browser.path, &ctx.home_dir);
    let empty = Empty {
        message: "nothing in this folder",
        hint: None,
    };
    render_list_panel(
        frame,
        rects.list,
        colors,
        &title,
        rows,
        &picker.list,
        Some(empty),
    );

    let title = picker
        .current()
        .map_or_else(|| "files".to_string(), |entry| entry.name.clone());
    let detail = files_detail(colors, wizard, picker);
    render_detail(frame, rects.detail, colors, &title, &detail);
}

fn source_label(source: Source) -> &'static str {
    match source {
        Source::Empty => "empty package",
        Source::LiveFiles => "move live files in",
    }
}

/// A group's facts: where its packages link from and to, and which packages it holds.
fn group_detail(
    ctx: &AppContext,
    colors: &ThemeColors,
    group: &str,
    packages: &[String],
) -> Detail {
    let config = &ctx.core.config;
    let source = config.get_group_source(group, &ctx.config_dir, &ctx.home_dir);
    let target = target_text(ctx, config, group);
    let listed = if packages.is_empty() {
        "none yet".to_string()
    } else {
        format!("{}: {}", packages.len(), packages.join(", "))
    };
    Detail {
        headline: vec![Line::styled(
            format!("Packages here link into {target}."),
            Style::default().fg(colors.text),
        )],
        facts: vec![
            (
                Cow::Borrowed("source"),
                source.map_or("not found".to_string(), |path| shown(ctx, &path)),
            ),
            (Cow::Borrowed("target"), target),
            (Cow::Borrowed("packages"), listed),
        ],
        sections: Vec::new(),
    }
}

fn no_group_detail(colors: &ThemeColors) -> Detail {
    Detail {
        headline: vec![Line::styled(
            "Groups come from andre.yml; none are set up yet.",
            Style::default().fg(colors.text_dim),
        )],
        ..Detail::default()
    }
}

fn name_detail(ctx: &AppContext, colors: &ThemeColors, wizard: &NewPackageWizard) -> Detail {
    let group = wizard.group.clone().unwrap_or_default();
    let name = wizard.name.value.as_str();
    let rows = wizard.name_rows();
    let headline = match rows.get(wizard.name_list.cursor) {
        Some(NameRow::Existing(package)) => format!("Adds files to {group}/{package}."),
        Some(NameRow::Create) if wizard.name_exists() => format!("Adds files to {group}/{name}."),
        Some(NameRow::Create) if name.is_empty() => {
            "Type a name, or pick a package that exists.".to_string()
        }
        Some(NameRow::Create) => format!("Creates {group}/{name}/, an empty package folder."),
        None => String::new(),
    };
    let package_dir = wizard
        .group
        .as_deref()
        .and_then(|group| {
            ctx.core
                .config
                .get_group_source(group, &ctx.config_dir, &ctx.home_dir)
        })
        .map(|source| source.join(name));
    let mut facts = Vec::new();
    if let Some(path) = package_dir {
        facts.push((Cow::Borrowed("path"), shown(ctx, &path)));
    }
    Detail {
        headline: vec![Line::styled(headline, Style::default().fg(colors.text))],
        facts,
        sections: vec![Line::styled(
            "Type to filter the packages here; pick one to add files to it.",
            Style::default().fg(colors.text_dim),
        )],
    }
}

fn source_detail(
    ctx: &AppContext,
    colors: &ThemeColors,
    wizard: &NewPackageWizard,
    source: Source,
) -> Detail {
    let text = Style::default().fg(colors.text);
    let dim = Style::default().fg(colors.text_dim);
    match source {
        Source::Empty => Detail {
            headline: vec![Line::styled(
                "An empty package folder. Nothing is moved or linked.",
                text,
            )],
            ..Detail::default()
        },
        Source::LiveFiles => {
            let target = wizard.group.as_deref().map_or_else(String::new, |group| {
                target_text(ctx, &ctx.core.config, group)
            });
            Detail {
                headline: vec![
                    Line::styled(format!("Pick files that already live in {target}."), text),
                    Line::styled(
                        "andre moves them into the package and links them back.",
                        text,
                    ),
                ],
                facts: Vec::new(),
                sections: vec![
                    Line::styled("Nothing is lost; the files keep working.", dim),
                    Line::styled(format!("Next: a file picker opens at {target}."), dim),
                ],
            }
        }
    }
}

fn files_detail(colors: &ThemeColors, wizard: &NewPackageWizard, picker: &FilePicker) -> Detail {
    let dim = Style::default().fg(colors.text_dim);
    let Some(entry) = picker.current() else {
        return Detail {
            headline: vec![Line::styled("Nothing in this folder to move.", dim)],
            ..Detail::default()
        };
    };
    let kind = if entry.is_dir {
        "folder".to_string()
    } else if entry.is_symlink {
        "symlink".to_string()
    } else if entry.is_artifact {
        "artifact".to_string()
    } else {
        match picker.current_size() {
            Some(len) => format!("file, {}", size_text(len)),
            None => "file".to_string(),
        }
    };
    let marked = picker.marks.len();
    let mut facts = vec![(Cow::Borrowed("kind"), kind)];
    let mut sections = vec![Line::styled("~ symlinks cannot be moved in.", dim)];
    if entry.is_selectable() {
        facts.push((Cow::Borrowed("moves to"), moves_to(wizard, picker)));
    } else if entry.is_artifact {
        sections.push(Line::styled(format!("{}.", refusal(entry)), dim));
    }
    facts.push((
        Cow::Borrowed("marked"),
        format!("{marked} {}", plural(marked, "item", "items")),
    ));
    Detail {
        headline: Vec::new(),
        facts,
        sections,
    }
}

/// Where the cursor's entry would land: `group/package/folder/` inside the package.
fn moves_to(wizard: &NewPackageWizard, picker: &FilePicker) -> String {
    let group = wizard.group.as_deref().unwrap_or_default();
    let package = format!("{group}/{}", wizard.name.value);
    match picker.relative_dir() {
        rel if rel.is_empty() => format!("{package}/"),
        rel => format!("{package}/{rel}/"),
    }
}

/// The group's target folder as the detail shows it, or `not found`.
fn target_text(ctx: &AppContext, config: &Config, group: &str) -> String {
    config
        .get_group_target(group, &ctx.config_dir, &ctx.home_dir)
        .map_or("not found".to_string(), |path| shown(ctx, &path))
}

/// A path with `~` for home, as the details show it.
fn shown(ctx: &AppContext, path: &Path) -> String {
    display_path(path, &ctx.home_dir)
}

/// A byte count as the details show it: `812 B`, `2.1 KB`, `3.5 MB`.
fn size_text(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KB", "MB", "GB", "TB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_panels_split_the_field_off_the_top_of_the_list_pane() {
        let list = Rect::new(0, 2, 34, 14);
        let (field, matches) = name_panels(list, false);

        assert_eq!(field, Rect::new(0, 2, 34, 3));
        assert_eq!(matches, Rect::new(0, 5, 34, 11));
    }

    #[test]
    fn test_name_panels_give_an_error_its_own_row_in_the_field() {
        let list = Rect::new(0, 2, 34, 14);
        let (field, matches) = name_panels(list, true);

        assert_eq!(field, Rect::new(0, 2, 34, 4));
        assert_eq!(matches, Rect::new(0, 6, 34, 10));
    }

    #[test]
    fn test_name_panels_never_take_more_rows_than_the_pane_has() {
        let (field, matches) = name_panels(Rect::new(0, 0, 34, 2), false);

        assert_eq!(field.height, 2);
        assert_eq!(matches.height, 0);
    }

    #[test]
    fn test_size_text_uses_bytes_then_one_decimal_kilobytes_and_up() {
        assert_eq!(size_text(812), "812 B");
        assert_eq!(size_text(2150), "2.1 KB");
        assert_eq!(size_text(3_670_016), "3.5 MB");
    }
}
