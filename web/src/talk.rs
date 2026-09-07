//! # talk — four people in a room, hearing each other
//!
//! ## What this file is, and what it is not
//!
//! **No voice passes through here.** This is a post office: it carries the
//! half-dozen notes two browsers must exchange before they can open a direct
//! connection to one another, and then it gets out of the way. The audio goes
//! peer to peer.
//!
//! That matters for a reason worth being exact about. Routing four people's
//! audio through a server means every word travels to the server and back —
//! two trips instead of one, and a server that must decode, mix and re-encode
//! four streams in real time. Peer to peer, a word makes one trip and the
//! server does arithmetic on nothing at all.
//!
//! ## Why polling and not a WebSocket
//!
//! Signalling is perhaps ten messages per person for the whole session: an
//! offer, an answer, and a handful of candidates. The page already asks this
//! server for a scene thirty-odd times a second, so the notes ride along with
//! traffic that exists anyway. A WebSocket would be the textbook answer and
//! would add a dependency to carry ten messages.
//!
//! The media does **not** ride along. It never touches this server.
//!
//! ## How two browsers find each other
//!
//! The protocol is **WebRTC**, and the part that looks like magic is ICE —
//! *Interactive Connectivity Establishment*, RFC 8445, largely the work of
//! **Jonathan Rosenberg**, who also wrote SIP. The problem it solves: two
//! machines behind home routers have no address the other can reach. Neither
//! can be dialled.
//!
//! ICE's answer is to gather every address a peer might be reachable at, try
//! all of them at once, and keep whichever works:
//!
//! | | |
//! |---|---|
//! | **host** | the address on its own network — works if you are in the same room |
//! | **srflx** | what a **STUN** server says your address looks like from outside (RFC 5389) |
//! | **relay** | a **TURN** server that forwards for you (RFC 8656) when nothing else works |
//!
//! STUN is a two-line protocol — *tell me what address this packet came
//! from* — and it works because most home routers keep the same outside port
//! for a given inside socket, so telling the other peer that address lets them
//! reach in. That is **hole punching**, and Bryan Ford's 2005 paper *Peer-to-Peer
//! Communication Across Network Address Translators* is the readable account of
//! why it works and when it does not.
//!
//! When it does not — a *symmetric* NAT gives a different outside port per
//! destination, so the address STUN reports is useless to anybody else — the
//! only remedy is a relay, and a relay costs bandwidth. That is why TURN
//! servers are the part of a voice application nobody can get for nothing, and
//! it is worth knowing before promising four friends it will work everywhere.
//!
//! ## The one thing that will stop this working
//!
//! A browser will not hand a page a microphone unless the page is a **secure
//! context**: `https://`, or `localhost`. Over a network on plain `http://`
//! there is no prompt and no error worth reading — `getUserMedia` is simply
//! not there. See [`Room::advice`], which says so out loud rather than leaving
//! somebody to find out.

use std::collections::HashMap;

/// How long a peer may go quiet before it is assumed gone, in seconds.
///
/// **Ninety, and it was twelve, and twelve was wrong.** A browser throttles a
/// backgrounded tab's timers to about once a minute, and a handset suspends
/// them entirely — so twelve seconds meant that switching to a messaging app
/// to send somebody the link made you leave the room you had just made.
///
/// Ninety survives the throttle. It does not survive a suspended handset, which
/// is why leaving is also announced outright when a page is closed, and why the
/// controls have a much longer grace of their own — see [`auth::AWAY`].
pub const PATIENCE: f64 = 90.0;

/// One note from one peer to another, waiting to be collected.
#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub from: String,
    /// `offer`, `answer` or `ice` — this file does not care which and never
    /// looks inside `body`.
    pub kind: String,
    pub body: String,
}

/// Who is here, and what is waiting for them.
///
/// Deliberately ignorant: it does not know what an offer is, what audio is, or
/// what the game is. A room is a set of names and a pile of letters.
#[derive(Debug, Default)]
pub struct Room {
    /// Peer id to when it was last heard from.
    seen: HashMap<String, f64>,
    /// Peer id to the notes waiting for it.
    post: HashMap<String, Vec<Note>>,
    /// What each peer is called.
    ///
    /// Not a login and not an identity — a label, so three other people can
    /// tell whose turn it is without counting colours round the board. Kept
    /// beside the seats rather than in them because somebody may want a name
    /// before they have decided which colour they are.
    names: HashMap<String, String>,
    /// Whether the game has begun.
    ///
    /// **On the room, not in each browser.** It was in the browser, and the
    /// second person to open the link got their own "start the game" screen
    /// over a game already in progress — and pressing it set the house rules
    /// again underneath everybody.
    begun: bool,
    /// Who is at the table, who is sitting where, and who holds the controls.
    ///
    /// In [`auth`] rather than here, because "who may do what" turned out to
    /// be the thing this file kept getting wrong — the host was *worked out*
    /// from the lowest occupied seat, so it moved whenever anybody sat down,
    /// and a player watched the start button appear and disappear as the
    /// others chose their colours.
    table: auth::Table,
}

impl Room {
    pub fn new() -> Room {
        Room::default()
    }

    /// Say that a peer is here, and collect whatever is waiting for it.
    ///
    /// One call does both because a peer that is collecting is, by that fact,
    /// still here — so there is nothing a client can forget to do.
    pub fn call(&mut self, me: &str, now: f64) -> Vec<Note> {
        if me.is_empty() {
            return Vec::new();
        }
        self.seen.insert(me.to_string(), now);
        self.table.arrive(me, now);
        self.forget(now);
        // Whether the game has lost its host is a patient question, asked here
        // rather than every time somebody goes quiet.
        self.table.settle(now);
        self.post.remove(me).unwrap_or_default()
    }

    /// Leave a note for somebody.
    ///
    /// A note for a peer nobody has heard of is **kept**, not dropped: two
    /// browsers starting at the same moment will each send before the other has
    /// called in, and losing that first offer means a connection that is never
    /// made and no error anywhere.
    pub fn send(&mut self, to: &str, note: Note) {
        if to.is_empty() || note.from.is_empty() {
            return;
        }
        let waiting = self.post.entry(to.to_string()).or_default();
        // A cap, because a peer that never collects would otherwise grow
        // without limit -- and because a hundred candidates is already far
        // more than any connection needs.
        if waiting.len() < 64 {
            waiting.push(note);
        }
    }

    /// Everybody in the room, in a settled order so two peers agree about who
    /// is who.
    pub fn here(&self) -> Vec<String> {
        let mut who: Vec<String> = self.seen.keys().cloned().collect();
        who.sort();
        who
    }

    /// Take a seat, if it is free.
    ///
    /// **First come, first served, and one seat each.** Taking a second seat
    /// gives up the first rather than holding both, because somebody who
    /// changes their mind about which colour they are should not thereby
    /// remove a chair from the table.
    ///
    /// Returns whether the seat is now theirs — including when it already was,
    /// since a client that repeats itself should not be told no.
    pub fn sit(&mut self, me: &str, seat: usize, how_many: usize) -> bool {
        self.table.sit(me, seat, how_many)
    }

    /// Call somebody something. Empty puts them back to being nobody in
    /// particular.
    pub fn call_them(&mut self, me: &str, name: &str) {
        if me.is_empty() {
            return;
        }
        // Trimmed, capped, and single-line: this goes on other people's
        // screens, and a name with forty spaces in it is a name that pushes
        // three seats off the edge.
        let tidy: String = name.trim().chars().filter(|c| !c.is_control()).take(16).collect();
        if tidy.is_empty() {
            self.names.remove(me);
        } else {
            self.names.insert(me.to_string(), tidy);
        }
    }

    /// What somebody is called, or what to call them if they have not said.
    ///
    /// A seated player with no name is *player 1* and not a peer id: the id is
    /// eight characters of nothing and helps nobody work out whose turn it is.
    pub fn name_of(&self, me: &str) -> String {
        if let Some(name) = self.names.get(me) {
            return name.clone();
        }
        match self.seat_of(me) {
            Some(seat) => format!("player {}", seat + 1),
            None => "watching".to_string(),
        }
    }

    /// Who decides when the game starts.
    ///
    /// **One person, or everybody gets a start button.** Four people each
    /// offered "start the game" is four people setting the house rules
    /// underneath one another, and whoever presses last wins an argument
    /// nobody knew they were having.
    ///
    /// It is the lowest occupied seat — not the first to arrive, because
    /// arriving is not a commitment and sitting down is. Somebody who takes
    /// red is saying they are playing; somebody who opened the link may be
    /// looking. If nobody has sat down at all, the first name in the room
    /// holds it, so a game with one person in it is not waiting for a host who
    /// does not exist.
    /// Which seats nobody is sitting in.
    ///
    /// What the bots take when the game begins. Worked out rather than stored,
    /// so somebody standing up mid-game does not leave a seat that is neither
    /// a person nor a bot.
    pub fn empty_seats(&self, how_many: usize) -> Vec<usize> {
        self.table.empty_seats(how_many)
    }

    /// Somebody has closed the page and said so.
    ///
    /// Believed at once, unlike going quiet: the difference is that they told
    /// us.
    pub fn depart(&mut self, me: &str, now: f64) {
        self.seen.remove(me);
        self.post.remove(me);
        self.names.remove(me);
        self.table.depart(me, now);
    }

    /// Has the game begun?
    pub fn begun(&self) -> bool {
        self.begun
    }

    /// Begin it, for everybody, **sitting down whoever has not**.
    ///
    /// Being in the room is meant to be enough. Two people opened a link,
    /// neither pressed a colour, and starting gave every seat to a bot and both
    /// of them a game to watch — which is right by the letter of "a bot plays
    /// an empty seat" and obviously not what anybody wanted.
    ///
    /// So arriving is taken as playing. Anybody who wants to watch can stand up
    /// again, and that is a deliberate choice they have made rather than one
    /// made for them by not noticing a button.
    ///
    /// Lowest free seat first, in a settled order, so the same room of people
    /// is seated the same way twice.
    pub fn begin(&mut self, how_many: usize) {
        self.begun = true;
        self.table.seat_everybody(how_many);
    }

    /// Which seat somebody is in, if any.
    pub fn seat_of(&self, me: &str) -> Option<usize> {
        self.table.seat_of(me)
    }

    /// Who holds the controls, and whether that is me.
    pub fn host(&self) -> Option<String> {
        self.table.host().map(str::to_string)
    }

    pub fn is_host(&self, me: &str) -> bool {
        self.table.is_host(me)
    }

    /// Give the controls away. Only the holder may, and only to somebody here.
    pub fn hand_over(&mut self, from: &str, to: &str) -> bool {
        self.table.hand_over(from, to)
    }

    /// May this person do this? See [`auth::Deed`].
    pub fn may(&self, who: &str, deed: auth::Deed, turn: Option<usize>) -> bool {
        self.table.may(who, deed, turn)
    }

    /// Who is in which seat, lowest first.
    pub fn seated(&self) -> Vec<(usize, String)> {
        self.table.seated()
    }

    /// Stand up, so somebody else may sit down.
    pub fn stand(&mut self, me: &str) {
        self.table.stand(me);
    }

    /// Drop anybody who has gone quiet, and their post — and their seat — with
    /// them.
    ///
    /// A seat held by somebody who has closed their laptop is a game of three
    /// people waiting for a fourth who is not coming.
    fn forget(&mut self, now: f64) {
        let gone: Vec<String> = self
            .seen
            .iter()
            .filter(|(_, at)| now - **at >= PATIENCE)
            .map(|(who, _)| who.clone())
            .collect();
        self.seen.retain(|_, at| now - *at < PATIENCE);
        let here: Vec<String> = self.seen.keys().cloned().collect();
        self.post.retain(|who, _| here.contains(who));
        self.names.retain(|who, _| here.contains(who));
        for who in gone {
            self.table.leave_at(&who, now);
        }
    }

    /// **Who calls whom.** Both peers must not offer at once, or each answers
    /// the other's offer and two connections form where one was wanted — the
    /// *glare* condition, and the reason every peer-to-peer protocol ever
    /// written has a rule like this one.
    ///
    /// The rule is: the lexicographically smaller id makes the offer. Any total
    /// order would do; what matters is that both sides work out the same answer
    /// with no exchange of messages, which they can, because both know both
    /// names.
    pub fn calls(a: &str, b: &str) -> bool {
        a < b
    }

    /// What to tell somebody whose microphone will not work.
    ///
    /// `None` when all is well. A browser gives no useful error for this: on a
    /// plain `http://` address over a network, `getUserMedia` is not missing
    /// permission — it is simply *absent*, and the page fails with a
    /// `TypeError` about `undefined`.
    pub fn advice(secure: bool, host: &str) -> Option<&'static str> {
        if secure || host.starts_with("localhost") || host.starts_with("127.") {
            None
        } else {
            Some(
                "a browser will not give a page a microphone over plain http. \
                 open this on localhost, or put it behind https.",
            )
        }
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn note(from: &str, kind: &str) -> Note {
        Note { from: from.into(), kind: kind.into(), body: "{}".into() }
    }

    /// ★ A note left for somebody is there when they call, and gone once they
    /// have it. Handing the same offer over twice would have the far side
    /// answer a connection it has already answered.
    #[test]
    fn a_note_is_delivered_once() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.send("bob", note("ann", "offer"));
        assert_eq!(r.call("bob", 0.1), vec![note("ann", "offer")]);
        assert!(r.call("bob", 0.2).is_empty(), "and not a second time");
    }

    /// ★ **A note for somebody who has not arrived yet is kept.** Two browsers
    /// opened together will each send before the other has called in, and
    /// dropping that first offer is a connection that never forms with nothing
    /// anywhere saying why.
    #[test]
    fn a_note_waits_for_somebody_who_is_not_here_yet() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.send("bob", note("ann", "offer"));
        assert_eq!(r.call("bob", 0.1), vec![note("ann", "offer")], "bob gets it on arrival");
    }

    /// Calling in is what says you are here — there is nothing separate to
    /// forget to do.
    #[test]
    fn calling_in_is_being_here() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        assert_eq!(r.here(), vec!["ann", "bob"]);
    }

    /// ★ Somebody who goes quiet leaves the room, and their post goes with
    /// them — otherwise a closed laptop is a name in the list for ever and a
    /// pile of undelivered candidates behind it.
    #[test]
    fn going_quiet_is_leaving() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.send("bob", note("ann", "ice"));
        // Ann keeps calling; Bob does not.
        r.call("ann", PATIENCE + 1.0);
        assert_eq!(r.here(), vec!["ann"], "bob has gone");
        assert!(r.call("bob", PATIENCE + 1.1).is_empty(), "and his post with him");
    }

    /// And coming back is just calling again.
    #[test]
    fn coming_back_is_calling_again() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("ann", PATIENCE + 1.0);
        assert_eq!(r.here(), vec!["ann"]);
    }

    /// ★ **Exactly one of any two peers offers.** Both offering at once is the
    /// glare condition: each answers the other and two connections form where
    /// one was wanted.
    #[test]
    fn exactly_one_of_two_peers_calls() {
        for (a, b) in [("ann", "bob"), ("bob", "ann"), ("a", "aa"), ("z1", "z2")] {
            assert_ne!(Room::calls(a, b), Room::calls(b, a), "{a} and {b} must disagree");
        }
    }

    /// Both sides work it out alone, with nothing exchanged — which they can,
    /// because each knows both names.
    #[test]
    fn both_sides_agree_without_asking() {
        let who = ["ann", "bob", "cat", "dan"];
        for a in who {
            for b in who {
                if a != b {
                    assert_eq!(Room::calls(a, b), !Room::calls(b, a));
                }
            }
        }
    }

    /// ★ Four people is six connections, and every pair has exactly one
    /// caller. This is the whole of a mesh: `n(n−1)/2` links, which is why a
    /// mesh is right for four and wrong for forty.
    #[test]
    fn four_people_make_six_connections_and_no_arguments() {
        let mut r = Room::new();
        for who in ["ann", "bob", "cat", "dan"] {
            r.call(who, 0.0);
        }
        let here = r.here();
        let mut links = 0;
        for (i, a) in here.iter().enumerate() {
            for b in &here[i + 1..] {
                links += 1;
                assert!(Room::calls(a, b) ^ Room::calls(b, a), "{a} and {b}");
            }
        }
        assert_eq!(links, 4 * 3 / 2);
    }

    /// A peer that never collects does not grow without limit.
    #[test]
    fn undelivered_post_is_capped() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        for _ in 0..500 {
            r.send("bob", note("ann", "ice"));
        }
        assert!(r.call("bob", 0.1).len() <= 64);
    }

    /// Nameless peers are refused rather than sharing one empty pigeonhole.
    #[test]
    fn a_peer_must_have_a_name() {
        let mut r = Room::new();
        assert!(r.call("", 0.0).is_empty());
        assert!(r.here().is_empty(), "an empty name is not somebody");
        // Neither a note to nobody nor a note from nobody is kept.
        r.send("", note("ann", "offer"));
        r.send("bob", note("", "offer"));
        assert!(r.call("bob", 0.1).is_empty());
    }

    /// ★ **One seat each, first come first served.** Two people in one colour
    /// is not a thing this should be able to represent, let alone allow.
    #[test]
    fn a_seat_is_taken_once() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        assert!(r.sit("ann", 0, 4));
        assert!(!r.sit("bob", 0, 4), "bob cannot have ann's chair");
        assert!(r.sit("bob", 1, 4));
        assert_eq!(r.seat_of("ann"), Some(0));
        assert_eq!(r.seat_of("bob"), Some(1));
    }

    /// ★ Changing your mind gives the old seat up rather than holding both —
    /// otherwise somebody trying the colours removes chairs from the table.
    #[test]
    fn moving_seats_frees_the_old_one() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        assert!(r.sit("ann", 0, 4));
        assert!(r.sit("ann", 2, 4));
        assert_eq!(r.seat_of("ann"), Some(2));
        assert_eq!(r.seated(), vec![(2, "ann".to_string())], "and only the new one");
        // Bob has to be in THIS room to sit in it -- the test used to call him
        // into a different one and pass, because sitting did not check.
        r.call("bob", 0.0);
        assert!(r.sit("bob", 0, 4), "seat 0 is free again");
    }

    /// Sitting where you already sit is not an error. A client that repeats
    /// itself should not be told no.
    #[test]
    fn sitting_twice_is_fine() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        assert!(r.sit("ann", 1, 4));
        assert!(r.sit("ann", 1, 4));
    }

    /// A seat that is not at the table cannot be taken.
    #[test]
    fn there_are_only_so_many_chairs() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        assert!(!r.sit("ann", 4, 4), "seats are 0..3");
        assert!(!r.sit("ann", 99, 4));
        assert!(!r.sit("", 0, 4), "and nobody cannot sit");
    }

    /// ★ **A colour is kept through a lapse.** A phone that locks its screen
    /// stops calling in, and freeing the seat at once meant coming back a
    /// different colour -- or being put somewhere else entirely by the next
    /// seating. Green, then blue, without anybody touching anything.
    #[test]
    fn a_seat_survives_going_quiet() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.sit("bob", 1, 4);
        // Bob's phone sleeps. Ann keeps playing.
        r.call("ann", PATIENCE + 1.0);
        assert_eq!(r.seat_of("bob"), Some(1), "green is still his");
        // And he comes back to the same colour.
        r.call("bob", PATIENCE + 2.0);
        assert_eq!(r.seat_of("bob"), Some(1));
    }

    /// ★ But a chair is not held for ever. Away as long as it takes to lose
    /// the controls, and it goes.
    #[test]
    fn a_seat_is_let_go_after_a_long_absence() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.sit("bob", 1, 4);
        // Noticed gone after PATIENCE, and the chair kept for AWAY after THAT
        // -- the absence is timed from when it was noticed, since that is the
        // last moment anybody knows he was there.
        r.call("ann", PATIENCE + 1.0);
        assert_eq!(r.seat_of("bob"), Some(1), "noticed, but the colour is kept");
        r.call("ann", PATIENCE + auth::AWAY + 2.0);
        assert_eq!(r.seat_of("bob"), None, "he really has gone");
    }

    /// And closing the page gives it up at once.
    #[test]
    fn closing_the_page_gives_up_the_seat() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.sit("bob", 1, 4);
        r.depart("bob", 1.0);
        assert_eq!(r.seat_of("bob"), None);
    }

    /// And standing up is standing up.
    #[test]
    fn standing_up_frees_a_seat() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.sit("ann", 3, 4);
        r.stand("ann");
        assert_eq!(r.seat_of("ann"), None);
    }

    /// ★ **Somebody with no name is "player 1", not eight characters of
    /// nothing.** The peer id helps nobody work out whose turn it is.
    #[test]
    fn an_unnamed_player_is_called_by_their_seat() {
        let mut r = Room::new();
        r.call("a7f3k2p9", 0.0);
        assert_eq!(r.name_of("a7f3k2p9"), "watching", "not seated yet");
        r.sit("a7f3k2p9", 2, 4);
        assert_eq!(r.name_of("a7f3k2p9"), "player 3");
        r.call_them("a7f3k2p9", "Sushant");
        assert_eq!(r.name_of("a7f3k2p9"), "Sushant");
    }

    /// A name goes on other people's screens, so it is trimmed, capped and
    /// stripped of anything that is not a character.
    #[test]
    fn a_name_cannot_wreck_the_board() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.sit("ann", 0, 4);
        r.call_them("ann", "   Ann   ");
        assert_eq!(r.name_of("ann"), "Ann");
        r.call_them("ann", &"x".repeat(400));
        assert_eq!(r.name_of("ann").len(), 16);
        r.call_them("ann", "a\nb\tc");
        assert_eq!(r.name_of("ann"), "abc");
        // And clearing it goes back to the seat.
        r.call_them("ann", "  ");
        assert_eq!(r.name_of("ann"), "player 1");
    }

    /// A name goes when its owner does.
    #[test]
    fn a_name_leaves_with_its_owner() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.call_them("bob", "Bob");
        r.call("ann", PATIENCE + 1.0);
        assert_eq!(r.name_of("bob"), "watching", "bob has gone");
    }

    /// ★ **The game begins once, for the room.** It was per-browser, and the
    /// second person to open the link got their own "start the game" over a
    /// game already running — and pressing it set the house rules again
    /// underneath everybody.
    #[test]
    fn a_game_begins_once_for_everybody() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        assert!(!r.begun(), "nobody has started it");
        r.begin(4);
        assert!(r.begun(), "and now it has, for both of them");
    }

    /// ★ **Starting sits down whoever has not.** Two people opened a link,
    /// neither pressed a colour, and starting gave every seat to a bot and both
    /// of them a game to watch. Being in the room is meant to be enough.
    #[test]
    fn starting_seats_everybody_who_is_here() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        assert_eq!(r.empty_seats(4), vec![0, 1, 2, 3], "nobody has chosen a colour");
        r.begin(4);
        assert_eq!(r.seated().len(), 2, "and now both are playing");
        assert_eq!(r.empty_seats(4), vec![2, 3], "with two seats left for bots");
    }

    /// Somebody who did choose keeps what they chose.
    #[test]
    fn starting_leaves_a_chosen_seat_alone() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.sit("ann", 3, 4);
        r.begin(4);
        assert_eq!(r.seat_of("ann"), Some(3), "she wanted yellow");
        assert_eq!(r.seat_of("bob"), Some(0), "and bob got the lowest free one");
    }

    /// More people than seats: the ones who fit sit, and the rest watch rather
    /// than shoving somebody out.
    #[test]
    fn more_people_than_seats_is_not_a_crash() {
        let mut r = Room::new();
        for who in ["a", "b", "c", "d", "e", "f"] {
            r.call(who, 0.0);
        }
        r.begin(4);
        assert_eq!(r.seated().len(), 4);
        assert!(r.empty_seats(4).is_empty(), "no bots needed");
    }

    /// ★ **Hostship does not move when people sit down.** It used to be
    /// worked out from the lowest occupied seat, so it changed every time
    /// anybody took a colour -- and a player watched the start button appear
    /// and disappear as the others chose. It is held now; see [`auth`], which
    /// is where the rule and the rest of its tests live.
    #[test]
    fn the_controls_stay_with_whoever_has_them() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 1.0);
        assert_eq!(r.host(), Some("ann".into()), "she was here first");
        r.sit("bob", 0, 4);
        assert_eq!(r.host(), Some("ann".into()), "and taking the lowest seat does not take them");
        r.sit("ann", 3, 4);
        assert_eq!(r.host(), Some("ann".into()));
    }

    /// A room always has somebody holding them, so a game with one person in
    /// it is not waiting for a host who does not exist.
    #[test]
    fn somebody_always_holds_the_start() {
        let mut r = Room::new();
        assert_eq!(r.host(), None, "an empty room has nobody");
        r.call("zoe", 0.0);
        assert_eq!(r.host(), Some("zoe".into()));
    }

    /// ★ **Going quiet does not hand them over.** Sending somebody the link
    /// backgrounds the page, and a backgrounded page stops calling in — which
    /// is how the person who made the room lost it to their own guest.
    #[test]
    fn going_quiet_does_not_hand_the_controls_over() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 1.0);
        assert_eq!(r.host(), Some("ann".into()));
        // Ann is off in a messaging app for a good while. Bob keeps calling.
        r.call("bob", PATIENCE + 2.0);
        assert_eq!(r.host(), Some("ann".into()), "she is quiet, not gone");
        r.call("bob", auth::AWAY - 1.0);
        assert_eq!(r.host(), Some("ann".into()), "still hers");
    }

    /// ★ But away long enough and they pass, or the room waits for somebody
    /// who is not coming.
    #[test]
    fn the_controls_pass_on_after_a_long_absence() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 1.0);
        r.call("bob", auth::AWAY + PATIENCE + 2.0);
        assert_eq!(r.host(), Some("bob".into()), "ann really has gone");
    }

    /// ★ And closing the page is believed at once, because they said so.
    #[test]
    fn closing_the_page_hands_them_over_at_once() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 1.0);
        r.depart("ann", 2.0);
        assert_eq!(r.host(), Some("bob".into()));
    }

    /// ★ Seats nobody took are what the bots get. Worked out rather than
    /// stored, so standing up mid-game does not leave a seat that is neither a
    /// person nor a bot.
    #[test]
    fn the_empty_seats_are_the_bots() {
        let mut r = Room::new();
        r.call("ann", 0.0);
        r.call("bob", 0.0);
        r.sit("ann", 0, 4);
        r.sit("bob", 2, 4);
        assert_eq!(r.empty_seats(4), vec![1, 3]);
        r.stand("bob");
        assert_eq!(r.empty_seats(4), vec![1, 2, 3], "his seat is a bot now");
    }

    /// ★ The advice a browser will not give you. On plain `http` over a
    /// network `getUserMedia` is not blocked, it is **absent**, and the page
    /// fails with a `TypeError` about `undefined`.
    #[test]
    fn it_says_why_the_microphone_is_missing() {
        assert!(Room::advice(false, "192.0.2.20:8088").is_some());
        assert!(Room::advice(true, "192.0.2.20:8088").is_none(), "https is fine");
        assert!(Room::advice(false, "localhost:8088").is_none(), "and so is localhost");
        assert!(Room::advice(false, "127.0.0.1:8088").is_none());
    }
}
