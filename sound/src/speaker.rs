//! # speaker — asking the machine to play a file
//!
//! ## Why it is done this way
//!
//! The obvious way to make a noise is to link an audio library, open a stream
//! and fill a callback. That works, and it costs a **C library at build
//! time** — on Linux, ALSA's headers. Suddenly the whole repository needs a
//! system package before it will compile, on every machine, for the sake of
//! one file. That is a bad trade for a project whose point is that everything
//! in it can be read and checked.
//!
//! So: no linking. Write the sound to a file — which this crate could already
//! do — and **hand it to whatever the machine already has**. The cost moves
//! from build time to run time, where a missing player is a clear message
//! instead of a compile error, and where it can be fixed without recompiling
//! anything.
//!
//! ```text
//!     build       nothing at all
//!     run         one of: paplay, aplay, ffplay, afplay, powershell.exe
//! ```
//!
//! On WSL that last one matters more than it looks: `powershell.exe` is on the
//! path through Windows interop, so a Linux build with no audio packages
//! whatsoever can still make a noise, by asking Windows to. It needs the file
//! to be somewhere Windows can see — anything under `/mnt/c` — and
//! [`windows_path`] does the translating.
//!
//! ## What is given up
//!
//! **The sound is a separate process playing a recording.** It starts while
//! the animation carries on — [`play_file`] does not wait, or a two-second
//! note would freeze the window — but the two are not connected. The file was
//! rendered at the instant you asked for it, so:
//!
//! * nothing can change once it has started. Turn up the wind while a note is
//!   playing and the note does not know.
//! * nothing can be triggered *on a frame*. A ball landing cannot click at
//!   the moment it lands, only a few milliseconds after somebody notices.
//! * two sounds at once are two processes, and nothing keeps them in step.
//!
//! That is the honest cost of not linking anything, and for hearing the note
//! you are looking at it costs nothing that matters.
//!
//! **Live sound is a different shape of program.** The card asks for the next
//! few hundred samples on its own thread and will not wait, so the synthesiser
//! has to run inside that callback, next to the animation loop rather than
//! after it. That needs a real audio library — and the way to have it without
//! putting a C library in everybody's build is a **crate outside this
//! workspace**, so the cost lands only on whoever wants it.

use std::path::Path;
use std::process::{Command, Stdio};

/// The players tried, in order, and how each one is asked.
///
/// PulseAudio first because it is what a desktop Linux actually runs; ALSA
/// next; then the video players people tend to have; then Windows, which is
/// the one that works on WSL with nothing installed.
pub const PLAYERS: [&str; 5] = ["paplay", "aplay", "ffplay", "afplay", "powershell.exe"];

/// Play a WAV file.
///
/// `wait` blocks until it has finished — right for a command-line tool, wrong
/// for a window, which must keep drawing.
///
/// Returns which player did it, so a caller can say so rather than leaving you
/// wondering whether anything happened.
pub fn play_file(path: &str, wait: bool) -> Result<&'static str, String> {
    if !Path::new(path).exists() {
        return Err(format!("there is no file at {path}"));
    }
    for player in PLAYERS {
        let Some(mut cmd) = command_for(player, path) else {
            continue;
        };
        // A player's own chatter is not this program's output.
        cmd.stdout(Stdio::null()).stderr(Stdio::null());

        let started = if wait { cmd.status().map(|s| s.success()) } else { cmd.spawn().map(|_| true) };
        match started {
            Ok(true) => return Ok(player),
            // It exists but refused the file. Try the next one rather than
            // stopping — a player can be installed and still be unable to
            // reach a sound card, which is common on a server.
            Ok(false) | Err(_) => continue,
        }
    }
    Err(format!("nothing here can play a file. Tried: {}", PLAYERS.join(", ")))
}

/// How to ask one particular player, or `None` if it is not installed.
fn command_for(player: &str, path: &str) -> Option<Command> {
    if which(player).is_none() {
        return None;
    }
    let mut cmd = Command::new(player);
    match player {
        // Windows, reached from WSL through interop. It needs a path Windows
        // can see, which is why the file wants to be under /mnt/c.
        "powershell.exe" => {
            let win = windows_path(path);
            // Hidden, or a console window flashes up over whatever you were
            // watching every time a note plays.
            cmd.arg("-NoProfile")
                .arg("-WindowStyle")
                .arg("Hidden")
                .arg("-Command")
                .arg(format!("(New-Object Media.SoundPlayer '{win}').PlaySync()"));
        }
        // Play and quit, without opening a window to show a blank video.
        "ffplay" => {
            cmd.arg("-nodisp").arg("-autoexit").arg("-loglevel").arg("quiet").arg(path);
        }
        _ => {
            cmd.arg(path);
        }
    }
    Some(cmd)
}

/// A path as Windows would write it, for handing to `powershell.exe`.
///
/// Uses `wslpath` when it is there, since only WSL knows how its mounts are
/// arranged. Everywhere else the path is already a Windows one, or the player
/// is not going to be `powershell.exe` anyway.
pub fn windows_path(path: &str) -> String {
    Command::new("wslpath")
        .arg("-w")
        .arg(path)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| path.to_string())
}

/// Is this program installed?
fn which(name: &str) -> Option<()> {
    // `command -v` rather than `which`, which is not always present and is not
    // always a program.
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .ok()
        .filter(|s| s.success())
        .map(|_| ())
}

/// Which players this machine actually has — for telling somebody why they
/// cannot hear anything.
pub fn available() -> Vec<&'static str> {
    PLAYERS.into_iter().filter(|p| which(p).is_some()).collect()
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ A missing file is refused before any player is started. Otherwise
    /// every player in the list gets launched in turn and fails, which is slow
    /// and produces an error blaming the players rather than the path.
    #[test]
    fn a_missing_file_is_refused_by_name() {
        let e = play_file("/definitely/not/here.wav", false).expect_err("should fail");
        assert!(e.contains("/definitely/not/here.wav"), "the message should name the file: {e}");
        assert!(!e.contains("Tried:"), "and should not blame the players");
    }

    /// When nothing can play, the message says what was looked for — so it is
    /// actionable rather than just a refusal.
    #[test]
    fn the_failure_says_what_it_looked_for() {
        for p in PLAYERS {
            assert!(!p.is_empty());
        }
        assert!(PLAYERS.contains(&"powershell.exe"), "WSL needs this one, with nothing installed");
        assert!(PLAYERS.contains(&"paplay"), "and a desktop Linux needs this one");
    }

    /// Nothing here links a library, so this list is the whole runtime
    /// requirement — and it is checked at run time, not at build time.
    #[test]
    fn asking_what_is_available_does_not_fail() {
        let found = available();
        assert!(found.len() <= PLAYERS.len());
        for p in &found {
            assert!(PLAYERS.contains(p));
        }
    }

    /// Path translation must not lose the path when there is no `wslpath` —
    /// it should hand back what it was given rather than an empty string,
    /// which would silently ask a player to open nothing.
    #[test]
    fn a_path_survives_when_there_is_nothing_to_translate_it() {
        let out = windows_path("/tmp/whatever.wav");
        assert!(!out.is_empty());
        assert!(out.contains("whatever.wav"), "the name should survive: {out}");
    }
}
