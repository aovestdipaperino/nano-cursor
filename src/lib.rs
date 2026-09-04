//! Draw a terminal cursor as a styled cell in a ratatui buffer.
//!
//! Terminals are free to ignore OSC 12, and some do: Warp renders the cursor
//! as a widget in its own UI and colours it from its theme, so no escape can
//! reach it. This crate sidesteps the negotiation by painting the cursor into
//! the frame the application is already drawing.
#![warn(missing_docs)]

mod color;
mod cursor;

pub use cursor::{Cursor, Ink, Report, Shape};
