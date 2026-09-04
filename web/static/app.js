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
function keep() {
  if (scene.still) {
    still = scene.still;
    held = scene.stillv;
  } else if (scene.stillv !== undefined && scene.stillv !== held) {
    // It changed and we were not sent one: ask again rather than draw a board
    // that is no longer the board.
    held = 0;
  }
}

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
  paper.style.cursor = scene.watching ? 'pointer' : 'crosshair';
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
  if (e.shiftKey || e.button === 1) {
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
  if (wanted.get('play')) {
    setFull(true);
    await ask({ do: 'Play', on: true });
  }
})();
