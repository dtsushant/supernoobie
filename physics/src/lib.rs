//! # physics — how things move
//!
//! Separate from [`shapes`](../shapes/index.html) on purpose. `shapes` knows
//! what things **look like**; this knows what they **do**. A `Cyclone` is a
//! drawing that turns; an [`Oscillator`] is an equation that does not care
//! whether anybody is watching.
//!
//! Nothing here draws anything of its own, and nothing here needs a window —
//! so every claim in it can be checked in the dark, which is the only way to
//! be sure a simulation is right rather than merely plausible.
//!
//! ## What is in here
//!
//! | | the formula | who |
//! |---|---|---|
//! | [`oscillator`] | `ẍ + 2ζω ẋ + ω² x = f` | Laplace, and Heaviside who made it useful |
//! | [`fall`] | `s = s₀ + v₀t + ½gt²` | Galileo, by slowing gravity down until he could time it |
//! | [`trigger`] | edges, not levels | — the small thing that turns a moving number into an event |
//!
//! ## Why the comments read like a history book
//!
//! Because knowing that `ζ = 1` is where two poles collide is worth more than
//! knowing that `zeta` should be about `1`. Every module here says: **the
//! formula, what it does, who found it and how, and what it is used for in
//! this repository.** The intent is that the engineering is never a black box
//! you are trusting — it is a thing you could re-derive on paper.
//!
//! ## The thread running through both
//!
//! ```text
//!     e^{−t/τ}
//! ```
//!
//! A branch settling after a gust. A raindrop approaching terminal velocity. A
//! cyclone spending its circulation. All the same shape, because all three are
//! *"the rate of change is proportional to how far there is left to go"* —
//! which is the one differential equation worth recognising on sight.
//!
//! ## And the thread to the rest of the repository
//!
//! [`shapes::fourier`](../shapes/fourier/index.html) breaks things into
//! `e^{iωt}` — oscillation with no growth. Laplace uses `e^{st}` with
//! `s = σ + iω` — oscillation **times** growth or decay. **Fourier is Laplace
//! on the imaginary axis.**
//!
//! * Fourier answers *"what is this made of?"*
//! * Laplace answers *"how does this respond to being pushed?"*
//!
//! ## Going to three dimensions
//!
//! Neither module has a dimension in it. [`Oscillator`] moves one number, and
//! three of them side by side is a mass on a spring in space — or, with the
//! same equation on each axis of a quaternion, a rotation settling. [`fall`]
//! carries a `Cx` only for convenience; the mathematics is a scalar
//! `½gt²` along one direction, and the direction can be anything.

pub mod fall;
pub mod oscillator;
pub mod trigger;

pub use fall::Fall;
pub use oscillator::Oscillator;
pub use trigger::{Edge, Trigger};
