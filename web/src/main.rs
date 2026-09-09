//! # web — the studio in a browser
//!
//! ```text
//!     cargo run -p web --release
//!     cargo run -p web --release -- samples/adding.easel
//!     cargo run -p web --release -- --open        and let the network in
//!     then open http://127.0.0.1:8088
//! ```
//!
//! ## Rooms
//!
//! One server holds many games. A room is a name in the address —
//! `?room=PEAR` — and the board, the seats and the post office all hang off
//! it. **A link is the invitation:** the first person to use a name makes the
//! room, so there is no create step, no list, and nothing to tidy if nobody
//! turns up.
//!
//! Somebody who asks for no room gets one anyway, so a person opening the
//! studio to draw is never made to think about rooms, and every link that
//! worked before this existed still works.
//!
//! See [`rooms`] for the alphabet — codes get read down a telephone, so it has
//! no letter that sounds like another — and for why a room ends by being
//! forgotten rather than by being left.
//!
//! ## Letting another machine in
//!
//! `--open` binds every interface instead of loopback. Off by default because
//! this server opens files by name, and a thing that reads files should not
//! appear on a network because somebody forgot that it could.
//!
//! **The game works over plain `http` on a network; the talking does not.** A
//! browser will not hand a page a microphone unless the page is a *secure
//! context* — `https`, or `localhost` — and on `http://192.0.2.10` the
//! microphone API is not blocked, it is **absent**, so the failure is a
//! `TypeError` about `undefined` rather than a refusal anybody could act on.
//!
//! The cheapest way round it needs no certificate at all: forward the port from
//! the other machine, so *its* browser is talking to `localhost`.
//!
//! ```text
//!     ssh -N -L 8088:localhost:8088 you@this-machine
//! ```
//!
//! ## Testing the talking with two tabs
//!
//! Two tabs on one machine is `localhost`, so the microphone works with no
//! certificate and nothing forwarded. Open `http://127.0.0.1:8088/studio`
//! twice and press the microphone in both. They get different names because
//! the peer id lives in `sessionStorage`, which is per-tab.
//!
//! Two things to know before deciding it is broken:
//!
//! - **Use headphones, or expect a howl.** Each tab's microphone hears the
//!   other tab's speaker. Echo cancellation is meant for a person in a room,
//!   not for two copies of the same page a centimetre apart.
//! - **Watch the meter rather than listening.** Sound arriving into a muted
//!   element is indistinguishable from no sound at all, so the level bars are
//!   the only honest answer to *is it connected*.
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

use std::fmt::Write as _;

mod rooms;
mod talk;

/// Every room on this server, and what it takes to make another.
///
/// One lock over all of them rather than one each: a lock per room would be
/// the right shape if rooms were busy, and they are not — a room is four
/// people, and the work behind the lock is a scene, which is milliseconds.
/// Splitting it would buy nothing and cost the ability to reason about it.
struct House {
    rooms: rooms::Rooms<Studio>,
    /// What a new room starts from.
    file: String,
    began: std::time::Instant,
}

impl House {
    fn now(&self) -> f64 {
        self.began.elapsed().as_secs_f64()
    }

    /// The room by that name, opening the starting file if it is new.
    fn room(&mut self, name: &str) -> &mut Studio {
        let now = self.now();
        let file = self.file.clone();
        self.rooms.get(name, now, || {
            let mut board = Board::new();
            let say = if std::path::Path::new(&file).exists() {
                match board.load(&file) {
                    Ok(0) => String::new(),
                    Ok(bad) => format!("opened {file} -- {bad} lines lost"),
                    Err(e) => format!("could not open {file}: {e}"),
                }
            } else {
                String::new()
            };
            Studio { board, file: file.clone(), say, room: talk::Room::new(), began: std::time::Instant::now(), last_bot: 0.0, ticked: std::time::Instant::now(), last_scene: None }
        })
    }
}

type Shared = Arc<Mutex<House>>;

/// The drawing, and the one thing about it the browser cannot hold.
struct Studio {
    /// Who is in the room and what post is waiting for them. Only signalling —
    /// no voice passes through this server. See [`talk`].
    room: talk::Room,
    /// When the server started, so a peer's "last heard from" is a number
    /// rather than a clock reading.
    began: std::time::Instant,
    /// The board clock when a bot last played, so they do not play instantly.
    last_bot: f64,
    /// The last scene built, and what it was built from.
    ///
    /// Four people in a room are watching one board. With the clock landing on
    /// a frame grid, their requests fall into the same instant — so the second,
    /// third and fourth get the answer the first one paid for.
    last_scene: Option<(u64, u64, u64, String)>,
    /// When the clock was last moved on.
    ///
    /// **The room owns its clock.** Every browser used to send its own tick to
    /// the same board, so two players ran the game at twice real time and four
    /// at four times -- and each tick cost a whole scene. The server advances
    /// it once by however long has really passed, however many people are
    /// watching.
    ticked: std::time::Instant,
    board: Board,
    file: String,
    say: String,
}

#[tokio::main]
async fn main() {
    // `--open` puts the server on every interface so another machine on the
    // same network can reach it. Off by default, and deliberately: this server
    // opens files by name, and a thing that reads files should not appear on a
    // network because somebody forgot it could.
    let open = std::env::args().any(|a| a == "--open");
    let file = std::env::args()
        .skip(1)
        .find(|a| !a.starts_with("--"))
        .unwrap_or_else(|| "drawing.easel".to_string());
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

        // The room somebody alone in the studio is in, opened with whatever file
    // was named on the command line. Every other room starts from the same
    // file, which is what makes a link an invitation to *this* game.
    let mut house = House { rooms: rooms::Rooms::new(), file: file.clone(), began: std::time::Instant::now() };
    let now = house.now();
    house.rooms.get(rooms::ALONE, now, || Studio {
        board,
        file,
        say,
        room: talk::Room::new(),
        began: std::time::Instant::now(),
        last_bot: 0.0,
        ticked: std::time::Instant::now(),
        last_scene: None,
    });
    let shared: Shared = Arc::new(Mutex::new(house));
    let app = Router::new()
        .route("/", get(home))
        .route("/studio", get(page))
        .route("/list", get(list_files))
        .route("/app.js", get(js))
        .route("/app.css", get(css))
        .route("/scene", get(scene))
        .route("/do", post(act))
        .route("/talk", post(chat))
        .route("/room", post(new_room))
        .with_state(shared);

    let at = SocketAddr::from((if open { [0, 0, 0, 0] } else { [127, 0, 0, 1] }, 8088));
    // A plain `expect` here says "could not take the port" and nothing else,
    // which is the least useful thing it could say: the reason is almost always
    // a server from an earlier run still holding it, and the symptom is a
    // browser quietly showing you a page built before your last change.
    let listener = match tokio::net::TcpListener::bind(at).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("could not take port 8088: {e}");
            eprintln!();
            eprintln!("  Almost certainly an older copy of this server is still running, and");
            eprintln!("  your browser has been talking to IT -- showing a page built before");
            eprintln!("  whatever you just changed. Stop it and try again:");
            eprintln!("    lsof -ti:8088 | xargs kill        (or: pkill -f target/.*/web)");
            std::process::exit(1);
        }
    };
    println!("http://127.0.0.1:8088");
    if open {
        println!("open to the network on port 8088 -- find this machine's address with `ip addr` or `ipconfig`");
        println!();
        // The one text, from the one place that is tested for it.
        if let Some(why) = talk::Room::advice(false, "0.0.0.0") {
            println!("  NOTE ON VOICE: {why}");
        }
        println!("  It is not blocked, it is ABSENT -- the page fails with a TypeError.");
        println!("  The game itself works over http perfectly well; only the talking does not.");
        println!();
        println!("  Three ways round it, cheapest first:");
        println!("   1. From the other machine, forward the port so its browser sees");
        println!("      localhost -- which IS a secure context, with no certificate:");
        println!("        ssh -N -L 8088:localhost:8088 you@this-machine");
        println!("      then open http://localhost:8088 there.");
        println!("   2. Tell that browser to trust this origin. In Chrome, at");
        println!("      chrome://flags/#unsafely-treat-insecure-origin-as-secure,");
        println!("      add http://<this machine>:8088 and restart it.");
        println!("   3. Put it behind https properly -- a reverse proxy or a tunnel.");
        println!();
        println!("  To try the talking WITHOUT another machine: open this page twice on");
        println!("  this one. Two tabs are two peers. Use headphones -- each tab's");
        println!("  microphone hears the other's speaker -- and watch the level bars");
        println!("  rather than listening, since audio arriving into a muted element");
        println!("  sounds exactly like no audio at all.");
        println!();
    } else {
        println!("(only this machine -- pass --open to let others on the network in)");
    }
    axum::serve(listener, app).await.expect("the server stopped");
}

/// A number that changes whenever the page's own code does.
///
/// **Because "hard-reload it" is not a fix.** Three times running, a change was
/// deployed, tested, and reported as not working -- and the browser was serving
/// a copy of `app.js` from before the change. That is not the user's mistake: a
/// page that ships new behaviour under an old URL is asking to be cached.
///
/// So the assets are fetched as `/app.js?v=<this>`, a different URL the moment
/// their contents differ, which the browser fetches because it has never seen
/// it. The HTML is never cached, since it carries the number.
///
/// FNV over the two files, worked out once. Same trick and same reasoning about
/// non-cryptographic hashes as `easel::wire::still_mark`.
fn assets_version() -> u64 {
    use std::sync::OnceLock;
    static V: OnceLock<u64> = OnceLock::new();
    *V.get_or_init(|| {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for part in [include_str!("../static/app.js"), include_str!("../static/app.css")] {
            for b in part.as_bytes() {
                h ^= *b as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    })
}

/// Stamp the asset version into a page, and refuse to let the page be cached.
fn served(html: &'static str) -> impl IntoResponse {
    let v = assets_version();
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        html.replace("/app.js", &format!("/app.js?v={v}"))
            .replace("/app.css", &format!("/app.css?v={v}")),
    )
}

async fn home() -> impl IntoResponse {
    served(include_str!("../static/home.html"))
}

async fn page() -> impl IntoResponse {
    served(include_str!("../static/index.html"))
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
    (
        [
            (header::CONTENT_TYPE, "text/javascript"),
            // Cached hard, because the URL carries the version: a changed file
            // is a changed URL and is fetched.
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        include_str!("../static/app.js"),
    )
}

async fn css() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/css"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
        ],
        include_str!("../static/app.css"),
    )
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
    /// Who is asking. Empty when nobody has taken a seat, which is every
    /// drawing that is not a game with seats in it.
    #[serde(default)]
    me: String,
    /// Which game. Absent means the one somebody alone in the studio is in.
    #[serde(default)]
    room: String,
}

impl From<&Where> for Look {
    fn from(w: &Where) -> Look {
        Look::new(Cx::new(w.lox, w.loy), Cx::new(w.hix, w.hiy), w.px)
    }
}

async fn scene(State(s): State<Shared>, Query(w): Query<Where>) -> impl IntoResponse {
    let mut house = s.lock().expect("the drawing");
    let studio = house.room(&w.room);
    advance(studio);
    (
        [(header::CONTENT_TYPE, "application/json")],
        scene_for(studio, (&w).into(), "", w.have),
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
    let mut house = s.lock().expect("the drawing");
    let studio = house.room(&w.room);
    advance(studio);
    match refuse(studio, &w.me, &ask) {
        Some(why) => studio.say = why,
        None => apply(studio, ask),
    }
    let word = std::mem::take(&mut studio.say);
    ([(header::CONTENT_TYPE, "application/json")], scene_for(studio, (&w).into(), &word, w.have))
}

/// Make a room nobody is using, and say what it is called.
///
/// The **server** names it, because the server is the one that knows which
/// names are taken. A page that made up its own would sooner or later drop two
/// sets of friends into one game.
async fn new_room(State(s): State<Shared>, body: Option<Json<Chat>>) -> impl IntoResponse {
    let mut house = s.lock().expect("the drawing");
    let now = house.now();
    let name = house.rooms.make(now);
    // Made here and now, so the name is genuinely taken by the time the page
    // is told it -- otherwise two people pressing together could be handed the
    // same one.
    let studio = house.room(&name);
    // **Whoever pressed the button holds the controls**, said here rather than
    // left to whichever browser calls in first. The page has not even loaded
    // yet, and without this the host is decided by a race between the person
    // who made the room and the first guest to open the link.
    if let Some(Json(chat)) = body {
        if !chat.me.is_empty() {
            studio.room.call(&chat.me, now);
        }
    }
    (
        [(header::CONTENT_TYPE, "application/json")],
        format!("{{\"room\":{}}}", serde_json::to_string(&name).unwrap_or_default()),
    )
}

/// How many seats this drawing has, or none if it is not that sort of drawing.
fn seats_for_bots(studio: &Studio) -> usize {
    studio.board.sheet.script.seats(studio.board.clock).map_or(0, |(_, n)| n)
}

/// Move the clock on by however long has really passed, and let the bots play.
///
/// Called once per request rather than once per client, which is the whole
/// point: the clock belongs to the room. A board being watched by four people
/// runs at the same speed as one being watched by nobody.
///
/// Clamped, so a room nobody has looked at for an hour does not leap an hour
/// forward the moment somebody opens it -- and a paused board does not move at
/// all, which is what pausing means.
///
/// The clamp has to be generous, though, and 0.25 was not: whatever it discards
/// is time the clock never gets back, so a gap between requests left the board
/// running behind real time and the die settling late. Two seconds covers any
/// plausible gap between frames and still refuses to replay an idle afternoon.
fn advance(studio: &mut Studio) {
    let gone = studio.ticked.elapsed().as_secs_f64();
    studio.ticked = std::time::Instant::now();
    advance_by(studio, gone);
}

/// The same, by a said amount — so a test can move a clock without waiting.
///
/// Split out because the whole point of the other one is that it reads the
/// real clock, and a test that had to sleep for a second to watch a die settle
/// would be a test nobody runs.
fn advance_by(studio: &mut Studio, seconds: f64) {
    if !studio.board.playing {
        return;
    }
    studio.board.tick(seconds.clamp(0.0, 2.0));
    // **On to a frame grid.** Not for the animation -- a thirtieth of a second
    // is finer than anybody sees -- but so that four people watching one board
    // ask about the same instant. Land the clock anywhere and every request is
    // a different drawing; land it on a grid and the second, third and fourth
    // get the answer the first one paid for.
    studio.board.clock = (studio.board.clock * FRAMES).round() / FRAMES;
    bot_turn(studio);
}

/// Frames a second the clock lands on.
const FRAMES: f64 = 30.0;

/// The scene, built once per room per frame however many people ask.
fn scene_for(studio: &mut Studio, look: easel::Look, word: &str, have: u64) -> String {
    let mark = easel::wire::still_mark(&studio.board, look);
    // What the answer depends on: the instant, the drawing, and the window it
    // is drawn for. Nothing else goes into it.
    let when = studio.board.clock.to_bits();
    let window = {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for v in [look.lo.re, look.lo.im, look.hi.re, look.hi.im, look.px as f64, have as f64] {
            for b in v.to_bits().to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x100_0000_01b3);
            }
        }
        h
    };
    // A word for the page is said once to one person, so a scene carrying one
    // is never shared.
    if word.is_empty() {
        if let Some((w, m, k, body)) = &studio.last_scene {
            if *w == when && *m == mark && *k == window {
                return body.clone();
            }
        }
    }
    let body = easel::wire::since(&studio.board, look, word, have);
    if word.is_empty() {
        studio.last_scene = Some((when, mark, window, body.clone()));
    }
    body
}

/// How long a bot waits between moves, in game seconds.
///
/// Not for the bot's sake. A player who taps the die and sees three other
/// turns happen in the same frame has not watched a game, they have been shown
/// a result -- and the die takes about two and a half seconds to settle, so
/// anything faster than that is playing before the number is known.
pub const BOT_PAUSE: f64 = 1.1;

/// Let a bot take one action, if it is a bot's turn and it has waited.
///
/// ## Deliberately witless
///
/// It knows nothing about Ludo, or about what any tap does. It tries each
/// tappable figure in turn and keeps the first one that **changed something**
/// -- and since the game's own rules already refuse every illegal move, the
/// first tap that changes anything is by construction a legal one.
///
/// So the die gets thrown because tapping it changes `rolled`; nothing happens
/// while the die is still rolling because every move is refused until it
/// settles; and a token moves because moving it changes `at`. None of which is
/// written here.
///
/// It plays *a* legal move rather than a good one -- the first that works,
/// which on this board means the lowest-numbered token that can go. That is
/// the whole brief for now. A better one would score the moves it found
/// instead of taking the first, and it would need to know what the numbers
/// mean, which is where the game would have to start telling it.
fn bot_turn(studio: &mut Studio) {
    if !studio.room.begun() {
        return;
    }
    let Some((_, how_many)) = studio.board.sheet.script.seats(studio.board.clock) else { return };
    let Some(turn) = studio.board.whose_turn() else { return };
    if !studio.room.empty_seats(how_many).contains(&turn) {
        return;
    }
    if studio.board.clock - studio.last_bot < BOT_PAUSE {
        return;
    }
    studio.last_bot = studio.board.clock;

    let mut groups: Vec<u32> = studio.board.sheet.marks.iter().map(|m| m.group).collect();
    groups.sort_unstable();
    groups.dedup();
    groups.retain(|g| *g != 0);

    let was_here = board_now(&studio.board);
    let was_turn = studio.board.whose_turn();
    for group in groups {
        let before = studio.board.tally.values.clone();
        studio.board.play_tap(group);
        if board_now(&studio.board) != was_here || studio.board.whose_turn() != was_turn {
            return;
        }
        // Nothing moved, so put back what the tap wrote. A refused move still
        // sets its own working-out down -- `ok`, `was`, `cut` -- and leaving
        // that behind would make the NEXT thing the bot tried look as though
        // it had worked.
        studio.board.tally.values = before;
    }
}

/// Where everything on the board is, as numbers.
///
/// **The question a bot has to ask is "did anything move", not "did anything
/// change".** A refused move still writes its own working-out to the tally, so
/// comparing the tally says yes to every tap -- which it did, and the bot
/// happily "played" by tapping the first token and moving nothing.
///
/// This is every mark's position, which is cheap: a mark is placed by two
/// expressions and evaluating them is arithmetic, not drawing. It says nothing
/// about what any of them mean, which is the point.
fn board_now(board: &Board) -> Vec<(i64, i64)> {
    let env = board.sheet.script.env(board.clock, &board.tally);
    board
        .sheet
        .marks
        .iter()
        .map(|m| {
            let at = m.pose_in(board.clock, &env).apply(plotkit::Cx::ZERO);
            // To the hundredth, so a hair of floating-point drift is not a move.
            ((at.re * 100.0).round() as i64, (at.im * 100.0).round() as i64)
        })
        .collect()
}

/// Why this person may not do this, if they may not.
///
/// **The server never learns what a tap would have done.** It asks the drawing
/// whose turn it is — the drawing already works that out, and a second copy
/// here would be a second thing to keep in step — and compares that with the
/// seat this person is sitting in. Everything else goes through untouched.
///
/// A drawing with no `seats(…)` row has no seats, so nothing is ever refused:
/// that is a sketch, or a game round one screen, and both are right.
///
/// Only *taps* are refused. Panning, zooming and looking are nobody's turn.
fn refuse(studio: &Studio, me: &str, ask: &Ask) -> Option<String> {
    // A lift is a tap; a press is not, and neither is anything else.
    if !matches!(ask, Ask::Pointer { down: false, .. }) {
        return None;
    }
    let (_, how_many) = studio.board.sheet.script.seats(studio.board.clock)?;
    let seated = studio.room.seated();
    // Until somebody has actually sat down, everybody plays -- otherwise a game
    // opened by one person to look at it cannot be touched at all.
    if seated.is_empty() {
        return None;
    }
    let turn = studio.board.whose_turn()?;
    if studio.room.may(me, auth::Deed::Play(turn), Some(turn)) {
        return None;
    }
    match studio.room.seat_of(me) {
        Some(mine) => Some(format!("seat {} to play, and you are seat {}", turn + 1, mine + 1)),
        None => Some(format!(
            "take one of the {how_many} seats before playing -- seat {} is to move",
            turn + 1
        )),
    }
}

/// What a page sends to the post office, and gets back.
///
/// One call does both halves: it says the peer is still here, hands over
/// anything it wants delivered, and collects whatever is waiting. A peer that
/// is collecting is by that fact still here, so there is nothing a client can
/// forget to do.
#[derive(serde::Deserialize)]
struct Chat {
    /// Who is calling.
    me: String,
    /// Notes to leave for other people, if any.
    #[serde(default)]
    post: Vec<Outbound>,
    /// A seat to take, if this call is claiming one. `-1` gives one up.
    #[serde(default)]
    sit: Option<i64>,
    #[serde(default)]
    room: String,
    /// What to be called. Absent leaves it as it was.
    #[serde(default)]
    name: Option<String>,
    /// Begin the game, for everybody in the room.
    #[serde(default)]
    start: bool,
    /// Give the controls to somebody else. Only the holder may.
    #[serde(default)]
    give: Option<String>,
    /// The page is closing. Sent by a beacon on the way out, so a real
    /// departure is prompt where merely going quiet is not.
    #[serde(default)]
    gone: bool,
}

#[derive(serde::Deserialize)]
struct Outbound {
    to: String,
    kind: String,
    body: String,
}

/// **Signalling only.** No voice passes through this server -- see [`talk`].
async fn chat(State(s): State<Shared>, Json(chat): Json<Chat>) -> impl IntoResponse {
    let mut house = s.lock().expect("the drawing");
    let studio = house.room(&chat.room);
    let now = studio.began.elapsed().as_secs_f64();
    for out in chat.post {
        let note = talk::Note { from: chat.me.clone(), kind: out.kind, body: out.body };
        studio.room.send(&out.to, note);
    }
    // Sitting down rides on the same call as everything else, because a peer
    // that is claiming a seat is by that fact still here.
    match chat.sit {
        Some(n) if n < 0 => studio.room.stand(&chat.me),
        Some(n) => {
            let how_many =
                studio.board.sheet.script.seats(studio.board.clock).map_or(0, |(_, n)| n);
            studio.room.sit(&chat.me, n as usize, how_many);
        }
        None => {}
    }
    if let Some(name) = &chat.name {
        studio.room.call_them(&chat.me, name);
    }
    if chat.gone {
        studio.room.depart(&chat.me, now);
    }
    if let Some(to) = &chat.give {
        // Refused rather than ignored if it is not theirs to give -- see
        // `auth::Table::hand_over`.
        studio.room.hand_over(&chat.me, to);
    }
    if chat.start && studio.room.may(&chat.me, auth::Deed::Start, None) {
        let how_many = seats_for_bots(studio);
        studio.room.begin(how_many);
    }
    let mine = studio.room.call(&chat.me, now);
    let here = studio.room.here();
    let seated = studio.room.seated();
    // Worked out before the loop, since `seated` borrows the room.
    let name_by: std::collections::HashMap<usize, String> =
        seated.iter().map(|(seat, who)| (*seat, studio.room.name_of(who))).collect();
    let my_name = studio.room.name_of(&chat.me);
    let begun = studio.room.begun();
    let host = studio.room.host().is_some_and(|h| h == chat.me);
    let bots = studio.room.empty_seats(seats_for_bots(studio));
    let my_seat = studio.room.seat_of(&chat.me);
    let seats = seats_for_bots(studio);
    let turn = studio.board.whose_turn();

    // Who this peer should ring. Worked out HERE, with the tested rule, rather
    // than in the page: both sides deciding independently is how a rule ends up
    // in two places and disagrees with itself.
    let ring: Vec<&String> =
        here.iter().filter(|w| **w != chat.me && talk::Room::calls(&chat.me, w)).collect();

    let mut body = String::from("{\"here\":[");
    for (k, who) in here.iter().enumerate() {
        if k > 0 {
            body.push(',');
        }
        body.push_str(&serde_json::to_string(who).unwrap_or_else(|_| "\"\"".into()));
    }
    body.push_str("],\"ring\":[");
    for (k, who) in ring.iter().enumerate() {
        if k > 0 {
            body.push(',');
        }
        body.push_str(&serde_json::to_string(who).unwrap_or_else(|_| "\"\"".into()));
    }
    // Everybody in the room, seated or not. `seats` only ever carried the
    // people who had chosen a colour, so somebody who had opened the link and
    // was deciding did not appear anywhere -- and "who is here" is exactly the
    // question a room full of people waiting to start is asking.
    body.push_str("],\"who\":[");
    for (k, id) in here.iter().enumerate() {
        if k > 0 {
            body.push(',');
        }
        let _ = write!(
            body,
            "{{\"id\":{},\"name\":{},\"seat\":{}}}",
            serde_json::to_string(id).unwrap_or_default(),
            serde_json::to_string(&studio.room.name_of(id)).unwrap_or_default(),
            studio.room.seat_of(id).map_or("null".into(), |n| n.to_string())
        );
    }
    body.push_str("],\"seats\":[");
    for (k, (seat, who)) in seated.iter().enumerate() {
        if k > 0 {
            body.push(',');
        }
        let _ = write!(
            body,
            "{{\"seat\":{seat},\"who\":{},\"name\":{}}}",
            serde_json::to_string(who).unwrap_or_default(),
            serde_json::to_string(name_by.get(seat).map_or("", |s| s.as_str())).unwrap_or_default()
        );
    }
    let _ = write!(
        body,
        "],\"howmany\":{seats},\"begun\":{begun},\"host\":{host},\"hostid\":{},\"bots\":{},\"myname\":{},\"mine\":{},\"turn\":{}",
        serde_json::to_string(&studio.room.host()).unwrap_or_else(|_| "null".into()),
        serde_json::to_string(&bots).unwrap_or_else(|_| "[]".into()),
        serde_json::to_string(&my_name).unwrap_or_default(),
        my_seat.map_or("null".into(), |n| n.to_string()),
        turn.map_or("null".into(), |n| n.to_string())
    );
    body.push_str(",\"post\":[");
    for (k, n) in mine.iter().enumerate() {
        if k > 0 {
            body.push(',');
        }
        body.push_str(&format!(
            "{{\"from\":{},\"kind\":{},\"body\":{}}}",
            serde_json::to_string(&n.from).unwrap_or_default(),
            serde_json::to_string(&n.kind).unwrap_or_default(),
            serde_json::to_string(&n.body).unwrap_or_default()
        ));
    }
    body.push_str("]}");
    ([(header::CONTENT_TYPE, "application/json")], body)
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
        // Kept so an older page does not break, and deliberately doing
        // nothing: the clock is the room's, moved on by `advance`. A browser
        // that could push the clock along is a browser that could push it four
        // times as fast by being four browsers.
        Ask::Tick { seconds: _ } => {}
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
        Studio { board, file: String::new(), say: String::new(), room: talk::Room::new(), began: std::time::Instant::now(), last_bot: 0.0, ticked: std::time::Instant::now(), last_scene: None }
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
        Studio { board, file: String::new(), say: String::new(), room: talk::Room::new(), began: std::time::Instant::now(), last_bot: 0.0, ticked: std::time::Instant::now(), last_scene: None }
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
            advance_by(&mut st, 0.05);
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

    /// ★ **The post office, over the wire.** Four peers, six links, and every
    /// pair with exactly one caller — worked out on this side so the rule is
    /// not written twice.
    #[test]
    fn the_room_tells_each_peer_who_to_ring() {
        let mut room = talk::Room::new();
        for who in ["ann", "bob", "cat", "dan"] {
            room.call(who, 0.0);
        }
        let here = room.here();
        let mut rings = 0;
        for me in &here {
            for other in &here {
                if me != other && talk::Room::calls(me, other) {
                    rings += 1;
                }
            }
        }
        assert_eq!(rings, 6, "four people, six connections, nobody ringing twice");
    }

    /// ★ **A tap from the wrong seat is refused**, and the server never learns
    /// what the tap would have done — it asks the drawing whose turn it is and
    /// compares.
    #[test]
    fn a_tap_from_the_wrong_seat_is_refused() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.call("bob", 0.0);
        st.room.sit("ann", 0, 4);
        st.room.sit("bob", 1, 4);
        assert_eq!(st.board.whose_turn(), Some(0), "seat 0 begins");

        let lift = Ask::Pointer { x: 0.0, y: 0.0, down: false };
        assert!(refuse(&st, "ann", &lift).is_none(), "seat 0 may play");
        assert!(refuse(&st, "bob", &lift).is_some(), "seat 1 may not");
        assert!(refuse(&st, "nobody", &lift).is_some(), "and a bystander may not");
    }

    /// Only taps. Looking, panning and pressing are nobody's turn.
    #[test]
    fn only_a_tap_is_refused() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("bob", 0.0);
        st.room.sit("bob", 1, 4);
        assert!(refuse(&st, "bob", &Ask::Pointer { x: 0.0, y: 0.0, down: true }).is_none());
        assert!(refuse(&st, "bob", &Ask::Tick { seconds: 0.1 }).is_none());
        assert!(refuse(&st, "bob", &Ask::Play { on: true }).is_none());
    }

    /// ★ Until somebody sits down, everybody plays. A game opened by one
    /// person to look at must still be touchable, and a drawing with no seats
    /// at all is a sketch.
    #[test]
    fn nothing_is_refused_before_anybody_sits() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        let lift = Ask::Pointer { x: 0.0, y: 0.0, down: false };
        assert!(refuse(&st, "ann", &lift).is_none(), "nobody has sat down yet");

        let mut plain = a_game();
        apply(&mut plain, Ask::Play { on: true });
        plain.room.call("ann", 0.0);
        assert!(refuse(&plain, "zed", &lift).is_none(), "the adding game has no seats");
    }

    /// ★ And the refusal follows the game: when the turn passes, so does who
    /// may move. Read from the drawing, not kept beside it.
    #[test]
    fn the_refusal_follows_whose_turn_it_is() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.sit("ann", 0, 4);
        st.room.call("bob", 0.0);
        st.room.sit("bob", 1, 4);
        let lift = Ask::Pointer { x: 0.0, y: 0.0, down: false };
        assert!(refuse(&st, "ann", &lift).is_none());

        st.board.tally.values.insert("turn".into(), 1.0);
        assert_eq!(st.board.whose_turn(), Some(1));
        assert!(refuse(&st, "ann", &lift).is_some(), "not any more");
        assert!(refuse(&st, "bob", &lift).is_none(), "bob's turn now");
    }

    /// ★ **A bot plays the seats nobody took**, and knows nothing about Ludo
    /// to do it: it taps each figure until one changes something, and the
    /// game's own rules refuse everything illegal, so the first tap that
    /// changes anything is by construction a legal move.
    #[test]
    fn a_bot_plays_an_empty_seat() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.sit("ann", 1, 4); // ann is green; seats 0, 2 and 3 are bots
        st.room.begin(4);
        assert_eq!(st.board.whose_turn(), Some(0), "a bot's turn");

        let before = st.board.tally.values.clone();
        for _ in 0..60 {
            advance_by(&mut st, 0.1);
        }
        assert_ne!(st.board.tally.values, before, "the bot did something");
        let rolled = st.board.written().vars.iter().find(|(n, _)| n == "rolls").map(|(_, v)| v.re);
        assert!(rolled.unwrap_or(0.0) >= 1.0, "and what it did was throw the die");
    }

    /// ★ It does not play a seat somebody is sitting in, however long it waits.
    #[test]
    fn a_bot_leaves_a_taken_seat_alone() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.sit("ann", 0, 4); // ann has the seat that plays first
        st.room.begin(4);
        let before = st.board.tally.values.clone();
        for _ in 0..60 {
            advance_by(&mut st, 0.1);
        }
        assert_eq!(st.board.tally.values, before, "it waited for her");
    }

    /// And nothing moves until somebody starts the game.
    #[test]
    fn no_bot_plays_before_the_game_begins() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.sit("ann", 1, 4);
        let before = st.board.tally.values.clone();
        for _ in 0..40 {
            advance_by(&mut st, 0.1);
        }
        assert_eq!(st.board.tally.values, before, "nobody has pressed start");
    }

    /// ★ A bot pauses between moves. A player who taps the die and sees three
    /// turns happen in the same frame has been shown a result rather than
    /// having watched a game.
    #[test]
    fn a_bot_waits_between_moves() {
        let mut st = ludo();
        apply(&mut st, Ask::Play { on: true });
        st.room.call("ann", 0.0);
        st.room.sit("ann", 3, 4);
        st.room.begin(4);
        advance_by(&mut st, 0.1);
        let after_one = st.board.tally.values.clone();
        // Well within the pause: nothing more should happen.
        advance_by(&mut st, 0.1);
        advance_by(&mut st, 0.1);
        assert_eq!(st.board.tally.values, after_one, "it is still waiting");
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
        let mut st = Studio { board: Board::new(), file: String::new(), say: String::new(), room: talk::Room::new(), began: std::time::Instant::now(), last_bot: 0.0, ticked: std::time::Instant::now(), last_scene: None };
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

        let mut st = Studio { board: Board::new(), file: String::new(), say: String::new(), room: talk::Room::new(), began: std::time::Instant::now(), last_bot: 0.0, ticked: std::time::Instant::now(), last_scene: None };
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
