//! Paint for wizard steps 3–5. Every function reads the wizard's state and draws it; none
//! changes it.

use std::borrow::Cow;

use andre_core::ignore::GITIGNORE_SECRETS_TEXT;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Margin, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};

use super::finish::{Done, Gitignore, ReviewLine};
use super::NewPackageWizard;
use crate::components::AppContext;
use crate::theme::ThemeColors;
use crate::ui::detail::{render_detail, Detail};
use crate::ui::dialog::{render_action_bar, Button};
use crate::ui::focus_panel;
use crate::ui::icons::Icons;
use crate::ui::layout::ScreenRects;
use crate::ui::list_panel::{self, render_list_panel, Empty};
use crate::ui::selection::row_with_glyphs;

pub fn render_ignore(
    frame: &mut Frame,
    rects: ScreenRects,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    icons: &Icons,
) {
    let width = list_panel::inner(rects.list).width;
    let cursor = wizard.ignore_list.cursor;
    let gitignore_meta = match wizard.gitignore {
        Gitignore::Skip => "none",
        Gitignore::CommonSecrets => "common secrets",
    };
    let rows = vec![
        ignore_row(
            ".gitignore",
            gitignore_meta,
            wizard.gitignore != Gitignore::Skip,
            cursor == 0,
            icons,
            colors,
            width,
        ),
        ignore_row(
            ".stow-local-ignore",
            "",
            wizard.stow_local_ignore,
            cursor == 1,
            icons,
            colors,
            width,
        ),
    ];
    render_list_panel(
        frame,
        rects.list,
        colors,
        "ignore files",
        rows,
        &wizard.ignore_list,
        None,
    );

    let (title, detail) = if cursor == 0 {
        (".gitignore", gitignore_detail(colors, wizard))
    } else {
        (".stow-local-ignore", stow_local_detail(colors, wizard))
    };
    render_detail(frame, rects.detail, colors, title, &detail);
}

/// One row of the ignore step: a mark, the file name, and what it holds.
#[allow(clippy::too_many_arguments)]
fn ignore_row(
    name: &str,
    meta: &str,
    on: bool,
    selected: bool,
    icons: &Icons,
    colors: &ThemeColors,
    width: u16,
) -> Line<'static> {
    let mark_color = if on { colors.accent } else { colors.text_dim };
    let mut content = vec![Span::styled(
        name.to_string(),
        Style::default().fg(colors.text),
    )];
    if !meta.is_empty() {
        content.push(Span::styled(
            format!("  {meta}"),
            Style::default().fg(colors.text_dim),
        ));
    }
    row_with_glyphs(
        vec![Span::styled(
            format!("{} ", icons.checkbox(on)),
            Style::default().fg(mark_color),
        )],
        content,
        selected,
        colors,
        width,
    )
}

fn gitignore_detail(colors: &ThemeColors, wizard: &NewPackageWizard) -> Detail {
    let text = Style::default().fg(colors.text);
    let dim = Style::default().fg(colors.text_dim);
    let sections = match wizard.gitignore {
        Gitignore::CommonSecrets => GITIGNORE_SECRETS_TEXT
            .lines()
            .filter(|line| !line.starts_with('#'))
            .map(|line| Line::styled(line.to_string(), Style::default().fg(colors.value)))
            .collect(),
        Gitignore::Skip => vec![Line::styled(
            "Not written. Space or \u{2190}\u{2192} adds the template.",
            dim,
        )],
    };
    Detail {
        headline: vec![Line::styled(
            "Keeps secrets out of git. andre writes it to the package root.",
            text,
        )],
        facts: Vec::new(),
        sections,
    }
}

fn stow_local_detail(colors: &ThemeColors, wizard: &NewPackageWizard) -> Detail {
    let text = Style::default().fg(colors.text);
    let dim = Style::default().fg(colors.text_dim);
    let sections = if wizard.stow_local_ignore {
        vec![
            Line::styled(
                "REPLACES stow's built-in list. andre copies that list in,",
                dim,
            ),
            Line::styled("so only the lines you add change.", dim),
        ]
    } else {
        vec![Line::styled(
            "Not written. stow uses its built-in list.",
            dim,
        )]
    };
    Detail {
        headline: vec![Line::styled(
            "Files stow must never link into the target.",
            text,
        )],
        facts: Vec::new(),
        sections,
    }
}

pub fn render_profiles(
    frame: &mut Frame,
    rects: ScreenRects,
    ctx: &AppContext,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    icons: &Icons,
) {
    let width = list_panel::inner(rects.list).width;
    let cursor = wizard.profile_list.cursor;
    let rows = wizard
        .profile_names
        .iter()
        .enumerate()
        .map(|(i, name)| {
            let on = wizard.chosen_profiles.contains(name);
            let mark_color = if on { colors.accent } else { colors.text_dim };
            row_with_glyphs(
                vec![Span::styled(
                    format!("{} ", icons.checkbox(on)),
                    Style::default().fg(mark_color),
                )],
                vec![Span::styled(name.clone(), Style::default().fg(colors.text))],
                i == cursor,
                colors,
                width,
            )
        })
        .collect();
    let empty = Empty {
        message: "no profiles yet",
        hint: None,
    };
    render_list_panel(
        frame,
        rects.list,
        colors,
        "add to profiles",
        rows,
        &wizard.profile_list,
        Some(empty),
    );

    match wizard.profile_names.get(cursor) {
        Some(name) => {
            let detail = profile_detail(ctx, colors, wizard, name);
            render_detail(frame, rects.detail, colors, name, &detail);
        }
        None => {
            let detail = Detail {
                headline: vec![Line::styled(
                    "No profiles yet. None is fine: the package still shows under packages.",
                    Style::default().fg(colors.text_dim),
                )],
                ..Detail::default()
            };
            render_detail(frame, rects.detail, colors, "profiles", &detail);
        }
    }
}

/// A profile's facts: its packages with includes resolved, and the count once the package
/// joins it. The counts were resolved when the step was entered.
fn profile_detail(
    ctx: &AppContext,
    colors: &ThemeColors,
    wizard: &NewPackageWizard,
    name: &str,
) -> Detail {
    let dim = Style::default().fg(colors.text_dim);
    let headline = ctx
        .core
        .profiles
        .get(name)
        .and_then(|profile| profile.description.clone())
        .map(|text| vec![Line::styled(text, Style::default().fg(colors.text))])
        .unwrap_or_default();
    let count = wizard.profile_counts.get(name).copied().unwrap_or_default();
    let mut facts = vec![(Cow::Borrowed("packages"), count.packages.to_string())];
    if wizard.chosen_profiles.iter().any(|chosen| chosen == name) {
        facts.push((
            Cow::Borrowed("after"),
            format!("{} (+ {})", count.after, wizard.name.value),
        ));
    }
    Detail {
        headline,
        facts,
        sections: vec![Line::styled(
            "None is fine: the package still shows under packages.",
            dim,
        )],
    }
}

/// The rows the review's head and lines are drawn in: inside the panel's border, with one
/// column of inset each side, as the detail panels have.
pub fn review_body(panel: Rect) -> Rect {
    list_panel::inner(panel).inner(Margin::new(1, 0))
}

pub fn render_review(
    frame: &mut Frame,
    rects: ScreenRects,
    wizard: &NewPackageWizard,
    colors: &ThemeColors,
    icons: &Icons,
) {
    let title = match &wizard.plan {
        Some(plan) if plan.package_exists => {
            format!("review \u{b7} add to {}", plan.package)
        }
        Some(plan) => format!("review \u{b7} new package {}", plan.package),
        None => "review".to_string(),
    };
    let block = focus_panel(colors, true).title(format!(" {title} "));
    frame.render_widget(block, rects.list);

    let body = review_body(rects.list);
    let head_rows = wizard.head_rows().min(body.height);
    let head = Rect {
        height: head_rows,
        ..body
    };
    let rest = Rect {
        y: body.y + head_rows,
        height: body.height - head_rows,
        ..body
    };
    if let Some(done) = wizard.done {
        frame.render_widget(
            Paragraph::new(done_lines(wizard, done, colors, icons)),
            head,
        );
    }
    let lines = wizard.review.iter().map(|line| review_line(line, colors));
    let scroll = u16::try_from(wizard.review_scroll).unwrap_or(u16::MAX);
    frame.render_widget(
        Paragraph::new(lines.collect::<Vec<_>>()).scroll((scroll, 0)),
        rest,
    );

    if let Some(actions) = rects.actions {
        render_action_bar(frame, actions, colors, &review_buttons(wizard.done));
    }
}

/// The result line and a blank row, shown once the review has ended.
fn done_lines(
    wizard: &NewPackageWizard,
    done: Done,
    colors: &ThemeColors,
    icons: &Icons,
) -> Vec<Line<'static>> {
    let package = wizard
        .plan
        .as_ref()
        .map_or("", |plan| plan.package.as_str());
    let line = match done {
        Done::Created => Line::from(vec![
            Span::styled(
                format!("{} ", icons.check),
                Style::default().fg(colors.ok).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("created {package}"),
                Style::default().fg(colors.text),
            ),
        ]),
        Done::Added => Line::from(vec![
            Span::styled(
                format!("{} ", icons.check),
                Style::default().fg(colors.ok).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("added files to {package}"),
                Style::default().fg(colors.text),
            ),
        ]),
        Done::DryRun => Line::styled(
            "dry-run: nothing was written",
            Style::default().fg(colors.warn),
        ),
    };
    vec![line, Line::raw("")]
}

fn review_line(line: &ReviewLine, colors: &ThemeColors) -> Line<'static> {
    let dim = Style::default().fg(colors.text_dim);
    if line.verb.is_empty() {
        return Line::styled(line.text.clone(), dim);
    }
    let text_color = if line.verb == "skip" {
        colors.warn
    } else {
        colors.text
    };
    Line::from(vec![
        Span::styled(format!("{:<7}", line.verb), dim),
        Span::styled(line.text.clone(), Style::default().fg(text_color)),
    ])
}

/// Esc and Enter before the review ends; Enter alone after it.
pub(super) fn review_buttons(done: Option<Done>) -> Vec<Button> {
    let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);
    match done {
        Some(_) => vec![Button {
            key: enter,
            label: Cow::Borrowed("done"),
            primary: true,
        }],
        None => vec![
            Button {
                key: KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE),
                label: Cow::Borrowed("back"),
                primary: false,
            },
            Button {
                key: enter,
                label: Cow::Borrowed("create"),
                primary: true,
            },
        ],
    }
}
