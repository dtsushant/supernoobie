//! Is the die fair?
//!
//! Kept because the answer was **no**, and nothing else would have said so.
//! The face used to be derived from how far the die had turned and how many
//! walls it had struck — a pleasing idea that made an unfair die, because
//! `quarters` ran over fifteen values and fifteen does not divide by six. Ones
//! and fours came up eleven per cent more often than twos and threes.
//!
//! A die that is slightly unfair is not something anybody notices by playing.
//! It is something you notice by counting.

use plotkit::dice::thrown;

const N: usize = 6000;

fn faces(seed: f64) -> Vec<u8> {
    (0..N).map(|r| thrown(seed, r as f64, 99.0, 6.4).face).collect()
}

/// ★ **Every face comes up equally often.** Chi-square with five degrees of
/// freedom: 11.07 is the 5% line, 15.09 the 1% line. The old die scored 38.8.
///
/// The test is **Karl Pearson's**, from *On the criterion that a given system
/// of deviations…* (1900) — the first general goodness-of-fit test and, by most
/// accounts, the beginning of modern statistics. The statistic is
/// `Σ (observed − expected)² / expected`, and the reason it is compared against
/// a chi-square distribution is that a sum of squares of independent standard
/// normals has that distribution; the counts are approximately normal for large
/// samples by the central limit theorem, hence the shape.
///
/// Degrees of freedom are five and not six because the counts must add up to
/// the number of throws: fix five of them and the sixth follows. Pearson
/// originally got this wrong — he used `k` rather than `k − 1` — and **Ronald
/// Fisher** corrected him in 1922, which began a feud that lasted the rest of
/// Pearson's life.
///
/// **To read further:** any introductory statistics text; for the quarrel,
/// Salsburg's *The Lady Tasting Tea*.
#[test]
fn the_die_is_fair() {
    for seed in [137.0, 41.0, 9001.0] {
        let mut count = [0usize; 7];
        for f in faces(seed) {
            count[f as usize] += 1;
        }
        let expect = N as f64 / 6.0;
        let chi: f64 = (1..=6)
            .map(|f| {
                let d = count[f] as f64 - expect;
                d * d / expect
            })
            .sum();
        assert!(chi < 15.09, "seed {seed} is not fair: chi-square {chi:.1}, counts {:?}", &count[1..]);
    }
}

/// ★ And a throw does not remember the last one. A die that repeated itself
/// would feel wrong long before anybody could say why.
#[test]
fn one_throw_does_not_follow_from_the_last() {
    let f = faces(137.0);
    let repeats = f.windows(2).filter(|w| w[0] == w[1]).count();
    let expect = (N - 1) as f64 / 6.0;
    // Three standard deviations of a binomial with p = 1/6.
    let slack = 3.0 * (N as f64 * (1.0 / 6.0) * (5.0 / 6.0)).sqrt();
    assert!(
        (repeats as f64 - expect).abs() < slack,
        "{repeats} repeats in {N}, expected about {expect:.0}"
    );
}

/// ★ Nor does it run in a sequence. The faces used to count *down* to the one
/// it would land on, which is tidy and which the eye picks out of a tumbling
/// die at once.
#[test]
fn the_faces_do_not_walk_in_step() {
    let f = faces(137.0);
    let steps = f.windows(2).filter(|w| (w[1] as i32 - w[0] as i32).rem_euclid(6) == 1).count();
    let expect = (N - 1) as f64 / 6.0;
    let slack = 3.0 * (N as f64 * (1.0 / 6.0) * (5.0 / 6.0)).sqrt();
    assert!((steps as f64 - expect).abs() < slack, "{steps} single steps in {N}");
}

/// ★ **A different seed is a different game.** Two tables with the same seed
/// would play the same match, which is the point of the seed — and two with
/// different ones must not.
#[test]
fn a_different_seed_is_a_different_game() {
    let a = faces(137.0);
    let b = faces(138.0);
    let same = a.iter().zip(&b).filter(|(x, y)| x == y).count();
    assert!(same < N / 4, "{same} of {N} throws agreed, which is far too many");
}
