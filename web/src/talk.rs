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
/// Long enough to survive a slow frame or a tab being backgrounded, short
/// enough that somebody closing a laptop lid stops being in the room before
/// anybody wonders why they are so silent.
pub const PATIENCE: f64 = 12.0;

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
        self.forget(now);
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

    /// Drop anybody who has gone quiet, and their post with them.
    fn forget(&mut self, now: f64) {
        self.seen.retain(|_, at| now - *at < PATIENCE);
        let here: Vec<String> = self.seen.keys().cloned().collect();
        self.post.retain(|who, _| here.contains(who));
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

    /// ★ The advice a browser will not give you. On plain `http` over a
    /// network `getUserMedia` is not blocked, it is **absent**, and the page
    /// fails with a `TypeError` about `undefined`.
    #[test]
    fn it_says_why_the_microphone_is_missing() {
        assert!(Room::advice(false, "192.168.1.20:8088").is_some());
        assert!(Room::advice(true, "192.168.1.20:8088").is_none(), "https is fine");
        assert!(Room::advice(false, "localhost:8088").is_none(), "and so is localhost");
        assert!(Room::advice(false, "127.0.0.1:8088").is_none());
    }
}
