//! The cursor itself: a post-pass that styles the cell under the caret.

use crate::color::contrast_ink;
use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::Color;
use std::time::{Duration, Instant};
use unicode_width::UnicodeWidthStr;

/// What the cursor looks like.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Shape {
    /// Fills the cell. The glyph underneath survives, re-inked for contrast.
    #[default]
    Block,
    /// A vertical bar at the left edge of the cell.
    ///
    /// Lossy: the bar replaces the glyph in that cell rather than sitting
    /// between cells the way a terminal's own bar cursor does. In an input
    /// widget the caret usually rests on a blank cell, so this rarely shows.
    Bar,
    /// Underlines the cell, leaving glyph, foreground and background alone.
    Underline,
}

/// The colour of the glyph under a [`Shape::Block`] cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ink {
    /// Pick automatically, so the glyph stays readable on the cursor colour.
    #[default]
    Auto,
    /// Always this colour.
    Fixed(Color),
}

/// Whether the cursor blinks, and on whose clock.
///
/// `since` is the caller's, not the crate's. A library-owned epoch would be
/// hidden state and untestable timing, and real terminals restart the blink
/// on each keystroke — which a caller that already knows its last key press
/// gets for free by passing it here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Blink {
    /// Always visible.
    #[default]
    Off,
    /// Alternates every `period`, measured from `since`.
    On {
        /// Half-cycle: how long the cursor stays on, then off.
        period: Duration,
        /// When the current blink cycle started.
        since: Instant,
    },
}

impl Blink {
    /// Whether the cursor is visible at `now`, and how long until that flips.
    fn phase(self, now: Instant) -> (bool, Option<Duration>) {
        let Blink::On { period, since } = self else {
            return (true, None);
        };
        if period.is_zero() {
            return (true, None);
        }
        let elapsed = now.saturating_duration_since(since);
        let cycles = elapsed.as_nanos() / period.as_nanos();
        let visible = cycles.is_multiple_of(2);
        let consumed = period * u32::try_from(cycles).unwrap_or(u32::MAX);
        (
            visible,
            Some(period.saturating_sub(elapsed.saturating_sub(consumed))),
        )
    }
}

/// What the caller must do after rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[must_use = "the real cursor must be positioned here or IME placement breaks"]
pub struct Report {
    /// Where to put the real terminal cursor, which the caller should keep
    /// hidden. Terminals place the IME candidate window and screen-reader
    /// focus at the position they track, whether or not a cursor is drawn.
    pub position: Position,
    /// How long until the cursor's appearance next changes, when it blinks.
    /// `None` when steady. A caller driving its own event loop should shorten
    /// its poll timeout to this.
    pub redraw_after: Option<Duration>,
}

/// A cursor to paint into a buffer.
#[derive(Debug, Clone, Copy)]
pub struct Cursor {
    color: Color,
    shape: Shape,
    ink: Ink,
    blink: Blink,
}

impl Cursor {
    /// A block cursor in `color` that does not blink.
    #[must_use]
    pub fn new(color: Color) -> Self {
        Self {
            color,
            shape: Shape::default(),
            ink: Ink::default(),
            blink: Blink::default(),
        }
    }

    /// Sets the shape.
    #[must_use]
    pub fn shape(mut self, shape: Shape) -> Self {
        self.shape = shape;
        self
    }

    /// Sets how the glyph under a [`Shape::Block`] cursor is coloured.
    #[must_use]
    pub fn ink(mut self, ink: Ink) -> Self {
        self.ink = ink;
        self
    }

    /// Sets whether the cursor blinks.
    #[must_use]
    pub fn blink(mut self, blink: Blink) -> Self {
        self.blink = blink;
        self
    }

    /// Paints the cursor at `pos`, clamped into `area`.
    ///
    /// Must run *after* the text has been rendered into `buf`: the contrast
    /// pick reads the glyph already in the cell, and a wide glyph's trailing
    /// cell has to be there to be found.
    pub fn render(self, area: Rect, buf: &mut Buffer, pos: Position, now: Instant) -> Report {
        let position = clamp(area, pos);
        // Clamping is for the *report* only. A caret outside the area paints
        // nothing: clamping first and then painting would drop a block on an
        // unrelated cell at the edge.
        if area.is_empty() || !area.contains(pos) {
            return Report {
                position,
                redraw_after: None,
            };
        }
        let (visible, redraw_after) = self.blink.phase(now);
        if !visible {
            return Report {
                position,
                redraw_after,
            };
        }
        let width = buf
            .cell(position)
            .map_or(1, |c| UnicodeWidthStr::width(c.symbol()).max(1));
        for dx in 0..u16::try_from(width).unwrap_or(1) {
            let p = Position::new(position.x + dx, position.y);
            if !area.contains(p) {
                break;
            }
            self.paint(buf, p);
        }
        Report {
            position,
            redraw_after,
        }
    }

    /// Styles one cell.
    fn paint(self, buf: &mut Buffer, p: Position) {
        let Some(cell) = buf.cell_mut(p) else { return };
        match self.shape {
            Shape::Block => {
                let ink = match self.ink {
                    Ink::Auto => contrast_ink(self.color),
                    Ink::Fixed(c) => c,
                };
                cell.set_bg(self.color).set_fg(ink);
            }
            Shape::Bar => {
                cell.set_symbol("\u{258f}").set_fg(self.color);
            }
            Shape::Underline => {
                use ratatui::style::{Modifier, Style};
                cell.set_style(
                    Style::default()
                        .add_modifier(Modifier::UNDERLINED)
                        .underline_color(self.color),
                );
            }
        }
    }
}

/// The nearest position inside `area`.
fn clamp(area: Rect, pos: Position) -> Position {
    if area.is_empty() {
        return Position::new(area.x, area.y);
    }
    Position::new(
        pos.x.clamp(area.x, area.right() - 1),
        pos.y.clamp(area.y, area.bottom() - 1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const GREEN: Color = Color::Rgb(0x87, 0xaf, 0x5f);

    fn area() -> Rect {
        Rect::new(0, 0, 6, 1)
    }

    #[test]
    fn block_sets_the_background_and_keeps_the_glyph() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let report =
            Cursor::new(GREEN).render(area(), &mut buf, Position::new(2, 0), Instant::now());
        let cell = buf.cell(Position::new(2, 0)).unwrap();
        assert_eq!(cell.symbol(), "c");
        assert_eq!(cell.bg, GREEN);
        assert_eq!(report.position, Position::new(2, 0));
        assert_eq!(report.redraw_after, None);
    }

    #[test]
    fn block_flips_a_same_coloured_glyph_to_readable_ink() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        buf.cell_mut(Position::new(2, 0)).unwrap().set_fg(GREEN);
        let _ = Cursor::new(GREEN).render(area(), &mut buf, Position::new(2, 0), Instant::now());
        let cell = buf.cell(Position::new(2, 0)).unwrap();
        assert_ne!(cell.fg, GREEN, "the glyph would be invisible");
        assert_eq!(cell.fg, Color::Rgb(0x11, 0x11, 0x11));
    }

    #[test]
    fn fixed_ink_overrides_the_contrast_pick() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let _ = Cursor::new(GREEN)
            .ink(Ink::Fixed(Color::Rgb(1, 2, 3)))
            .render(area(), &mut buf, Position::new(0, 0), Instant::now());
        assert_eq!(
            buf.cell(Position::new(0, 0)).unwrap().fg,
            Color::Rgb(1, 2, 3)
        );
    }

    #[test]
    fn a_wide_glyph_gets_both_of_its_cells() {
        // The CJK glyph occupies columns 0 and 1; ratatui stores the trailing
        // cell as an empty symbol. Covering only the first tears the row.
        let mut buf = Buffer::with_lines(vec!["\u{4f60}abcd"]);
        let _ = Cursor::new(GREEN).render(area(), &mut buf, Position::new(0, 0), Instant::now());
        assert_eq!(buf.cell(Position::new(0, 0)).unwrap().bg, GREEN);
        assert_eq!(
            buf.cell(Position::new(1, 0)).unwrap().bg,
            GREEN,
            "spacer cell left behind"
        );
    }

    #[test]
    fn a_position_outside_the_area_paints_nothing_but_still_reports() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let report =
            Cursor::new(GREEN).render(area(), &mut buf, Position::new(99, 0), Instant::now());
        assert_eq!(buf.cell(Position::new(5, 0)).unwrap().bg, Color::Reset);
        assert_eq!(
            report.position,
            Position::new(5, 0),
            "clamped into the area"
        );
    }

    #[test]
    fn underline_preserves_the_cell_and_colours_the_line() {
        use ratatui::style::Modifier;
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        buf.cell_mut(Position::new(2, 0))
            .unwrap()
            .set_fg(Color::Blue);
        let _ = Cursor::new(GREEN).shape(Shape::Underline).render(
            area(),
            &mut buf,
            Position::new(2, 0),
            Instant::now(),
        );
        let cell = buf.cell(Position::new(2, 0)).unwrap();
        assert_eq!(cell.symbol(), "c", "glyph must survive");
        assert_eq!(cell.fg, Color::Blue, "foreground must survive");
        assert_eq!(cell.bg, Color::Reset, "background must survive");
        assert!(cell.modifier.contains(Modifier::UNDERLINED));
        assert_eq!(cell.underline_color, GREEN);
    }

    #[test]
    fn blink_paints_in_the_on_phase_and_skips_the_off_phase() {
        let since = Instant::now();
        let period = Duration::from_millis(500);
        let mut on = Buffer::with_lines(vec!["abcdef"]);
        let _ = Cursor::new(GREEN)
            .blink(Blink::On { period, since })
            .render(
                area(),
                &mut on,
                Position::new(0, 0),
                since + Duration::from_millis(100),
            );
        assert_eq!(
            on.cell(Position::new(0, 0)).unwrap().bg,
            GREEN,
            "first half is on"
        );

        let mut off = Buffer::with_lines(vec!["abcdef"]);
        let _ = Cursor::new(GREEN)
            .blink(Blink::On { period, since })
            .render(
                area(),
                &mut off,
                Position::new(0, 0),
                since + Duration::from_millis(600),
            );
        assert_eq!(
            off.cell(Position::new(0, 0)).unwrap().bg,
            Color::Reset,
            "second half is off"
        );
    }

    #[test]
    fn redraw_after_counts_down_to_the_next_phase_boundary() {
        let since = Instant::now();
        let period = Duration::from_millis(500);
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let report = Cursor::new(GREEN)
            .blink(Blink::On { period, since })
            .render(
                area(),
                &mut buf,
                Position::new(0, 0),
                since + Duration::from_millis(400),
            );
        assert_eq!(report.redraw_after, Some(Duration::from_millis(100)));
    }

    #[test]
    fn a_steady_cursor_asks_for_no_redraw() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let report =
            Cursor::new(GREEN).render(area(), &mut buf, Position::new(0, 0), Instant::now());
        assert_eq!(report.redraw_after, None);
    }

    #[test]
    fn bar_replaces_the_glyph_which_is_documented_and_lossy() {
        let mut buf = Buffer::with_lines(vec!["abcdef"]);
        let _ = Cursor::new(GREEN).shape(Shape::Bar).render(
            area(),
            &mut buf,
            Position::new(2, 0),
            Instant::now(),
        );
        let cell = buf.cell(Position::new(2, 0)).unwrap();
        assert_eq!(cell.symbol(), "\u{258f}");
        assert_eq!(cell.fg, GREEN);
        assert_eq!(cell.bg, Color::Reset, "the bar tints ink, not ground");
    }
}
