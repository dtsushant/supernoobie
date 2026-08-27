//! # A spatial hash — finding neighbours without asking everyone
//!
//! Every simulation so far has compared every pair. `rigid.rs` says so out
//! loud: *"brute-force O(n^2) pair test, fine to a few hundred bodies"*. At
//! 500 bodies that is 125,000 tests a step; at 5,000 it is 12.5 million, and
//! the frame is gone.
//!
//! The fix is embarrassingly simple. **Chop space into square cells the size
//! of your interaction radius.** Then anything close enough to matter is in
//! your cell or one of the eight around it, and everything else can be ignored
//! without being looked at.
//!
//! ```text
//!   +-----+-----+-----+      a particle in the middle cell can only
//!   |  .  |  .. |     |      interact with the 3x3 block around it,
//!   +-----+-----+-----+      because the cell side equals the radius
//!   | .   | (*) |  .  |
//!   +-----+-----+-----+      -> cost per particle depends on DENSITY,
//!   |     | .   | ..  |         not on how many particles exist
//!   +-----+-----+-----+
//! ```
//!
//! Cost per particle becomes proportional to how many neighbours it actually
//! has, so the whole sweep is O(n) rather than O(n^2) — for evenly spread
//! particles, at least. All of them piled into one cell and you are back where
//! you started, which is why the cell size should track the interaction
//! radius rather than the world size.
//!
//! Cells are keyed by integer coordinates in a hash map, so the world can be
//! unbounded and negative coordinates cost nothing. A flat array indexed by
//! cell would be faster still, but needs fixed bounds — this stays general.
//!
//! **One structure, three uses:** neighbour search for `fluid.rs`, a
//! broadphase for `rigid.rs`, and eventually self-collision for `soft.rs`.

use crate::complex::Cx;
use std::collections::HashMap;

pub struct SpatialHash {
    cell: f64,
    cells: HashMap<(i32, i32), Vec<usize>>,
}

impl SpatialHash {
    /// `cell` should be the interaction radius. Smaller means more cells to
    /// visit; larger means more irrelevant candidates inside each one.
    pub fn new(cell: f64) -> Self {
        SpatialHash { cell: cell.max(1e-9), cells: HashMap::new() }
    }

    /// Cell coordinates of a point.
    ///
    /// Two defensive details, both learned the hard way:
    /// * `floor`, not `as i32` — truncation folds -0.5 and +0.5 into the same
    ///   cell and puts a seam through the origin.
    /// * clamped, and NaN mapped to zero. When a simulation goes unstable the
    ///   positions become huge or NaN, and an unclamped cast overflows the
    ///   arithmetic here — so the *hash* panics and hides the fact that the
    ///   real problem was upstream. Better to file the rubbish somewhere and
    ///   let the caller's own diagnostics report it.
    #[inline]
    fn key(&self, p: Cx) -> (i32, i32) {
        let f = |x: f64| {
            let c = (x / self.cell).floor();
            if c.is_nan() {
                0
            } else {
                c.clamp(-1.0e9, 1.0e9) as i32
            }
        };
        (f(p.re), f(p.im))
    }

    /// Discard everything and re-file `points`. Rebuilding wholesale each step
    /// is normal: it is a linear pass, and cheaper than tracking movement.
    pub fn build(&mut self, points: &[Cx]) {
        for v in self.cells.values_mut() {
            v.clear(); // keep the allocations, drop the contents
        }
        for (i, &p) in points.iter().enumerate() {
            self.cells.entry(self.key(p)).or_default().push(i);
        }
        self.cells.retain(|_, v| !v.is_empty());
    }

    /// Append every index whose cell could hold something within `radius`.
    ///
    /// These are **candidates**, not confirmed neighbours: the 3x3 block of
    /// cells covers a square, so corners reach further than `radius`. The
    /// caller still checks the actual distance — which it was going to do
    /// anyway to compute the force.
    pub fn candidates(&self, p: Cx, radius: f64, out: &mut Vec<usize>) {
        out.clear();
        let reach = (radius / self.cell).ceil() as i32;
        let (cx, cy) = self.key(p);
        for gy in (cy - reach)..=(cy + reach) {
            for gx in (cx - reach)..=(cx + reach) {
                if let Some(v) = self.cells.get(&(gx, gy)) {
                    out.extend_from_slice(v);
                }
            }
        }
    }

    /// Candidates filtered down to genuine neighbours, for callers that want
    /// the simple thing.
    pub fn neighbours(&self, points: &[Cx], p: Cx, radius: f64, out: &mut Vec<usize>) {
        self.candidates(p, radius, out);
        let r2 = radius * radius;
        out.retain(|&i| (points[i] - p).abs_sq() <= r2);
    }

    pub fn occupied_cells(&self) -> usize {
        self.cells.len()
    }

    /// Largest number of points sharing one cell. If this climbs towards `n`
    /// the hash has stopped helping and the cell size is wrong.
    pub fn worst_bucket(&self) -> usize {
        self.cells.values().map(|v| v.len()).max().unwrap_or(0)
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn lattice(n: i32, step: f64, origin: Cx) -> Vec<Cx> {
        let mut v = Vec::new();
        for j in 0..n {
            for i in 0..n {
                v.push(origin + Cx::new(i as f64 * step, j as f64 * step));
            }
        }
        v
    }

    /// The only property that really matters: the hash must find exactly what
    /// brute force finds. If this passes, the hash is a pure optimisation.
    #[test]
    fn the_hash_agrees_with_brute_force() {
        let pts = lattice(30, 7.0, Cx::new(-90.0, -60.0));
        let radius = 15.0;
        let mut h = SpatialHash::new(radius);
        h.build(&pts);

        let mut got = Vec::new();
        for (i, &p) in pts.iter().enumerate() {
            h.neighbours(&pts, p, radius, &mut got);
            let mut want: Vec<usize> = (0..pts.len())
                .filter(|&j| (pts[j] - p).abs() <= radius)
                .collect();
            got.sort_unstable();
            want.sort_unstable();
            assert_eq!(got, want, "mismatch around point {i}");
        }
    }

    /// Negative coordinates must not fold onto positive ones - the classic
    /// `as i32` truncation bug puts a seam through the origin.
    #[test]
    fn negative_coordinates_get_their_own_cells() {
        let pts = vec![Cx::new(-0.5, -0.5), Cx::new(0.5, 0.5)];
        let mut h = SpatialHash::new(1.0);
        h.build(&pts);
        assert_eq!(h.occupied_cells(), 2, "the two sides of the origin collided");
    }

    /// Candidates may over-report (square cells, round radius) but must never
    /// under-report - a missed neighbour is a missed force.
    #[test]
    fn candidates_are_a_superset_of_the_true_neighbours() {
        let pts = lattice(20, 5.0, Cx::new(-50.0, -50.0));
        let radius = 12.0;
        let mut h = SpatialHash::new(radius);
        h.build(&pts);
        let mut cand = Vec::new();
        for &p in &pts {
            h.candidates(p, radius, &mut cand);
            for (j, &q) in pts.iter().enumerate() {
                if (q - p).abs() <= radius {
                    assert!(cand.contains(&j), "missed a real neighbour");
                }
            }
        }
    }

    /// Rebuilding must not leave ghosts from the previous frame.
    #[test]
    fn rebuilding_clears_the_old_contents() {
        let mut h = SpatialHash::new(10.0);
        h.build(&lattice(10, 9.0, Cx::ZERO));
        let first = h.occupied_cells();
        h.build(&[Cx::new(5.0, 5.0)]);
        assert_eq!(h.occupied_cells(), 1, "stale cells survived (was {first})");
        let mut out = Vec::new();
        h.neighbours(&[Cx::new(5.0, 5.0)], Cx::new(5.0, 5.0), 10.0, &mut out);
        assert_eq!(out, vec![0]);
    }

    /// The point of the whole exercise: work per query stays flat as the
    /// population grows, provided the particles stay spread out.
    #[test]
    fn work_per_query_does_not_grow_with_the_population() {
        let count = |n: i32| {
            let pts = lattice(n, 7.0, Cx::ZERO);
            let mut h = SpatialHash::new(15.0);
            h.build(&pts);
            let mut out = Vec::new();
            // probe the middle, where the neighbourhood is full
            let mid = pts[(n * n / 2) as usize];
            h.candidates(mid, 15.0, &mut out);
            out.len()
        };
        let small = count(12); // 144 points
        let large = count(40); // 1600 points
        assert!(
            large <= small + 2,
            "candidates grew with n: {small} -> {large} (hash is not working)"
        );
    }

    /// A degenerate case worth knowing about: everything in one spot means the
    /// hash cannot help, and it should say so rather than pretend.
    #[test]
    fn a_single_pile_defeats_the_hash_and_reports_it() {
        let pts = vec![Cx::new(1.0, 1.0); 500];
        let mut h = SpatialHash::new(10.0);
        h.build(&pts);
        assert_eq!(h.occupied_cells(), 1);
        assert_eq!(h.worst_bucket(), 500);
    }
}
