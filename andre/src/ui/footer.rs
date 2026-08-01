use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::Paragraph,
    Frame,
};

use crate::theme::ThemeColors;

fn key_span(s: &str, colors: &ThemeColors) -> Span<'static> {
    Span::styled(s.to_string(), Style::default().fg(colors.indicator))
}

pub fn render_footer(frame: &mut Frame, area: Rect, colors: &ThemeColors, screen_id: &str) {
    let v = key_span("v", colors);
    let d = key_span("d", colors);
    let n = key_span("n", colors);
    let a = key_span("a", colors);
    let t = key_span("t", colors);
    let q = key_span("q", colors);

    let line1 = Line::from(vec![
        v,
        Span::raw("erbose - "),
        d,
        Span::raw("ry-run - "),
        n,
        Span::raw("o-fold - "),
        a,
        Span::raw("dopt - "),
        t,
        Span::raw("heme - "),
        Span::raw("d"),
        key_span("o", colors),
        Span::raw("tfiles - "),
        q,
        Span::raw("uit"),
    ]);

    let line2 = {
        let arrows = key_span("\u{2191}\u{2193}", colors);
        let enter = key_span("Enter", colors);
        let space = key_span("Space", colors);
        let esc = key_span("Esc", colors);
        let ky = key_span("y", colors);
        let kn = key_span("n", colors);

        let nav: Vec<Span> = match screen_id {
            "MainMenu" => vec![
                arrows,
                Span::raw(" navigate  "),
                key_span("1\u{2013}7", colors),
                Span::raw(" jump  "),
                enter,
                Span::raw(" select"),
            ],
            "GroupSelect" | "PackageSelect" => vec![
                arrows,
                Span::raw(" navigate  "),
                space,
                Span::raw(" toggle  "),
                enter,
                Span::raw(" confirm  "),
                esc,
                Span::raw(" cancel"),
            ],
            "Status" | "Execute" => vec![enter, Span::raw(" continue")],
            "Help" => vec![esc, Span::raw(" back")],
            "Settings" => vec![
                arrows,
                Span::raw(" navigate  "),
                enter,
                Span::raw(" group menu  "),
                esc,
                Span::raw(" back"),
            ],
            "Confirm" => vec![ky, Span::raw(" confirm  "), kn, Span::raw(" cancel")],
            "AdoptTargetSelect" | "UnstowTargetSelect" => vec![
                arrows,
                Span::raw(" navigate  "),
                enter,
                Span::raw(" select  "),
                esc,
                Span::raw(" back"),
            ],
            "AdoptFileSelect" => vec![
                arrows,
                Span::raw(" navigate  "),
                space,
                Span::raw(" toggle  "),
                key_span("\u{2192}", colors),
                Span::raw(" into  "),
                enter,
                Span::raw(" confirm  "),
                key_span("\u{2190}", colors),
                Span::raw(" up  "),
                esc,
                Span::raw(" back"),
            ],
            "AdoptConfirm" => vec![ky, Span::raw(" proceed  "), kn, Span::raw(" back")],
            "AdoptNamePrompt" => vec![enter, Span::raw(" confirm  "), esc, Span::raw(" back")],
            "AdoptPreview" => vec![ky, Span::raw(" execute  "), kn, Span::raw(" cancel")],
            "UnstowBrowse" => vec![
                arrows,
                Span::raw(" navigate  "),
                key_span("\u{2192}", colors),
                Span::raw(" into  "),
                enter,
                Span::raw(" unlink  "),
                key_span("\u{2190}", colors),
                Span::raw(" up  "),
                esc,
                Span::raw(" back"),
            ],
            "UnstowConfirm" => vec![ky, Span::raw(" remove  "), kn, Span::raw(" cancel")],
            _ => vec![Span::raw("")],
        };
        Line::from(nav)
    };

    let max = area.width as usize;
    let line1_width: usize = line1
        .spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();
    let line2_width: usize = line2
        .spans
        .iter()
        .map(|s| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
        .sum();

    let text = if line1_width > max || line2_width > max {
        // Narrow terminal: show only the screen-specific hints (line2),
        // preserving key/value color coding.
        Text::from(vec![line2])
    } else {
        Text::from(vec![line1, line2])
    };

    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(colors.muted));
    frame.render_widget(paragraph, area);
}
