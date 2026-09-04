//! Resolving a ratatui [`Color`] to RGB, and picking readable ink over it.

use ratatui::style::Color;

/// The xterm defaults for the 16 named ANSI colours.
///
/// These are an approximation by construction: the named colours are defined
/// by the user's terminal theme and cannot be read from inside the process.
const ANSI16: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00),
    (0x80, 0x00, 0x00),
    (0x00, 0x80, 0x00),
    (0x80, 0x80, 0x00),
    (0x00, 0x00, 0x80),
    (0x80, 0x00, 0x80),
    (0x00, 0x80, 0x80),
    (0xc0, 0xc0, 0xc0),
    (0x80, 0x80, 0x80),
    (0xff, 0x00, 0x00),
    (0x00, 0xff, 0x00),
    (0xff, 0xff, 0x00),
    (0x00, 0x00, 0xff),
    (0xff, 0x00, 0xff),
    (0x00, 0xff, 0xff),
    (0xff, 0xff, 0xff),
];

/// One component level of the 6x6x6 colour cube.
fn cube_level(v: u8) -> u8 {
    if v == 0 { 0 } else { 55 + 40 * v }
}

/// Resolves `color` to RGB, or `None` when it depends on the terminal's theme
/// and so cannot be known here (`Color::Reset`).
pub(crate) fn to_rgb(color: Color) -> Option<(u8, u8, u8)> {
    let idx = match color {
        Color::Rgb(r, g, b) => return Some((r, g, b)),
        Color::Reset => return None,
        Color::Black => 0,
        Color::Red => 1,
        Color::Green => 2,
        Color::Yellow => 3,
        Color::Blue => 4,
        Color::Magenta => 5,
        Color::Cyan => 6,
        Color::Gray => 7,
        Color::DarkGray => 8,
        Color::LightRed => 9,
        Color::LightGreen => 10,
        Color::LightYellow => 11,
        Color::LightBlue => 12,
        Color::LightMagenta => 13,
        Color::LightCyan => 14,
        Color::White => 15,
        Color::Indexed(i) => i,
    };
    Some(match idx {
        0..=15 => ANSI16[idx as usize],
        16..=231 => {
            let n = idx - 16;
            (
                cube_level(n / 36),
                cube_level((n % 36) / 6),
                cube_level(n % 6),
            )
        }
        _ => {
            let v = 8 + 10 * (idx - 232);
            (v, v, v)
        }
    })
}

/// WCAG relative luminance, 0.0 (black) to 1.0 (white).
pub(crate) fn luminance((r, g, b): (u8, u8, u8)) -> f32 {
    fn chan(c: u8) -> f32 {
        let s = f32::from(c) / 255.0;
        if s <= 0.039_28 {
            s / 12.92
        } else {
            ((s + 0.055) / 1.055).powf(2.4)
        }
    }
    0.2126 * chan(r) + 0.7152 * chan(g) + 0.0722 * chan(b)
}

/// Ink that stays readable on top of `bg`.
///
/// An unresolvable `bg` gets light ink: plank-style TUIs sit on dark grounds,
/// so light ink is the safer guess when the theme is unknown.
pub(crate) fn contrast_ink(bg: Color) -> Color {
    const DARK: Color = Color::Rgb(0x11, 0x11, 0x11);
    const LIGHT: Color = Color::Rgb(0xee, 0xee, 0xee);
    match to_rgb(bg) {
        Some(rgb) if luminance(rgb) > 0.179 => DARK,
        _ => LIGHT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cube_colors_resolve_to_their_xterm_rgb() {
        // 107 is #87af5f, the green plank spells out for OSC 12.
        assert_eq!(to_rgb(Color::Indexed(107)), Some((0x87, 0xaf, 0x5f)));
        // 106, plank's `status::THEME_COLOR`, is the neighbouring #87af00.
        assert_eq!(to_rgb(Color::Indexed(106)), Some((0x87, 0xaf, 0x00)));
        // 167 is the busy red, #d75f5f.
        assert_eq!(to_rgb(Color::Indexed(167)), Some((0xd7, 0x5f, 0x5f)));
    }

    #[test]
    fn the_grey_ramp_resolves() {
        assert_eq!(to_rgb(Color::Indexed(232)), Some((8, 8, 8)));
        assert_eq!(to_rgb(Color::Indexed(255)), Some((238, 238, 238)));
    }

    #[test]
    fn rgb_passes_through_and_reset_is_unknown() {
        assert_eq!(to_rgb(Color::Rgb(1, 2, 3)), Some((1, 2, 3)));
        assert_eq!(to_rgb(Color::Reset), None);
    }

    #[test]
    fn ink_is_dark_on_light_cursors_and_light_on_dark_ones() {
        assert_eq!(
            contrast_ink(Color::Rgb(0xff, 0xff, 0xff)),
            Color::Rgb(0x11, 0x11, 0x11)
        );
        assert_eq!(
            contrast_ink(Color::Rgb(0x00, 0x00, 0x00)),
            Color::Rgb(0xee, 0xee, 0xee)
        );
        // The theme green is light enough to need dark ink.
        assert_eq!(
            contrast_ink(Color::Indexed(107)),
            Color::Rgb(0x11, 0x11, 0x11)
        );
    }

    #[test]
    fn an_unresolvable_color_gets_light_ink() {
        // `Reset` depends on the user's theme and cannot be resolved in
        // process; light ink is the documented fallback.
        assert_eq!(contrast_ink(Color::Reset), Color::Rgb(0xee, 0xee, 0xee));
    }
}
