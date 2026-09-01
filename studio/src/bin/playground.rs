//! # playground — somewhere to try things
//!
//! ```text
//!     cargo run -p studio --release --bin playground
//! ```
//!
//! Two ways to get at the library, and they are equivalent:
//!
//! ```text
//!     use studio::prelude::*;                   // everything, one line
//!
//!     use plotkit::{Cx, Frame};                 // or name what you want
//!     use shapes::{digit, face};
//!     use studio::Graph;
//! ```
//!
//! Where things live, since it is reasonable to wonder:
//!
//! | | |
//! |---|---|
//! | `plotkit` | the drawing — `Cx`, `Shape`, `Frame`, `View`, `Canvas` |
//! | `shapes` | things to draw — digits, faces, cyclones, waves |
//! | `studio` | the window — `Graph`, `Sketch`, `Keys`, `Tape` |
//!
//! `Frame` is plotkit's because a frame is a drawing. `Graph` is studio's
//! because it owns a window. Nothing is defined twice.

use plotkit::{Cx, Frame};
use shapes::{digit, face};
use studio::Graph;

fn main() {
    Graph::new("playground").animate(scene);
}

fn scene(t: f64) -> Frame {
    let mut f = Frame::new();
    f.place(face::smiley(1.0), Cx::polar(3.0, t));
    f.place(digit::glyph(7, 40), Cx::ZERO);
    f
}
