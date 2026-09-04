# nano-cursor

Draw a terminal cursor as a styled cell in a [ratatui] buffer, so its colour
does not depend on whether the terminal honours OSC 12.

Terminals may ignore OSC 12, and some do. Warp renders the cursor as a widget
in its own UI, coloured from its theme, so no escape sequence can reach it: the
same OSC 12 that recolours the cursor in Ghostty does nothing there. This crate
sidesteps the negotiation by painting the cursor into the frame you are already
drawing.

## Usage

```rust
use nano_cursor::{Cursor, Shape};
use ratatui::style::Color;
use std::time::Instant;

// `area` is the widget's Rect, `buf` its Buffer, `caret` the logical
// cursor position — all already in hand after the widget has drawn its text.
let report = Cursor::new(Color::Rgb(0x87, 0xaf, 0x5f))
    .shape(Shape::Block)
    .render(area, buf, caret, Instant::now());

// Keep the real cursor hidden but positioned at `report.position`, or IME
// candidate placement and screen-reader focus break.
```

`render` must run **after** the text is painted into the buffer: the contrast
pick reads the glyph already in the cell.

## Known properties

- `Shape::Bar` replaces the glyph in its cell rather than sitting between
  cells. In an input widget the caret usually rests on a blank cell, so this
  rarely shows.
- Contrast over the 16 named ANSI colours is approximate. Those colours are
  defined by the user's terminal theme and cannot be read from the process, so
  the xterm defaults are assumed.
- You must position the hidden real cursor from `Report::position`. Terminals
  place the IME candidate window and screen-reader focus there, whether or not
  a cursor is drawn.

[ratatui]: https://ratatui.rs
