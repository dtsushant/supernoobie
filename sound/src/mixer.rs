//! # mixer — sound that can be started on a frame, and changed while it plays
//!
//! ## Why this exists and [`speaker`](super::speaker) is not enough
//!
//! Writing a WAV and handing it to a player gets a noise out of the machine,
//! and for hearing the note you are looking at that is enough. It cannot do the
//! thing this module is for:
//!
//! ```text
//!     a recording          rendered once, then played. Frozen.
//!     a mixer              asked for the next few hundred samples, forever.
//! ```
//!
//! A ball landing has to click **at the moment it lands**, not a few
//! milliseconds after somebody notices. Wind has to get louder **while it is
//! already blowing**. Three things at once have to be *one* sound rather than
//! three programs. None of that survives being rendered in advance.
//!
//! ## The shape of live sound
//!
//! The sound card does not ask politely. It runs on its own thread, wakes up
//! every few milliseconds and says *"give me 256 samples"*, and if you are slow
//! it plays whatever was in the buffer — a click. So the whole design is:
//!
//! ```text
//!     the animation           the card's thread
//!     ------------            ----------------
//!     strike(a bounce)  --->  [ Mixer ] ---> fill(&mut [f32])
//!     level(WIND, 0.6)  --->            ---> fill(&mut [f32])
//! ```
//!
//! Two kinds of message go left to right, and they are exactly the two kinds
//! of thing that happen in an animation:
//!
//! * [`strike`](Mixer::strike) — **something happened.** A bounce, a tap, a
//!   key. It has a moment and then it is over. Fire and forget.
//! * [`level`](Mixer::level) — **something is going on.** Wind, a motor, a
//!   drone. It has no beginning or end, only a strength that changes.
//!
//! That is the same split as [`physics::Trigger`] on the other side: an edge is
//! a strike, a value is a level. *"A state persists, an edge happens once."*
//!
//! ## Nothing in here touches hardware
//!
//! Deliberately. [`fill`](Mixer::fill) writes into a slice, and a slice is a
//! slice whether it came from a sound card or a test. So every claim below is
//! checked without a speaker, and the crate still links no C library — the
//! binding to real hardware lives outside this workspace, where the cost lands
//! only on whoever wants to hear it.
//!
//! ## The two things that are hard, and are why this is tested
//!
//! **Blocks must not be seams.** The card asks in chunks, and the chunk size
//! is not yours to choose — it changes with the machine, the driver, the
//! weather. If a voice restarts its clock each block, every block boundary is
//! a click, and the clicks arrive at a few hundred a second, which is itself a
//! tone. So each voice keeps its own clock and the block size must be
//! *inaudible*: filling 512 samples must give exactly what filling two lots of
//! 256 gives. There is a test that says so.
//!
//! **The sum must not clip.** Sixteen voices each of amplitude 1 is amplitude
//! 16, and anything past 1 is not loud — it is a *crunch*, because the samples
//! past the limit are simply cut off, and a cut-off sine is full of harmonics
//! that were never in the sound. So the sum is squashed rather than cut:
//! `tanh` bends the loud parts back towards the limit and leaves the quiet
//! parts alone, which is what a valve does and part of why valves are liked.

use crate::tone::Tone;

/// How many things can sound at once before the quietest is dropped.
///
/// Not a technical limit; a musical one. Past a dozen or so, another voice
/// does not add anything you can pick out, it just uses up the room between
/// silence and the limit that the ones you *can* hear need.
pub const VOICES: usize = 16;

/// What a voice is actually making.
#[derive(Clone, Debug)]
pub enum Source {
    /// A pitched note with an envelope, from [`Tone`]. Ends by itself.
    Note(Tone),
    /// A knock: filtered noise that **dies away**. One thing striking another.
    ///
    /// [`Rustle`](Source::Rustle) is noise that goes on until stopped, which is
    /// wind. A knock is the same noise with an envelope on it, and the two are
    /// not the same primitive: a die landing on a board is not a very short
    /// gust.
    ///
    /// It is not a *note* either, and that was the mistake worth recording. A
    /// tone with a decay is a pitched thing being struck — a bell, a bar, a
    /// spoon on metal, all of which ring. A die on card does not ring: the
    /// contact is broadband and the board absorbs it, so what is left is a dull
    /// edge with no pitch to speak of. `cut` is the whole of the difference
    /// between plastic on card and something metallic.
    Knock { cut: f64, decay: f64 },
    /// Filtered noise: wind, rustling, rushing. Goes on until stopped.
    ///
    /// `cut` is the low-pass corner in Hz. Low is a distant rumble, high is a
    /// hiss — and moving it is most of what makes wind sound like it is
    /// getting up rather than merely getting louder.
    Rustle { cut: f64 },
}

/// One thing making a noise.
#[derive(Clone, Debug)]
pub struct Voice {
    pub source: Source,
    /// How loud, 0 to 1. For a held voice this is changed while it plays.
    pub gain: f64,
    /// Seconds since it started — its **own** clock, which is what makes the
    /// block size inaudible.
    t: f64,
    /// Noise generator state. A plain integer shuffle, so the same strike
    /// always gives the same noise and a recorded run replays exactly.
    grit: u32,
    /// The low-pass filter's memory. One number is a whole filter.
    memory: f64,
}

impl Voice {
    /// A note: a bounce, a creak, a tap.
    pub fn note(tone: Tone) -> Voice {
        Voice { source: Source::Note(tone), gain: 1.0, t: 0.0, grit: 0x2545_F491, memory: 0.0 }
    }

    /// One thing striking another: filtered noise that dies away.
    pub fn knock(cut: f64, decay: f64) -> Voice {
        Voice {
            source: Source::Knock { cut, decay },
            gain: 1.0,
            t: 0.0,
            grit: 0x1D8E_3B4F,
            memory: 0.0,
        }
    }

    /// Noise through a low-pass: wind, rushing, rustling.
    pub fn rustle(cut: f64) -> Voice {
        Voice { source: Source::Rustle { cut }, gain: 1.0, t: 0.0, grit: 0x9E37_79B9, memory: 0.0 }
    }

    /// How loud to start.
    pub fn at(mut self, gain: f64) -> Voice {
        self.gain = gain.clamp(0.0, 1.0);
        self
    }

    /// Give it its own noise, so two rustles together are not the identical
    /// sound twice — which does not sound like two of anything, it sounds like
    /// one louder one.
    pub fn seeded(mut self, seed: u32) -> Voice {
        self.grit = seed | 1;
        self
    }

    /// Has it finished? Only notes ever do; a rustle is stopped from outside.
    pub fn done(&self) -> bool {
        match &self.source {
            // Below a thousandth of full scale nothing is audible, and a
            // voice held open for an envelope that will never quite reach zero
            // is a slot that something else needed.
            Source::Note(tone) => self.t > 0.0 && tone.envelope(self.t) * self.gain < 1e-3,
            Source::Knock { decay, .. } => self.t > 4.0 * decay.max(1e-4),
            Source::Rustle { .. } => false,
        }
    }

    /// Roughly how loud it is *right now* — for deciding which voice to drop
    /// when they run out. Dropping the quietest is the one choice nobody
    /// hears.
    pub fn loudness(&self) -> f64 {
        match &self.source {
            Source::Note(tone) => tone.envelope(self.t) * self.gain,
            Source::Knock { decay, .. } => (-self.t / decay.max(1e-4)).exp() * self.gain,
            Source::Rustle { .. } => self.gain,
        }
    }

    /// White noise through a one-pole low-pass — shared by a knock and a
    /// rustle, because they differ by an envelope and nothing else.
    ///
    /// The noise is a shuffle of bits, not a random number: **deterministic**,
    /// so a recorded run replays exactly. The filter is
    /// `memory += (x − memory)·a`, which moves part of the way towards each
    /// new value — the same equation as everything else here that approaches a
    /// target, with τ = 1/2πcut.
    fn hiss(&mut self, cut: f64, dt: f64) -> f64 {
        self.grit ^= self.grit << 13;
        self.grit ^= self.grit >> 17;
        self.grit ^= self.grit << 5;
        let white = f64::from(self.grit) / f64::from(u32::MAX) * 2.0 - 1.0;
        let a = (1.0 - (-std::f64::consts::TAU * cut * dt).exp()).clamp(0.0, 1.0);
        self.memory += (white - self.memory) * a;
        // Filtering takes energy out, so put some back -- otherwise a low cut
        // is not a rumble, it is silence.
        self.memory * (1.0 + 2.0 / a.max(1e-6).sqrt()).min(6.0)
    }

    /// The next sample, and move on by `dt` seconds.
    pub fn next(&mut self, dt: f64) -> f64 {
        // Copied out before the noise generator is touched, since making a
        // sample changes the shuffle and the filter's memory.
        let out = match self.source {
            Source::Note(ref tone) => tone.at(self.t),
            Source::Knock { cut, decay } => {
                let raw = self.hiss(cut, dt);
                // The same e^{-t/tau} as everything else here. A knock is a
                // gust that lasts fifteen milliseconds.
                raw * (-self.t / decay.max(1e-4)).exp()
            }
            Source::Rustle { cut } => self.hiss(cut, dt),
        };
        self.t += dt;
        out * self.gain
    }
}

/// Everything sounding at once.
///
/// Held by the animation, read by the sound card. Nothing in it allocates
/// while filling, because allocating on the card's thread is how a program
/// misses a deadline and clicks.
#[derive(Clone, Debug, Default)]
pub struct Mixer {
    shots: Vec<Voice>,
    held: Vec<(u32, Voice)>,
}

impl Mixer {
    pub fn new() -> Mixer {
        Mixer { shots: Vec::with_capacity(VOICES), held: Vec::with_capacity(4) }
    }

    /// **Something happened.** Start a sound that will end by itself.
    ///
    /// This is what a [`physics::Trigger`] firing turns into: a ball's
    /// `Turning` edge with a strength becomes a note struck that hard.
    ///
    /// If everything is busy the quietest voice is taken — the one choice
    /// nobody can hear. Never refuses, because a bounce that is silently
    /// dropped is a bug you will spend an evening on.
    pub fn strike(&mut self, voice: Voice) {
        if self.shots.len() >= VOICES {
            let quietest = self
                .shots
                .iter()
                .enumerate()
                .min_by(|(_, a), (_, b)| a.loudness().total_cmp(&b.loudness()))
                .map(|(k, _)| k);
            if let Some(k) = quietest {
                self.shots.swap_remove(k);
            }
        }
        self.shots.push(voice);
    }

    /// **Something is going on.** Start a sound that keeps going until stopped,
    /// under a name you can reach it by afterwards.
    ///
    /// Starting one that is already held replaces it, so calling this every
    /// frame is safe.
    pub fn hold(&mut self, id: u32, voice: Voice) {
        match self.held.iter_mut().find(|(k, _)| *k == id) {
            Some(slot) => slot.1 = voice,
            None => self.held.push((id, voice)),
        }
    }

    /// Change how loud a held sound is, **while it is playing**. The thing a
    /// recording cannot do.
    ///
    /// Silently does nothing if that sound is not held — so an animation can
    /// set the wind level every frame without caring whether sound is on.
    pub fn level(&mut self, id: u32, gain: f64) {
        if let Some((_, v)) = self.held.iter_mut().find(|(k, _)| *k == id) {
            v.gain = gain.clamp(0.0, 1.0);
        }
    }

    /// Change the brightness of a held rustle. Wind getting up is not only
    /// louder, it is *hissier*, and the ear reads the brightness as speed more
    /// than it reads the volume.
    pub fn colour(&mut self, id: u32, cut: f64) {
        if let Some((_, v)) = self.held.iter_mut().find(|(k, _)| *k == id) {
            if let Source::Rustle { cut: c } = &mut v.source {
                *c = cut.max(1.0);
            }
        }
    }

    /// Stop a held sound.
    pub fn release(&mut self, id: u32) {
        self.held.retain(|(k, _)| *k != id);
    }

    /// Stop everything at once.
    pub fn hush(&mut self) {
        self.shots.clear();
        self.held.clear();
    }

    /// How many things are sounding — for showing on screen.
    pub fn sounding(&self) -> usize {
        self.shots.len() + self.held.len()
    }

    /// Fill a block of samples. **This is the whole interface to the hardware.**
    ///
    /// `out` is interleaved: with `channels = 2` that is L,R,L,R. Both channels
    /// get the same thing, because nothing here has a position yet — that
    /// arrives with three dimensions, where a source is somewhere and the two
    /// ears hear it at slightly different times.
    ///
    /// Finished voices are cleared out here, which is the only place it needs
    /// to happen and costs nothing.
    pub fn fill(&mut self, out: &mut [f32], channels: usize, rate: u32) {
        let dt = 1.0 / f64::from(rate.max(1));
        let channels = channels.max(1);

        for frame in out.chunks_mut(channels) {
            let mut sum = 0.0;
            for v in self.shots.iter_mut().chain(self.held.iter_mut().map(|(_, v)| v)) {
                sum += v.next(dt);
            }
            // Squashed, not cut. `tanh` leaves anything quiet almost exactly
            // alone — it is x for small x — and bends the loud back towards
            // the limit, so a busy moment gets compressed rather than torn.
            let s = sum.tanh() as f32;
            for slot in frame.iter_mut() {
                *slot = s;
            }
        }

        self.shots.retain(|v| !v.done());
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{pitch, Timbre};

    fn ping() -> Voice {
        Voice::note(Tone::pluck(pitch::A4).with_timbre(Timbre::pure()).with_decay(0.05))
    }

    /// ★ **The block size must be inaudible.** The card chooses how much to ask
    /// for and it changes with the machine; if a voice restarted its clock each
    /// block, every boundary would be a click, arriving a few hundred a second
    /// — which is itself a tone, and a horrible one.
    #[test]
    fn one_big_block_is_the_same_as_two_small_ones() {
        let build = || {
            let mut m = Mixer::new();
            m.strike(ping());
            m.hold(1, Voice::rustle(800.0).at(0.4));
            m
        };

        let mut whole = build();
        let mut one = vec![0.0f32; 512];
        whole.fill(&mut one, 1, 8_000);

        let mut halves = build();
        let mut two = vec![0.0f32; 512];
        let (a, b) = two.split_at_mut(256);
        halves.fill(a, 1, 8_000);
        halves.fill(b, 1, 8_000);

        assert_eq!(one, two, "the seam between blocks must not exist");
    }

    /// ★ **The sum must not clip.** Sixteen voices of amplitude 1 is amplitude
    /// 16, and past 1 the samples are simply cut off — which is not loudness,
    /// it is a crunch, because a cut-off sine is full of harmonics that were
    /// never in the sound.
    #[test]
    fn everything_at_once_does_not_clip() {
        let mut m = Mixer::new();
        for k in 0..VOICES {
            m.strike(Voice::note(Tone::bowed(200.0 + k as f64 * 37.0).with_timbre(Timbre::saw(12))));
        }
        m.hold(1, Voice::rustle(4_000.0));

        let mut block = vec![0.0f32; 4_000];
        m.fill(&mut block, 1, 8_000);
        for s in &block {
            assert!(s.abs() <= 1.0, "it clipped at {s}");
        }
        assert!(block.iter().any(|s| s.abs() > 0.5), "and it should still be loud");
    }

    /// And quiet sounds are left alone by the squashing — a limiter that
    /// changed everything would make one loud moment turn the whole piece
    /// down.
    #[test]
    fn squashing_leaves_the_quiet_alone() {
        let mut m = Mixer::new();
        m.strike(ping().at(0.05));
        let mut block = vec![0.0f32; 400];
        m.fill(&mut block, 1, 8_000);

        let mut bare = ping().at(0.05);
        for (k, s) in block.iter().enumerate() {
            let want = bare.next(1.0 / 8_000.0);
            assert!((f64::from(*s) - want).abs() < 1e-4, "sample {k} was bent: {s} vs {want}");
        }
    }

    /// ★ A note ends by itself and frees its slot. Without that, a long
    /// animation accumulates thousands of silent voices, each still being
    /// evaluated on the card's thread, until it misses a deadline and clicks.
    #[test]
    fn a_note_clears_itself_away() {
        let mut m = Mixer::new();
        m.strike(ping());
        assert_eq!(m.sounding(), 1);

        let mut block = vec![0.0f32; 8_000]; // a full second; the decay is 0.05
        m.fill(&mut block, 1, 8_000);
        assert_eq!(m.sounding(), 0, "it should have gone by itself");
    }

    /// But a held sound does not — it is stopped from outside, because it has
    /// no natural end.
    #[test]
    fn a_held_sound_stays_until_it_is_released() {
        let mut m = Mixer::new();
        m.hold(7, Voice::rustle(500.0));
        let mut block = vec![0.0f32; 16_000];
        m.fill(&mut block, 1, 8_000);
        assert_eq!(m.sounding(), 1, "wind does not stop on its own");

        m.release(7);
        assert_eq!(m.sounding(), 0);
    }

    /// ★ **The thing a recording cannot do**: turn the wind up while it is
    /// already blowing.
    #[test]
    fn a_held_sound_changes_while_it_plays() {
        let mut m = Mixer::new();
        m.hold(1, Voice::rustle(2_000.0).at(0.1));

        let energy = |m: &mut Mixer| {
            let mut b = vec![0.0f32; 2_000];
            m.fill(&mut b, 1, 8_000);
            b.iter().map(|s| f64::from(*s).abs()).sum::<f64>() / b.len() as f64
        };

        let quiet = energy(&mut m);
        m.level(1, 0.8);
        let loud = energy(&mut m);
        assert!(loud > quiet * 2.0, "turning it up should be heard: {quiet} -> {loud}");

        m.level(1, 0.0);
        assert!(energy(&mut m) < quiet, "and turning it down");
    }

    /// Setting the level of something that is not playing is not an error — so
    /// an animation can push the wind strength every frame without knowing
    /// whether sound is switched on at all.
    #[test]
    fn talking_to_a_sound_that_is_not_there_is_harmless() {
        let mut m = Mixer::new();
        m.level(99, 0.5);
        m.colour(99, 1_000.0);
        m.release(99);
        assert_eq!(m.sounding(), 0);
    }

    /// ★ Running out of voices steals the **quietest**, and never refuses. A
    /// bounce that is silently dropped because the pool was full is an evening
    /// spent looking for a bug that is not in the physics.
    #[test]
    fn a_full_pool_drops_the_one_nobody_can_hear() {
        let mut m = Mixer::new();
        for _ in 0..VOICES {
            m.strike(ping().at(0.001)); // all but inaudible
        }
        assert_eq!(m.shots.len(), VOICES);

        m.strike(ping().at(1.0)); // the one that matters
        assert_eq!(m.shots.len(), VOICES, "the pool should not grow");
        assert!(m.shots.iter().any(|v| v.gain > 0.9), "and the loud one should have got in");
    }

    /// Nothing playing is silence, not noise — an uninitialised buffer handed
    /// straight to a card is the loudest sound a computer can make.
    #[test]
    fn nothing_playing_is_actual_silence() {
        let mut m = Mixer::new();
        let mut block = vec![9.0f32; 100];
        m.fill(&mut block, 1, 8_000);
        assert!(block.iter().all(|s| *s == 0.0), "it must write the block, not skip it");
    }

    /// Stereo gets both channels written. Half-filling an interleaved buffer
    /// leaves one ear with the last block's contents, which is a buzz in one
    /// side.
    #[test]
    fn both_ears_get_the_same_sound() {
        let mut m = Mixer::new();
        m.strike(ping());
        let mut block = vec![0.0f32; 200];
        m.fill(&mut block, 2, 8_000);
        for pair in block.chunks(2) {
            assert_eq!(pair[0], pair[1]);
        }
        assert!(block.iter().any(|s| *s != 0.0));
    }

    /// ★ The noise is deterministic, so a recorded run replays as the same
    /// sound. `grit` is a bit-shuffle, not a random number generator — the
    /// whole repository's tapes depend on nothing being genuinely random.
    #[test]
    fn the_same_run_makes_the_same_noise() {
        let take = || {
            let mut m = Mixer::new();
            m.hold(1, Voice::rustle(1_000.0));
            let mut b = vec![0.0f32; 500];
            m.fill(&mut b, 1, 8_000);
            b
        };
        assert_eq!(take(), take());
    }

    /// But two rustles at once must not be the identical noise twice — that
    /// does not sound like two of anything, only like one louder one.
    #[test]
    fn two_rustles_are_two_different_noises() {
        let mut m = Mixer::new();
        m.hold(1, Voice::rustle(1_000.0).at(0.5));
        m.hold(2, Voice::rustle(1_000.0).seeded(0x1234_5678).at(0.5));

        let mut both = vec![0.0f32; 500];
        m.fill(&mut both, 1, 8_000);

        let mut alone = Mixer::new();
        alone.hold(1, Voice::rustle(1_000.0).at(0.5));
        let mut one = vec![0.0f32; 500];
        alone.fill(&mut one, 1, 8_000);

        let doubled = both.iter().zip(&one).filter(|(a, b)| (*a - *b * 2.0).abs() < 1e-6).count();
        assert!(doubled < 100, "the second rustle is just the first one twice");
    }

    /// ★ Brightness is what the ear reads as wind speed — more than volume.
    /// A low cut is a distant rumble and a high one is a hiss, and moving it is
    /// most of what makes wind sound like it is getting up.
    #[test]
    fn a_higher_cut_makes_a_brighter_noise() {
        // Brightness measured as how often the signal crosses zero: a hiss
        // crosses constantly, a rumble hardly at all.
        let crossings = |cut: f64| {
            let mut m = Mixer::new();
            m.hold(1, Voice::rustle(cut));
            let mut b = vec![0.0f32; 4_000];
            m.fill(&mut b, 1, 16_000);
            b.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
        };
        assert!(crossings(4_000.0) > crossings(200.0) * 2, "a high cut should hiss and a low one rumble");
    }

    /// And the colour of a held rustle can be changed while it plays, which is
    /// how a gust rises rather than merely appearing.
    #[test]
    fn a_gust_can_brighten_while_it_blows() {
        let mut m = Mixer::new();
        m.hold(1, Voice::rustle(200.0));
        let crossings = |m: &mut Mixer| {
            let mut b = vec![0.0f32; 4_000];
            m.fill(&mut b, 1, 16_000);
            b.windows(2).filter(|w| (w[0] < 0.0) != (w[1] < 0.0)).count()
        };
        let dull = crossings(&mut m);
        m.colour(1, 5_000.0);
        assert!(crossings(&mut m) > dull * 2, "it should have brightened without restarting");
    }

    /// Hushing stops everything — for a reset, where a note left ringing over
    /// a fresh start is confusing.
    #[test]
    fn hushing_stops_the_lot() {
        let mut m = Mixer::new();
        m.strike(ping());
        m.hold(1, Voice::rustle(500.0));
        m.hush();
        assert_eq!(m.sounding(), 0);
    }
}
