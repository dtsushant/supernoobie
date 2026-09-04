//! # rooms — more than one game on one server
//!
//! Until now this server held a single board, so everybody who connected was
//! in the same game whether they meant to be or not. A room is a name, and
//! everything else — the board, the seats, the post office — hangs off it.
//!
//! ## A link is the invitation
//!
//! A room is made by the first person to *use* its name. There is no create
//! step, no list of rooms, and nothing to clean up if nobody turns up: sending
//! `?room=PEAR` to three friends is the whole of organising a game.
//!
//! The alternative — create, then join — needs a room to exist in a state
//! where it has no players, which means a way to see them, delete them, and
//! decide what a room with nobody in it *is*. All of that to save typing a
//! name once.
//!
//! ## Codes people read aloud
//!
//! Somebody is going to say this down a telephone, so the alphabet leaves out
//! every letter and digit that sounds or looks like another:
//!
//! ```text
//!     ABCDEFGHJKMNPQRSTVWXYZ23456789
//!     no I, no L, no 1        no O, no 0        no U
//! ```
//!
//! This is roughly the **Crockford base-32** alphabet (Douglas Crockford,
//! 2002), designed for exactly this: *"excludes the letters I, L, O and U to
//! avoid confusion and abuse"*. Crockford drops U to avoid accidental
//! obscenities, which matters when codes are generated in the thousands.
//!
//! Four characters from 32 is about a million rooms. That is not a security
//! boundary and is not meant to be — see [`Rooms::make`].
//!
//! ## Rooms end by being forgotten
//!
//! There is no *leave*. A room that nobody has touched for [`STALE`] is
//! dropped, which handles the only two cases that happen: everybody finished,
//! and everybody wandered off. Asking people to close a game properly means
//! writing the code that handles them not doing it anyway.

use std::collections::HashMap;

/// Letters and digits that cannot be mistaken for one another when read out.
pub const ALPHABET: &[u8] = b"ABCDEFGHJKMNPQRSTVWXYZ23456789";

/// How long a name is. Four is a million rooms, and is four characters to say.
pub const LENGTH: usize = 4;

/// How long a room may go untouched before it is forgotten, in seconds.
///
/// Twenty minutes: long enough to survive an argument about the rules or
/// somebody answering the door, short enough that a server left running for a
/// week is not holding a hundred abandoned boards.
pub const STALE: f64 = 20.0 * 60.0;

/// The name everything uses when nobody asked for a room.
///
/// So a single person opening the studio to draw something is not made to
/// think about rooms at all, and every link that worked before still works.
pub const ALONE: &str = "ALONE";

/// One thing per room, whatever that thing is.
#[derive(Debug)]
pub struct Rooms<T> {
    at: HashMap<String, T>,
    /// When each was last touched.
    touched: HashMap<String, f64>,
    /// Nudged on every name made, so two rooms made in the same moment differ.
    counter: u64,
}

impl<T> Default for Rooms<T> {
    fn default() -> Self {
        Rooms { at: HashMap::new(), touched: HashMap::new(), counter: 0 }
    }
}

impl<T> Rooms<T> {
    pub fn new() -> Rooms<T> {
        Rooms::default()
    }

    /// Tidy a name as typed: upper case, and only letters this alphabet has.
    ///
    /// So `pear`, `PEAR` and `p-e-a-r` are one room. Somebody reading a code
    /// off a screen and typing it into a phone should not be defeated by a
    /// shift key.
    pub fn tidy(name: &str) -> String {
        let cleaned: String = name
            .chars()
            .map(|c| c.to_ascii_uppercase())
            .filter(|c| ALPHABET.contains(&(*c as u8)))
            .take(16)
            .collect();
        if cleaned.is_empty() {
            ALONE.to_string()
        } else {
            cleaned
        }
    }

    /// A new name nothing is using.
    ///
    /// **Not a secret.** A four-character code is guessable by anybody willing
    /// to try, and the thing it protects is a game of Ludo. If this ever
    /// guarded something worth guarding, the answer would not be a longer code
    /// — it would be that the room knows who is allowed in.
    ///
    /// The randomness is the clock and a counter stirred together. There is no
    /// generator here on purpose: everything about a *game* in this repository
    /// is worked out so a match replays exactly, and a room name is the one
    /// thing that must be different every time. Keeping them apart means the
    /// rule is never bent.
    pub fn make(&mut self, now: f64) -> String {
        for _ in 0..64 {
            self.counter = self.counter.wrapping_add(1);
            let mut x = (now * 1e6) as u64 ^ self.counter.wrapping_mul(0x9E37_79B9_7F4A_7C15);
            let mut name = String::new();
            for _ in 0..LENGTH {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                name.push(ALPHABET[(x >> 33) as usize % ALPHABET.len()] as char);
            }
            if !self.at.contains_key(&name) && name != ALONE {
                return name;
            }
        }
        // Sixty-four collisions in a million names means something is very
        // wrong; a name that is merely ugly is better than a loop that spins.
        format!("{ALONE}{}", self.counter)
    }

    /// The room by that name, making it if this is the first anybody has asked.
    ///
    /// `fresh` is only called when one has to be made, so opening a room does
    /// not cost the work of loading a board that already exists.
    pub fn get(&mut self, name: &str, now: f64, fresh: impl FnOnce() -> T) -> &mut T {
        let name = Self::tidy(name);
        self.sweep(now);
        self.touched.insert(name.clone(), now);
        self.at.entry(name).or_insert_with(fresh)
    }

    /// The room by that name, if it exists — without making one.
    pub fn peek(&self, name: &str) -> Option<&T> {
        self.at.get(&Self::tidy(name))
    }

    pub fn count(&self) -> usize {
        self.at.len()
    }

    pub fn names(&self) -> Vec<String> {
        let mut all: Vec<String> = self.at.keys().cloned().collect();
        all.sort();
        all
    }

    /// Forget what nobody has touched.
    pub fn sweep(&mut self, now: f64) {
        let stale: Vec<String> = self
            .touched
            .iter()
            .filter(|(_, at)| now - **at > STALE)
            .map(|(name, _)| name.clone())
            .collect();
        for name in stale {
            self.at.remove(&name);
            self.touched.remove(&name);
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn rooms() -> Rooms<String> {
        Rooms::new()
    }

    /// ★ **A link is the invitation.** The first person to use a name makes
    /// the room; there is no create step to forget, and nothing to clean up if
    /// nobody turns up.
    #[test]
    fn using_a_name_makes_the_room() {
        let mut r = rooms();
        assert_eq!(r.count(), 0);
        assert_eq!(r.get("PEAR", 0.0, || "board".into()), "board");
        assert_eq!(r.count(), 1);
        // And the second person to use it gets the same one, not a new one.
        assert_eq!(r.get("PEAR", 1.0, || "another".into()), "board");
        assert_eq!(r.count(), 1);
    }

    /// ★ Somebody reading a code off a screen and typing it into a phone
    /// should not be defeated by a shift key or a stray dash.
    #[test]
    fn a_name_is_read_generously() {
        for typed in ["PEAR", "pear", "Pear", " p e a r ", "p-e-a-r", "pear!!"] {
            assert_eq!(Rooms::<String>::tidy(typed), "PEAR", "{typed}");
        }
    }

    /// ★ Nothing that could be misheard is in the alphabet: no I or L against
    /// 1, no O against 0, no U at all.
    #[test]
    fn the_alphabet_has_nothing_ambiguous_in_it() {
        for bad in [b'I', b'L', b'O', b'U', b'0', b'1'] {
            assert!(!ALPHABET.contains(&bad), "{} should not be in it", bad as char);
        }
        // And no repeats, or one letter would come up twice as often.
        let mut seen = std::collections::HashSet::new();
        for c in ALPHABET {
            assert!(seen.insert(c), "{} appears twice", *c as char);
        }
    }

    /// A made-up name is one of ours and is the right length.
    #[test]
    fn a_made_name_is_sayable() {
        let mut r = rooms();
        for k in 0..200 {
            let name = r.make(k as f64 * 0.37);
            assert_eq!(name.len(), LENGTH, "{name}");
            for c in name.bytes() {
                assert!(ALPHABET.contains(&c), "{name} has a {} in it", c as char);
            }
        }
    }

    /// ★ And two rooms made in the same instant are still two rooms. The clock
    /// alone is not enough — two people pressing at once share a millisecond.
    #[test]
    fn names_made_together_are_still_different() {
        let mut r = rooms();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..500 {
            let name = r.make(1234.5678);
            // Take it, so `make` has to avoid it next time.
            r.get(&name, 0.0, || "board".into());
            assert!(seen.insert(name.clone()), "{name} came up twice");
        }
    }

    /// A made name never collides with one in use.
    #[test]
    fn a_made_name_is_free() {
        let mut r = rooms();
        for k in 0..300 {
            let name = r.make(k as f64);
            assert!(r.peek(&name).is_none(), "{name} was already taken");
            r.get(&name, k as f64, || "board".into());
        }
        assert_eq!(r.count(), 300);
    }

    /// ★ **Rooms end by being forgotten.** There is no leave, because asking
    /// people to close a game properly means writing the code that handles
    /// them not doing it anyway.
    #[test]
    fn an_untouched_room_is_forgotten() {
        let mut r = rooms();
        r.get("PEAR", 0.0, || "board".into());
        r.get("DATE", 0.0, || "board".into());
        // Pear is kept up; pear is not.
        r.get("PEAR", STALE + 1.0, || "board".into());
        assert_eq!(r.names(), vec!["PEAR"], "pear went, pear stayed");
    }

    /// Touching a room keeps it, and touching it is anything at all.
    #[test]
    fn any_touch_keeps_a_room() {
        let mut r = rooms();
        r.get("PEAR", 0.0, || "board".into());
        for k in 1..40 {
            r.get("PEAR", k as f64 * (STALE / 2.0), || "new".into());
        }
        assert_eq!(r.count(), 1);
        assert_eq!(r.peek("PEAR"), Some(&"board".to_string()), "and it is the same board");
    }

    /// ★ Somebody who asked for no room at all gets a real one, so a person
    /// opening the studio to draw is never made to think about rooms.
    #[test]
    fn no_name_is_still_a_room() {
        let mut r = rooms();
        assert_eq!(Rooms::<String>::tidy(""), ALONE);
        r.get("", 0.0, || "board".into());
        assert_eq!(r.names(), vec![ALONE]);
    }

    /// And a made-up name is never that one, or a stranger would walk into
    /// somebody's private drawing.
    #[test]
    fn a_made_name_is_never_the_lonely_one() {
        let mut r = rooms();
        for k in 0..400 {
            assert_ne!(r.make(k as f64), ALONE);
        }
    }

    /// A silly name is cut down rather than kept.
    #[test]
    fn a_name_cannot_be_enormous() {
        let long = "PEAR".repeat(50);
        assert!(Rooms::<String>::tidy(&long).len() <= 16);
    }

    /// The board is only made when one has to be — opening a room that exists
    /// does not cost the work of loading a file.
    #[test]
    fn an_existing_room_is_not_remade() {
        let mut r = rooms();
        let mut made = 0;
        for _ in 0..10 {
            r.get("PEAR", 0.0, || {
                made += 1;
                "board".into()
            });
        }
        assert_eq!(made, 1);
    }
}
