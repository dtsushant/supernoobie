//! # auth — who may do what
//!
//! Not logins. **Who holds the controls**, which is a different and smaller
//! question, and the only one a game round a table actually asks.
//!
//! ## Why this is a crate and not four lines in the room
//!
//! It was four lines in the room, and they were wrong. The host was worked out
//! as *the lowest occupied seat* — derived, not held — so it moved every time
//! anybody sat down, stood up, or arrived. One player watched the start button
//! appear and disappear as other people took their colours, which is not a
//! flicker in the drawing: it is two people believing they are in charge.
//!
//! **Control is held, not derived.** Somebody has it; it changes only when
//! they hand it over or they leave. That single sentence is the whole of this
//! crate, and it is a crate because the next game will want to say something
//! more than *"one of you is in charge"* — teams, spectators, a referee,
//! somebody who may set the rules but not start — and all of those are more
//! [`Deed`]s and a different [`Table::may`], not a rewrite of a room.
//!
//! ## The two failure modes it exists to prevent
//!
//! **Two hosts.** Anything derived from a set that changes underneath you can
//! be true for two people at once, briefly, and briefly is enough for both to
//! press start.
//!
//! **No host.** The one holding the controls closes their laptop, and a room
//! of four people waits for somebody who is not coming. So hostship is passed
//! on when its holder leaves — to whoever has been here longest, because that
//! is the least arbitrary rule that always has an answer.

use std::collections::HashMap;

/// How long somebody may be away before the controls pass on.
///
/// **Much longer than presence, and deliberately.** Going quiet is not the same
/// as leaving, and the commonest reason to go quiet is *sending somebody the
/// link*: switching to a messaging app puts the page in the background, where a
/// browser throttles its timers to about once a minute and a handset suspends
/// it altogether.
///
/// So the person who made the room, invited everybody, and is still sitting
/// there looking at it stops calling in for as long as it takes to paste a URL
/// — and the only person still in the foreground is whoever just arrived. That
/// is exactly how the host who had not left, and had started the game, lost the
/// controls to their own guest.
///
/// Five minutes: longer than any plausible detour to another app, short enough
/// that a genuinely abandoned room is not held for ever by somebody who has
/// gone to lunch.
pub const AWAY: f64 = 5.0 * 60.0;

/// Something somebody might be allowed to do.
///
/// Deliberately about *the game* rather than about the machinery: a deed is a
/// thing a person at a table would recognise. Adding one is how this grows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deed {
    /// Begin the game.
    Start,
    /// Change the house rules everybody will play by.
    SetRules,
    /// Give the controls to somebody else.
    HandOver,
    /// Take a turn as a given seat.
    Play(usize),
}

/// Who is at the table and who is holding the controls.
#[derive(Debug, Default, Clone)]
pub struct Table {
    /// Who holds the controls. **Held, not worked out.**
    host: Option<String>,
    /// Everybody present, and when they first arrived — for passing the
    /// controls on when the holder leaves.
    came: HashMap<String, f64>,
    /// Seat to person.
    chairs: HashMap<usize, String>,
    /// When the holder was last heard from, so going quiet for a moment is not
    /// the same as leaving.
    host_seen: f64,
    /// When somebody stopped being present, so their chair can be kept for a
    /// while and then let go.
    gone: HashMap<String, f64>,
}

impl Table {
    pub fn new() -> Table {
        Table::default()
    }

    /// Somebody is here.
    ///
    /// The first to arrive takes the controls, so a room is never without a
    /// host — including a room of one, who would otherwise be waiting for
    /// themselves.
    pub fn arrive(&mut self, who: &str, now: f64) {
        if who.is_empty() {
            return;
        }
        self.came.entry(who.to_string()).or_insert(now);
        self.gone.remove(who);
        if self.host.is_none() {
            self.host = Some(who.to_string());
            self.host_seen = now;
        }
        if self.is_host(who) {
            self.host_seen = now;
        }
    }

    /// Somebody has gone.
    ///
    /// If they were holding the controls, they pass to whoever has been here
    /// longest — the least arbitrary rule that always has an answer, and one
    /// both sides can work out without asking.
    /// Somebody has gone quiet.
    ///
    /// They stop being *present* at once — their seat frees, and the room stops
    /// listing them. The **controls do not move**, because going quiet is not
    /// leaving: see [`AWAY`], and [`settle`](Table::settle), which is what
    /// eventually passes them on.
    pub fn leave_at(&mut self, who: &str, now: f64) {
        if self.came.remove(who).is_some() {
            self.gone.insert(who.to_string(), now);
        }
        // **The chair is kept.** Going quiet is not leaving, and a seat is a
        // colour somebody chose: freeing it the moment a phone locks its
        // screen meant coming back to find yourself a different colour, or
        // seated somewhere else entirely by the next `seat_everybody`.
        //
        // It is freed by `settle` after the same long absence that moves the
        // controls, or at once by `depart` when they say they are going.
    }

    /// Hand the controls on if their holder has been away long enough, and
    /// there is somebody to hand them to.
    ///
    /// Separate from [`leave`](Table::leave) so that the two questions stay
    /// apart: *is this person here* is asked every few seconds, and *has the
    /// game lost its host* should be asked patiently.
    pub fn settle(&mut self, now: f64) {
        // Chairs belonging to people who have been away a long time. Their
        // `gone` moment is when they stopped being present; until then the
        // colour stays theirs.
        let long_gone: Vec<String> = self
            .gone
            .iter()
            .filter(|(who, at)| now - **at > AWAY && !self.came.contains_key(*who))
            .map(|(who, _)| who.clone())
            .collect();
        for who in long_gone {
            self.chairs.retain(|_, sitter| sitter != &who);
            self.gone.remove(&who);
        }

        let Some(host) = self.host.clone() else { return };
        if self.came.contains_key(&host) {
            self.host_seen = now;
            return;
        }
        if now - self.host_seen < AWAY {
            return;
        }
        // Away too long. Somebody who is actually here takes them; if the room
        // is empty there is nobody to give them to and they wait.
        if let Some(next) = self.longest_here() {
            self.host = Some(next);
            self.host_seen = now;
        }
    }

    /// Whoever has been here longest, with the name as a tie-break so two
    /// people who arrived in the same millisecond do not disagree.
    fn longest_here(&self) -> Option<String> {
        let mut all: Vec<(&String, &f64)> = self.came.iter().collect();
        all.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
        all.first().map(|(who, _)| (*who).clone())
    }

    /// Who holds the controls.
    pub fn host(&self) -> Option<&str> {
        self.host.as_deref()
    }

    pub fn is_host(&self, who: &str) -> bool {
        self.host.as_deref() == Some(who)
    }

    /// Hand the controls to somebody else.
    ///
    /// Only the holder may, and only to somebody who is here — handing them to
    /// a person who has gone is how a room ends up with no host at all, which
    /// is the failure this crate exists to prevent.
    pub fn hand_over(&mut self, from: &str, to: &str) -> bool {
        if !self.is_host(from) || !self.came.contains_key(to) || from == to {
            return false;
        }
        self.host = Some(to.to_string());
        self.host_seen = f64::MAX;
        true
    }

    /// Give the controls up on purpose, on closing the page.
    ///
    /// The one case where leaving should be believed at once: somebody who has
    /// shut the tab is not coming back in five minutes, and the difference
    /// between this and going quiet is that they said so.
    pub fn depart(&mut self, who: &str, now: f64) {
        self.leave_at(who, now);
        self.chairs.retain(|_, sitter| sitter != who);
        self.gone.remove(who);
        if self.is_host(who) {
            self.host = self.longest_here();
            self.host_seen = now;
        }
    }

    /// Take a seat, if it is free. One each: taking a second gives up the
    /// first.
    pub fn sit(&mut self, who: &str, seat: usize, seats: usize) -> bool {
        if who.is_empty() || seat >= seats || !self.came.contains_key(who) {
            return false;
        }
        if self.chairs.get(&seat).is_some_and(|sitter| sitter != who) {
            return false;
        }
        self.chairs.retain(|_, sitter| sitter != who);
        self.chairs.insert(seat, who.to_string());
        true
    }

    pub fn stand(&mut self, who: &str) {
        self.chairs.retain(|_, sitter| sitter != who);
    }

    pub fn seat_of(&self, who: &str) -> Option<usize> {
        self.chairs.iter().find(|(_, sitter)| *sitter == who).map(|(seat, _)| *seat)
    }

    /// Who is in which seat, lowest first.
    pub fn seated(&self) -> Vec<(usize, String)> {
        let mut all: Vec<(usize, String)> = self.chairs.iter().map(|(k, v)| (*k, v.clone())).collect();
        all.sort();
        all
    }

    /// Everybody here, longest-present first — the order the controls would
    /// pass in.
    pub fn here(&self) -> Vec<String> {
        let mut all: Vec<(&String, &f64)> = self.came.iter().collect();
        all.sort_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(b.0)));
        all.into_iter().map(|(who, _)| who.clone()).collect()
    }

    /// Seats nobody is sitting in.
    pub fn empty_seats(&self, seats: usize) -> Vec<usize> {
        (0..seats).filter(|seat| !self.chairs.contains_key(seat)).collect()
    }

    /// Sit down everybody who has not, lowest free seat first.
    pub fn seat_everybody(&mut self, seats: usize) {
        for who in self.here() {
            if self.seat_of(&who).is_some() {
                continue;
            }
            let Some(seat) = self.empty_seats(seats).first().copied() else { break };
            self.sit(&who, seat, seats);
        }
    }

    /// **May this person do this?**
    ///
    /// The one question worth asking, and the one place to change when the
    /// answer should be more interesting than it is now.
    pub fn may(&self, who: &str, deed: Deed, turn: Option<usize>) -> bool {
        match deed {
            // The controls. One person, and they know who they are.
            Deed::Start | Deed::SetRules | Deed::HandOver => self.is_host(who),
            // A turn belongs to whoever is sitting in that seat, host or not.
            // Being in charge of the game is not being better at it.
            Deed::Play(seat) => {
                self.seat_of(who) == Some(seat) && turn.is_none_or(|whose| whose == seat)
            }
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn table() -> Table {
        Table::new()
    }

    /// ★ **Control is held, not derived.** It was worked out from the lowest
    /// occupied seat, so it moved every time anybody sat down — and one player
    /// watched the start button appear and disappear as the others took their
    /// colours.
    #[test]
    fn the_controls_do_not_move_when_people_sit_down() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        assert!(t.is_host("ann"), "she was here first");

        // Bob takes the lowest seat -- which used to make him the host.
        t.sit("bob", 0, 4);
        assert!(t.is_host("ann"), "and she still holds them");
        t.sit("ann", 3, 4);
        assert!(t.is_host("ann"));
        t.stand("ann");
        assert!(t.is_host("ann"), "even standing up entirely");
    }

    /// ★ There is always exactly one host, and never two.
    #[test]
    fn there_is_exactly_one_host() {
        let mut t = table();
        assert_eq!(t.host(), None, "an empty room has nobody");
        for (k, who) in ["ann", "bob", "cat", "dan"].iter().enumerate() {
            t.arrive(who, k as f64);
        }
        let hosts: Vec<&str> = ["ann", "bob", "cat", "dan"].into_iter().filter(|w| t.is_host(w)).collect();
        assert_eq!(hosts, vec!["ann"]);
    }

    /// ★ **Going quiet is not leaving.** The commonest reason to go quiet is
    /// sending somebody the link: switching to a messaging app backgrounds the
    /// page, where timers are throttled to about once a minute and a handset
    /// suspends them altogether.
    ///
    /// So the host who made the room, invited everybody and is sitting there
    /// looking at it stops calling in for as long as it takes to paste a URL —
    /// and used to lose the controls to the guest who had just arrived.
    #[test]
    fn going_quiet_does_not_hand_the_controls_over() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        // Ann switches to a messaging app. Bob keeps calling in.
        t.leave_at("ann", 1.0);
        t.settle(30.0);
        assert!(t.is_host("ann"), "half a minute away is not leaving");
        t.settle(AWAY - 1.0);
        assert!(t.is_host("ann"), "nor is four minutes");
        // And she comes back.
        t.arrive("ann", AWAY - 1.0);
        t.settle(AWAY + 60.0);
        assert!(t.is_host("ann"), "she never went anywhere");
    }

    /// ★ **But a room is never left without one.** Away long enough and the
    /// controls pass, or three people wait for somebody who is not coming.
    #[test]
    fn the_controls_pass_on_when_the_holder_is_really_gone() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.arrive("cat", 2.0);
        t.leave_at("ann", 1.0);
        t.settle(AWAY + 1.0);
        assert!(t.is_host("bob"), "the longest here takes them");
    }

    /// ★ Shutting the tab is believed at once. The difference between this and
    /// going quiet is that they said so.
    #[test]
    fn closing_the_page_hands_them_over_immediately() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.depart("ann", 2.0);
        assert!(t.is_host("bob"), "she closed it on purpose");
    }

    /// An empty room keeps them for whoever comes back, rather than handing
    /// them to nobody.
    #[test]
    fn an_empty_room_does_not_lose_the_controls() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.leave_at("ann", 1.0);
        t.settle(AWAY * 10.0);
        assert!(t.is_host("ann"), "there was nobody to give them to");
    }

    /// Somebody else leaving does not disturb them.
    #[test]
    fn somebody_else_leaving_changes_nothing() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.leave_at("bob", 1.0);
        t.settle(AWAY + 1.0);
        assert!(t.is_host("ann"));
    }

    /// ★ The controls can be given away, which is the point of knowing who has
    /// them.
    #[test]
    fn the_controls_can_be_handed_over() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        assert!(t.hand_over("ann", "bob"));
        assert!(t.is_host("bob"));
        assert!(!t.is_host("ann"));
    }

    /// ★ And only by the person holding them. Otherwise "who is in charge" is
    /// decided by whoever asks last.
    #[test]
    fn only_the_holder_may_hand_them_over() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.arrive("cat", 2.0);
        assert!(!t.hand_over("bob", "cat"), "bob does not have them to give");
        assert!(t.is_host("ann"));
        assert!(!t.hand_over("ann", "zoe"), "and zoe is not here");
        assert!(!t.hand_over("ann", "ann"), "nor to oneself, which would be a no-op that looked like a change");
    }

    /// ★ Being in charge of the game is not being better at it: a turn belongs
    /// to whoever is sitting in that seat.
    #[test]
    fn a_turn_belongs_to_the_seat_and_not_to_the_host() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.sit("ann", 0, 4);
        t.sit("bob", 1, 4);
        assert!(t.is_host("ann"));
        assert!(t.may("bob", Deed::Play(1), Some(1)), "bob's seat, bob's turn");
        assert!(!t.may("ann", Deed::Play(1), Some(1)), "not hers to take");
        assert!(!t.may("bob", Deed::Play(1), Some(0)), "and not when it is not his turn");
    }

    /// The controls are the controls, whoever is sitting where.
    #[test]
    fn only_the_host_starts_or_sets_rules() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.sit("bob", 0, 4);
        for deed in [Deed::Start, Deed::SetRules, Deed::HandOver] {
            assert!(t.may("ann", deed, None), "{deed:?} is hers");
            assert!(!t.may("bob", deed, None), "{deed:?} is not his");
        }
    }

    /// ★ Sitting everybody down leaves the ones who chose where they were.
    #[test]
    fn seating_everybody_respects_a_choice() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        t.arrive("cat", 2.0);
        t.sit("cat", 0, 4);
        t.seat_everybody(4);
        assert_eq!(t.seat_of("cat"), Some(0), "she wanted red");
        assert_eq!(t.seat_of("ann"), Some(1));
        assert_eq!(t.seat_of("bob"), Some(2));
        assert_eq!(t.empty_seats(4), vec![3]);
    }

    /// More people than seats: the ones who fit sit, and nobody is shoved out.
    #[test]
    fn more_people_than_seats_is_not_a_crash() {
        let mut t = table();
        for (k, who) in ["a", "b", "c", "d", "e", "f"].iter().enumerate() {
            t.arrive(who, k as f64);
        }
        t.seat_everybody(4);
        assert_eq!(t.seated().len(), 4);
        assert!(t.empty_seats(4).is_empty());
    }

    /// A seat is taken once, and taking a second gives up the first.
    #[test]
    fn one_seat_each() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("bob", 1.0);
        assert!(t.sit("ann", 0, 4));
        assert!(!t.sit("bob", 0, 4));
        assert!(t.sit("ann", 2, 4));
        assert!(t.sit("bob", 0, 4), "hers to give up, and she did");
        assert_eq!(t.seated(), vec![(0, "bob".into()), (2, "ann".into())]);
    }

    /// Somebody who is not here cannot sit down. A seat held by a name nobody
    /// has heard of is a seat nobody can take.
    #[test]
    fn a_stranger_cannot_sit() {
        let mut t = table();
        assert!(!t.sit("ghost", 0, 4));
        assert!(t.seated().is_empty());
    }

    /// Arriving twice is arriving once — a client that repeats itself must not
    /// look like a second person.
    #[test]
    fn arriving_twice_is_arriving_once() {
        let mut t = table();
        t.arrive("ann", 0.0);
        t.arrive("ann", 5.0);
        assert_eq!(t.here(), vec!["ann"]);
        assert!(t.is_host("ann"));
    }
}
