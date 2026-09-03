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
        .route("/", get(page))
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

async fn page() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
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
}

impl From<&Where> for Look {
    fn from(w: &Where) -> Look {
        Look::new(Cx::new(w.lox, w.loy), Cx::new(w.hix, w.hiy), w.px)
    }
}

async fn scene(State(s): State<Shared>, Query(w): Query<Where>) -> impl IntoResponse {
    let studio = s.lock().expect("the drawing");
    ([(header::CONTENT_TYPE, "application/json")], easel::wire::scene(&studio.board, (&w).into()))
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
    Tick { seconds: f64 },
}

async fn act(State(s): State<Shared>, Query(w): Query<Where>, Json(ask): Json<Ask>) -> impl IntoResponse {
    let mut studio = s.lock().expect("the drawing");
    apply(&mut studio, ask);
    let word = std::mem::take(&mut studio.say);
    ([(header::CONTENT_TYPE, "application/json")], easel::wire::with_word(&studio.board, (&w).into(), &word))
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

    /// Colours arrive as `#RRGGBB`, which is what an `<input type=color>`
    /// gives — and anything else is refused rather than turned into black.
    #[test]
    fn a_colour_arrives_as_the_browser_writes_it() {
        assert_eq!(hex("#E0A44A"), Some(0xE0_A4_4A));
        assert_eq!(hex("E0A44A"), Some(0xE0_A4_4A));
        assert_eq!(hex("nonsense"), None);
    }
}
