//! # web — the studio in a browser
//!
//! ```text
//!     cargo run -p web --release
//!     cargo run -p web --release -- samples/adding.easel
//!     then open http://127.0.0.1:8088
//! ```
//!
//! ## Why a server and not WebAssembly
//!
//! Both put the drawing in a browser. WebAssembly needs a second compiler
//! target, a JavaScript glue generator and a bundler; a server needs `cargo
//! run` and a URL. Everything here is already pure Rust, so a server adds
//! nothing to the build — and this repository has spent some effort on not
//! needing a toolchain nobody asked for.
//!
//! If the round trip ever hurts, the same [`easel::wire`] format is what a
//! WebAssembly build would produce anyway, so that door stays open.
//!
//! ## What each side is for
//!
//! ```text
//!     Rust        what the drawing IS   -- shapes, poses, rules, the file
//!     the browser what it LOOKS like    -- a canvas, and real inputs
//! ```
//!
//! The split is not arbitrary. Everything that gave trouble in the desktop
//! window — a text caret, arrow keys, scrolling, a colour picker, a layout
//! that reflows, a font with lower case in it — is something a browser has had
//! for thirty years and I was writing by hand. None of it is mathematics, and
//! none of it was worth writing.
//!
//! Everything that *is* mathematics stays in Rust, tested, and does not move.
//!
//! ## Panning and zooming never reach here
//!
//! Shapes go over in the numbers the drawing is written in, and the browser
//! applies the view. So a drag of the paper is a matrix on the client at
//! whatever rate the hand moves. Only what changes the *drawing* is a request.
//!
//! ## One board, one lock
//!
//! There is a single drawing and a single person drawing it, so a mutex round
//! it is honest rather than lazy. If two people ever share one, the thing to
//! reach for is the event log this repository already believes in — send the
//! edits, not the state — and not a finer-grained lock.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::header;
use axum::response::{Html, IntoResponse};
use axum::routing::{get, post};
use axum::{Json, Router};
use easel::wire::Look;
use easel::{Action, Board, Tool};
use plotkit::Cx;
use serde::Deserialize;

type Shared = Arc<Mutex<Studio>>;

/// The drawing, and the one thing about it the browser cannot hold.
struct Studio {
    board: Board,
    file: String,
    say: String,
}

#[tokio::main]
async fn main() {
    let file = std::env::args().nth(1).unwrap_or_else(|| "drawing.easel".to_string());
    let mut board = Board::new();
    let mut say = String::new();
    if std::path::Path::new(&file).exists() {
        say = match board.load(&file) {
            Ok(0) => format!("opened {file}"),
            Ok(bad) => format!("opened {file} -- {bad} lines lost"),
            Err(e) => format!("could not open {file}: {e}"),
        };
        println!("{say}");
    }

    let shared: Shared = Arc::new(Mutex::new(Studio { board, file, say }));
    let app = Router::new()
        .route("/", get(home))
        .route("/studio", get(page))
        .route("/list", get(list_files))
        .route("/app.js", get(js))
        .route("/app.css", get(css))
        .route("/scene", get(scene))
        .route("/do", post(act))
        .with_state(shared);

    let at = SocketAddr::from(([127, 0, 0, 1], 8088));
    let listener = tokio::net::TcpListener::bind(at).await.expect("could not take the port");
    println!("http://{at}");
    axum::serve(listener, app).await.expect("the server stopped");
}

async fn home() -> Html<&'static str> {
    Html(include_str!("../static/home.html"))
}

async fn page() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// The drawings this server will open.
///
/// Listed by looking, not by a list kept somewhere — a list would go stale the
/// first time somebody saved something new.
async fn list_files() -> impl IntoResponse {
    let mut found: Vec<String> = Vec::new();
    for dir in ["samples", "."] {
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for e in entries.flatten() {
            let path = e.path();
            if path.extension().and_then(|x| x.to_str()) != Some("easel") {
                continue;
            }
            let name = path.to_string_lossy().replace('\\', "/");
            let name = name.strip_prefix("./").unwrap_or(&name).to_string();
            if !found.contains(&name) {
                found.push(name);
            }
        }
    }
    found.sort();
    let body = format!("[{}]", found.iter().map(|f| format!("\"{f}\"")).collect::<Vec<_>>().join(","));
    ([(header::CONTENT_TYPE, "application/json")], body)
}

/// Is this a file this server is willing to touch?
///
/// Only `.easel` and `.rec`, only relative, and nothing with `..` in it. A
/// server that opens whatever path it is handed will one day be asked for
/// `../../../etc/passwd`, and the fact that this one is meant for one person
/// on one machine is not a reason to leave the door open — it is a reason
/// nobody would notice it was open.
fn allowed(name: &str) -> bool {
    let ok_kind = name.ends_with(".easel") || name.ends_with(".rec");
    let traversal = name.split(['/', '\\']).any(|part| part == "..");
    let absolute = name.starts_with('/') || name.contains(':');
    ok_kind && !traversal && !absolute && !name.is_empty()
}

async fn js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/javascript")], include_str!("../static/app.js"))
}

async fn css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], include_str!("../static/app.css"))
}

/// Where the client is looking, so curves sampled against the window are
/// sampled against the right one.
#[derive(Deserialize)]
struct Where {
    lox: f64,
    loy: f64,
    hix: f64,
    hiy: f64,
    px: usize,
    /// Which still half the page already holds. Nought means none.
    #[serde(default)]
    have: u64,
}

impl From<&Where> for Look {
    fn from(w: &Where) -> Look {
        Look::new(Cx::new(w.lox, w.loy), Cx::new(w.hix, w.hiy), w.px)
    }
}

async fn scene(State(s): State<Shared>, Query(w): Query<Where>) -> impl IntoResponse {
    let studio = s.lock().expect("the drawing");
    (
        [(header::CONTENT_TYPE, "application/json")],
        easel::wire::since(&studio.board, (&w).into(), "", w.have),
    )
}

/// Everything the browser can ask the drawing to do.
///
/// One shape of message rather than a route each. A route per verb reads
/// tidily and then every new verb is a new route, a new handler and a new
/// place to forget the lock — where this is one `match` arm.
#[derive(Deserialize)]
#[serde(tag = "do")]
enum Ask {
    Pointer { x: f64, y: f64, down: bool },
    Tool { name: String },
    Nib { which: usize },
    Paint { colour: String },
    Verb { name: String },
    Row { id: usize, text: String },
    RowOn { id: usize, on: bool },
    Dial { id: usize, value: f64 },
    AddRow,
    AddShape,
    Choose { mark: Option<usize>, group: Option<u32> },
    Fold { group: u32 },
    Group,
    Ungroup,
    Play { on: bool },
    /// Watch it rather than edit it: the pen taps and nothing else.
    Watch { on: bool },
    Rewind,
    Key,
    Unkey,
    Step { forwards: bool },
    Undo,
    Redo,
    Smooth,
    Clear,
    Save,
    Open,
    /// Open a different drawing, by name.
    OpenFile { name: String },
    Tick { seconds: f64 },
}

async fn act(State(s): State<Shared>, Query(w): Query<Where>, Json(ask): Json<Ask>) -> impl IntoResponse {
    let mut studio = s.lock().expect("the drawing");
    apply(&mut studio, ask);
    let word = std::mem::take(&mut studio.say);
    ([(header::CONTENT_TYPE, "application/json")], easel::wire::since(&studio.board, (&w).into(), &word, w.have))
}

/// Do one thing to the drawing.
///
/// Separate from the handler on purpose: the handler is a lock, a call and a
/// scene, and everything that could be *wrong* is in here, where it can be
/// tested without a socket.
fn apply(st: &mut Studio, ask: Ask) {
    match ask {
        Ask::Pointer { x, y, down } => {
            st.board.pointer(Cx::new(x, y), down);
            if !down {
                // What a lift actually did, in the page's own words. A studio
                // that silently does nothing is one you cannot tell from a
                // studio that is broken -- which is exactly the position I was
                // in a moment ago.
                st.say = match (st.board.playing_game, st.board.selected.len()) {
                    (true, _) => String::new(),
                    (false, 0) => "nothing chosen".into(),
                    (false, n) => format!("{n} chosen"),
                };
            }
        }
        Ask::Tool { name } => {
            st.board.tool = match name.as_str() {
                "pick" => Tool::Pick,
                "rub" => Tool::Erase,
                _ => Tool::Draw,
            }
        }
        Ask::Nib { which } => {
            let w = match st.board.nib {
                shapes::Nib::Round(w) => w,
                shapes::Nib::Quill { slow, .. } => slow,
                shapes::Nib::Broad { width, .. } => width,
            };
            st.board.nib = match which {
                1 => shapes::Nib::Round(w),
                2 => shapes::Nib::Broad { width: w, angle: std::f64::consts::FRAC_PI_4 },
                _ => shapes::Nib::Quill { slow: w, fast: w * 0.15, pace: 0.16 },
            };
        }
        Ask::Paint { colour } => {
            if let Some(c) = hex(&colour) {
                st.board.paint(c);
            }
        }
        Ask::Verb { name } => {
            let action = verb(&name);
            st.say = match action {
                None => {
                    if st.board.stop_doing() {
                        "it does nothing now".into()
                    } else {
                        "choose a shape first".into()
                    }
                }
                Some(a) if st.board.give(a, Some(easel::tree::STEP)) => format!("{name} added"),
                Some(_) => "choose a shape first".into(),
            };
        }
        // The text arrives whole rather than as keystrokes, because the input
        // it came from is a real one -- the caret, the arrow keys, selecting,
        // pasting and undoing inside the box are all the browser's business
        // and none of them need to be sent.
        Ask::Row { id, text } => {
            if let Some(r) = st.board.sheet.script.rows.get_mut(id) {
                r.text = text;
            }
        }
        Ask::RowOn { id, on } => {
            if st.board.sheet.script.rows.get(id).is_some_and(|r| r.on != on) {
                st.board.toggle_row(id);
            }
        }
        Ask::Dial { id, value } => {
            st.board.set_dial(id, value);
        }
        Ask::AddRow => st.board.add_row(),
        Ask::AddShape => {
            let k = st.board.add_shape();
            st.say = format!("shape {k} added");
        }
        Ask::Choose { mark, group } => match (mark, group) {
            (Some(k), _) => st.board.choose_only(k),
            (_, Some(g)) => st.board.choose_group(g),
            _ => st.board.selected.clear(),
        },
        Ask::Fold { group } => st.board.fold(group),
        Ask::Group => {
            st.say = if st.board.group() { "one figure now".into() } else { "choose two or more".into() };
        }
        Ask::Ungroup => {
            st.board.ungroup();
        }
        Ask::Play { on } => {
            st.board.play(on);
            st.board.playing_game = on && !st.board.sheet.script.rules().is_empty();
        }
        Ask::Watch { on } => st.board.watch(on),
        Ask::Rewind => {
            st.board.rewind();
            st.board.restart();
            st.board.playing_game = false;
        }
        Ask::Key => {
            st.board.key();
        }
        Ask::Unkey => {
            st.board.unkey();
        }
        Ask::Step { forwards } => {
            st.board.next_key(forwards);
        }
        Ask::Undo => {
            st.board.undo();
        }
        Ask::Redo => {
            st.board.redo();
        }
        Ask::Smooth => st.board.smooth_all(4),
        Ask::Clear => st.board.clear(),
        Ask::Save => {
            st.say = match st.board.save(&st.file) {
                Ok(()) => format!("saved {}", st.file),
                Err(e) => format!("could not save: {e}"),
            }
        }
        Ask::OpenFile { name } => {
            st.say = if !allowed(&name) {
                format!("{name} is not a drawing this will open")
            } else if name.ends_with(".rec") {
                match std::fs::read_to_string(&name) {
                    Ok(text) => {
                        st.board = Board::new();
                        st.board.sheet.script = easel::Script::from_rec(&text);
                        st.file = name.replace(".rec", ".easel");
                        format!("imported {name} -- saving goes to {}", st.file)
                    }
                    Err(e) => format!("could not read {name}: {e}"),
                }
            } else {
                let mut fresh = Board::new();
                match fresh.load(&name) {
                    Ok(bad) => {
                        st.board = fresh;
                        st.file = name.clone();
                        if bad == 0 {
                            format!("opened {name}")
                        } else {
                            format!("opened {name} -- {bad} lines lost")
                        }
                    }
                    // A name that is not there yet is a new drawing, not a
                    // mistake: that is how a blank page is asked for.
                    Err(_) => {
                        st.board = Board::new();
                        st.file = name.clone();
                        format!("a blank page, which will save to {name}")
                    }
                }
            };
        }
        Ask::Open => {
            st.say = match st.board.load(&st.file) {
                Ok(_) => format!("opened {}", st.file),
                Err(e) => format!("could not open: {e}"),
            }
        }
        // The clock is stepped by the client, because the client is what knows
        // when it drew last. A server ticking on its own would run at a rate
        // nobody was watching at.
        Ask::Tick { seconds } => st.board.tick(seconds.clamp(0.0, 0.2)),
    }
}

fn hex(s: &str) -> Option<u32> {
    u32::from_str_radix(s.trim_start_matches('#'), 16).ok()
}

fn verb(name: &str) -> Option<Action> {
    easel::tree::verbs_list().into_iter().find(|(n, _)| *n == name).and_then(|(_, a)| a)
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    /// ★ Every verb the browser can name is one the studio knows. A button
    /// that posts a word nothing matches does nothing and says nothing, which
    /// is the hardest kind of broken to notice.
    #[test]
    fn every_verb_the_page_offers_is_one_the_studio_knows() {
        for (name, action) in easel::tree::verbs_list() {
            assert_eq!(verb(name).is_some(), action.is_some(), "{name}");
        }
        assert!(verb("cartwheel").is_none());
    }

    /// The page and the server must agree on the verbs, or a button is a
    /// button that does nothing.
    #[test]
    fn the_page_names_the_same_verbs_the_server_does() {
        let page = include_str!("../static/index.html");
        for (name, _) in easel::tree::verbs_list() {
            assert!(page.contains(&format!("data-verb=\"{name}\"")), "the page never offers {name}");
        }
    }

    fn a_game() -> Studio {
        let mut board = Board::new();
        board.load("../samples/adding.easel").expect("the game opens");
        Studio { board, file: String::new(), say: String::new() }
    }

    fn score(st: &Studio) -> f64 {
        st.board.written().vars.iter().find(|(n, _)| n == "score").map(|(_, v)| v.re).unwrap_or(f64::NAN)
    }

    /// ★ The whole path a browser takes: press play, put the pointer down on
    /// the right box, lift it. Through the same messages the page sends, so
    /// what is tested is what will actually happen.
    #[test]
    fn a_tap_through_the_wire_scores() {
        let mut st = a_game();
        apply(&mut st, Ask::Play { on: true });
        assert!(st.board.playing_game, "a drawing with rules should play as a game");

        apply(&mut st, Ask::Pointer { x: 0.0, y: -2.2, down: true });
        apply(&mut st, Ask::Pointer { x: 0.0, y: -2.2, down: false });
        assert_eq!(score(&st), 1.0, "the middle box is the right answer");

        apply(&mut st, Ask::Pointer { x: -3.4, y: -2.2, down: true });
        apply(&mut st, Ask::Pointer { x: -3.4, y: -2.2, down: false });
        assert_eq!(score(&st), 0.0, "and a wrong one takes it back");
    }

    fn ludo() -> Studio {
        let mut board = Board::new();
        board.load("../samples/ludogame.easel").expect("the game opens");
        Studio { board, file: String::new(), say: String::new() }
    }

    /// Where the die is lying. The board throws it across the whole square, so
    /// there is no fixed spot to tap any more.
    fn die_at(st: &Studio) -> Cx {
        let age = (st.board.clock - var(st, "flung")).max(0.0);
        plotkit::dice::thrown(var(st, "seed"), var(st, "rolls"), age, 6.4).at
    }

    fn var(st: &Studio, name: &str) -> f64 {
        st.board.written().vars.iter().find(|(n, _)| n == name).map(|(_, v)| v.re).unwrap_or(f64::NAN)
    }

    /// ★ The whole path a browser takes to roll the die: open the game, press
    /// play, put the pointer down on the die and lift it. Through the same
    /// messages the page sends, because the board being right says nothing
    /// about the page reaching it.
    #[test]
    fn the_die_rolls_through_the_wire() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        assert!(st.board.playing_game, "ludo has rules, so it plays as a game");

        let d = die_at(&st);
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: true });
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: false });
        assert_eq!(var(&st, "rolled"), 1.0, "the die was thrown");
        assert_eq!(var(&st, "rolls"), 1.0, "and counted");
    }

    /// ★ **A thrown die needs the clock.** Everything about the throw is a
    /// function of `time - flung`, so with no ticks `age` stays at nought, the
    /// die reads 1 for ever and `settled` never comes true — which looks
    /// exactly like a die that will not roll, and means no token can be moved
    /// either, since a move waits for it to stop.
    #[test]
    fn the_die_needs_the_clock_to_settle() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        let d = die_at(&st);
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: true });
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: false });
        assert_eq!(var(&st, "settled"), 0.0, "it has only just left the hand");

        // Past `plotkit::dice::OVER`, which is where the throw ends. Taken
        // from there rather than written down, so a change to the physics
        // moves this test with it instead of breaking it.
        let ticks = (plotkit::dice::OVER / 0.05).ceil() as usize + 2;
        for _ in 0..ticks {
            apply(&mut st, Ask::Tick { seconds: 0.05 });
        }
        assert_eq!(var(&st, "settled"), 1.0, "and now it has stopped");
        let face = var(&st, "die");
        assert!((1.0..=6.0).contains(&face), "on a real face: {face}");
    }

    /// ★ **A tap that drifts is still a tap.** A mouse click lands on one
    /// pixel; a pen or a finger never does. Every earlier test pressed and
    /// released on the very same point, which is the one gesture no human
    /// makes — and so all of them missed that in a browser a press was quietly
    /// starting an ink stroke, the drift was ruled a drag, and the die could
    /// not be rolled at all.
    #[test]
    fn a_tap_that_drifts_still_rolls_the_die() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        let d = die_at(&st);
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: true });
        // A few pixels of drift, which is what a pen does.
        apply(&mut st, Ask::Pointer { x: d.re + 0.09, y: d.im + 0.08, down: true });
        apply(&mut st, Ask::Pointer { x: d.re + 0.12, y: d.im + 0.1, down: false });
        assert_eq!(var(&st, "rolled"), 1.0, "the die was thrown");
    }

    /// And a drifting tap in play leaves no scribble behind, which is the
    /// other half of the same bug.
    #[test]
    fn a_drifting_tap_in_play_draws_nothing() {
        let mut st = ludo();
        let marks = st.board.sheet.len();
        apply(&mut st, Ask::Play { on: true });
        apply(&mut st, Ask::Pointer { x: 2.0, y: 2.0, down: true });
        for k in 1..12 {
            apply(&mut st, Ask::Pointer { x: 2.0 + 0.2 * k as f64, y: 2.0, down: true });
        }
        apply(&mut st, Ask::Pointer { x: 4.2, y: 2.0, down: false });
        assert_eq!(st.board.sheet.len(), marks, "nothing was drawn");
    }

    /// And the other edge of the same rule: a slide **from** one mark **to**
    /// another is a tap on neither. Otherwise a swipe across the board would
    /// move whichever token it happened to lift over.
    #[test]
    fn a_slide_between_marks_taps_neither() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        // Down on the die, up on a token in seat 0's yard.
        let yard = plotkit::ludo::waiting(0, 0);
        let d = die_at(&st);
        apply(&mut st, Ask::Pointer { x: d.re, y: d.im, down: true });
        apply(&mut st, Ask::Pointer { x: yard.re, y: yard.im, down: true });
        apply(&mut st, Ask::Pointer { x: yard.re, y: yard.im, down: false });
        assert_eq!(var(&st, "rolled"), 0.0, "the die was not thrown");
        assert!(var(&st, "at0") < 0.0, "and the token did not move");
    }

    /// ★ A server that opens whatever path it is handed will one day be
    /// asked for something it should not have. That this one is meant for one
    /// person on one machine is not a reason to leave the door open -- it is a
    /// reason nobody would notice it was open.
    #[test]
    fn it_will_not_open_just_any_path() {
        assert!(allowed("samples/ludo.easel"));
        assert!(allowed("drawing.easel"));
        assert!(allowed("scripts/playground.rec"));

        assert!(!allowed("../../../etc/passwd"), "no walking up");
        assert!(!allowed("samples/../../secret.easel"), "not even hidden in the middle");
        assert!(!allowed("/etc/passwd"), "nothing absolute");
        assert!(!allowed("C:/Windows/win.ini"), "nor on the other kind of machine");
        assert!(!allowed("Cargo.toml"), "and only drawings");
        assert!(!allowed(""));
    }

    /// ★ Opening a name that is not there is a **blank page**, not a
    /// mistake: that is how you ask for one.
    #[test]
    fn a_name_that_is_not_there_yet_is_a_blank_page() {
        let mut st = Studio { board: Board::new(), file: String::new(), say: String::new() };
        apply(&mut st, Ask::OpenFile { name: "nothing-here-yet.easel".into() });
        assert!(st.board.sheet.is_empty());
        assert_eq!(st.file, "nothing-here-yet.easel", "and saving will go there");
        assert!(st.say.contains("blank"));
    }

    /// And opening a real one replaces what was there.
    ///
    /// Written next to where the test runs rather than reached for with `..`,
    /// because `..` is exactly what the guard above refuses -- and a test that
    /// needs the guard turned off is testing something else.
    #[test]
    fn opening_a_drawing_replaces_the_one_before_it() {
        let mut first = Board::new();
        first.sheet.script.add("circle(0, 3)");
        first.save("web-test-open.easel").expect("wrote one");

        let mut st = Studio { board: Board::new(), file: String::new(), say: String::new() };
        st.board.sheet.script.add("ngon(0, 1, 5)");
        apply(&mut st, Ask::OpenFile { name: "web-test-open.easel".into() });

        assert_eq!(st.board.sheet.script.rows.len(), 1, "{}", st.say);
        assert_eq!(st.board.sheet.script.rows[0].text, "circle(0, 3)", "the one before it is gone");
        assert_eq!(st.file, "web-test-open.easel", "and saving goes to the new one");
        let _ = std::fs::remove_file("web-test-open.easel");
    }

    /// ★ The Ludo board opens and draws, which is what the front page links to.
    #[test]
    fn the_ludo_board_opens_and_draws() {
        let mut b = Board::new();
        if b.load("../samples/ludo.easel").is_err() {
            return; // not generated in this checkout
        }
        let made = b.written();
        assert!(made.errors.is_empty(), "{:?}", made.errors);
        assert!(made.shapes.len() > plotkit::ludo::TRACK, "the board and some tokens");

        // And the tokens walk when the clock runs.
        b.clock = 3.0;
        let later = b.written().shapes.len();
        assert!(later > 0);
        b.clock = 0.0;
        let at_rest = easel::wire::scene(&b, easel::Look::default());
        b.clock = 3.0;
        assert_ne!(easel::wire::scene(&b, easel::Look::default()), at_rest, "the tokens should have moved");
    }

    /// Colours arrive as `#RRGGBB`, which is what an `<input type=color>`
    /// gives — and anything else is refused rather than turned into black.
    #[test]
    fn a_colour_arrives_as_the_browser_writes_it() {
        assert_eq!(hex("#E0A44A"), Some(0xE0_A4_4A));
        assert_eq!(hex("E0A44A"), Some(0xE0_A4_4A));
        assert_eq!(hex("nonsense"), None);
    }
}
