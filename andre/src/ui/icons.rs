use andre_core::IconSet;

/// Glyphs for one icon set. Every field is a single display column wide, so
/// icons never shift the layout the way a double-width emoji would.
pub struct Icons {
    pub toggle_on: &'static str,
    pub toggle_off: &'static str,
    pub link: &'static str,
    pub import: &'static str,
    pub unlink: &'static str,
    pub check: &'static str,
    pub gear: &'static str,
    pub question: &'static str,
    pub power: &'static str,
}

/// Nerd Font codepoints (Font Awesome block).
pub static NERD: Icons = Icons {
    toggle_on: "\u{f205}",
    toggle_off: "\u{f204}",
    link: "\u{f0c1}",
    import: "\u{f019}",
    unlink: "\u{f127}",
    check: "\u{f00c}",
    gear: "\u{f013}",
    question: "\u{f059}",
    power: "\u{f011}",
};

/// Common Unicode symbols from long-established blocks (Latin-1, Arrows,
/// Geometric Shapes, Dingbats, Mathematical Operators). No Nerd Font needed.
pub static UNICODE: Icons = Icons {
    toggle_on: "●",
    toggle_off: "○",
    link: "⇢",
    import: "⇣",
    unlink: "≠",
    check: "✓",
    gear: "≡",
    question: "?",
    power: "×",
};

impl Icons {
    pub fn for_set(set: IconSet) -> &'static Icons {
        match set {
            IconSet::Nerd => &NERD,
            IconSet::Unicode => &UNICODE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn all_glyphs(icons: &Icons) -> [&'static str; 9] {
        [
            icons.toggle_on,
            icons.toggle_off,
            icons.link,
            icons.import,
            icons.unlink,
            icons.check,
            icons.gear,
            icons.question,
            icons.power,
        ]
    }

    #[test]
    fn test_nerd_toggle_glyphs_are_the_fontawesome_codepoints() {
        assert_eq!(NERD.toggle_on, "\u{f205}");
        assert_eq!(NERD.toggle_off, "\u{f204}");
    }

    #[test]
    fn test_unicode_set_uses_plain_symbols_for_toggles() {
        assert_eq!(UNICODE.toggle_on, "●");
        assert_eq!(UNICODE.toggle_off, "○");
    }

    #[test]
    fn test_every_glyph_is_one_column_wide() {
        for set in [IconSet::Nerd, IconSet::Unicode] {
            for glyph in all_glyphs(Icons::for_set(set)) {
                assert_eq!(
                    glyph.width(),
                    1,
                    "{:?} glyph {:?} must be one column wide",
                    set,
                    glyph
                );
            }
        }
    }

    #[test]
    fn test_unicode_set_contains_no_private_use_glyphs() {
        for glyph in all_glyphs(&UNICODE) {
            assert!(
                glyph
                    .chars()
                    .all(|c| !('\u{e000}'..='\u{f8ff}').contains(&c)),
                "unicode set must not depend on a Nerd Font: {glyph:?}"
            );
        }
    }

    #[test]
    fn test_unicode_set_draws_only_from_well_covered_blocks() {
        // Latin-1, Arrows, Mathematical Operators, Geometric Shapes, Dingbats.
        // Emoji-presentation and Supplemental symbols are missing from many
        // monospace fonts or widen to two columns under an emoji fallback.
        let blocks = [
            ('\u{0000}', '\u{00ff}'),
            ('\u{2190}', '\u{21ff}'),
            ('\u{2200}', '\u{22ff}'),
            ('\u{25a0}', '\u{25ff}'),
            ('\u{2700}', '\u{27bf}'),
        ];
        for glyph in all_glyphs(&UNICODE) {
            for c in glyph.chars() {
                assert!(
                    blocks.iter().any(|(lo, hi)| (*lo..=*hi).contains(&c)),
                    "unicode glyph {glyph:?} (U+{:04X}) is outside the well-covered blocks",
                    c as u32
                );
            }
        }
    }

    #[test]
    fn test_unicode_unlink_and_quit_glyphs_are_distinct_codepoints() {
        // Codepoints only; ≠ vs × was chosen so they also look different.
        assert_ne!(UNICODE.unlink, UNICODE.power);
    }

    #[test]
    fn test_for_set_selects_matching_table() {
        assert!(std::ptr::eq(Icons::for_set(IconSet::Nerd), &NERD));
        assert!(std::ptr::eq(Icons::for_set(IconSet::Unicode), &UNICODE));
    }
}
