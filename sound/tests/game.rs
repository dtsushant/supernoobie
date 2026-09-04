//! The game noises, measured.
//!
//! Not by comparing samples — that pins the arithmetic rather than the sound,
//! and breaks on a rounding change. These assert what a listener would notice,
//! and each one exists because getting it wrong is audible.

use sound::kit;
use sound::noise::{brightness, fades, knocks, length, peak, render, tonality};

const RATE: u32 = 44_100;

/// ★ **A die rolls; it does not clang.** "Like a spoon hitting metal, not
/// quite rolling" was the complaint, and it was two faults at once: one hit
/// where there should be many, and a *note* where there should be a knock.
#[test]
fn a_roll_is_many_knocks_and_none_of_them_ring() {
    let s = render(&kit::roll(11), RATE);
    let hits = knocks(&s, RATE);
    assert!(hits >= 6, "a tumble is a series of contacts, not one: {hits}");
    assert!(tonality(&s) < 0.55, "it should have no pitch to speak of: {:.3}", tonality(&s));
}

/// ★ **And it slows down.** The gaps between contacts grow, because the die is
/// losing speed — the same curve it is drawn with, read backwards.
#[test]
fn the_contacts_get_further_apart() {
    let g = kit::roll(12);
    let gaps: Vec<f64> = g.windows(2).map(|w| w[1].at - w[0].at).collect();
    for k in 1..gaps.len() {
        assert!(gaps[k] > gaps[k - 1], "gap {k} did not grow: {gaps:?}");
    }
    assert!(gaps[gaps.len() - 1] > gaps[0] * 3.0, "and by a lot by the end: {gaps:?}");
}

/// The throw is over when the die is. It would be odd to still hear it
/// rattling after it had come to rest.
#[test]
fn the_roll_ends_when_the_throw_does() {
    let over = plotkit::dice::OVER;
    let len = length(&kit::roll(11));
    assert!(len <= over + 0.1, "{len} against {over}");
}

/// ★ Each contact is quieter and duller than the one before — the die is
/// rocking to a stop, not bouncing as hard as it did.
#[test]
fn the_contacts_die_away() {
    let g = kit::roll(10);
    for k in 1..g.len() {
        assert!(g[k].gain < g[k - 1].gain, "hit {k} was not quieter");
        assert!(g[k].cut < g[k - 1].cut, "hit {k} was not duller");
    }
}

/// ★ **A step is short enough to hear ten of.** A token walking four squares
/// makes four of these in under a second, so anything with a tail on it turns
/// into a drum roll.
#[test]
fn a_step_is_short_and_quiet() {
    let s = render(&kit::step(), RATE);
    assert!(length(&kit::step()) < 0.05, "{}", length(&kit::step()));
    assert!(peak(&s) < 0.4, "quiet enough to sit under everything: {}", peak(&s));
    assert!(peak(&s) > 0.02, "but audible: {}", peak(&s));
}

/// A capture is the loudest thing in the game, and the lowest. It should feel
/// like something happened.
#[test]
fn a_capture_is_low_and_loud() {
    let cut = render(&kit::cut(), RATE);
    let step = render(&kit::step(), RATE);
    assert!(peak(&cut) > peak(&step) * 1.5, "it should land harder than a step");
    assert!(
        brightness(&cut) < brightness(&step),
        "and lower: {:.3} against {:.3}",
        brightness(&cut),
        brightness(&step)
    );
}

/// ★ Getting home is the one place a **note** is right — it is an
/// announcement, not something being hit. So this one *should* ring.
#[test]
fn getting_home_rings() {
    let s = render(&kit::home(), RATE);
    assert!(tonality(&s) > 0.7, "it should have a clear pitch: {:.3}", tonality(&s));
    // Two notes, and the second higher -- counted from the grains and not from
    // the envelope, because they overlap on purpose. A gap between them would
    // be two sounds; the overlap is what makes it one phrase.
    let g = kit::home();
    assert_eq!(g.len(), 2);
    assert!(g[1].at > g[0].at && g[1].at < g[0].at + 4.0 * g[0].tau, "they overlap");
    assert!(g[1].freq > g[0].freq * 1.4, "and the second is higher: a fifth up");
    assert!(tonality(&s) > tonality(&render(&kit::roll(11), RATE)), "unlike the die");
}

/// Everything fades and nothing clips. Two properties every sound in the kit
/// has to have, and they pull against each other.
#[test]
fn everything_fades_and_nothing_clips() {
    for (name, g) in
        [("roll", kit::roll(11)), ("step", kit::step()), ("cut", kit::cut()), ("home", kit::home())]
    {
        let s = render(&g, RATE);
        assert!(peak(&s) > 0.02, "{name} is inaudible: {}", peak(&s));
        assert!(peak(&s) <= 1.0, "{name} clips: {}", peak(&s));
        assert!(fades(&s) < 0.3, "{name} does not die away: {}", fades(&s));
    }
}

/// ★ And the same throw makes the same noise, every time. No random number
/// generator anywhere, so a recorded game replays exactly — sound and all.
#[test]
fn the_same_throw_sounds_the_same() {
    assert_eq!(render(&kit::roll(9), RATE), render(&kit::roll(9), RATE));
    assert_ne!(kit::roll(9), kit::roll(14), "and a different tumble does not");
}
