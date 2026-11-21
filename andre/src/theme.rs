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
            Theme::Default => Self {
                bg: Color::Reset,
                border: Color::Rgb(100, 100, 100),
                accent: Color::Rgb(255, 185, 60),
                on_accent: Color::Reset,
                ok: Color::Rgb(80, 200, 120),
                warn: Color::Rgb(255, 200, 50),
                err: Color::Rgb(255, 80, 80),
                text: Color::Rgb(200, 200, 200),
                text_dim: Color::Rgb(140, 140, 140),
                value: Color::Rgb(255, 215, 0),
                key: Color::Rgb(0, 212, 170),
                selection: Color::Rgb(60, 60, 60),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::Dracula => Self {
                bg: Color::Rgb(40, 42, 54),
                border: Color::Rgb(68, 71, 90),
                accent: Color::Rgb(189, 147, 249),
                on_accent: Color::Rgb(40, 42, 54),
                ok: Color::Rgb(80, 250, 123),
                warn: Color::Rgb(241, 250, 140),
                err: Color::Rgb(255, 85, 85),
                text: Color::Rgb(248, 248, 242),
                text_dim: Color::Rgb(98, 114, 164),
                value: Color::Rgb(255, 121, 198),
                key: Color::Rgb(139, 233, 253),
                selection: Color::Rgb(68, 71, 90),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::CatppuccinMocha => Self {
                bg: Color::Rgb(30, 30, 46),
                border: Color::Rgb(69, 71, 90),
                accent: Color::Rgb(203, 166, 247),
                on_accent: Color::Rgb(30, 30, 46),
                ok: Color::Rgb(166, 227, 161),
                warn: Color::Rgb(249, 226, 175),
                err: Color::Rgb(243, 139, 168),
                text: Color::Rgb(205, 214, 244),
                text_dim: Color::Rgb(108, 112, 134),
                value: Color::Rgb(250, 179, 135),
                key: Color::Rgb(137, 220, 235),
                selection: Color::Rgb(69, 71, 90),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::CatppuccinLatte => Self {
                bg: Color::Rgb(239, 241, 245),
                border: Color::Rgb(188, 192, 204),
                accent: Color::Rgb(136, 57, 239),
                on_accent: Color::Rgb(239, 241, 245),
                ok: Color::Rgb(64, 160, 43),
                warn: Color::Rgb(223, 142, 29),
                err: Color::Rgb(210, 15, 57),
                text: Color::Rgb(76, 79, 105),
                text_dim: Color::Rgb(156, 160, 176),
                value: Color::Rgb(254, 100, 11),
                key: Color::Rgb(4, 165, 229),
                selection: Color::Rgb(205, 209, 219),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::CatppuccinFrappe => Self {
                bg: Color::Rgb(48, 52, 70),
                border: Color::Rgb(81, 87, 109),
                accent: Color::Rgb(202, 158, 230),
                on_accent: Color::Rgb(48, 52, 70),
                ok: Color::Rgb(166, 209, 137),
                warn: Color::Rgb(229, 200, 144),
                err: Color::Rgb(231, 130, 132),
                text: Color::Rgb(198, 208, 245),
                text_dim: Color::Rgb(115, 121, 148),
                value: Color::Rgb(239, 159, 118),
                key: Color::Rgb(153, 209, 219),
                selection: Color::Rgb(81, 87, 109),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::CatppuccinMacchiato => Self {
                bg: Color::Rgb(36, 39, 58),
                border: Color::Rgb(73, 77, 100),
                accent: Color::Rgb(198, 160, 246),
                on_accent: Color::Rgb(36, 39, 58),
                ok: Color::Rgb(166, 218, 149),
                warn: Color::Rgb(238, 212, 159),
                err: Color::Rgb(237, 135, 150),
                text: Color::Rgb(202, 211, 245),
                text_dim: Color::Rgb(110, 115, 141),
                value: Color::Rgb(245, 169, 127),
                key: Color::Rgb(145, 215, 227),
                selection: Color::Rgb(73, 77, 100),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::Nord => Self {
                bg: Color::Rgb(46, 52, 64),
                border: Color::Rgb(76, 86, 106),
                accent: Color::Rgb(136, 192, 208),
                on_accent: Color::Rgb(46, 52, 64),
                ok: Color::Rgb(163, 190, 140),
                warn: Color::Rgb(235, 203, 139),
                err: Color::Rgb(191, 97, 106),
                text: Color::Rgb(216, 222, 233),
                text_dim: Color::Rgb(97, 110, 136),
                value: Color::Rgb(129, 161, 193),
                key: Color::Rgb(180, 142, 173),
                selection: Color::Rgb(76, 86, 106),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
                ],
            },
            Theme::Gruvbox => Self {
                bg: Color::Rgb(40, 40, 40),
                border: Color::Rgb(80, 73, 69),
                accent: Color::Rgb(215, 153, 33),
                on_accent: Color::Rgb(40, 40, 40),
                ok: Color::Rgb(152, 151, 26),
                warn: Color::Rgb(214, 93, 14),
                err: Color::Rgb(204, 36, 29),
                text: Color::Rgb(235, 219, 178),
                text_dim: Color::Rgb(146, 131, 116),
                value: Color::Rgb(177, 98, 134),
                key: Color::Rgb(69, 133, 136),
                selection: Color::Rgb(80, 73, 69),
                banner: [
                    Color::Rgb(255, 80, 80),
                    Color::Rgb(255, 160, 40),
                    Color::Rgb(255, 215, 0),
                    Color::Rgb(80, 200, 80),
                    Color::Rgb(0, 180, 255),
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
