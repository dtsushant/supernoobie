//! # dice — a die you throw across a board
//!
//! Not a random number with a picture on it. A die is **flung**, it slides,
//! it strikes the walls and comes back off them at the angle it hit, it turns
//! more slowly as it goes, and it stops. The number it shows is what it was
//! left lying on.
//!
//! Everything here is a function of **how long ago it was thrown**. Nothing is
//! stepped frame by frame, which is what lets a throw be replayed, scrubbed,
//! or watched at half speed and look the same every time — and it is why a
//! whole game of Ludo can be replayed from one seed.
//!
//! ## One decay does all of it
//!
//! ```text
//!     v(t)  = v0 · e^{−t/τ}                 speed, dying away
//!     s(t)  = v0·τ · (1 − e^{−t/τ})         so distance is BOUNDED
//!     a(t)  = a∞  · (1 − e^{−t/τ})          and so is the angle turned
//! ```
//!
//! The integral of a decaying speed is a distance that approaches a limit and
//! never passes it. That is the whole reason a die thrown twice as hard does
//! not travel twice as far for ever: `v0·τ` *is* the reach. The same
//! `e^{−t/τ}` settles a branch after a gust and kills a note after it is
//! struck — Laplace, doing the only thing it ever does.
//!
//! ## Who this decay belongs to
//!
//! `dy/dt = −y/τ` — a thing losing a constant *fraction* of itself each
//! moment rather than a constant amount — is the most reused equation in this
//! repository, and it turns up under a different name every time:
//!
//! | | |
//! |---|---|
//! | **Newton**, 1701 | a hot body cools in proportion to how much hotter it is |
//! | **Fourier**, 1822 | the same, done properly, in *Théorie analytique de la chaleur* |
//! | **Laplace**, 1785 | the transform that turns it into algebra |
//! | **Rutherford & Soddy**, 1902 | radioactive decay, and the half-life |
//! | **Kelvin**, 1850s | charge leaking off a submarine telegraph cable |
//!
//! Laplace is the name attached to it here because his transform is what makes
//! a decaying system a *number* rather than a differential equation: the
//! transform of `e^{−t/τ}` is `1/(s + 1/τ)`, and the pole at `s = −1/τ` **is**
//! the decay. Everything in this crate that settles — a die, a branch after a
//! gust, a note after it is struck — is that one pole moved about.
//!
//! Fourier is worth the detour: he wrote the heat equation while Napoleon's
//! prefect of Isère, having earlier gone to Egypt with the expedition, and the
//! series he invented to solve it were rejected by Lagrange as insufficiently
//! rigorous. Lagrange was right about the rigour and wrong about the idea.
//!
//! **To read further:** any first course on ODEs; for the transform, Bracewell's
//! *The Fourier Transform and Its Applications* is the friendly one.
//!
//! ## Bouncing is folding
//!
//! A ball bouncing between two walls travels the same path as one going
//! straight for ever through a mirrored world. So the position is worked out
//! **unfolded** — a straight line of length `s(t)` — and then folded back into
//! the box:
//!
//! ```text
//!     fold(u) = the triangle wave of u between lo and hi
//! ```
//!
//! This is exact, not an approximation, and it is why the angle of the bounce
//! is right without a single line about angles: a reflection *is* a fold. It
//! also means the wall never has to be *detected*. There is no frame on which
//! the die is a little way through the wall and has to be pushed back, which is
//! the usual way this goes wrong.
//!
//! ### Whose trick this is
//!
//! **Unfolding** is the standard move in the study of *mathematical billiards*.
//! Rather than reflect the ball at the wall, reflect the *table* and let the
//! ball go straight — the two pictures are identical, and the second has no
//! collisions in it at all. For a rectangle the reflected copies tile the
//! plane, so a billiard path becomes a straight line on a torus, and questions
//! about bouncing become questions about a line of given slope, which are
//! much easier.
//!
//! The idea goes back to **Hermann Schwarz** and to Fagnano's problem (1775:
//! inscribe the shortest triangle in a triangle — solved by unfolding).
//! **George Birkhoff** made billiards a subject in its own right in *Dynamical
//! Systems* (1927), and the rectangle case connects to Weyl's equidistribution
//! theorem (1916): an irrational slope visits every part of the table, a
//! rational one closes into a loop. That is why a die thrown at a "nice" angle
//! would retrace its own path and one thrown at any other angle does not.
//!
//! Reflection itself — that the angle in equals the angle out — is **Hero of
//! Alexandria**, c. 60 AD, who derived it from the assumption that light takes
//! the shortest path. Fermat generalised it to *least time* in 1662, which is
//! how refraction falls out of the same principle.
//!
//! **To read further:** Tabachnikov, *Geometry and Billiards* — short, and the
//! unfolding picture is on about page ten.
//!
//! ## Which face it lands on
//!
//! **The face is drawn first, and the tumble is the show.** This was the other
//! way round to begin with — the face fell out of how far the die had turned
//! and how many walls it had struck, which was a pleasing idea and made an
//! unfair die. `quarters` runs over fifteen values and fifteen does not divide
//! by six, so some faces got three chances and others two: ones and fours came
//! up eleven per cent more often than twos and threes, and a chi-square of 38.8
//! against a 11.07 threshold. Four fours in a row in the first thirty throws.
//!
//! A die has to be fair before it is anything else, so now:
//!
//! ```text
//!     face = 1 + floor(6 · spread(…))
//! ```
//!
//! and the tumble is arranged to *finish* on that face. Physically that is
//! backwards. It is also the only way to get a fair die out of a flat drawing
//! with one angle, and every die in every game does it — the honest thing is
//! to say so rather than to dress it up.
//!
//! What is kept is what actually mattered: the throw is **entirely determined
//! by the seed and the throw number**, so it is unpredictable to play against
//! and exactly repeatable, which is what makes "he cheated" answerable.
//!
//! ## Tumbling, rather than spinning
//!
//! A square turning smoothly on the spot looks like a plate on a stick. A die
//! **flips**, face over face, and the giveaway in two dimensions is that it
//! foreshortens: the face you are looking at narrows to nothing as it goes over
//! its edge, and the next one opens out.
//!
//! ```text
//!     flips(t) = n · ease(t)         how many times it has gone over
//!     squash   = |cos(π · frac(flips))|
//! ```
//!
//! One at the start of a flip, nothing in the middle of it, one again at the
//! end — and the face changes exactly where the die is edge-on and nobody can
//! see it change. That is the whole trick, and without it no amount of
//! rotation reads as a die.

use crate::Cx;
use std::f64::consts::{FRAC_PI_2, TAU};

/// How long the throw takes to die away, in seconds.
///
/// Four of these is 98% of the way there, which is the usual answer to *when
/// has an exponential finished*.
pub const REST: f64 = 0.62;

/// When a throw counts as over.
pub const OVER: f64 = 4.0 * REST;

/// A die in the middle of being thrown.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roll {
    /// Where it is.
    pub at: Cx,
    /// How far round it has turned, in radians.
    pub turn: f64,
    /// The face it is showing, 1 to 6.
    pub face: u8,
    /// How wide the near face looks, 1 flat on and 0 edge-on.
    pub squash: f64,
    /// The face rolling into view beside it.
    ///
    /// A cube going over its edge shows **two** faces, and their widths are the
    /// cosine and sine of one angle. Drawing only the near one, foreshortened,
    /// gives a plank: full height, narrowing width, nothing anywhere saying the
    /// thing is solid.
    pub next: u8,
    /// Whether it has stopped.
    pub done: bool,
}

/// A number between 0 and 1, worked out rather than drawn from anywhere.
///
/// No random number generator anywhere in this crate. `sin` of a large
/// multiple of a whole number wanders across its range without settling into a
/// pattern anybody spots, and the seed is a number nobody is looking at. So a
/// throw is unpredictable in play and **exactly repeatable** from its seed.
/// ## Where this hash comes from, and what it is not
///
/// `fract(sin(x) * 43758.5453)` is **folklore from graphics shaders**, and it
/// is worth being plain that nobody published it: it appears in shader code
/// from the late 2000s, is repeated in thousands of demos, and the constant is
/// carried along without anybody quite knowing who chose it. It works because
/// `sin` is smooth but its *high-order digits* are not — multiplying by a large
/// number and keeping the fraction throws away everything the smoothness lives
/// in.
///
/// It is a **hash**, not a random number generator, and the difference matters:
/// it maps an input to a number that looks unrelated, which is what is wanted
/// here, but it has no period, no state, and no guarantees. It fails serious
/// statistical tests and varies between machines when `sin` is implemented
/// differently. For a die that must be *fair*, that is not good enough on its
/// own — which is why `plotkit/tests/fair.rs` measures the result with a
/// chi-square test (**Karl Pearson**, 1900, the first of the modern
/// goodness-of-fit tests) rather than trusting it.
///
/// The respectable alternatives, if this ever needs replacing: **PCG**
/// (Melissa O'Neill, 2014) or **xoshiro** (Blackman & Vigna, 2018). Both are a
/// few lines and both have real analysis behind them.
fn spread(seed: f64, roll: f64, salt: f64) -> f64 {
    let x = (seed + salt) * 12.9898 + roll * 78.233;
    let v = x.sin() * 43758.5453;
    v - v.floor()
}

/// The triangle wave of `u` in `[lo, hi]` — a straight path folded back at
/// each wall, which is what a bounce is.
pub fn fold(u: f64, lo: f64, hi: f64) -> f64 {
    let w = hi - lo;
    if w <= 0.0 {
        return lo;
    }
    let t = (u - lo).rem_euclid(2.0 * w);
    lo + if t <= w { t } else { 2.0 * w - t }
}

/// How many walls a straight path of this length crossed — the number of times
/// it was folded, which is the number of times it bounced.
pub fn knocks(u: f64, lo: f64, hi: f64) -> u32 {
    let w = hi - lo;
    if w <= 0.0 {
        return 0;
    }
    ((u - lo) / w).floor().abs().min(1e6) as u32
}

/// Where a die thrown `roll` throws into a game with this `seed` has got to,
/// `age` seconds after it left the hand, on a square board of half-width
/// `span` centred on the origin.
pub fn thrown(seed: f64, roll: f64, age: f64, span: f64) -> Roll {
    let span = span.max(1e-6);
    // **Settled means not moving.** An exponential approaches its limit from
    // below and never arrives, so `gone` goes on creeping for ever -- by a
    // hair, but enough for a wall crossing to tick over and change the face of
    // a die that has been lying still for ten seconds. Which it did.
    //
    // So the throw is *over* at four time constants, and asking later gives
    // the same answer as asking then.
    let live = age.clamp(0.0, OVER);
    let done = age > OVER;

    // The decay, scaled so it **arrives**. A bare `1 - e^{-t/tau}` is still
    // 1.8% short at four time constants, which is a die coming to rest a
    // fortieth of a turn askew -- the one thing that would give away at once
    // that this is an angle and not a die. Dividing by its own value at the
    // end keeps the shape of the curve, and every claim about it: the rate
    // still decays, so it whirls, eases and stops.
    let ease = (1.0 - (-live / REST).exp()) / (1.0 - (-OVER / REST).exp());

    // How hard, and which way. Different every throw, and worked out rather
    // than drawn from anywhere, so the same match replays exactly.
    let aim = spread(seed, roll, 1.0) * TAU;
    // A fling that always crosses the board at least once, so it always
    // reaches the walls -- a die that stopped in the middle every time would
    // look thrown by somebody being careful.
    let reach = span * (2.4 + 3.2 * spread(seed, roll, 2.0));
    // Where it leaves the hand: near a corner, so it has room to run.
    let from = Cx::new(
        span * (0.55 - 1.1 * spread(seed, roll, 3.0)),
        span * (0.55 - 1.1 * spread(seed, roll, 4.0)),
    );

    // The slide. Distance approaches `reach` and never passes it.
    let gone = reach * ease;
    let straight = from + Cx::polar(gone, aim);
    let at = Cx::new(fold(straight.re, -span, span), fold(straight.im, -span, span));

    // How it lies when it stops. A whole number of right angles, so a stopped
    // die sits square rather than askew -- which is the one thing that would
    // give away that this is an angle and not a die.
    let quarters = (2.0 + 6.0 * spread(seed, roll, 5.0)).round();
    let turn = quarters * FRAC_PI_2 * ease;

    // The face it will land on, drawn fairly. See the note at the top: this
    // is chosen first and the tumble is arranged to finish on it.
    let rest = 1 + (6.0 * spread(seed, roll, 6.0)).floor().clamp(0.0, 5.0) as u8;

    // The tumble: how many times it has gone over so far.
    let all = (5.0 + 9.0 * spread(seed, roll, 7.0)).round();
    let flips = all * ease;
    let gone_over = flips.floor();
    // Edge-on in the middle of each flip, flat on at either end -- and the
    // face changes exactly where nobody can see it change.
    let squash = (std::f64::consts::PI * (flips - gone_over)).cos().abs();
    // What is showing part way through. Jumbled rather than counted down: the
    // face changing by one each flip is a tidy little sequence, and the eye
    // picks that out of a tumbling die at once.
    let left = all - gone_over;
    let shown = |k: f64| 1 + (6.0 * spread(seed, roll + 1000.0 * k, 8.0)).floor().clamp(0.0, 5.0) as u8;
    let face = if done || left < 1.0 { rest } else { shown(gone_over) };
    // The one coming round behind it -- and if this is the last flip, the face
    // it is going to land on, so the roll ends on the number it means.
    let next = if done {
        rest
    } else if left < 2.0 {
        rest
    } else {
        shown(gone_over + 1.0)
    };

    Roll { at, turn, face, next, squash: if done { 1.0 } else { squash }, done }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ **A bounce is a fold.** A path that runs past the wall comes back
    /// inside it, at the distance it overshot — which is the reflection,
    /// exactly, with nothing said about angles.
    #[test]
    fn a_path_folds_at_the_walls() {
        let near = |a: f64, b: f64| assert!((a - b).abs() < 1e-9, "{a} vs {b}");
        near(fold(0.5, -1.0, 1.0), 0.5); // inside, untouched
        near(fold(1.2, -1.0, 1.0), 0.8); // 0.2 past the wall is 0.2 back
        near(fold(-1.2, -1.0, 1.0), -0.8); // and the same at the other one
        near(fold(3.0, -1.0, 1.0), -1.0); // a whole width past lands on the far wall
    }

    /// However far it is flung, it stays on the board. This is the property
    /// worth testing, because it is the one a detect-and-push-back scheme
    /// quietly fails on the frame the die is moving fastest.
    #[test]
    fn it_never_leaves_the_board() {
        for roll in 0..200 {
            for step in 0..60 {
                let r = thrown(137.0, roll as f64, step as f64 * 0.05, 6.0);
                assert!(r.at.re.abs() <= 6.0 + 1e-9, "x {} on roll {roll}", r.at.re);
                assert!(r.at.im.abs() <= 6.0 + 1e-9, "y {} on roll {roll}", r.at.im);
            }
        }
    }

    /// ★ **Distance is bounded because speed decays.** The integral of
    /// `v0·e^{−t/τ}` approaches `v0·τ` and never passes it, which is why a die
    /// thrown hard does not slide for ever.
    #[test]
    fn it_slows_to_a_stop() {
        let d = |a: f64| {
            let (u, v) = (thrown(7.0, 3.0, a, 6.0), thrown(7.0, 3.0, a + 0.05, 6.0));
            (v.at - u.at).abs()
        };
        // Three time constants apart, so the ratio should be about e^3 ≈ 20.
        let early = d(0.0);
        let late = d(2.0);
        assert!(early > late * 10.0, "it should be far slower by then: {early} then {late}");
        assert!(late < 0.1, "and nearly stopped: {late} per fiftieth of a second");
        assert!(d(6.0) < 1e-4, "and stopped for all practical purposes: {}", d(6.0));
    }

    /// The same, for the turn: it eases to rest rather than running at one
    /// speed and halting.
    #[test]
    fn the_turn_eases_to_rest() {
        let d = |a: f64| thrown(7.0, 3.0, a + 0.05, 6.0).turn - thrown(7.0, 3.0, a, 6.0).turn;
        // A time constant and a half apart is a factor of about e^1.6 ≈ 5.
        assert!(d(0.0) > d(1.0) * 4.0, "the spin should be dying away: {} then {}", d(0.0), d(1.0));
        assert!(d(3.0) < d(0.0) / 100.0, "and all but stopped by three seconds");
    }

    /// ★ A stopped die sits **square**. An angle that came to rest at 37° would
    /// give away at once that this is a rotation and not a die.
    #[test]
    fn it_comes_to_rest_on_a_right_angle() {
        for roll in 0..50 {
            let r = thrown(137.0, roll as f64, 20.0, 6.0);
            let quarters = r.turn / FRAC_PI_2;
            assert!((quarters - quarters.round()).abs() < 1e-6, "askew by {} on {roll}", quarters - quarters.round());
        }
    }

    /// ★ Every face is a real one, at every moment of every throw.
    #[test]
    fn it_always_shows_a_real_face() {
        for roll in 0..100 {
            for step in 0..40 {
                let f = thrown(41.0, roll as f64, step as f64 * 0.07, 5.0).face;
                assert!((1..=6).contains(&f), "face {f}");
            }
        }
    }

    /// ★ **Every face comes up.** A die that never showed a six would be a
    /// long time being noticed and would ruin every game quietly.
    #[test]
    fn all_six_faces_come_up() {
        let mut seen = [0usize; 7];
        for roll in 0..600 {
            seen[thrown(137.0, roll as f64, 99.0, 6.0).face as usize] += 1;
        }
        for face in 1..=6 {
            assert!(seen[face] > 40, "face {face} came up {} times in 600", seen[face]);
        }
    }

    /// ★ No random number generator: the same throw is the same throw, every
    /// time, which is what lets a match be replayed.
    #[test]
    fn a_throw_is_exactly_repeatable() {
        let a = thrown(137.0, 9.0, 1.3, 6.0);
        let b = thrown(137.0, 9.0, 1.3, 6.0);
        assert_eq!(a, b);
        assert_ne!(a.face, thrown(138.0, 9.0, 1.3, 6.0).face, "and a different seed is a different game");
    }

    /// It starts where it was let go and has not moved at all.
    #[test]
    fn at_the_moment_of_the_throw_it_has_not_moved() {
        let r = thrown(137.0, 2.0, 0.0, 6.0);
        assert_eq!(r.turn, 0.0);
        assert!(!r.done);
    }

    /// ★ **Once it has stopped, it has stopped.** The face of a die lying
    /// still must not change ten seconds later — and it did, because an
    /// exponential approaches its limit from below for ever and a hair of
    /// further creep is enough to cross a wall and tip the die.
    #[test]
    fn nothing_moves_after_it_has_settled() {
        for roll in 0..40 {
            let rest = thrown(137.0, roll as f64, OVER, 6.0);
            for later in [OVER + 0.1, OVER + 5.0, 300.0, 1e6] {
                let r = thrown(137.0, roll as f64, later, 6.0);
                assert_eq!(r.face, rest.face, "the face changed at {later} on roll {roll}");
                assert_eq!(r.at, rest.at, "and it moved");
                assert_eq!(r.turn, rest.turn, "and it turned");
            }
        }
    }

    /// And it is over after four time constants.
    #[test]
    fn it_is_over_after_four_time_constants() {
        assert!(!thrown(137.0, 2.0, OVER - 0.01, 6.0).done);
        assert!(thrown(137.0, 2.0, OVER + 0.01, 6.0).done);
    }

    /// ★ It really does reach the walls, rather than stopping politely in the
    /// middle — the whole point of throwing it across the board.
    #[test]
    fn it_uses_the_whole_board() {
        let mut hit = 0;
        for roll in 0..60 {
            let far = (0..40)
                .map(|s| thrown(137.0, roll as f64, s as f64 * 0.05, 6.0).at)
                .map(|p| p.re.abs().max(p.im.abs()))
                .fold(0.0f64, f64::max);
            if far > 5.0 {
                hit += 1;
            }
        }
        assert!(hit > 45, "only {hit} of 60 throws got near the edge");
    }
}
