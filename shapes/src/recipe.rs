//! # Recipes — a shape, plus how it was built
//!
//! A [`Shape`] is the finished picture. A [`Recipe`] is the picture *and the
//! working*: an ordered list of steps, each one a sentence of mathematics and
//! the piece of geometry that sentence produces.
//!
//! ```text
//!   smiley
//!     1. the face: |z| = r
//!     2. eyes at ±0.36r + 0.28i·r, each of radius 0.09r
//!     3. the mouth: 0.58r·e^{iθ} for θ from 3.6 to 5.8 rad
//! ```
//!
//! This is what lets `cargo run -p shapes -- smiley --steps` show the shape
//! being drawn one construction line at a time instead of just appearing.
//! Every shape in this crate is defined *as* a recipe, and its plain `Shape`
//! is the recipe with the commentary dropped — so the two can never drift
//! apart, because there is only one of them.

use plotkit::{Cx, Shape};

/// One construction line.
#[derive(Clone)]
pub struct Step {
    /// The mathematics, in words. What you would say aloud while drawing it.
    pub says: String,
    pub shape: Shape,
    pub colour: u32,
}

/// Colours steps cycle through, so consecutive construction lines are told
/// apart at a glance.
pub const STEP_COLOURS: [u32; 6] = [0x4FBCD4, 0xE0A44A, 0xE585AC, 0x6FCF97, 0x9B7BD4, 0xE0704A];

#[derive(Clone)]
pub struct Recipe {
    pub name: String,
    /// The one line of mathematics the whole shape comes down to.
    pub maths: String,
    pub steps: Vec<Step>,
}

impl Recipe {
    pub fn new(name: impl Into<String>, maths: impl Into<String>) -> Recipe {
        Recipe { name: name.into(), maths: maths.into(), steps: Vec::new() }
    }

    /// Add a construction line. Chains, so a recipe reads top to bottom like
    /// the instructions it is.
    pub fn step(mut self, says: impl Into<String>, shape: Shape) -> Recipe {
        let colour = STEP_COLOURS[self.steps.len() % STEP_COLOURS.len()];
        self.steps.push(Step { says: says.into(), shape, colour });
        self
    }

    /// Everything drawn so far after `n` steps. `upto(len())` is the finished
    /// shape.
    pub fn upto(&self, n: usize) -> Shape {
        Shape::group(self.steps.iter().take(n).map(|s| s.shape.clone()).collect::<Vec<_>>())
    }

    /// The finished shape, with the commentary dropped.
    pub fn shape(&self) -> Shape {
        self.upto(self.steps.len())
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// Apply a map to every step, so the construction lines travel with the
    /// thing they build. [`crate::Place`] is written in terms of this.
    pub fn map_all(mut self, f: impl Fn(Cx) -> Cx + Send + Sync + Clone + 'static) -> Recipe {
        for s in &mut self.steps {
            s.shape = s.shape.clone().map(f.clone());
        }
        self
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn r() -> Recipe {
        Recipe::new("t", "m")
            .step("a", Shape::point(Cx::ZERO))
            .step("b", Shape::point(Cx::new(1.0, 0.0)))
            .step("c", Shape::point(Cx::new(2.0, 0.0)))
    }

    /// ★ The finished shape and the last step of the recipe are the same
    /// object, so the picture can never disagree with its own working.
    #[test]
    fn the_shape_is_the_last_step_of_the_recipe() {
        let r = r();
        let count = |s: &Shape| s.polylines(Cx::new(-9.0, -9.0), Cx::new(9.0, 9.0), 200).len();
        assert_eq!(count(&r.shape()), count(&r.upto(r.len())));
        assert_eq!(count(&r.upto(0)), 0);
        assert_eq!(count(&r.upto(2)), 2);
    }

    #[test]
    fn steps_take_different_colours() {
        let r = r();
        assert_ne!(r.steps[0].colour, r.steps[1].colour);
    }

    /// Placing a recipe moves every step, not just the finished shape —
    /// otherwise the construction lines would drift away from what they build.
    #[test]
    fn placing_moves_every_step_together() {
        let p = r().map_all(|z| z.scale(2.0) + Cx::new(5.0, 0.0));
        let where_ = |s: &Shape| s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 200)[0][0];
        assert!((where_(&p.steps[0].shape) - Cx::new(5.0, 0.0)).abs() < 1e-12);
        assert!((where_(&p.steps[1].shape) - Cx::new(7.0, 0.0)).abs() < 1e-12);
    }
}
