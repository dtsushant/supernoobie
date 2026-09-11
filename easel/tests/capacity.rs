//! What a room costs, measured.

use easel::{wire, Board};
use std::time::Instant;

fn look() -> wire::Look {
    wire::Look::new(plotkit::Cx::new(-12.0, -9.0), plotkit::Cx::new(12.0, 9.0), 900)
}

/// The drawing half of a scene — everything that says where things ARE, with
/// the clock and the room's chatter left out.
fn drawing(s: &str) -> String {
    let a = s.find("\"at\":").unwrap_or(0);
    let z = s.find("\"rings\":").unwrap_or(s.len());
    s[a..z].to_string()
}

#[test]
#[ignore]
fn capacity() {
    let mut b = Board::new();
    b.load("../samples/ludogame.easel").unwrap();
    b.playing_game = true;
    b.playing = true;
    b.clock = 40.0;
    let mark = wire::still_mark(&b, look());

    // A room where nothing is happening -- waiting for somebody to tap, which
    // is what a game of Ludo is most of the time.
    let mut idle = Vec::new();
    for k in 0..30 {
        b.clock = 40.0 + k as f64 / 30.0;
        idle.push(wire::since(&b, look(), "", mark));
    }
    let same = idle.windows(2).filter(|w| w[0] == w[1]).count();
    println!("IDLE whole response identical: {} of {}", same, idle.len() - 1);
    let drawn = idle.windows(2).filter(|w| drawing(&w[0]) == drawing(&w[1])).count();
    println!("IDLE drawing identical:        {} of {}", drawn, idle.len() - 1);

    let t = Instant::now();
    for k in 0..60 {
        b.clock = 40.0 + k as f64 / 30.0;
        let _ = wire::since(&b, look(), "", mark);
    }
    println!("IDLE render cost:              {:.1}ms", t.elapsed().as_secs_f64() * 1000.0 / 60.0);

    // And during a throw, when the die really is moving.
    b.clock = 100.0;
    b.play_tap(17);
    let mut live = Vec::new();
    for k in 0..30 {
        b.clock = 100.0 + k as f64 / 30.0;
        live.push(wire::since(&b, look(), "", mark));
    }
    let moving = live.windows(2).filter(|w| drawing(&w[0]) != drawing(&w[1])).count();
    println!("THROWING drawing changed:      {} of {}", moving, live.len() - 1);
}
