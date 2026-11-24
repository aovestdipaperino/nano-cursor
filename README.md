<img src="https://raw.githubusercontent.com/aovestdipaperino/nano-cursor/main/assets/logo.png" alt="" width="96" align="right">

# nano-cursor

[![GitHub stars](https://img.shields.io/github/stars/aovestdipaperino/nano-cursor?logo=github&color=brightgreen)](https://github.com/aovestdipaperino/nano-cursor/stargazers)

Draw a terminal cursor as a styled cell in a [ratatui] buffer, so its colour
does not depend on whether the terminal honours OSC 12.

[![crates.io](https://img.shields.io/crates/v/nano-cursor.svg)](https://crates.io/crates/nano-cursor)
[![docs.rs](https://docs.rs/nano-cursor/badge.svg)](https://docs.rs/nano-cursor)
[![MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

## Why

The usual way to colour a terminal cursor is the OSC 12 escape sequence —
`ESC ] 12 ; #rrggbb BEL` — and its companion DECSCUSR for shape and blink.
Both are *requests*. The terminal is free to ignore them, and some do.

Warp is the case that motivated this crate. It does not emulate a character
grid and then draw a cursor cell in it; it renders the cursor as a widget in
its own UI, coloured from the `cursor` field of its theme (which defaults to
the theme accent). There is no grid cell for OSC 12 to recolour and no escape
path into that widget. The same sequence that recolours the cursor in Ghostty
does nothing at all in Warp — it is not an error, it is silence, which is the
worst failure mode for a visual indicator: it works on your machine.

If you are using the cursor to carry meaning — idle versus busy, insert versus
normal mode, connected versus disconnected — then "works on some terminals" is
not good enough. The only way out is to stop asking the terminal and paint the
cursor yourself, into the frame you are already drawing. That is all this crate
does.

## How

`render` is a **post-pass over an already-painted buffer**, not a widget you
compose. You draw your text as usual, then hand the buffer back:

```rust
use nano_cursor::{Cursor, Shape};
use ratatui::style::Color;
use std::time::Instant;

// `area` is the text region's Rect, `buf` its Buffer, `caret` the cell the
// cursor sits on — all already in hand once the widget has drawn its text.
let report = Cursor::new(Color::Rgb(0x87, 0xaf, 0x5f))
    .shape(Shape::Block)
    .render(area, buf, caret, Instant::now());
```

The ordering is not a style preference. `render` **reads** the cell it is about
to paint, and it needs two things that only exist after the text is there:

- **the glyph's colour**, so it can re-ink it. A green block over green text
  erases the character. `Ink::Auto` computes the WCAG relative luminance of
  your cursor colour and picks near-black or near-white accordingly, so the
  character under the cursor stays readable whatever colour you choose.
- **the glyph's width**, so a double-width CJK character or emoji gets *both*
  of its cells painted instead of being sliced down the middle.

Because the cursor is now yours, the terminal's own cursor must get out of the
way — and that is the one thing you cannot skip:

```rust
// Do NOT call `frame.set_cursor_position`. With no cursor position set,
// ratatui emits `Hide`. Then move the hidden cursor yourself:
queue!(stdout, MoveTo(report.position.x, report.position.y))?;
```

Hidden is not the same as absent. Terminals place the IME candidate window and
screen-reader focus at the cursor position they are tracking, whether or not
anything is drawn there. Hiding without moving is how you break CJK input, so
`Report` is `#[must_use]` to make that hard to forget.

### Shapes and blink

`Shape::Block` fills the cell. `Shape::Underline` adds an underline in your
colour and touches nothing else. `Shape::Bar` draws a left edge.

Blink is opt-in, and the clock is yours:

```rust
Cursor::new(color)
    .blink(Blink::On { period: Duration::from_millis(530), since: last_keypress })
    .render(area, buf, caret, Instant::now());
```

`since` is a parameter rather than crate-internal state for two reasons: a
library-owned epoch is hidden state and untestable timing, and real terminals
restart the blink on each keystroke — which you get for free by passing the
time of the last key. The returned `Report::redraw_after` tells you how long
until the cursor's appearance next changes, so an event loop can shorten its
poll timeout instead of sampling the blink on whatever cadence it happened to
have.

## Known properties

- `Shape::Bar` replaces the glyph in its cell rather than sitting between
  cells the way a terminal's own bar cursor does. In an input widget the caret
  usually rests on a blank cell, so this rarely shows.
- Contrast over the 16 *named* ANSI colours is approximate. Those colours are
  defined by the user's terminal theme and cannot be read from inside the
  process, so the xterm defaults are assumed. Indexed and RGB colours are
  exact.
- A caret outside `area` paints nothing, but still reports a position clamped
  into `area`.
- Painting the cursor takes cursor presentation away from the terminal, so the
  user's own cursor preferences no longer apply. Consider making it opt-in
  rather than replacing the OSC 12 path outright.

## License

MIT. See [LICENSE](LICENSE).

[ratatui]: https://ratatui.rs
