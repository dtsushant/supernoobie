//! # terrain — things standing on the ground, and what knocks them down
//!
//! A field of trees on a plane. Each one stands up out of the ground, so it
//! projects like everything else and recedes with the plane rather than being
//! stuck on the glass.
//!
//! ## Scattered without a random number
//!
//! The positions come from the **R2 sequence**: take an irrational `a` and put
//! the `k`-th point at the fractional part of `k·a`. Do that in two dimensions
//! with the two irrationals from the *plastic number* — the root of
//! `g³ = g + 1` — and you get a scatter that looks random but has no clumps
//! and no gaps, because a point can only land near an earlier one if `k·a` is
//! nearly a whole number, and an irrational never obliges.
//!
//! Same reason [`crate::motion::wander_at`] never repeats, put to a different
//! job. And the same payoff: it is a pure function of the index, so a recorded
//! run lays the trees out identically without taping a seed.
//!
//! ## What knocks a tree down
//!
//! Not "the storm is close". A vortex carries a wind speed that falls off with
//! distance,
//!
//! ```text
//!     v(r) = Γ / 2πr
//! ```
//!
//! and a tree goes over when the wind where it stands beats what it can take.
//! So the reach of the damage is not a number anybody chose — it follows:
//!
//! ```text
//!     r = Γ / (2π · strength)
//! ```
//!
//! which means a storm losing its circulation stops being able to flatten
//! anything, all by itself.

use plotkit::{Cx, Shape};

/// One tree.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    /// Where it stands on the ground.
    pub at: Cx,
    pub height: f64,
    /// The wind it can take before it goes over.
    pub strength: f64,
    /// When it fell, and which way — `None` while it is still standing.
    pub fell: Option<(f64, Cx)>,
}

impl Tree {
    /// How far over it has gone, in radians from upright.
    ///
    /// It does not snap flat: it takes about a second to go down, which is
    /// what makes a passing storm read as a wave going through the trees
    /// rather than a row of switches being flipped.
    pub fn lean(&self, t: f64) -> f64 {
        match self.fell {
            None => 0.0,
            Some((when, _)) => (t - when).max(0.0).min(1.0) * std::f64::consts::FRAC_PI_2,
        }
    }

    /// Where its tip is, in space: `(x, y, z)`.
    pub fn tip(&self, t: f64) -> (f64, f64, f64) {
        let lean = self.lean(t);
        let along = self.fell.map_or(Cx::ZERO, |(_, d)| d).scale(self.height * lean.sin());
        (self.at.re + along.re, self.at.im + along.im, self.height * lean.cos())
    }

    pub fn standing(&self) -> bool {
        self.fell.is_none()
    }
}

/// A field of trees.
#[derive(Clone, Debug)]
pub struct Field {
    pub trees: Vec<Tree>,
}

impl Field {
    /// `n` trees scattered over `±extent`, evenly but without a pattern.
    pub fn new(n: usize, extent: f64) -> Field {
        Field {
            trees: scatter(n, extent)
                .into_iter()
                .enumerate()
                .map(|(k, at)| Tree {
                    at,
                    // A little variety, from the same sequence rather than
                    // from a generator that would need taping.
                    height: 0.5 + 0.5 * frac(k as f64 * A1 + 0.31),
                    // In the same units as the wind, which is `Γ/2πr` — so
                    // these are small numbers, and a storm of Γ ≈ 4 can fell
                    // the weakest out to about five units and the toughest
                    // only close in.
                    strength: 0.12 + 0.5 * frac(k as f64 * A2 + 0.77),
                    fell: None,
                })
                .collect(),
        }
    }

    /// Blow. Anything that cannot stand the wind where it stands goes over.
    ///
    /// `wind` is asked for the speed at a ground position — for a vortex that
    /// is `Γ/2πr`, so the reach follows from the circulation rather than being
    /// chosen.
    ///
    /// A tree that is already down stays down. Storms do not put trees back.
    pub fn blow(&mut self, t: f64, wind: impl Fn(Cx) -> f64) {
        for tree in &mut self.trees {
            if tree.fell.is_some() {
                continue;
            }
            if wind(tree.at) > tree.strength {
                // It goes over the way the wind is going — round the storm,
                // not away from it, which is why fallen trees near a tornado
                // lie in a curve rather than pointing outward.
                tree.fell = Some((t, curl(tree.at)));
            }
        }
    }

    /// How many are still up.
    pub fn standing(&self) -> usize {
        self.trees.iter().filter(|t| t.standing()).count()
    }

    /// The trees still up, and the ones down — separately, so they can be
    /// drawn differently.
    ///
    /// `project` takes a point in space and returns where it lands on the
    /// page. Passing it in keeps this module knowing nothing about cyclones,
    /// cameras or tilts.
    pub fn shapes(&self, t: f64, project: impl Fn(f64, f64, f64) -> Cx) -> (Shape, Shape) {
        self.shapes_if(t, project, |_| true)
    }

    /// The same, but only the trees you pick.
    ///
    /// For depth: draw the ones further off than something, then the thing,
    /// then the ones nearer. Without that a storm is painted over the trees
    /// standing in front of it, which reads as the trees being inside it.
    pub fn shapes_if(
        &self,
        t: f64,
        project: impl Fn(f64, f64, f64) -> Cx,
        keep: impl Fn(&Tree) -> bool,
    ) -> (Shape, Shape) {
        let mut up = Vec::new();
        let mut down = Vec::new();
        for tree in self.trees.iter().filter(|tr| keep(tr)) {
            let (x, y, z) = tree.tip(t);
            let trunk = Shape::path(vec![project(tree.at.re, tree.at.im, 0.0), project(x, y, z)]);
            if tree.standing() {
                up.push(trunk);
            } else {
                down.push(trunk);
            }
        }
        (Shape::group(up), Shape::group(down))
    }
}

// The plastic number: the root of g^3 = g + 1. Its powers give the two
// irrationals the R2 sequence needs.
const G: f64 = 1.324_717_957_244_746;
const A1: f64 = 1.0 / G;
const A2: f64 = 1.0 / (G * G);

fn frac(x: f64) -> f64 {
    x - x.floor()
}

/// `n` points over `±extent`, evenly spread and looking unplanned.
///
/// See the module note: the `k`-th point is at `frac(k·a)` for irrational `a`,
/// which cannot clump because clumping would need `k·a` to be nearly whole.
pub fn scatter(n: usize, extent: f64) -> Vec<Cx> {
    (0..n)
        .map(|k| {
            let k = k as f64 + 1.0;
            Cx::new(extent * (2.0 * frac(0.5 + A1 * k) - 1.0), extent * (2.0 * frac(0.5 + A2 * k) - 1.0))
        })
        .collect()
}

/// The direction the wind is going at a point, for a vortex at the origin of
/// the offset — round it, not away from it.
///
/// A quarter turn from "outward" is `i` times it. That is all a circulation
/// is: the flow perpendicular to the radius.
fn curl(from_storm: Cx) -> Cx {
    if from_storm.abs() < 1e-9 {
        Cx::new(1.0, 0.0)
    } else {
        (from_storm.unit() * Cx::I).unit()
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    /// ★ Scattered evenly but without a pattern: no two trees on top of each
    /// other, and none of the plane left bare. A generator seeded from the
    /// clock would clump; a grid would look like a grid.
    #[test]
    fn the_scatter_has_no_clumps_and_no_gaps() {
        let p = scatter(400, 10.0);

        let mut closest = f64::MAX;
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                closest = closest.min((*a - *b).abs());
            }
        }
        assert!(closest > 0.25, "two trees almost on top of each other: {closest}");

        // Every cell of a 4x4 grid over the field gets something.
        let mut cells = [[false; 4]; 4];
        for q in &p {
            let cell = |v: f64| (((v + 10.0) / 20.0 * 4.0) as usize).min(3);
            cells[cell(q.re)][cell(q.im)] = true;
        }
        assert!(cells.iter().flatten().all(|c| *c), "part of the field has no trees at all");
    }

    /// ★ And it is a pure function of the index, so a recorded run lays the
    /// same field out without taping a seed.
    #[test]
    fn the_same_field_comes_back_every_time() {
        assert_eq!(scatter(50, 7.0), scatter(50, 7.0));
        let (a, b) = (Field::new(30, 8.0), Field::new(30, 8.0));
        for (x, y) in a.trees.iter().zip(&b.trees) {
            assert_eq!(x.at, y.at);
            assert_eq!(x.height, y.height);
            assert_eq!(x.strength, y.strength);
        }
    }

    /// ★ What falls is decided by the wind where it stands, not by a distance
    /// somebody picked. So the reach of the damage follows from `Γ` — and a
    /// storm that loses its circulation stops being able to flatten anything,
    /// with nothing extra written to make that happen.
    #[test]
    fn the_reach_of_the_damage_follows_from_the_circulation() {
        let vortex = |gamma: f64| move |p: Cx| gamma / (TAU * p.abs().max(0.3));

        let mut weak = Field::new(300, 12.0);
        weak.blow(0.0, vortex(0.5));

        let mut strong = Field::new(300, 12.0);
        strong.blow(0.0, vortex(5.0));

        assert!(strong.standing() < weak.standing(), "a stronger storm should flatten more");
        assert!(weak.standing() > 0, "a weak one should not flatten everything");

        // And what fell is near the middle, because that is where the wind is.
        let far = strong.trees.iter().filter(|t| !t.standing()).fold(0.0f64, |m, t| m.max(t.at.abs()));
        let out = strong.trees.iter().filter(|t| t.standing()).fold(f64::MAX, |m, t| m.min(t.at.abs()));
        assert!(far > out * 0.5, "the damage should be a region, not a scatter");
    }

    /// A storm with no circulation left knocks nothing down. This is the whole
    /// point of tying damage to the wind rather than to proximity.
    #[test]
    fn a_spent_storm_flattens_nothing() {
        let mut f = Field::new(200, 10.0);
        let before = f.standing();
        f.blow(0.0, |p| 0.001 / (TAU * p.abs().max(0.3)));
        assert_eq!(f.standing(), before);
    }

    /// Storms do not put trees back up.
    #[test]
    fn what_is_down_stays_down() {
        let mut f = Field::new(100, 6.0);
        f.blow(0.0, |_| 1e6); // flatten everything
        assert_eq!(f.standing(), 0);
        f.blow(1.0, |_| 0.0); // dead calm
        assert_eq!(f.standing(), 0, "the calm put them back up");
    }

    /// ★ A tree goes down over about a second rather than snapping flat, so a
    /// passing storm reads as a wave going through the trees instead of a row
    /// of switches being flipped.
    #[test]
    fn a_tree_takes_a_moment_to_go_over() {
        let t = Tree { at: Cx::new(2.0, 0.0), height: 1.0, strength: 1.0, fell: Some((10.0, Cx::new(0.0, 1.0))) };
        assert!(t.lean(10.0).abs() < 1e-12, "upright at the instant it goes");
        assert!(t.lean(10.5) > 0.5 && t.lean(10.5) < 1.2, "halfway over halfway through");
        assert!((t.lean(11.0) - std::f64::consts::FRAC_PI_2).abs() < 1e-12, "flat after a second");
        assert!((t.lean(90.0) - std::f64::consts::FRAC_PI_2).abs() < 1e-12, "and no further, ever");
    }

    /// Upright, a tree's tip is straight above its foot. Flat, it is a whole
    /// height away along the ground and no height at all.
    #[test]
    fn a_tree_lies_down_where_it_should() {
        let at = Cx::new(3.0, -1.0);
        let up = Tree { at, height: 2.0, strength: 1.0, fell: None };
        assert_eq!(up.tip(5.0), (3.0, -1.0, 2.0));

        let over = Tree { fell: Some((0.0, Cx::new(1.0, 0.0))), ..up };
        let (x, y, z) = over.tip(1.0);
        assert!((x - 5.0).abs() < 1e-9 && (y + 1.0).abs() < 1e-9, "it should lie along the ground");
        assert!(z.abs() < 1e-9, "and flat");
    }

    /// ★ Trees near a tornado lie in a curve, not pointing outward, because
    /// the wind goes **round** the storm. A quarter turn from outward is `i`
    /// times it — which is the whole definition of a circulation.
    #[test]
    fn trees_fall_across_the_wind_not_away_from_it() {
        for angle in [0.0, 0.9, 2.2, 4.7] {
            let out = Cx::expi(angle);
            let way = curl(out.scale(3.0));
            assert!(way.dot(out).abs() < 1e-9, "it fell outward instead of round");
            assert!((way - out * Cx::I).abs() < 1e-9, "a quarter turn from outward is i times it");
        }
    }

    #[test]
    fn a_tree_at_the_very_middle_still_falls_somewhere() {
        assert!((curl(Cx::ZERO).abs() - 1.0).abs() < 1e-12, "it needs some direction");
    }

    /// ★ Depth: everything the storm is drawn over must be behind it, and
    /// everything drawn after must be in front. Painting a storm over the
    /// trees standing between you and it reads as the trees being inside it.
    #[test]
    fn a_field_can_be_split_at_a_depth() {
        let f = Field::new(200, 10.0);
        let flat = |x: f64, y: f64, z: f64| Cx::new(x, y + z);
        let count = |s: &Shape| s.polylines(Cx::new(-99.0, -99.0), Cx::new(99.0, 99.0), 400).len();

        let (far, _) = f.shapes_if(0.0, flat, |tr| tr.at.im >= 0.0);
        let (near, _) = f.shapes_if(0.0, flat, |tr| tr.at.im < 0.0);
        let (all, _) = f.shapes(0.0, flat);

        assert_eq!(count(&far) + count(&near), count(&all), "the split lost or duplicated trees");
        assert!(count(&far) > 0 && count(&near) > 0, "the split should actually divide them");
    }

    #[test]
    fn an_empty_field_is_harmless() {
        let mut f = Field::new(0, 5.0);
        f.blow(1.0, |_| 1e9);
        assert_eq!(f.standing(), 0);
    }
}
