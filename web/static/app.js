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
  return `lox=${lox}&loy=${loy}&hix=${hix}&hiy=${hiy}&px=${Math.round(r.width)}&have=${held}`;
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

function keep() {
  if (scene.still) {
    still = scene.still;
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

function paint() {
  const r = size();
  ctx.clearRect(0, 0, r.width, r.height);

  // Points arrive as whole numbers of hundredths of a world unit -- see
  // `wire::GRAIN`. Formatting thirty thousand floats as decimal text was
  // taking 84ms a scene, which is what made every tap feel slow.
  const G = 0.01;
  for (const piece of still.concat(scene.pieces)) {
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

  for (const line of scene.tree) {
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
  document.getElementById('setup').hidden = started || !(scene.rules || []).length;
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

// One decaying tone. `tau` is how long it takes to fall to a thirty-seventh of
// where it started, and four of those is the whole of it.
function tone(freq, tau, at, gain = 0.2, kind = 'sine', bend = 1) {
  const c = listen();
  if (!c) return;
  const t = c.currentTime + at;
  const o = c.createOscillator();
  const g = c.createGain();
  o.type = kind;
  o.frequency.setValueAtTime(freq, t);
  if (bend !== 1) o.frequency.exponentialRampToValueAtTime(freq * bend, t + 4 * tau);
  // setTargetAtTime IS the exponential decay -- the same one, done in the
  // audio thread rather than approximated with line segments.
  g.gain.setValueAtTime(gain, t);
  g.gain.setTargetAtTime(0.0001, t, tau);
  o.connect(g).connect(c.destination);
  o.start(t);
  o.stop(t + 4 * tau + 0.05);
}

// A burst of noise, shaped the same way. What a die clattering is: no pitch,
// just a lot of edges dying away.
function rattle(tau, at, gain = 0.3, cut = 2200) {
  const c = listen();
  if (!c) return;
  const t = c.currentTime + at;
  const n = Math.floor(c.sampleRate * (4 * tau + 0.05));
  const buf = c.createBuffer(1, Math.max(n, 1), c.sampleRate);
  const d = buf.getChannelData(0);
  // Worked out, not drawn from anywhere -- the same sin-of-a-large-multiple
  // trick the die itself uses, so a throw sounds the same on every replay.
  for (let k = 0; k < n; k++) d[k] = Math.sin(k * 12.9898) * 43758.5453 % 1;
  const src = c.createBufferSource();
  src.buffer = buf;
  const f = c.createBiquadFilter();
  f.type = 'lowpass';
  f.frequency.setValueAtTime(cut, t);
  const g = c.createGain();
  g.gain.setValueAtTime(gain, t);
  g.gain.setTargetAtTime(0.0001, t, tau);
  src.connect(f).connect(g).connect(c.destination);
  src.start(t);
  src.stop(t + 4 * tau + 0.05);
}

const NOISES = {
  // A die thrown: a clatter, then two knocks as it lands and settles.
  roll: () => {
    rattle(0.09, 0, 0.32, 3000);
    rattle(0.05, 0.16, 0.22, 1800);
    tone(190, 0.05, 0.3, 0.1, 'triangle', 0.8);
  },
  // A token on a square: short, soft, and quiet enough to hear ten of.
  step: () => tone(660, 0.028, 0, 0.07, 'sine', 0.85),
  // A capture: low, with a bite on it.
  cut: () => {
    tone(150, 0.1, 0, 0.22, 'square', 0.5);
    rattle(0.06, 0, 0.14, 900);
  },
  // A token home: two notes going up, a fifth apart.
  home: () => {
    tone(523, 0.12, 0, 0.16, 'triangle');
    tone(784, 0.18, 0.1, 0.16, 'triangle');
  },
};

function noises() {
  for (const s of scene.sounds || []) {
    const was = heard.get(s.name);
    heard.set(s.name, s.at);
    // Only ever on the way UP, and never on the first sighting -- otherwise
    // opening a game part way through plays every sound it has ever made.
    if (was === undefined || s.at <= was) continue;
    const play = NOISES[s.name];
    if (play) play();
  }
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
  const key = JSON.stringify(rules.map((r) => [r.name, r.label]));
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
  setFull(true);
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
  // A command outranks the clock, and the time it waits is carried rather than
  // lost -- so a tap goes out at once and the animation catches up on the next
  // frame instead of running slow.
  if (scene.playing && !waiting && !pending && owed > 0) {
    const dt = owed;
    owed = 0;
    ask({ do: 'Tick', seconds: dt });
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
