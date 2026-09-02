//! # live — the same world, with a speaker
//!
//! ```text
//!     cargo run --manifest-path live/Cargo.toml --release
//! ```
//!
//! The ball clicks **on the frame it lands**. The tree creaks when it swings,
//! and not while it is merely leaning. The wind gets louder and hissier while
//! it is already blowing. None of that is possible with a recording, and none
//! of it is new code — the world worked all of this out already, in
//! [`studio::world`], where it is tested in silence. This file only carries
//! the answers to a sound card.
//!
//! ## Why this is a separate crate
//!
//! Because it links `cpal`, and on Linux `cpal` needs ALSA's headers at build
//! time. Put it in the workspace and the whole repository stops compiling on a
//! machine without a system package — for the sake of one file. So the
//! manifest here declares an empty `[workspace]`, which makes this its own
//! root: `cargo test --workspace` in the parent never sees it, never resolves
//! `cpal`, and still needs nothing at all.
//!
//! **The cost lands only on whoever wants to hear it.** That is the whole
//! arrangement, and it is why `sound::mixer` writes into a slice rather than
//! into a device — a slice is a slice whether it came from a sound card or a
//! test.
//!
//! ## The awkward part: two clocks
//!
//! ```text
//!     the window's thread          the card's thread
//!     -------------------          -----------------
//!     world.advance(t)             fill(&mut [f32])
//!     drain sounds  ---- Mutex ---->   strike / level
//!     draw
//! ```
//!
//! The card runs on its own thread and **will not wait**. It wakes every few
//! milliseconds, asks for a block, and plays whatever was in the buffer if you
//! are slow — which is a click. So two rules, and they are the only ones:
//!
//! * **Hold the lock for as little as possible.** The animation takes it only
//!   to hand over a few voices and a number, never while drawing.
//! * **Never allocate inside the callback.** Which is why
//!   [`Mixer::fill`](sound::Mixer::fill) does not, and why the voice pool is
//!   fixed.
//!
//! A `Mutex` is the wrong tool for real audio work and the right one here: the
//! critical section is a `Vec::drain` of at most eight items, and a lock held
//! for that long will not be noticed. A real engine uses a lock-free ring
//! buffer, and that is a thing to do when it is a problem rather than before.

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sound::{kit, Mixer};
use studio::world::{scene, World};
use studio::Graph;

/// What the window owns: the world, and a line to the sound card.
struct Live {
    world: World,
    ears: Arc<Mutex<Mixer>>,
}

impl Live {
    /// One step of the world, and everything it just said, handed over.
    ///
    /// This is the entire binding, and it is worth seeing how little it is.
    /// The world does not know a sound card exists; the mixer does not know a
    /// tree exists.
    fn advance(&mut self, t: f64) {
        self.world.advance(t);

        let Ok(mut mixer) = self.ears.lock() else {
            // The audio thread panicked. The animation is still worth watching,
            // so carry on in silence rather than bringing the window down.
            return;
        };
        // The things that HAPPENED: struck, once each, and then over.
        for voice in self.world.sounds.drain(..) {
            mixer.strike(voice);
        }
        // The thing that is GOING ON: not struck, adjusted. The wind is one
        // continuous sound whose level and colour are pushed at it every
        // frame — which is exactly what a recording cannot be.
        mixer.level(kit::WIND, self.world.gale);
        mixer.colour(kit::WIND, kit::gustiness(self.world.gale));
    }

    /// Something the person did, rather than something the world did.
    fn tapped(&mut self) {
        self.world.tapped();
    }
}

fn main() {
    let ears = Arc::new(Mutex::new(Mixer::new()));

    // Wind is held from the start and simply sits at zero until there is any.
    // Starting and stopping it with the weather would mean a click at each end
    // and a voice that has to be found again every time.
    if let Ok(mut m) = ears.lock() {
        m.hold(kit::WIND, kit::wind(0.0));
    }

    // Keep the stream alive for as long as the window is: dropping it stops
    // the sound, and a stream created and dropped inside a function is a
    // silence that looks exactly like a bug.
    let _stream = match open_speaker(Arc::clone(&ears)) {
        Ok(s) => {
            println!("sound is on. The ball clicks when it lands; the tree creaks when it swings.");
            Some(s)
        }
        Err(e) => {
            eprintln!("no sound: {e}");
            eprintln!("  the animation still runs. On Linux this usually means ALSA cannot reach a device.");
            None
        }
    };

    Graph::new("live -- the playground, with sound")
        .scale(44.0)
        .with(Live { world: World::new(), ears })
        .each_frame(Live::advance)
        .on_hold('>', |a| a.world.wind = (a.world.wind + 0.06).min(30.0))
        .on_hold('<', |a| a.world.wind = (a.world.wind - 0.06).max(-30.0))
        .on('0', |a| {
            a.world.wind = 0.0;
            a.tapped();
        })
        .on_hold('u', |a| a.world.g = (a.world.g + 0.12).min(60.0))
        .on_hold('j', |a| a.world.g = (a.world.g - 0.12).max(0.0))
        .on('m', |a| {
            a.world.g = physics::fall::gravity::MOON;
            a.tapped();
        })
        .on('e', |a| {
            a.world.g = physics::fall::gravity::EARTH;
            a.tapped();
        })
        // Every key you press answers, because feedback for your own action is
        // most of what makes a thing feel alive rather than laggy.
        .on('1', |a| {
            a.world.voice = 0;
            a.tapped();
        })
        .on('2', |a| {
            a.world.voice = 1;
            a.tapped();
        })
        .on('3', |a| {
            a.world.voice = 2;
            a.tapped();
        })
        .on('4', |a| {
            a.world.voice = 3;
            a.tapped();
        })
        .run(|a: &Live| scene(&a.world));
}

/// Open the default output and keep asking the mixer for samples.
///
/// Everything about the device is decided by the device: how many channels,
/// what rate, what sample format. Asking for a particular one is how a program
/// works on the machine it was written on and nowhere else.
fn open_speaker(ears: Arc<Mutex<Mixer>>) -> Result<cpal::Stream, String> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or("there is no output device")?;
    let config = device.default_output_config().map_err(|e| e.to_string())?;

    let rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    println!("speaker: {} at {rate} Hz, {channels} channel(s)", device.name().unwrap_or_else(|_| "?".into()));

    let format = config.sample_format();
    let config: cpal::StreamConfig = config.into();

    // The callback. It runs on the card's thread, it must not allocate, and it
    // must not block for long — so all it does is take the lock and fill.
    let fill = move |data: &mut [f32], _: &cpal::OutputCallbackInfo| match ears.lock() {
        Ok(mut mixer) => mixer.fill(data, channels, rate),
        // Better a moment of silence than the last block played again, which
        // is a buzz.
        Err(_) => data.fill(0.0),
    };
    let complain = |e| eprintln!("sound stopped: {e}");

    // f32 is what everything modern wants; the other two are for older cards,
    // and converting is the only difference.
    let stream = match format {
        cpal::SampleFormat::F32 => device.build_output_stream(&config, fill, complain, None),
        cpal::SampleFormat::I16 => {
            let mut scratch = vec![0.0f32; 0];
            device.build_output_stream(
                &config,
                move |data: &mut [i16], info| {
                    scratch.resize(data.len(), 0.0);
                    fill(&mut scratch, info);
                    for (out, s) in data.iter_mut().zip(&scratch) {
                        // Clamped, not wrapped. Wrapping a sample past the
                        // limit does not clip it — it flips it to the opposite
                        // extreme, which is a bang.
                        *out = (s.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                    }
                },
                complain,
                None,
            )
        }
        cpal::SampleFormat::U16 => {
            let mut scratch = vec![0.0f32; 0];
            device.build_output_stream(
                &config,
                move |data: &mut [u16], info| {
                    scratch.resize(data.len(), 0.0);
                    fill(&mut scratch, info);
                    for (out, s) in data.iter_mut().zip(&scratch) {
                        // Unsigned: silence is halfway up, not zero.
                        *out = ((s.clamp(-1.0, 1.0) * 0.5 + 0.5) * f32::from(u16::MAX)) as u16;
                    }
                },
                complain,
                None,
            )
        }
        other => return Err(format!("this card wants {other:?}, which is not handled")),
    };

    let stream = stream.map_err(|e| e.to_string())?;
    stream.play().map_err(|e| e.to_string())?;
    Ok(stream)
}
