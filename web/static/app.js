// The studio's front end.
//
// It holds two things and no more: **where you are looking**, and what the
// server last said the drawing was. Everything else — what a shape is, what a
// pose means, what a rule does — stays in Rust, where it is tested.
//
// The rule that keeps that honest: this file never decides anything about the
// drawing. It sends what you did and draws what came back.

const paper = document.getElementById('paper');
const ctx = paper.getContext('2d');

// Where we are looking: the middle of the view, and pixels per unit. Held here
// on purpose — panning and zooming never reach the server, so a drag of the
// paper runs at whatever rate the hand moves.
let view = { x: 0, y: 0, scale: 70 };
let scene = { pieces: [], rings: [], tree: [], clock: 0, playing: false };
// The half of the drawing that never changes -- a Ludo board is a hundred
// squares that never move. It is asked for once and kept, and `held` is the
// number naming the copy we have. The answer leaves it out when it matches,
// which is most frames.
let still = [];
let held = 0;
// The row list, kept between frames. It arrives with the still half and only
// when the drawing has changed -- it is the whole script as text, and sending
// it every frame was ninety per cent of a scene.
let tree = [];
let waiting = false;

// ---- the view ------------------------------------------------------------

function size() {
  const r = paper.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  paper.width = Math.round(r.width * dpr);
  paper.height = Math.round(r.height * dpr);
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  return r;
}

/// World to screen, and back. The y axis flips, because mathematics counts up
/// and screens count down.
function toScreen(z) {
  const r = paper.getBoundingClientRect();
  return [r.width / 2 + (z[0] - view.x) * view.scale, r.height / 2 - (z[1] - view.y) * view.scale];
}
function toWorld(px, py) {
  const r = paper.getBoundingClientRect();
  return [view.x + (px - r.width / 2) / view.scale, view.y - (py - r.height / 2) / view.scale];
}

/// What we are looking at, for the server — a curve sampled across the window
/// genuinely cannot be drawn without it.
function look() {
  const r = paper.getBoundingClientRect();
  const [lox, hiy] = toWorld(0, 0);
  const [hix, loy] = toWorld(r.width, r.height);
  return `lox=${lox}&loy=${loy}&hix=${hix}&hiy=${hiy}&px=${Math.round(r.width)}&have=${held}&me=${me}&room=${encodeURIComponent(room)}`;
}

// ---- talking to the drawing ---------------------------------------------

// A command must never be lost. Pressing play is not a redraw: if it is
// dropped the clock never starts, and everything downstream of the clock --
// a thrown die settling, a walk cycle, a note dying away -- silently does
// nothing at all. That is a hard thing to see, because the tap that was NOT
// dropped still works, so the game looks alive and merely stuck.
//
// So commands queue behind one another, and only the clock is allowed to skip
// -- with the skipped time carried, below, so it is delayed and not lost.
let chain = Promise.resolve();
// How many commands are waiting. The clock stands aside while any of them are,
// because a tap that has to queue behind a tick waits for a whole round trip
// before it is even sent -- and at sixty ticks a second there is nearly always
// one in the way. That is the entire reason a click felt slow: not the work,
// the queueing behind an animation frame.
let pending = 0;
function ask(body) {
  pending += 1;
  chain = chain
    .then(() => send(body))
    .catch(() => {})
    .finally(() => {
      pending -= 1;
    });
  return chain;
}

// A move of the pen while it is down is like a tick: only the latest one
// matters, and queueing them means the ink arrives seconds after the hand has
// stopped. So these are dropped when the line is busy -- the NEXT move carries
// the position, and the lift is sent through `ask` so it can never be lost.
function nudge(body) {
  if (waiting) return;
  ask(body);
}

async function send(body) {
  // One in flight at a time. Without this a fast hand queues a hundred
  // requests and the drawing arrives seconds after the pen has stopped.
  waiting = true;
  try {
    const r = await fetch(`/do?${look()}`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    scene = await r.json();
    keep();
    noises();
    show();
  } catch (e) {
    say(`the drawing is not answering: ${e}`);
  } finally {
    waiting = false;
  }
}

async function refresh() {
  if (waiting) return;
  waiting = true;
  try {
    scene = await (await fetch(`/scene?${look()}`)).json();
    keep();
    show();
  } catch (e) {
    /* it will be there next frame */
  } finally {
    waiting = false;
  }
}

function say(what) {
  document.getElementById('says').textContent = what || '';
}

// ---- drawing -------------------------------------------------------------

// Hold on to the still half when a new one arrives, and note which one it is.
// A drawing that says it is bounded is fitted to its box and left there. It
// has edges and is meant to be seen whole -- a wheel that scrolls a Ludo board
// away is a way to lose it, not a way to look at it. One that says nothing goes
// on for ever and keeps both the wheel and the drag.
function bounded() {
  return Array.isArray(scene.bounds);
}

function fit() {
  if (!bounded()) return;
  const [lox, loy, hix, hiy] = scene.bounds;
  const r = size();
  // A tenth of a turn of margin, so the edge of the board is not the edge of
  // the window.
  const pad = 1.06;
  view.x = (lox + hix) / 2;
  view.y = (loy + hiy) / 2;
  view.scale = Math.min(r.width / ((hix - lox) * pad), r.height / ((hiy - loy) * pad));
}

// The pieces that move, drawn once about their own middles. Sixteen tokens are
// one shape sixteen times; keeping the shape and moving it is the whole of the
// change.
let cast = [];

function keep() {
  if (scene.still) {
    still = scene.still;
    cast = scene.cast || [];
    if (scene.tree) tree = scene.tree;
    held = scene.stillv;
    fit();
  } else if (scene.stillv !== undefined && scene.stillv !== held) {
    // It changed and we were not sent one: ask again rather than draw a board
    // that is no longer the board.
    held = 0;
  }
}

// Whether the game has been started. The setup screen is shown until it has,
// and never again -- house rules are settled before the first throw, not
// half way through somebody's turn.
let started = false;
// Whether anybody has started it. From the server, so all four browsers agree.
let begun = false;
// Whether the start is mine to press. One person decides, or four people set
// the house rules underneath one another and whoever presses last wins an
// argument nobody knew they were having.
let amHost = false;
// Whether a seat has been chosen for or by this browser, so arriving takes one
// seat and not a new one every poll.
let satDown = false;

function paint() {
  const r = size();
  ctx.clearRect(0, 0, r.width, r.height);

  // Points arrive as whole numbers of hundredths of a world unit -- see
  // `wire::GRAIN`. Formatting thirty thousand floats as decimal text was
  // taking 84ms a scene, which is what made every tap feel slow.
  const G = 0.01;

  // Everything that does not move, then every piece that does -- each drawn
  // from the shape we already hold, put where the server says it is. What
  // arrives per frame is four numbers a piece rather than a fresh outline.
  const shapes = still.concat(scene.pieces || []);
  const where = new Map((scene.at || []).map((a) => [a.i, a]));
  for (const part of cast) {
    const a = where.get(part.i);
    if (!a) continue;
    ctx.beginPath();
    const p = part.p;
    for (const run of p) {
      if (run.length < 4) continue;
      // z -> a*z + at, in complex numbers: the multiplier carries the size and
      // the angle together, which is why there is no matrix here.
      const put = (k) => {
        const x = run[k] * G;
        const y = run[k + 1] * G;
        return toScreen([a.ar * x - a.ai * y + a.x * G, a.ar * y + a.ai * x + a.y * G]);
      };
      ctx.moveTo(...put(0));
      for (let k = 2; k < run.length; k += 2) ctx.lineTo(...put(k));
      if (part.fill) ctx.closePath();
    }
    if (part.fill) {
      ctx.fillStyle = part.c;
      ctx.fill('evenodd');
    } else {
      ctx.strokeStyle = part.c;
      ctx.lineWidth = part.w;
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      ctx.stroke();
    }
  }

  for (const piece of shapes) {
    const p = piece.p;
    if (p.length < 4) continue;
    ctx.beginPath();
    ctx.moveTo(...toScreen([p[0] * G, p[1] * G]));
    for (let k = 2; k < p.length; k += 2) ctx.lineTo(...toScreen([p[k] * G, p[k + 1] * G]));
    if (piece.fill) {
      ctx.closePath();
      ctx.fillStyle = piece.c;
      // Even-odd, the same rule the Rust rasteriser uses — so a stroke drawn
      // with a nib is a ring here too, and a letter O keeps its hole.
      ctx.fill('evenodd');
    } else {
      ctx.strokeStyle = piece.c;
      ctx.lineWidth = piece.w;
      ctx.lineJoin = 'round';
      ctx.lineCap = 'round';
      ctx.stroke();
    }
  }

  ctx.setLineDash([4, 4]);
  ctx.strokeStyle = '#6fcf97';
  ctx.lineWidth = 1;
  for (const ring of scene.rings) {
    if (ring.length < 4) continue;
    ctx.beginPath();
    ctx.moveTo(...toScreen([ring[0], ring[1]]));
    for (let k = 2; k < ring.length; k += 2) ctx.lineTo(...toScreen([ring[k], ring[k + 1]]));
    ctx.closePath();
    ctx.stroke();
  }
  ctx.setLineDash([]);
}

// ---- the list ------------------------------------------------------------

function show() {
  paint();
  // Nothing to draw with while watching, and the pointer says so.
  paper.style.cursor = scene.watching || scene.game ? 'pointer' : 'crosshair';
  document.getElementById('hint').hidden = bounded();
  const shapes = document.getElementById('shapes');
  const rows = document.getElementById('rows');
  // The focused input is rebuilt below, so where the caret was has to be put
  // back — otherwise typing a character sends you to the end of the row.
  const had = document.activeElement;
  const keep = had && had.dataset.row !== undefined
    ? { row: had.dataset.row, at: had.selectionStart, to: had.selectionEnd }
    : null;

  shapes.textContent = '';
  rows.textContent = '';

  for (const line of tree) {
    if (line.kind === 'title') continue;
    if (line.kind === 'group') shapes.append(groupLine(line));
    if (line.kind === 'mark') shapes.append(markLine(line));
    if (line.kind === 'row') rows.append(rowLine(line));
  }

  if (keep) {
    const back = rows.querySelector(`input[data-row="${keep.row}"]`);
    if (back) {
      back.focus();
      back.setSelectionRange(keep.at, keep.to);
    }
  }
  document.getElementById('play').classList.toggle('on', scene.playing);
  setup();
  // `begun` is the ROOM's, not this browser's. The second person to open the
  // link used to get their own "start the game" over a game already running --
  // and pressing it set the house rules again underneath everybody.
  document.getElementById('setup').hidden = started || begun || !(scene.rules || []).length;
}

function groupLine(g) {
  const el = document.createElement('div');
  el.className = 'line' + (g.chosen ? ' chosen' : '');
  el.innerHTML = `<span class="fold">${g.folded ? '&#9656;' : '&#9662;'}</span>`
    + `<span>figure ${g.id}</span><span class="count">${g.count}</span>`;
  el.querySelector('.fold').onclick = (e) => {
    e.stopPropagation();
    ask({ do: 'Fold', group: g.id });
  };
  el.onclick = () => ask({ do: 'Choose', group: g.id, mark: null });
  return el;
}

function markLine(m) {
  const el = document.createElement('div');
  el.className = 'line kid' + (m.chosen ? ' chosen' : '');
  el.innerHTML = `<span class="swatch" style="background:${m.colour}"></span>`
    + `<span>stroke ${m.id}</span>${m.moves ? '<span class="count">moves</span>' : ''}`;
  el.onclick = () => ask({ do: 'Choose', mark: m.id, group: null });
  return el;
}

function rowLine(r) {
  const el = document.createElement('div');
  el.className = 'row' + (r.on ? '' : ' off') + (r.wrong ? ' wrong' : '');

  const on = document.createElement('input');
  on.type = 'checkbox';
  on.checked = r.on;
  on.onchange = () => ask({ do: 'RowOn', id: r.id, on: on.checked });

  // A real text input. The caret, the arrow keys, selecting, pasting and
  // undoing inside the box are all the browser's, and none of them are sent.
  const text = document.createElement('input');
  text.type = 'text';
  text.value = r.text;
  text.dataset.row = r.id;
  text.spellcheck = false;
  text.oninput = () => ask({ do: 'Row', id: r.id, text: text.value });
  text.onkeydown = (e) => {
    if (e.key === 'Enter') ask({ do: 'AddRow' });
  };

  el.append(on, text);
  const box = document.createElement('div');
  box.append(el);

  if (r.wrong) {
    const why = document.createElement('div');
    why.className = 'why';
    why.textContent = r.wrong;
    box.append(why);
  }
  if (r.dial !== undefined) {
    const dial = document.createElement('div');
    dial.className = 'dial';
    const slide = document.createElement('input');
    slide.type = 'range';
    slide.min = -10;
    slide.max = 10;
    slide.step = 0.01;
    slide.value = r.value;
    const as = document.createElement('span');
    as.className = 'as';
    as.textContent = `${r.dial} = ${(+r.value).toFixed(2)}`;
    slide.oninput = () => {
      as.textContent = `${r.dial} = ${(+slide.value).toFixed(2)}`;
      ask({ do: 'Dial', id: r.id, value: +slide.value });
    };
    dial.append(as, slide);
    box.append(dial);
  }
  return box;
}

// ---- the pen -------------------------------------------------------------

let dragging = null;

paper.onpointerdown = (e) => {
  paper.setPointerCapture(e.pointerId);
  const r = paper.getBoundingClientRect();
  if ((e.shiftKey || e.button === 1) && !bounded()) {
    dragging = { pan: true, px: e.clientX - r.left, py: e.clientY - r.top };
    return;
  }
  dragging = { pan: false };
  const [x, y] = toWorld(e.clientX - r.left, e.clientY - r.top);
  ask({ do: 'Pointer', x, y, down: true });
};

paper.onpointermove = (e) => {
  if (!dragging) return;
  const r = paper.getBoundingClientRect();
  const px = e.clientX - r.left;
  const py = e.clientY - r.top;
  if (dragging.pan) {
    // Never a request. The paper moves under the hand at the rate of the hand.
    view.x -= (px - dragging.px) / view.scale;
    view.y += (py - dragging.py) / view.scale;
    dragging.px = px;
    dragging.py = py;
    paint();
    return;
  }
  const [x, y] = toWorld(px, py);
  nudge({ do: 'Pointer', x, y, down: true });
};

paper.onpointerup = (e) => {
  if (!dragging) return;
  const r = paper.getBoundingClientRect();
  const pan = dragging.pan;
  dragging = null;
  if (pan) {
    refresh();
    return;
  }
  const [x, y] = toWorld(e.clientX - r.left, e.clientY - r.top);
  // Always sent, even with one in flight: a release that is dropped leaves the
  // drawing thinking the pen is still down, and the next stroke joins the last.
  waiting = false;
  ask({ do: 'Pointer', x, y, down: false });
};

paper.onwheel = (e) => {
  if (bounded()) return;
  e.preventDefault();
  const r = paper.getBoundingClientRect();
  const before = toWorld(e.clientX - r.left, e.clientY - r.top);
  view.scale *= Math.exp(-e.deltaY * 0.0015);
  view.scale = Math.min(4000, Math.max(2, view.scale));
  const after = toWorld(e.clientX - r.left, e.clientY - r.top);
  // Keep the point under the pointer where it was: zoom about the pointer, not
  // about the middle, which is what makes it feel like a map.
  view.x += before[0] - after[0];
  view.y += before[1] - after[1];
  paint();
  refresh();
};

// ---- the tools -----------------------------------------------------------

function pressed(el, group) {
  for (const b of document.querySelectorAll(group)) b.classList.remove('on');
  el.classList.add('on');
}

for (const b of document.querySelectorAll('[data-nib]')) {
  b.onclick = () => {
    pressed(b, '[data-nib]');
    ask({ do: 'Nib', which: +b.dataset.nib });
  };
}
for (const b of document.querySelectorAll('[data-tool]')) {
  b.onclick = () => {
    pressed(b, '[data-tool]');
    ask({ do: 'Tool', name: b.dataset.tool });
  };
}
for (const b of document.querySelectorAll('[data-verb]')) {
  b.onclick = () => ask({ do: 'Verb', name: b.dataset.verb });
}
for (const b of document.querySelectorAll('[data-do]')) {
  b.onclick = () => {
    const body = { do: b.dataset.do };
    if (b.dataset.forwards !== undefined) body.forwards = b.dataset.forwards === 'true';
    ask(body);
  };
}
document.getElementById('ink').oninput = (e) => ask({ do: 'Paint', colour: e.target.value });
document.getElementById('add-row').onclick = () => ask({ do: 'AddRow' });
document.getElementById('add-shape').onclick = () => ask({ do: 'AddShape' });
document.getElementById('play').onclick = () => ask({ do: 'Play', on: !scene.playing });

// ---- noises --------------------------------------------------------------
//
// The drawing says what makes a noise -- `sound(roll, rolls)` -- and plays it
// when that number goes UP. The page keeps the last one it saw and knows
// nothing about what any of them mean.
//
// Every one of these is the same shape: a tone, and an envelope that decays as
// e^(-t/tau). The same decay that settles the die, and a branch after a gust,
// and a note after it is struck. Laplace, doing the only thing it ever does.
//
// A browser will not make a sound until somebody has clicked something, so the
// context is built on the first tap and not before.
let ear = null;
const heard = new Map();

function listen() {
  if (!ear) {
    const Ctx = window.AudioContext || window.webkitAudioContext;
    if (Ctx) ear = new Ctx();
  }
  if (ear && ear.state === 'suspended') ear.resume();
  return ear;
}

// One grain: a knock if it has no pitch, a note if it has.
//
// The numbers come from the server -- `sound::kit`, where they are measured and
// can be written to a wav and listened to. Nothing here decides what anything
// sounds like; this only plays what it is handed.
function grain(g, when) {
  const c = listen();
  if (!c) return;
  const t = when + g.at;
  const gain = c.createGain();
  gain.gain.setValueAtTime(g.gain, t);
  // setTargetAtTime IS e^(-t/tau) -- the same decay that settles the die, done
  // in the audio thread rather than approximated with line segments.
  gain.gain.setTargetAtTime(0.0001, t, Math.max(g.tau, 0.001));
  gain.connect(c.destination);
  const over = t + 4 * g.tau + 0.02;

  if (g.freq > 0) {
    const o = c.createOscillator();
    o.type = 'triangle';
    o.frequency.setValueAtTime(g.freq, t);
    o.connect(gain);
    o.start(t);
    o.stop(over);
    return;
  }

  // A knock: noise through a low-pass. `cut` is the whole of the difference
  // between a die on card and a spoon on a saucepan.
  const n = Math.max(1, Math.ceil(c.sampleRate * (4 * g.tau + 0.02)));
  const buf = c.createBuffer(1, n, c.sampleRate);
  const d = buf.getChannelData(0);
  // Worked out, not drawn from anywhere, so a replay sounds the same.
  for (let k = 0; k < n; k++) d[k] = ((Math.sin(k * 12.9898) * 43758.5453) % 2) - 1;
  const src = c.createBufferSource();
  src.buffer = buf;
  const f = c.createBiquadFilter();
  f.type = 'lowpass';
  f.frequency.setValueAtTime(Math.max(g.cut, 20), t);
  src.connect(f).connect(gain);
  src.start(t);
  src.stop(over);
}

function noises() {
  const c = ear;
  for (const s of scene.sounds || []) {
    const was = heard.get(s.name);
    heard.set(s.name, s.at);
    // Only ever on the way UP, and never on the first sighting -- otherwise
    // opening a game part way through plays every sound it has ever made.
    if (was === undefined || s.at <= was) continue;
    if (!c) continue;
    const now = c.currentTime;
    for (const g of s.grains || []) grain(g, now);
  }
}

// ---- talking to each other -----------------------------------------------
//
// Four people in a mesh: everybody connects to everybody, which is 4*3/2 = 6
// links. That is the right shape for four and the wrong shape for forty --
// each person sends their voice n-1 times, so a mesh grows as the square and a
// server-side mixer eventually wins. At four it does not.
//
// NO VOICE GOES THROUGH THE SERVER. It carries the half-dozen notes two
// browsers must swap to find each other -- see `web/src/talk.rs` -- and then
// gets out of the way.
//
// The one thing that will stop this working: a browser will not give a page a
// microphone unless the page is a SECURE CONTEXT. https, or localhost. On a
// plain http address over a network `navigator.mediaDevices` is not blocked,
// it is ABSENT, and the failure is a TypeError about undefined rather than
// anything a person could act on. Hence `canTalk()`.

// A name for this browser, for as long as the tab is open. Kept in
// sessionStorage so a reload is the same peer rather than a new one appearing
// beside the ghost of the old.
// Which game. From the address, so a link IS the invitation -- there is no
// create step and nothing to join.
const room = new URLSearchParams(location.search).get('room') || '';

// What to call yourself. Three other people have to tell whose turn it is, and
// eight characters of peer id helps nobody -- so an unnamed player is "player 1"
// after their seat, and this replaces it when they say otherwise.
let myName = sessionStorage.getItem('name') || '';

let me = sessionStorage.getItem('peer');
if (!me) {
  me = Math.random().toString(36).slice(2, 10);
  sessionStorage.setItem('peer', me);
}

let mine = null; // my microphone
const links = new Map(); // peer id -> RTCPeerConnection

// **Two things, not one.** `talking` used to mean both "I am in the audio
// mesh" and "my microphone is live", so the only way to HEAR anybody was to be
// broadcasting -- and turning yourself off disconnected you from everyone. That
// is why the sound came and went.
//
// `joined` is being in the mesh: connections exist and other people are
// audible. `muted` is whether my own track is enabled. Muting keeps every
// connection up, which is what every conferencing application does and what
// anybody pressing a microphone button expects.
let joined = false;
let muted = false;
// `joined` therefore means "my microphone is in", not "I am connected".

// Only the public STUN servers. STUN is cheap -- it answers one question,
// "what address did this packet come from" -- so running one costs nothing and
// several people give theirs away. TURN, which forwards actual audio, is the
// part nobody gives away, and without one two people behind strict routers
// cannot reach each other at all.
const ICE = { iceServers: [{ urls: ['stun:stun.l.google.com:19302', 'stun:stun1.l.google.com:19302'] }] };

function canTalk() {
  return !!(window.isSecureContext && navigator.mediaDevices && navigator.mediaDevices.getUserMedia);
}

function whyNot() {
  if (!window.isSecureContext) {
    return 'a browser will not give a page a microphone over plain http \u2014 open this on localhost, or put it behind https';
  }
  if (!navigator.mediaDevices) return 'this browser has no microphone support';
  return '';
}

// Somewhere to put the far end. An <audio> element per peer, off screen: the
// browser mixes them, and the operating system has done the echo cancellation
// before we ever see the samples.
// ---- hearing the others ---------------------------------------------------
//
// ## Why a phone plays it out of the earpiece
//
// A browser treats WebRTC audio as a CALL, and the default route for a call is
// the earpiece -- the little speaker you hold against your head. That is right
// for a telephone call and useless for a game with the handset lying on a
// table, which is what you get and why it sounded far away and could not be
// turned up.
//
// There is no single switch for this, and it is worth being plain about why:
//
//  - `setSinkId` chooses an output device and is the correct answer. Chrome on
//    Android has it. Safari does not, on any iPhone.
//  - On iOS the route is decided by the audio SESSION, which a page does not
//    control -- except that playing through Web Audio rather than an <audio>
//    element tends to be treated as media rather than as a call, and media
//    goes to the loudspeaker.
//
// So: `setSinkId` where it exists, Web Audio otherwise, and neither is
// promised. Which is why there is a button rather than a guess -- if it does
// not work, the operating system's own speaker control still does.
//
// Volume is separate and always works, because it is a gain node on our side
// of everything the platform decides.
let loud = false;
let volume = 1;
const ears = new Map();

function speaker(who) {
  let el = document.getElementById(`ear-${who}`);
  if (!el) {
    el = document.createElement('audio');
    el.id = `ear-${who}`;
    el.autoplay = true;
    // iOS refuses to play audio inline without this and offers a full-screen
    // player instead, which is not a thing anybody wants mid-game.
    el.setAttribute('playsinline', '');
    document.body.append(el);
  }
  return el;
}

// Route a peer's audio through Web Audio, so its level is ours to set and --
// on a handset -- so the platform reads it as media rather than as a call.
function route(who, stream) {
  const c = listen();
  if (!c) return;
  const old = ears.get(who);
  if (old) {
    try {
      old.src.disconnect();
    } catch (e) {
      /* already gone */
    }
  }
  const src = c.createMediaStreamSource(stream);
  const gain = c.createGain();
  gain.gain.value = volume * (level_of.get(who) ?? 1);
  src.connect(gain).connect(c.destination);
  ears.set(who, { src, gain });
}

// How loud each person is, over and above the master. One friend on a laptop
// microphone across a room and another on a headset are not the same loudness,
// and one master control cannot fix both.
const level_of = new Map();

function setPersonVolume(who, v) {
  level_of.set(who, v);
  const ear = ears.get(who);
  if (ear) ear.gain.gain.value = volume * v;
  const el = document.getElementById(`ear-${who}`);
  if (el) el.volume = Math.min(1, volume * v);
}

function setVolume(v) {
  volume = v;
  for (const [who, { gain }] of ears) gain.gain.value = v * (level_of.get(who) ?? 1);
  // The elements too, for whichever path is actually making the sound.
  for (const el of document.querySelectorAll('audio[id^="ear-"]')) {
    el.volume = Math.min(1, v);
  }
}

// Try to move the sound to the loudspeaker. Says whether it could.
async function useLoudspeaker(on) {
  loud = on;
  let moved = false;
  for (const el of document.querySelectorAll('audio[id^="ear-"]')) {
    // The correct way, where it exists.
    if (typeof el.setSinkId === 'function') {
      try {
        const outs = await navigator.mediaDevices.enumerateDevices();
        const out = outs.find(
          (d) => d.kind === 'audiooutput' && /speaker|loud/i.test(d.label)
        );
        await el.setSinkId(on && out ? out.deviceId : 'default');
        moved = true;
      } catch (e) {
        /* fall through to the other way */
      }
    }
    // The other way: silence the element and let Web Audio carry it, which a
    // handset is more likely to treat as media than as a call.
    el.muted = on;
  }
  for (const [who, ear] of ears) {
    ear.gain.gain.value = volume;
    void who;
  }
  return moved;
}

// ---- watching a voice rather than hearing it ------------------------------
//
// Two tabs on one machine is the easiest way to test this, and the hardest way
// to tell whether it works: each tab's microphone hears the other tab's
// speaker, so without headphones the two howl at each other, and with the
// speakers off there is nothing to hear at all.
//
// A level meter answers the question without listening. It also answers a
// question listening cannot: is the audio ARRIVING, as distinct from being
// audible -- a stream connected to a muted element looks and sounds identical
// to no stream at all.
//
// The measurement is root-mean-square, which is the loudness of a signal in the
// only sense that matters: the square root of the mean of the squares, which is
// the same average an engineer means by the "RMS" of an alternating current.
// Peak would flicker on every consonant; RMS is what an ear integrates.
const meters = new Map();

function watch(who, stream) {
  const c = listen();
  if (!c || !stream) return;
  const source = c.createMediaStreamSource(stream);
  const eye = c.createAnalyser();
  // 1024 samples is about 23 milliseconds at 44.1 kHz -- long enough to
  // average out a waveform, short enough to follow a syllable.
  eye.fftSize = 1024;
  source.connect(eye);
  meters.set(who, { eye, buf: new Float32Array(eye.fftSize) });
}

// How loud, 0 to 1, on a scale an ear would agree with.
function level(who) {
  const m = meters.get(who);
  if (!m) return 0;
  m.eye.getFloatTimeDomainData(m.buf);
  let sum = 0;
  for (const v of m.buf) sum += v * v;
  const rms = Math.sqrt(sum / m.buf.length);
  // Loudness is roughly logarithmic -- Weber and Fechner, 1860, and the reason
  // decibels exist at all. A linear bar spends its whole length on the loudest
  // tenth of what people actually say.
  const db = 20 * Math.log10(rms + 1e-9);
  return Math.max(0, Math.min(1, (db + 60) / 60));
}

// The strip of who is here, with a bar each. Redrawn from the animation frame
// rather than from the poll, because a meter that updates twice a second looks
// broken even when it is right.
function meterFrame() {
  const box = document.getElementById('voices');
  if (box && !box.hidden) {
    for (const el of box.children) {
      const bar = el.querySelector('.bar > i');
      if (bar) bar.style.width = `${Math.round(level(el.dataset.who) * 100)}%`;
    }
  }
  // The same measurement in the lobby, where there is still time to do
  // something about a microphone that is not working.
  const lobby = document.getElementById('lobby-list');
  const sheet = document.getElementById('setup');
  if (lobby && sheet && !sheet.hidden) {
    for (const el of lobby.children) {
      const bar = el.querySelector('.lvl > i');
      if (bar && el.dataset.who) bar.style.width = `${Math.round(level(el.dataset.who) * 100)}%`;
    }
  }
  requestAnimationFrame(meterFrame);
}
requestAnimationFrame(meterFrame);

// ---- taking a seat --------------------------------------------------------
//
// Four people at one board need to know who is who, and the server needs it
// too or anybody can move anybody's token. A seat is claimed on the same call
// that carries everything else, because a peer claiming a seat is by that fact
// still here.
//
// The rule the SERVER enforces is: a tap only counts from the seat whose turn
// it is. It never learns what the tap would have done -- it asks the drawing
// whose turn it is and compares. See `refuse` in web/src/main.rs.
let mySeat = null;
let seatWord = '';
const SEATNAMES = ['red', 'green', 'blue', 'yellow'];
const SEATINK = ['#E0704A', '#6FCF97', '#4FBCD4', '#E0A44A'];

// ---- the lobby ------------------------------------------------------------
//
// Everybody in the room, seated or not. `seats` only ever carried the people
// who had chosen a colour, so somebody who had opened the link and was still
// deciding appeared nowhere -- and "who is here" is the whole question a room
// waiting to start is asking.
//
// The voice bar is here for a reason beyond decoration: this is the one moment
// in a game when there is time to notice a microphone is not working and do
// something about it.
let lobbyWord = '';

// One line that is always true about the game, wherever you are looking.
//
// The old status came from three places -- the seat strip, the lobby note and
// whatever `say` had last been handed -- and they disagreed, which is why the
// controls seemed to move about. There is one sentence now and it is built
// from what the server said this instant.
function showState(answer) {
  const bar = document.getElementById('state');
  if (!bar) return;
  const who = answer.who || [];
  const holder = who.find((w) => w.id === answer.hostid);
  const holderName = holder ? (holder.id === me ? 'you' : holder.name) : 'nobody';

  if (!answer.howmany) {
    bar.hidden = true;
    return;
  }
  bar.hidden = false;

  if (!answer.begun) {
    bar.textContent = `waiting to start \u00b7 ${holderName} ${holder && holder.id === me ? 'have' : 'has'} the controls`;
    bar.className = 'waiting-state';
    return;
  }
  const turnSeat = (answer.seats || []).find((s) => s.seat === answer.turn);
  const mine = answer.mine === answer.turn && answer.mine !== null && answer.mine !== undefined;
  const bot = (answer.bots || []).includes(answer.turn);
  bar.className = mine ? 'my-turn' : '';
  bar.textContent = mine
    ? 'your turn'
    : bot
      ? `a bot is playing ${SEATNAMES[answer.turn] || 'seat ' + (answer.turn + 1)}`
      : `${turnSeat ? turnSeat.name : 'seat ' + (answer.turn + 1)} to play`;
}

async function handOver(to) {
  try {
    const answer = await (
      await fetch('/talk', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ me, post: [], room, give: to }),
      })
    ).json();
    lobbyWord = '';
    seatWord = '';
    showSeats(answer);
    showLobby(answer);
  } catch (e) {
    say('could not hand the controls over');
  }
}

// What to say about one person's sound, and what the reader can do about it.
//
// Ordered by what it is worth telling somebody: their microphone before their
// connection, because "he has not turned his on" is the answer nine times in
// ten and reads as a fault otherwise.
function soundOf(who) {
  if (who === me) {
    if (!joined) return ['your microphone is off', 'press talk to be heard'];
    if (muted) return ['muted', 'press unmute to be heard'];
    return ['talking', ''];
  }
  const how = state.get(who);
  if (!how || how === 'new' || how === 'connecting') return ['connecting', ''];
  if (how === 'failed' || how === 'disconnected') {
    return ['no connection', 'you two cannot reach each other directly'];
  }
  if (!hearing.get(who)) {
    return ['listening only', 'they have not pressed talk yet'];
  }
  return ['talking', ''];
}

// The game beginning, for everybody who did not begin it.
//
// The flag arrives on the room poll and the lobby is closed by the scene poll,
// and somebody sitting in a lobby is asking for scenes rarely -- so the news
// came in through one door and was only read at another. It is read here, where
// it arrives, and nothing else is allowed to come before it.
function enterGame(answer) {
  if (!answer.begun || begun) return;
  begun = true;
  if (started) return;
  started = true;
  const sheet = document.getElementById('setup');
  if (sheet) sheet.hidden = true;
  setFull(true);
  // Their clock has to run too, or they are shown a board that never moves --
  // which is a worse bug wearing the same clothes.
  ask({ do: 'Play', on: true });
  say('the game has started');
}

// What a room can play. The same list the front door offers, because the
// choice is the same choice -- and a game is a file, so adding one is a line.
const GAMES = [
  { file: 'samples/ludogame.easel', name: 'ludo', ready: true },
  { file: 'samples/ludogame.easel', name: 'ludo, with adventures', ready: false },
];

let gamesWord = '';
function showGames(answer) {
  const box = document.getElementById('lobby-games');
  if (!box) return;
  const key = `${answer.game}|${amHost}|${begun}`;
  if (key === gamesWord) return;
  gamesWord = key;
  box.innerHTML = '';
  for (const g of GAMES) {
    const b = document.createElement('button');
    b.type = 'button';
    b.textContent = g.ready ? g.name : `${g.name} — not built yet`;
    b.className = answer.game === g.file && g.ready ? 'on' : '';
    // Only the holder of the controls chooses, for the same reason only they
    // set the house rules: four people choosing is three having it chosen for
    // them by whoever clicked last.
    b.disabled = !g.ready || !amHost || begun;
    b.onclick = () => pick(g.file);
    box.append(b);
  }
}

async function pick(file) {
  await roomSays({ game: file });
}

// Whether the empty chairs are played by bots.
const botsBox = document.getElementById('bots-on');
if (botsBox) {
  botsBox.onchange = () => roomSays({ bots: botsBox.checked });
}

// One way to tell the room something, so every one of them goes through the
// same door and comes back with the same answer.
async function roomSays(what) {
  try {
    const answer = await (
      await fetch('/talk', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ me, post: [], room, ...what }),
      })
    ).json();
    seatWord = '';
    lobbyWord = '';
    gamesWord = '';
    showSeats(answer);
    showLobby(answer);
    showState(answer);
    // The drawing itself has changed if the game did, so the next scene must
    // be a whole one rather than a difference from a board that is gone.
    held = 0;
    refresh();
    return answer;
  } catch (e) {
    say('could not reach the room');
    return null;
  }
}

function lowestSeat(seats) {
  return seats.length ? Math.min(...seats.map((s) => s.seat)) : -1;
}

function showLobby(answer) {
  const list = document.getElementById('lobby-list');
  if (!list) return;
  const who = answer.who || [];
  const seats = answer.seats || [];
  const bots = begun ? [] : answer.bots || [];

  const title = document.getElementById('lobby-title');
  const note = document.getElementById('lobby-note');
  const others = who.filter((w) => w.id !== me).length;
  if (amHost) {
    title.textContent = others ? (others + 1) + ' in the room' : 'waiting for the others';
    note.textContent = others
      ? 'start when everybody is ready \u2014 any empty seat is played by a bot'
      : 'send them the link \u2014 they will appear here, and empty seats are played by bots';
  } else {
    // Named from `hostid`, which the server holds -- it used to be guessed
    // from the lowest seat, and the guess changed whenever anybody sat down.
    const holder = who.find((w) => w.id === answer.hostid);
    title.textContent = (others + 1) + ' in the room';
    note.textContent = (holder ? holder.name : 'the first player') + ' starts the game when everybody is ready';
  }

  const key = JSON.stringify([who.map((w) => [w.id, w.name, w.seat]), bots, amHost]);
  if (key === lobbyWord) return;
  lobbyWord = key;
  list.innerHTML = '';

  for (const w of who) {
    const row = document.createElement('div');
    row.dataset.who = w.id;
    const seated = w.seat !== null && w.seat !== undefined;
    const theirs = w.id === answer.hostid;
    row.innerHTML =
      '<span class="dot"></span><span class="nm"></span>' +
      '<span class="tag"></span><span class="lvl"><i></i></span>';
    row.querySelector('.dot').style.background = seated ? SEATINK[w.seat % 4] : 'transparent';
    row.querySelector('.nm').textContent = w.name + (w.id === me ? ' (you)' : '');
    row.querySelector('.tag').textContent = seated
      ? SEATNAMES[w.seat] || 'seat ' + (w.seat + 1)
      : 'watching';
    // A slider each, for everybody but yourself -- your own is the microphone.
    if (w.id !== me) {
      const slide = document.createElement('input');
      slide.type = 'range';
      slide.className = 'vol';
      slide.min = 0;
      slide.max = 200;
      slide.value = Math.round((level_of.get(w.id) ?? 1) * 100);
      slide.title = 'how loud they are';
      slide.oninput = () => setPersonVolume(w.id, Number(slide.value) / 100);
      row.append(slide);
    }
    const [how, todo] = soundOf(w.id);
    const sound = document.createElement('span');
    sound.className = `tag sound ${how.replace(/ /g, '-')}`;
    sound.textContent = how;
    if (todo) sound.title = todo;
    row.insertBefore(sound, row.querySelector('.lvl'));
    // Who is in charge, said on the row rather than left to be inferred from
    // who happens to have a button.
    if (theirs) {
      const crown = document.createElement('span');
      crown.className = 'tag host';
      crown.textContent = 'has the controls';
      row.insertBefore(crown, row.querySelector('.lvl'));
    } else if (amHost) {
      // Only the holder is offered the giving-away.
      const give = document.createElement('button');
      give.className = 'give';
      give.textContent = 'give controls';
      give.onclick = () => handOver(w.id);
      row.insertBefore(give, row.querySelector('.lvl'));
    }
    list.append(row);
  }

  // The bots box follows the room rather than this browser, and only the
  // holder of the controls may move it.
  const bb = document.getElementById('bots-on');
  if (bb) {
    if (document.activeElement !== bb) bb.checked = answer.botsOn !== false;
    bb.disabled = !amHost || begun;
  }

  // One line of advice, for whoever is reading it. Somebody who cannot be heard
  // should be told so on their own screen rather than by the others noticing.
  const advice = document.getElementById('lobby-sound');
  if (advice) {
    const mineNow = soundOf(me);
    const quiet = who.filter((w) => w.id !== me && soundOf(w.id)[0] === 'listening only');
    let line = '';
    if (!joined) {
      line = 'nobody can hear you — press talk. You can hear them already.';
    } else if (muted) {
      line = 'you are muted — press unmute.';
    } else if (quiet.length === 1) {
      line = `${quiet[0].name} has not pressed talk, so they cannot be heard.`;
    } else if (quiet.length > 1) {
      line = `${quiet.length} of them have not pressed talk yet.`;
    } else if (who.some((w) => w.id !== me && soundOf(w.id)[0] === 'no connection')) {
      line = 'somebody could not be reached directly — a strict network in the way.';
    }
    advice.hidden = !line;
    advice.textContent = line;
    void mineNow;
  }

  // The seats nobody has taken, so it is plain what starting now would mean.
  for (const seat of bots) {
    const row = document.createElement('div');
    row.innerHTML = '<span class="dot"></span><span class="nm">a bot</span><span class="tag"></span>';
    row.querySelector('.dot').style.background = SEATINK[seat % 4];
    row.querySelector('.tag').textContent = SEATNAMES[seat] || 'seat ' + (seat + 1);
    row.style.opacity = '0.6';
    list.append(row);
  }
}

function showSeats(answer) {
  const box = document.getElementById('seats');
  if (!box) return;
  const howMany = answer.howmany || 0;
  // The bar carries the seats, the name and the microphone, so it is up
  // whenever any of those is worth having -- a sketch with no seats still
  // wants the name and the talking.
  document.getElementById('hud').hidden = !howMany && !room;
  box.hidden = howMany === 0;
  amHost = !!answer.host;

  // **Before any early return.** This used to sit below `if (!howMany) return`,
  // so a drawing that momentarily reported no seats swallowed the one message
  // that has to arrive. The game beginning is not a detail of the seat strip.
  enterGame(answer);

  if (!howMany) return;
  mySeat = answer.mine;

  // **The game starting has to reach the people who did not start it.**
  //
  // This flag arrived correctly and nothing acted on it: the line that hides
  // the lobby lives in `show`, which runs when a SCENE arrives -- and somebody
  // sitting in the lobby with the game not yet running is asking for scenes
  // rarely and pressing nothing. So the host began, everybody else was told,
  // and everybody else went on looking at "waiting for the first player".
  //
  // The news arrives here, so it is acted on here.


  // **Arriving is playing.** Somebody who opens a game link means to play it,
  // and leaving them unseated until they notice a row of coloured buttons is
  // how two people ended up watching four bots.
  //
  // Only before the game begins, only once, and only if there is a seat: a
  // latecomer chooses for themselves, which is right, because the seat they
  // want is the one a bot is holding rather than whichever is lowest.
  if (!begun && !satDown && (answer.mine === null || answer.mine === undefined)) {
    const free = (answer.bots || []).filter((k) => !(answer.seats || []).some((s) => s.seat === k));
    if (free.length) {
      satDown = true;
      sit(free[0]);
      return;
    }
  }
  const taken = new Map((answer.seats || []).map((s) => [s.seat, s.who]));
  const key = `${howMany}|${[...taken].join(',')}|${answer.mine}|${answer.turn}`;
  if (key === seatWord) return;
  seatWord = key;
  box.innerHTML = '';
  for (let k = 0; k < howMany; k++) {
    const b = document.createElement('button');
    const seat = (answer.seats || []).find((s) => s.seat === k);
    const who = seat && seat.who;
    const mine = who && who === me;
    // The colour on top, and underneath it WHO -- because "blue" tells you
    // nothing across a telephone and "Ram" tells you everything.
    const label = SEATNAMES[k] || `seat ${k + 1}`;
    const bot = !who && begun && (answer.bots || []).includes(k);
    const under = mine ? 'you' : who ? seat.name || `player ${k + 1}` : bot ? 'bot' : 'free';
    b.innerHTML = `${label}<span class="name"></span>`;
    b.querySelector('.name').textContent = under;
    b.style.borderColor = SEATINK[k % 4];
    if (mine) b.style.background = SEATINK[k % 4];
    if (mine) b.style.color = '#08121a';
    b.classList.toggle('taken', !!who && !mine);
    b.classList.toggle('bot', bot);
    b.classList.toggle('mine', !!mine);
    b.classList.toggle('turn', answer.turn === k);
    // A bot's seat is still free to sit in -- somebody arriving late should be
    // able to take over from one, which is the whole point of a bot standing
    // in rather than the game refusing to start.
    b.disabled = !!who && !mine;
    b.title = mine ? 'tap to stand up' : who ? `${under} is here` : bot ? 'a bot is playing this -- sit here to take over' : 'sit here';
      b.onclick = () => {
      // Standing up is a decision, so it is remembered -- otherwise the next
      // poll would sit them straight back down.
      satDown = true;
      sit(mine ? -1 : k);
    };
    // Your own seat is where you change your name -- there is no other button
    // it could belong to, and a whole field for it would sit there empty all
    // game.
    if (mine) {
      b.oncontextmenu = (e) => {
        e.preventDefault();
        nameField.focus();
      };
    }
    box.append(b);
  }
  // Whose turn it is belongs to the status line, which is built in one place
  // from one answer. Saying it here as well is how two parts of a page come to
  // disagree about the same fact.
}

// A field rather than a prompt. Somebody who has to guess that their own seat
// can be long-pressed will never find out that it can.
// The same field twice: once in the lobby, once in the bar during the game.
// Kept in step rather than one being the real one, because either is where
// somebody will reach for it.
let namePending = null;
function wireName(field) {
  if (!field) return;
  field.value = myName;
  field.oninput = () => {
    myName = field.value.trim().slice(0, 16);
    sessionStorage.setItem('name', myName);
    for (const other of [document.getElementById('myname'), document.getElementById('lobby-name')]) {
      if (other && other !== field) other.value = myName;
    }
    // Sent when the typing stops, not on every letter -- otherwise "Sushant"
    // is seven requests and six of them show the others a half-typed name.
    clearTimeout(namePending);
    namePending = setTimeout(() => {
      seatWord = '';
      lobbyWord = '';
      callSeats();
// And start signalling at once if this is a room, so people can hear each
// other before anybody has thought about microphones.
if (room) callIn();
    }, 400);
  };
}
wireName(document.getElementById('myname'));
wireName(document.getElementById('lobby-name'));

async function sit(seat) {
  try {
    const answer = await (
      await fetch('/talk', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ me, post: [], sit: seat, room }),
      })
    ).json();
    seatWord = '';
    lobbyWord = '';
    showSeats(answer);
    showLobby(answer);
    say(answer.mine === null || answer.mine === undefined ? 'standing' : `you are ${SEATNAMES[answer.mine] || answer.mine + 1}`);
  } catch (e) {
    say('could not take that seat');
  }
}

// Calling in, so the room knows we are still here.
//
// A backgrounded tab has its timers throttled to roughly once a minute, and a
// handset suspends them altogether -- which is why the server waits ninety
// seconds before calling anybody gone, and waits much longer than that before
// moving the controls. Sending somebody the link should not make you leave the
// room you just made.
setInterval(() => {
  if (!room) callSeats();
}, 1500);

// And call in the instant we come back, rather than waiting for the next tick.
// Coming back from another app is exactly when the room most needs to hear
// from us.
document.addEventListener('visibilitychange', () => {
  if (!document.hidden) callSeats();
});

// Leaving on purpose is said out loud. A beacon is the only request a browser
// promises to send while a page is going away -- an ordinary fetch is
// abandoned with the page, which is why this is the one place not using `ask`.
window.addEventListener('pagehide', () => {
  try {
    navigator.sendBeacon(
      '/talk',
      new Blob([JSON.stringify({ me, post: [], room, gone: true })], { type: 'application/json' })
    );
  } catch (e) {
    /* going anyway */
  }
});

// Even without the microphone on, a player needs to see the seats -- talking
// and playing are separate things and somebody may want only one of them.
async function callSeats() {
  try {
    const answer = await (
      await fetch('/talk', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ me, post: [], room, name: myName || null }),
      })
    ).json();
    showSeats(answer);
  showLobby(answer);
  showState(answer);
  } catch (e) {
    /* next time */
  }
}
callSeats();

// The room's name, and a way to hand it to somebody. Shown only when there is
// a room -- a person drawing alone is never made to think about rooms.
(function showRoom() {
  const box = document.getElementById('room');
  if (!box) return;
  box.hidden = !room;
  if (!room) return;
  document.getElementById('room-code').textContent = room;
  document.getElementById('room-copy').onclick = async () => {
    try {
      await navigator.clipboard.writeText(location.href);
      say('link copied -- send it to the others');
    } catch (e) {
      // Clipboard needs a secure context too, and over plain http it is
      // simply absent. Show the thing to copy rather than failing quietly.
      say(location.href);
    }
  };
})();

// Who is in the room. Rebuilt only when the list changes, so a bar being
// animated is not thrown away sixty times a second.
let shownHere = '';
function showHere(here) {
  const box = document.getElementById('voices');
  if (!box) return;
  box.hidden = !joined;
  const key = here.join(',');
  if (key === shownHere) return;
  shownHere = key;
  box.innerHTML = '';
  for (const who of here) {
    const row = document.createElement('div');
    row.dataset.who = who;
    // The peer id is eight characters of nothing; the first four are enough to
    // tell two tabs apart and short enough not to matter.
    row.innerHTML = `<span class="who">${who.slice(0, 4)}</span><span class="bar"><i></i></span>`;
    box.append(row);
  }
  if (!here.length) box.innerHTML = '<div class="none">waiting for somebody else</div>';
}

function link(who) {
  if (links.has(who)) return links.get(who);
  const pc = new RTCPeerConnection(ICE);
  links.set(who, pc);

  // Every address this machine might be reachable at, as they are discovered.
  // They arrive over a second or two, which is why they are sent as they come
  // rather than waited for -- "trickle ICE", and it is most of the difference
  // between a connection that takes half a second and one that takes five.
  pc.onicecandidate = (e) => {
    if (e.candidate) outbox.push({ to: who, kind: 'ice', body: JSON.stringify(e.candidate) });
  };
  pc.ontrack = (e) => {
    const el = speaker(who);
    el.srcObject = e.streams[0];
    el.volume = Math.min(1, volume);
    el.muted = loud;
    route(who, e.streams[0]);
    watch(who, e.streams[0]);
    hearing.set(who, true);
  };
  pc.onconnectionstatechange = () => {
    // **Written down rather than guessed at.** "It worked once and then never
    // again" is not something anybody can debug from the outside, and it is
    // not something the person it is happening to should have to describe. The
    // page knows; it should say.
    state.set(who, pc.connectionState);
    if (pc.connectionState === 'failed' || pc.connectionState === 'closed') {
      drop(who);
    }
  };

  // **The bug that made it work once and then never again.** A connection
  // opened before this browser had a microphone carried no audio, and adding
  // the track later changes nothing on its own -- the far side has already
  // agreed what this connection contains. Somebody has to offer again.
  //
  // `negotiationneeded` fires exactly when that is true. Only the caller
  // re-offers, by the same rule that decides who calls: two sides re-offering
  // at once is the glare condition again, and the answer to it has not changed.
  pc.onnegotiationneeded = async () => {
    if (!ringers.has(who) || pc.signalingState !== 'stable') return;
    try {
      const offer = await pc.createOffer();
      await pc.setLocalDescription(offer);
      outbox.push({ to: who, kind: 'offer', body: JSON.stringify(offer) });
    } catch (e) {
      /* the next poll will try again */
    }
  };

  if (mine) for (const t of mine.getTracks()) pc.addTrack(t, mine);
  return pc;
}

// Who this browser is supposed to be calling, as the server last said. Kept so
// `negotiationneeded` knows whether re-offering is its job.
const ringers = new Set();

// What each connection is doing, and whether any sound is arriving on it. Two
// different questions: a connection can be perfectly healthy and carry silence,
// because the far side has not turned a microphone on -- which is the commonest
// reason for "I can talk but they cannot hear me", and used to look identical
// to a broken connection.
const state = new Map();
const hearing = new Map();

// Put my microphone on every connection that has not got it, which makes
// `negotiationneeded` fire and the offer go out again.
function shareMicrophone() {
  if (!mine) return;
  for (const [who, pc] of links) {
    const already = pc.getSenders().some((sn) => sn.track && sn.track.kind === 'audio');
    if (!already) {
      for (const t of mine.getTracks()) pc.addTrack(t, mine);
    }
    void who;
  }
}

function drop(who) {
  const pc = links.get(who);
  if (pc) pc.close();
  links.delete(who);
  ringers.delete(who);
  state.delete(who);
  hearing.delete(who);
  const ear = ears.get(who);
  if (ear) {
    try {
      ear.src.disconnect();
    } catch (e) {
      /* already gone */
    }
    ears.delete(who);
  }
  meters.delete(who);
  shownHere = '';
  const el = document.getElementById(`ear-${who}`);
  if (el) el.remove();
}

// Notes waiting to go out, sent with the next call.
let outbox = [];

// Who offers is decided by the SERVER, in `talk::Room::calls`, and arrives as
// `ring`. Both offering at once is the "glare" condition -- each answers the
// other and two connections form where one was wanted -- and a rule that
// mattered that much had no business being written twice.

async function gotNote(note) {
  const pc = link(note.from);
  const body = JSON.parse(note.body);
  if (note.kind === 'offer') {
    await pc.setRemoteDescription(body);
    const answer = await pc.createAnswer();
    await pc.setLocalDescription(answer);
    outbox.push({ to: note.from, kind: 'answer', body: JSON.stringify(answer) });
  } else if (note.kind === 'answer') {
    await pc.setRemoteDescription(body);
  } else if (note.kind === 'ice') {
    // A candidate can arrive before the description it belongs to. Swallowing
    // that is normal and not an error worth showing anybody.
    try {
      await pc.addIceCandidate(body);
    } catch (e) {
      /* it will be offered again */
    }
  }
}

async function ring(who) {
  const pc = link(who);
  if (pc.signalingState !== 'stable') return;
  const offer = await pc.createOffer();
  await pc.setLocalDescription(offer);
  outbox.push({ to: who, kind: 'offer', body: JSON.stringify(offer) });
}

// One call: say I am here, hand over the post, collect mine.
async function callIn() {
  // **In a room is in the mesh.** Connections form whether or not this browser
  // has a microphone, so somebody who never presses the button still HEARS
  // everybody -- which they could not before, because the connections were only
  // made when the microphone was.
  //
  // A connection with no local track is perfectly ordinary; it receives.
  if (!room) return;
  const post = outbox;
  outbox = [];
  let answer;
  try {
    answer = await (
      await fetch('/talk', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ me, post, room, name: myName || null }),
      })
    ).json();
  } catch (e) {
    outbox = post.concat(outbox); // keep it for the next try
    return;
  }
  for (const note of answer.post || []) await gotNote(note);

  const here = (answer.here || []).filter((w) => w !== me);
  for (const who of answer.ring || []) {
    ringers.add(who);
    if (!links.has(who)) await ring(who);
  }
  for (const who of [...links.keys()]) if (!here.includes(who)) drop(who);

  showHere(here);
  showSeats(answer);
  showLobby(answer);
  showGames(answer);
  showState(answer);
  say(here.length ? `talking to ${here.length}` : 'nobody else is here yet');
}

// The microphone button. **Mute, once you are in** -- not "leave", which is
// what it used to be, and which took your ears with it.
async function micPressed() {
  if (!joined) {
    await talk(true);
    return;
  }
  muted = !muted;
  if (mine) for (const t of mine.getAudioTracks()) t.enabled = !muted;
  showMic();
}

// What the two microphone buttons say. One place, because there are two of
// them and they must never disagree.
function showMic() {
  const label = !joined ? '\u{1F3A4} talk' : muted ? '\u{1F507} unmute' : '\u{1F3A4} mute';
  for (const b of [document.getElementById('mic'), document.getElementById('lobby-mic')]) {
    if (!b) continue;
    b.textContent = label;
    b.classList.toggle('on', joined && !muted);
    b.classList.toggle('muted', joined && muted);
    b.title = !joined
      ? 'join the talking'
      : muted
        ? 'your microphone is off -- you can still hear the others'
        : 'turn your microphone off';
  }
}

async function talk(on) {
  if (!on) {
    joined = false;
    document.getElementById('vol').hidden = true;
    document.getElementById('speaker').hidden = true;
    showHere([]);
    document.getElementById('voices').hidden = true;
    for (const who of [...links.keys()]) drop(who);
    if (mine) for (const t of mine.getTracks()) t.stop();
    mine = null;
    showMic();
    return;
  }
  if (!canTalk()) {
    say(whyNot());
    return;
  }
  try {
    mine = await navigator.mediaDevices.getUserMedia({
      // The browser's own echo cancellation, noise suppression and gain
      // control. Four people in a room without these is a howl -- each
      // microphone picks up the others' speakers and feeds it back round.
      audio: { echoCancellation: true, noiseSuppression: true, autoGainControl: true },
      video: false,
    });
  } catch (e) {
    say(`no microphone: ${e.name}`);
    return;
  }
  joined = true;
  muted = false;
  document.getElementById('vol').hidden = false;
  document.getElementById('speaker').hidden = false;
  showMic();
  // Any connection opened before the microphone existed carries no audio yet.
  shareMicrophone();
  // My own level too, so the meter shows something before anybody else joins
  // -- otherwise a working microphone and a broken one look the same until a
  // second person turns up.
  watch(me, mine);
  callIn();
}

// Called in twice a second while talking. Fast enough that an offer is
// answered before anybody notices, slow enough to be nothing at all beside
// thirty scenes a second.
setInterval(callIn, 500);

// If anything above this line throws at load, nothing below it runs -- and a
// button with no handler is a button that does nothing at all, silently. So
// this is wired up defensively and says so when it cannot be.
// The microphone is offered in the lobby as well as in the game, because the
// waiting is when people say hello.
const lobbyMic = document.getElementById('lobby-mic');
if (lobbyMic) {
  lobbyMic.onclick = () => micPressed();
}
const volumeBar = document.getElementById('volume');
if (volumeBar) {
  // Up to 150%, because "not loud enough" is the complaint and a slider that
  // stops at what the platform already gives you cannot answer it.
  volumeBar.oninput = () => setVolume(Number(volumeBar.value) / 100);
}
const speakerButton = document.getElementById('speaker');
if (speakerButton) {
  speakerButton.onclick = async () => {
    const on = !speakerButton.classList.contains('on');
    speakerButton.classList.toggle('on', on);
    const moved = await useLoudspeaker(on);
    say(
      on
        ? moved
          ? 'playing through the loudspeaker'
          : 'trying the loudspeaker -- if it is still quiet, use the phone’s own speaker button'
        : 'back to the earpiece'
    );
  };
}

const micButton = document.getElementById('mic');
if (micButton) {
  micButton.onclick = () => micPressed();
  if (!canTalk()) {
    micButton.title = whyNot();
    micButton.classList.add('cannot');
  }
} else {
  console.error('no microphone button in the page');
}

// ---- the setup screen ----------------------------------------------------

// Built from `rules` on the wire. Nothing here knows what Ludo is: a row that
// ends `# rule: what brings a token out` is a house rule, and any game gets a
// setup screen by writing one.
//
// Nought-or-one is a tick box and anything else a number, which is the only
// distinction worth making -- and it is made from the VALUE, so a rule that is
// a count reads as a count without having to say so.
let shownRules = '';
function setup() {
  const rules = scene.rules || [];
  const key = JSON.stringify([amHost, rules.map((r) => [r.name, r.label])]);
  if (key === shownRules) {
    // Only rebuild when the rules themselves change. Rebuilding every frame
    // would take the focus out of a box the moment anybody typed in it.
    for (const r of rules) {
      const el = document.getElementById(`rule-${r.name}`);
      if (el && document.activeElement !== el) {
        if (el.type === 'checkbox') el.checked = r.value > 0.5;
        else el.value = r.value;
      }
    }
    return;
  }
  shownRules = key;
  // Only the host may change them; everybody else sees what they will be
  // playing by, which is worth showing and not worth being able to edit.
  document.getElementById('begin').hidden = !amHost;
  let note = document.getElementById('waiting');
  if (!note) {
    note = document.createElement('div');
    note.id = 'waiting';
    note.className = 'waiting';
    document.getElementById('begin').after(note);
  }
  note.hidden = amHost;
  note.textContent = 'waiting for the first player to start';
  const box = document.getElementById('rules');
  box.innerHTML = '';
  for (const r of rules) {
    const line = document.createElement('label');
    const words = document.createElement('span');
    words.textContent = r.label;
    const yesno = r.value === 0 || r.value === 1;
    const input = document.createElement('input');
    input.id = `rule-${r.name}`;
    input.type = yesno ? 'checkbox' : 'number';
    if (yesno) input.checked = r.value > 0.5;
    else input.value = r.value;
    input.disabled = !amHost;
    input.oninput = () => {
      const v = yesno ? (input.checked ? 1 : 0) : Number(input.value);
      if (Number.isFinite(v)) ask({ do: 'Dial', id: r.id, value: v });
    };
    line.append(words, input);
    box.append(line);
  }
}

document.getElementById('begin').onclick = async () => {
  // The first click of the game, which is the only moment a browser will let
  // a page start making sounds.
  listen();
  document.getElementById('setup').hidden = true;
  started = true;
  begun = true;
  setFull(true);
  // Tell the room, so nobody else is offered a game that has already begun.
  try {
    await fetch('/talk', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ me, post: [], room, start: true, name: myName || null }),
    });
  } catch (e) {
    /* the next poll carries it */
  }
  await ask({ do: 'Play', on: true });
};
// Putting the tools away and picking the pen up are the same act: a drawing
// with no tools on screen invites a hand, and a hand that leaves a line
// through it is the first thing anybody does.
function setFull(on) {
  document.getElementById('app').classList.toggle('full', on);
  requestAnimationFrame(paint);
  return ask({ do: 'Watch', on });
}
document.getElementById('full').onclick = () =>
  setFull(!document.getElementById('app').classList.contains('full'));

// ---- the clock -----------------------------------------------------------

// Stepped from here, because this is what knows when it last drew. A server
// ticking on its own would run at a rate nobody was watching at.
let last = performance.now();
let owed = 0;
function frame(now) {
  owed += (now - last) / 1000;
  last = now;
  // The clock is the one thing allowed to skip a turn, because a tick is not
  // an instruction -- it is an amount. A skipped one is CARRIED rather than
  // dropped, so a slow answer makes the animation stutter and never makes it
  // run slow, which would look like the physics being wrong.
  // **The clock is the room's, not this browser's.** Every page used to send
  // its own tick to the same board, so two players ran the game at twice real
  // time and four at four times -- and every tick cost a whole scene, which is
  // most of why it felt slow.
  //
  // So this only ASKS for the picture. The server moves the clock on by
  // however long has really passed, once, however many people are watching.
  //
  // Twenty a second rather than sixty: a board game is not a shooter, and each
  // frame is a drawing over a network. The die tumbles perfectly well at
  // twenty, and the machine does a third of the work.
  if (scene.playing && !waiting && !pending && owed > 0.05) {
    owed = 0;
    refresh();
  }
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

window.onresize = () => {
  fit();
  paint();
  refresh();
};

// What to open, and whether to open it playing. From the address, so a link is
// the whole of it -- no session to keep, and a page you can bookmark.
const wanted = new URLSearchParams(location.search);
const file = wanted.get('file');
document.getElementById('shown-file').textContent = file || 'drawing.easel';

(async () => {
  if (file) await ask({ do: 'OpenFile', name: file });
  else await refresh();
  // A drawing with house rules asks about them first; one without just plays.
  if (wanted.get('play')) {
    if ((scene.rules || []).length) {
      setup();
      document.getElementById('setup').hidden = false;
    } else {
      started = true;
      setFull(true);
      await ask({ do: 'Play', on: true });
    }
  }
})();
