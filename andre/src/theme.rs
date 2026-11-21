use andre_core::Theme;
use ratatui::style::Color;

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub bg: Color,
    pub border: Color,
    pub accent: Color,
    pub on_accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub err: Color,
    pub text: Color,
    pub text_dim: Color,
    pub value: Color,
    pub key: Color,
    /// Fill of the selected menu row. Chosen per theme so the label and icon read on it.
    pub selection: Color,
    /// The A-N-D-R-E letters of the banner.
    pub banner: [Color; 5],
}

impl ThemeColors {
    pub fn from_theme(theme: Theme) -> Self {
        match theme {
            // Source: https://www.ditig.com/256-colors-cheat-sheet (xterm 256-colour table).
            // Paints no background (`Reset`) so it follows the terminal: a dark-terminal theme.
            Theme::Default => Self {
                bg: Color::Reset,
                border: Color::Rgb(0x5f, 0x5f, 0x5f), // xterm 59 Grey37
                accent: Color::Rgb(0xff, 0xd7, 0x5f), // xterm 221 LightGoldenrod2
                on_accent: Color::Rgb(0x1c, 0x1c, 0x1c), // xterm 234 Grey11
                ok: Color::Rgb(0x5f, 0xd7, 0x87),     // xterm 78 SeaGreen3
                warn: Color::Rgb(0xff, 0x87, 0x00),   // xterm 208 DarkOrange
                err: Color::Rgb(0xff, 0x5f, 0x5f),    // xterm 203 IndianRed1
                text: Color::Rgb(0xd0, 0xd0, 0xd0),   // xterm 252 Grey82
                text_dim: Color::Rgb(0x94, 0x94, 0x94), // xterm 246 Grey58
                value: Color::Rgb(0x87, 0xd7, 0xff),  // xterm 117 SkyBlue1
                key: Color::Rgb(0x00, 0xd7, 0xaf),    // xterm 43 Cyan3
                selection: Color::Rgb(0x3a, 0x3a, 0x3a), // xterm 237 Grey23
                banner: [
                    Color::Rgb(0xff, 0x5f, 0x5f),
                    Color::Rgb(0xff, 0x87, 0x00),
                    Color::Rgb(0xff, 0xd7, 0x00),
                    Color::Rgb(0x5f, 0xd7, 0x5f),
                    Color::Rgb(0x00, 0xaf, 0xff),
                ],
            },
            // Source: https://spec.draculatheme.com/
            Theme::Dracula => Self {
                bg: Color::Rgb(0x28, 0x2a, 0x36),
                border: Color::Rgb(0x62, 0x72, 0xa4),
                accent: Color::Rgb(0xbd, 0x93, 0xf9),
                on_accent: Color::Rgb(0x28, 0x2a, 0x36),
                ok: Color::Rgb(0x50, 0xfa, 0x7b),
                warn: Color::Rgb(0xff, 0xb8, 0x6c),
                err: Color::Rgb(0xff, 0x55, 0x55),
                text: Color::Rgb(0xf8, 0xf8, 0xf2),
                text_dim: Color::Rgb(0x88, 0x94, 0xb8), // derived: Comment #6272a4 mixed 25% toward Foreground #f8f8f2 (Comment alone is 3.03:1)
                value: Color::Rgb(0xf1, 0xfa, 0x8c),
                key: Color::Rgb(0x8b, 0xe9, 0xfd),
                selection: Color::Rgb(0x44, 0x47, 0x5a),
                banner: [
                    Color::Rgb(0xff, 0x55, 0x55),
                    Color::Rgb(0xff, 0xb8, 0x6c),
                    Color::Rgb(0xf1, 0xfa, 0x8c),
                    Color::Rgb(0x50, 0xfa, 0x7b),
                    Color::Rgb(0x8b, 0xe9, 0xfd),
                ],
            },
            // Source: https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md (accent from https://github.com/catppuccin/palette/blob/main/palette.json)
            Theme::CatppuccinMocha => Self {
                bg: Color::Rgb(0x1e, 0x1e, 0x2e),
                border: Color::Rgb(0x6c, 0x70, 0x86),
                accent: Color::Rgb(0xcb, 0xa6, 0xf7),
                on_accent: Color::Rgb(0x1e, 0x1e, 0x2e),
                ok: Color::Rgb(0xa6, 0xe3, 0xa1),
                warn: Color::Rgb(0xf9, 0xe2, 0xaf),
                err: Color::Rgb(0xf3, 0x8b, 0xa8),
                text: Color::Rgb(0xcd, 0xd6, 0xf4),
                text_dim: Color::Rgb(0xa6, 0xad, 0xc8),
                value: Color::Rgb(0xfa, 0xb3, 0x87),
                key: Color::Rgb(0x89, 0xb4, 0xfa),
                selection: Color::Rgb(0x45, 0x47, 0x5a),
                banner: [
                    Color::Rgb(0xf3, 0x8b, 0xa8),
                    Color::Rgb(0xfa, 0xb3, 0x87),
                    Color::Rgb(0xf9, 0xe2, 0xaf),
                    Color::Rgb(0xa6, 0xe3, 0xa1),
                    Color::Rgb(0x74, 0xc7, 0xec),
                ],
            },
            // Source: https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md (accent from https://github.com/catppuccin/palette/blob/main/palette.json)
            Theme::CatppuccinLatte => Self {
                bg: Color::Rgb(0xef, 0xf1, 0xf5),
                border: Color::Rgb(0x9c, 0xa0, 0xb0),
                accent: Color::Rgb(0x88, 0x39, 0xef),
                on_accent: Color::Rgb(0xef, 0xf1, 0xf5),
                ok: Color::Rgb(0x41, 0x9c, 0x2e), // derived: Green #40a02b mixed 5% toward Text #4c4f69 (Green is 2.96:1)
                warn: Color::Rgb(0xba, 0x7e, 0x30), // derived: Yellow #df8e1d mixed 25% toward Text #4c4f69 (Yellow is 2.31:1)
                err: Color::Rgb(0xd2, 0x0f, 0x39),
                text: Color::Rgb(0x4c, 0x4f, 0x69),
                text_dim: Color::Rgb(0x5c, 0x5f, 0x77), // Subtext 1; Subtext 0 is 4.37:1 on Base, below the floor
                value: Color::Rgb(0xe3, 0x61, 0x19), // derived: Peach #fe640b mixed 15% toward Text #4c4f69 (Peach is 2.64:1)
                key: Color::Rgb(0x1e, 0x66, 0xf5),   // Blue (Links and URLs)
                selection: Color::Rgb(0xcc, 0xd0, 0xda),
                banner: [
                    Color::Rgb(0xd2, 0x0f, 0x39),
                    Color::Rgb(0xe3, 0x61, 0x19),
                    Color::Rgb(0xba, 0x7e, 0x30),
                    Color::Rgb(0x41, 0x9c, 0x2e),
                    Color::Rgb(0x1e, 0x66, 0xf5),
                ],
            },
            // Source: https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md (accent from https://github.com/catppuccin/palette/blob/main/palette.json)
            Theme::CatppuccinFrappe => Self {
                bg: Color::Rgb(0x30, 0x34, 0x46),
                border: Color::Rgb(0x73, 0x79, 0x94),
                accent: Color::Rgb(0xca, 0x9e, 0xe6),
                on_accent: Color::Rgb(0x30, 0x34, 0x46),
                ok: Color::Rgb(0xa6, 0xd1, 0x89),
                warn: Color::Rgb(0xe5, 0xc8, 0x90),
                err: Color::Rgb(0xe7, 0x82, 0x84),
                text: Color::Rgb(0xc6, 0xd0, 0xf5),
                text_dim: Color::Rgb(0xa5, 0xad, 0xce),
                value: Color::Rgb(0xef, 0x9f, 0x76),
                key: Color::Rgb(0x8c, 0xaa, 0xee),
                selection: Color::Rgb(0x51, 0x57, 0x6d),
                banner: [
                    Color::Rgb(0xe7, 0x82, 0x84),
                    Color::Rgb(0xef, 0x9f, 0x76),
                    Color::Rgb(0xe5, 0xc8, 0x90),
                    Color::Rgb(0xa6, 0xd1, 0x89),
                    Color::Rgb(0x85, 0xc1, 0xdc),
                ],
            },
            // Source: https://github.com/catppuccin/catppuccin/blob/main/docs/style-guide.md (accent from https://github.com/catppuccin/palette/blob/main/palette.json)
            Theme::CatppuccinMacchiato => Self {
                bg: Color::Rgb(0x24, 0x27, 0x3a),
                border: Color::Rgb(0x6e, 0x73, 0x8d),
                accent: Color::Rgb(0xc6, 0xa0, 0xf6),
                on_accent: Color::Rgb(0x24, 0x27, 0x3a),
                ok: Color::Rgb(0xa6, 0xda, 0x95),
                warn: Color::Rgb(0xee, 0xd4, 0x9f),
                err: Color::Rgb(0xed, 0x87, 0x96),
                text: Color::Rgb(0xca, 0xd3, 0xf5),
                text_dim: Color::Rgb(0xa5, 0xad, 0xcb),
                value: Color::Rgb(0xf5, 0xa9, 0x7f),
                key: Color::Rgb(0x8a, 0xad, 0xf4),
                selection: Color::Rgb(0x49, 0x4d, 0x64),
                banner: [
                    Color::Rgb(0xed, 0x87, 0x96),
                    Color::Rgb(0xf5, 0xa9, 0x7f),
                    Color::Rgb(0xee, 0xd4, 0x9f),
                    Color::Rgb(0xa6, 0xda, 0x95),
                    Color::Rgb(0x7d, 0xc4, 0xe4),
                ],
            },
            // Source: https://www.nordtheme.com/docs/colors-and-palettes
            Theme::Nord => Self {
                bg: Color::Rgb(0x2e, 0x34, 0x40),
                border: Color::Rgb(0x4c, 0x56, 0x6a),
                accent: Color::Rgb(0x88, 0xc0, 0xd0),
                on_accent: Color::Rgb(0x2e, 0x34, 0x40),
                ok: Color::Rgb(0xa3, 0xbe, 0x8c),
                warn: Color::Rgb(0xeb, 0xcb, 0x8b),
                err: Color::Rgb(0xbf, 0x61, 0x6a),
                text: Color::Rgb(0xd8, 0xde, 0xe9),
                text_dim: Color::Rgb(0x97, 0xa0, 0xb4), // derived: nord3 #616e88 (brightened, as in Nord editor ports) mixed 45% toward nord4 #d8dee9
                value: Color::Rgb(0xb4, 0x8e, 0xad),
                key: Color::Rgb(0x81, 0xa1, 0xc1),
                selection: Color::Rgb(0x43, 0x4c, 0x5e),
                banner: [
                    Color::Rgb(0xbf, 0x61, 0x6a),
                    Color::Rgb(0xd0, 0x87, 0x70),
                    Color::Rgb(0xeb, 0xcb, 0x8b),
                    Color::Rgb(0xa3, 0xbe, 0x8c),
                    Color::Rgb(0x88, 0xc0, 0xd0),
                ],
            },
            // Source: https://github.com/morhetz/gruvbox/blob/master/colors/gruvbox.vim (dark mode uses the bright_* set for foreground)
            Theme::Gruvbox => Self {
                bg: Color::Rgb(0x28, 0x28, 0x28),
                border: Color::Rgb(0x66, 0x5c, 0x54),
                accent: Color::Rgb(0xfa, 0xbd, 0x2f),
                on_accent: Color::Rgb(0x28, 0x28, 0x28),
                ok: Color::Rgb(0xb8, 0xbb, 0x26),
                warn: Color::Rgb(0xfe, 0x80, 0x19),
                err: Color::Rgb(0xfb, 0x49, 0x34),
                text: Color::Rgb(0xeb, 0xdb, 0xb2),
                text_dim: Color::Rgb(0xa8, 0x99, 0x84),
                value: Color::Rgb(0xd3, 0x86, 0x9b),
                key: Color::Rgb(0x83, 0xa5, 0x98),
                selection: Color::Rgb(0x3c, 0x38, 0x36),
                banner: [
                    Color::Rgb(0xfb, 0x49, 0x34),
                    Color::Rgb(0xfe, 0x80, 0x19),
                    Color::Rgb(0xfa, 0xbd, 0x2f),
                    Color::Rgb(0xb8, 0xbb, 0x26),
                    Color::Rgb(0x83, 0xa5, 0x98),
                ],
            },
        }
    }
}

/// WCAG contrast ratio between two colours, from 1.0 to 21.0. `None` when either colour
/// is not an RGB value (for example the terminal's own `Reset`).
pub fn contrast_ratio(a: Color, b: Color) -> Option<f64> {
    fn luminance(c: Color) -> Option<f64> {
        let Color::Rgb(r, g, b) = c else {
            return None;
        };
        let linear = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        Some(0.2126 * linear(r) + 0.7152 * linear(g) + 0.0722 * linear(b))
    }
    let (la, lb) = (luminance(a)?, luminance(b)?);
    let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
    Some((hi + 0.05) / (lo + 0.05))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_contrast_ratio_is_21_for_black_on_white_and_symmetric() {
        let black = Color::Rgb(0, 0, 0);
        let white = Color::Rgb(255, 255, 255);
        assert!((contrast_ratio(black, white).unwrap() - 21.0).abs() < 1e-9);
        assert_eq!(contrast_ratio(white, black), contrast_ratio(black, white));
    }

    #[test]
    fn test_contrast_ratio_is_none_for_non_rgb_colours() {
        assert_eq!(contrast_ratio(Color::Reset, Color::Rgb(0, 0, 0)), None);
    }
}
