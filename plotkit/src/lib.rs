//! # plotkit — draw mathematics on a CPU framebuffer
//!
//! A small, dependency-free toolkit for putting equations on screen. It owns a
//! `Vec<u32>`, a world-to-screen mapping, an annotation pen, and a tiny
//! expression language — and nothing else. No GPU, no windowing, no `std`
//! beyond the basics.
//!
//! ```text
//!   shape     geometry as VALUES you can .map()
//!   frame     a layer of styled shapes;  animation is f(t) -> Frame
//!   expr      text -> values and drawing commands
//!     |
//!   script    commands -> plot calls
//!     |
//!   plot      curves, graph paper, implicit equations   (world coordinates)
//!   pen       dimensions, angle arcs, radii, arrows     (world coordinates)
//!     |
//!   view      world -> screen.  origin anywhere, y counts UP
//!     |
//!   raster    pixels. origin top-left, y counts DOWN
//! ```
//!
//! **The layer that matters is [`view`].** Everything above it speaks in world
//! units — "a circle of radius 1 at the origin" — and everything below it is
//! pixels. `View::new` puts the origin bottom-left at one pixel per unit;
//! `View::centred` puts it in the middle at however many pixels per unit you
//! ask for, which is what mathematics wants.
//!
//! ## The smallest useful program
//!
//! ```no_run
//! use plotkit::{raster::Canvas, view::View, script};
//!
//! let v = View::centred(800, 600, 90.0);      // 90 px per unit, origin centred
//! let mut c = Canvas::new(800, 600);
//! c.clear(0x0B1017);
//!
//! script::run(&mut c, &v, "
//!     a = 0 + 0i
//!     b = 1 + 2i
//!     polygon(a, b)
//!     circle(0, 1)
//!     plot(sin(x))
//! ", &script::Style::default());
//!
//! c.write_png("out.png").unwrap();
//! ```
//!
//! ## Why complex numbers rather than a point type
//!
//! Every position, offset and scalar is one [`complex::Cx`]. `a + b` means the
//! same thing for all of them, rotation is `z * e^(i t)`, scaling is `2z`, and
//! a whole affine transform is `a*z + b` written exactly like that. A separate
//! vector type would need all of that spelled out again and would buy nothing.

pub mod dice;
pub mod ludo;
pub mod complex;
pub mod expr;
pub mod frame;
pub mod pen;
pub mod plot;
pub mod raster;
pub mod script;
pub mod shape;
pub mod view;

pub use complex::Cx;
pub use raster::{colour, Canvas};
pub use frame::{Anchor, Frame, Placeable, Style};
pub use shape::Shape;
pub use view::View;
