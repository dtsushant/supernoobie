# Shapes — what there is, and how to use it

A cookbook. Every function you can call, what it means, and a line showing it
in use.

If you only read one thing, read **The one rule** below. Everything else falls
out of it.

---

## The one rule

**Every shape is built about its own origin, at roughly unit size.**

A shape does not know where it will end up. Putting it somewhere is a separate
step, and always the same step:

```rust
use shapes::{Place, Draw, face, digit};

frame.place(face::smiley(1.0), Cx::new(-3.0, 2.0));   // put it there
let seven = digit::glyph(7, 40).sized(1.5).at(Cx::new(1.0, 2.0));  // or as a value
```

Why insist on it: a shape that already knows where it lives can only be drawn
there. A shape about the origin can be placed, repeated, rotated about
anything, reflected, mapped through any function you like — because `at(z)` is
just `z ↦ z + at`, and it composes with everything else the same way.

```rust
// twelve smileys round a circle, because .at() is just addition
for k in 0..12 {
    let spot = Cx::expi(TAU * k as f64 / 12.0).scale(4.0);
    frame.place(face::smiley(0.5), spot);
}
```

> **Why `place` and not `draw`?** `Frame::draw(canvas, view)` already exists and
> means "render this frame onto a canvas". In Rust an inherent method silently
> beats a trait method of the same name, so a second `draw` would not have been
> a second `draw` — it would have been a puzzle. `place` says what it does.

### The two traits

| | |
|---|---|
| `Place` | `.at(z)` and `.sized(k)`. Works on a `Shape` **and** on a whole `Recipe`, construction lines included. |
| `Draw` | `frame.place(shape, at)`. Returns the style, so `.color(…)` and `.width(…)` chain off it. |

---

## Running a sketch: no window code

`Graph` owns the canvas, the view, the window and the loop, so none of them
appear in your file. A whole program:

```rust
use studio::prelude::*;

fn main() {
    Graph::new("my sketch").animate(scene);
}

fn scene(t: f64) -> Frame {
    let mut f = Frame::new();
    f.place(face::smiley(1.0), Cx::polar(3.0, t));
    f
}
```

```bash
cargo run -p studio --release --bin sketch     # a blank page to edit
```

| | |
|---|---|
| `Graph::plot(frame)` | one still, in a window |
| `Graph::animate(\|t\| ...)` | a film |
| `Graph::play(\|t, keys\| ...)` | a film that answers the keyboard |
| `Graph::png(path, frame)` | a file — **no window**, so it works in a test |
| `Graph::print(frame)` | the terminal, in braille — also no window |

Builders, all optional: `.size(w, h)`, `.scale(k)`, `.origin(x, y)`,
`.grid(on)`, `.background(c)`.

**Without `.scale`, the graph fits itself to your first frame.** Draw something
of size 1 or size 1000 and it fills the window either way, so a sketch never
has to guess a zoom level.

The graph keeps `Esc` and `G` for itself. In `plot` and `animate` it also takes
`,`/`.` to zoom, the arrows to pan, `Space` to pause and `S` to save a PNG. In
`play` it takes none of those, because a sketch reading the keyboard needs them
more than the graph does — everything reaches you through `Keys`:

```rust
keys.just('n')     // pressed this frame — for things that happen once
keys.held('w')     // held down — for things that happen continuously
keys.arrows()      // a direction as a Cx: right is 1, up is i
keys.digits()      // digits typed this frame, in order
keys.enter()  keys.backspace()

keys.at()          // the pointer, IN WORLD COORDINATES
keys.clicked()     // the moment the button went down, once per click
keys.down()  keys.over()
```

`keys.at()` comes back already through the view, in the same numbers the scene
is written in — so a hit test is the mathematics itself:

```rust
.on_click(|d, at| {
    if (at - centre).abs() <= radius {   // |z - c| <= r IS the disc
        d.colour = d.pick_colour();
    }
})
```

`keys.arrows()` returning a `Cx` is the point of the whole library in
miniature: a direction is a number, so `z + keys.arrows().scale(speed)` moves
a thing about and needs no vector type to do it.

### Binding keys, one per line

Give the graph some state and each key gets its own handler, so the code reads
like the table of controls it implements:

```rust
Graph::new("game")
    .with(Game::new(seed))
    .each_frame(|g, t| g.t = t)          // runs BEFORE the key handlers
    .on_digit(|g, d| g.typed.push(...))  // each digit typed, in order
    .on_enter(Game::check)
    .on_backspace(|g| { g.typed.pop(); })
    .on('n', Game::ask)                  // once per press
    .on_hold('w', |g| g.y += 0.1)        // every frame it is down
    .on_arrows(|g, dir| g.at = g.at + dir.scale(0.1))
    .run(scene);                         // scene: fn(&Game) -> Frame
```

**Why not `key('n').click(f)`.** In JavaScript every handler reaches the same
object because everything is shared and mutable. Rust will not let two closures
both hold `&mut game` — one could invalidate what the other is looking at, and
that is the class of bug the language exists to stop. So the state lives in the
`Sketch` and each handler is *lent* it for the moment it runs. Same shape of
code, none of the aliasing.

`Sketch::step(t, keys)` fires one frame's worth of bindings with no window, and
`Keys::pressing("n7")` builds a key state by hand — so what the keys do is
ordinary testable code:

```rust
// no .run(), so no window — the Sketch is the thing you step
let mut s = Graph::new("t").with(game).on('n', Game::ask);
s.step(0.0, &Keys::pressing("n"));
assert_eq!(s.state().moves, 1);
```

---

## The three crates

```
  plotkit    the paper.  Cx, View, Shape, Frame, plot, pen, raster, expr, script.
             zero dependencies. knows nothing about digits or ghosts.
      |
  shapes     things to draw. digits, faces, tallies, glyphs, waves, Fourier.
             depends on plotkit only. NO window, so it can be used from a test
             or the command line.
      |
  studio     the applications. depends on shapes + a window.
             contains no geometry at all — only state, layout, and a loop.
```

The split is load-bearing. `shapes` has no window, which is exactly why
`cargo run -p shapes -- ghost` can draw one in a terminal.

---

## Seeing a shape without writing any code

```bash
cargo run -p shapes -- list           # every name it knows
cargo run -p shapes -- smiley         # draw it, with its construction listed
cargo run -p shapes -- seven --steps  # draw it one construction line at a time
cargo run -p shapes -- ghost --big
cargo run -p shapes -- hexagon --grid
cargo run -p shapes -- three --png three.png
```

Output is Unicode braille — a braille cell is a 2×4 block of dots with one code
point per subset, which is 256 characters, which is exactly a byte. So the
canvas is dumped by setting bits, and a curve in a terminal still looks like a
curve.

Names are forgiving: `7`, `seven`, `SEVEN`, and `shape seven` all work, as does
`tally3` / `tally12` for any count.

---

## `shapes::digit` — 0 to 9, made of sine waves

There is no font. Each digit is a closed outline, transformed, and rebuilt from
its loudest few waves.

```rust
digit::outline(7)        // Vec<Cx> — the raw closed loop
digit::series(7)         // Series  — its Fourier coefficients (costs a transform)
digit::glyph(7, 40)      // Shape   — built from the 40 loudest waves
digit::recipe(7)         // Recipe  — the above, with the working shown
```

```rust
// a wobbly 7 and a crisp 7, side by side
frame.place(digit::glyph(7, 6),  Cx::new(-2.0, 0.0)).color(0xE0A44A);
frame.place(digit::glyph(7, 40), Cx::new( 2.0, 0.0)).color(0x4FBCD4);
```

`series` is not cheap — hold the result if you need it every frame:

```rust
let waves: Vec<Series> = (0..10).map(digit::series).collect();
// then later, free:
waves[7].curve(terms)
```

---

## `shapes::fourier` — any closed curve as a sum of rotating arrows

The engine underneath the digits. Give it any closed loop and it hands back the
arrows that draw it.

```rust
let s = Series::of(&my_closed_path, 256);

s.at(n, theta)        // Cx      — the first n waves added up
s.arrows(n, theta)    // Vec<Cx> — every partial sum, for drawing tip to tail
s.curve(n)            // Shape   — the curve the first n waves trace
s.machine(n, theta)   // Shape   — the arrows and their circles, at one instant
s.terms               // Vec<(i32, Cx)> — (frequency, coefficient), biggest first
```

The maths, in two lines:

```text
    z(θ)  =  Σ c_n e^{inθ}              a stack of arrows, tip to tail
    c_n   =  (1/N) Σ z_k e^{-inθ_k}     multiply by the conjugate and average
```

The second line works because multiplying by `e^{-inθ}` stops the term you want
from spinning, so it survives the average, while every other term keeps turning
and cancels itself out over a full lap.

Coefficients come back **sorted by size**, so "the first `n`" always means "the
`n` that matter most" and truncating is always the best answer at that budget.

Helpers you will want when building outlines:

```rust
fourier::arc(centre, rx, ry, a0, a1, n)   // a slice of an ellipse, as points
fourier::there_and_back(stroke)           // walk out and back, so an open
                                          // stroke becomes a closed loop
fourier::resample(path, n)                // even spacing ALONG the curve
```

> **`of` vs `of_samples`.** `of` closes the path and re-spaces it evenly by
> arclength — right for outlines, because a truncated series should spend its
> few terms on shape rather than on pacing. `of_samples` transforms exactly what
> you hand it. Use `of_samples` when the parametrisation is already the one you
> mean: an ellipse stepped at even **angles** is exactly two terms, but
> re-spaced by arclength it is not, because arclength is not proportional to
> angle.

---

## Hit testing — was that click on it?

For a circle the answer is its own definition, `|z - c| <= r`, and writing that
out is better than calling anything. For a shape without such a tidy
definition, `Shape` will answer:

```rust
shape.contains(p, lo, hi, width_px)        // inside a closed shape
shape.touches(p, tol, lo, hi, width_px)    // within tol of the outline
shape.distance(p, lo, hi, width_px)        // how far off it was
```

`contains` counts how many times a ray from `p` crosses the outline. Odd means
inside — walk in from infinity and every crossing swaps you between out and in,
so the parity is the answer.

**Inside and on-the-line are different questions.** The middle of a ring is
inside it but nowhere near it, and which one you want depends on whether the
shape reads as a button or as a wire.

`lo`, `hi` and `width_px` are the same arguments `polylines` takes, because a
shape that only exists relative to a view has to be sampled against one before
there is anything to measure.

---

## `shapes::grab` — shapes you can take hold of

```rust
let mut d = Disc::new(Cx::ZERO, 2.0);
d.drag(pointer, button_down);   // once a frame

d.resizing()   // dragging the rim
d.held()       // dragging anything
d.tapped()     // pressed and released without travelling — a click, not a drag
d.contains(p)  d.shape()  d.handles()
```

Drag the **rim** to resize, drag the **inside** to move. It needs only a
position and a bool, no window, so it lives with the geometry and is testable
without a screen.

> **What you have hold of is decided when the button goes down**, and kept
> until it comes up. Re-deciding every frame looks equivalent and is not: drag
> the rim quickly through the middle and a frame-by-frame test hands you the
> *inside* on the way past, so the disc stops resizing and starts following the
> cursor. There is a test that drags straight through the centre and demands it
> still be resizing on the far side.

---

## `shapes::motion` — spin, walk, run, as values

A motion is a function from time to a **pose**, and a pose is `z ↦ az + b`.
Two complex numbers: `a` turns and stretches, `b` moves. That pair is *every*
similarity of the plane, which is why one type covers all of these without a
special case:

```rust
Motion::still()                Motion::spin(turns_per_second)
Motion::travel(velocity)       Motion::orbit(radius, turns_per_second)
Motion::walk(velocity)         Motion::bob(height, per_second)
Motion::run(velocity)          Motion::pulse(amount, per_second)

m.then(other)    // both at once — ONE motion
m.about(centre)  // do it around a point instead of the origin
m.reversed()     m.speed(2.0)
m.at(t)          // -> Pose        m.shape(s, t)   // -> Shape
```

**Positive is anticlockwise.** Not a coin flip: `e^{iθ} = cos θ + i sin θ`, so
as `θ` grows the point goes from `1` towards `i` — right, then up. Clockwise is
`spin(-rate)`.

**`then` works because composing poses is multiplying the pairs:**

```text
    (a₂, b₂) ∘ (a₁, b₁)  =  (a₂a₁,  a₂b₁ + b₂)
```

That is the group law of the plane's similarities. It is why walking while
spinning is one motion rather than two things fighting over the same shape —
and why `walk` and `run` are *built* from `travel`, `bob` and `spin` rather
than written afresh. A run is a walk at three times the speed, bouncing higher,
leaning forward; that is three composed motions and no new code.

`Pose::inverse()` undoes a pose, which is what lets you click something that is
being carried by a motion.

---

## `shapes::troupe` — a group that is one of the things it groups

```rust
let mut t = Troupe::new()
    .and(Disc::new(Cx::new(-2.0, 0.0), 1.0))
    .and(Disc::new(Cx::new(2.0, 0.0), 0.7))
    .moving(Motion::spin(0.1));

t.tick(time);              // once a frame, before anything asks where it is
t.drag(pointer, down);     // dispatched to whichever member was grabbed
t.set_motion(m);           // swap the motion, starting its clock NOW
t.shape()                  // the whole group, as one shape
t.parts()                  // each member separately, for individual colours
t.now()                    // what the clock says
```

> **A motion is read at `t - t0`, not `t`.** `travel` is velocity × time, so on
> an absolute clock a group told to walk after thirty seconds is instantly
> thirty units away — off screen, and never seen again. `set_motion` starts the
> new motion's clock, and bakes whatever the old one had **translated** into
> the members first, so changing direction mid-walk carries on from where it
> got to. A rotation cannot be baked in — `nudge` moves members, it does not
> turn them — so swapping away from a spin snaps the group back square.
>
> The same trap catches anything that *rebuilds* a troupe mid-run: a fresh one
> starts its clock at zero, so hand it `now()` before the graph does.

**`Troupe` implements `Actor`, and so do its members.** Same trait — so a group
nests inside a group, and anything that takes one takes the other. That is what
makes "a group of shapes is itself a shape" true rather than merely convenient,
and it is the whole design:

```rust
pub trait Actor {
    fn shape(&self) -> Shape;
    fn hit(&self, p: Cx) -> bool;
    fn drag(&mut self, at: Cx, down: bool);
    fn nudge(&mut self, by: Cx);
    fn tapped(&self) -> bool { false }
}
```

Two rules make it behave:

1. **Which member you grabbed is decided on the press** — the same rule as a
   single shape, one level up.
2. **A moving group hit-tests through the inverse of its pose.** Members do not
   know they are being carried, so the pointer is taken back through the motion
   before they are asked. Without it, everything inside a spinning group would
   be grabbable only where it *used to be*.

`cargo run -p studio --release --bin stage` is the demonstration: five discs in
a ring, the group spinning or walking, and every disc still individually
draggable while it moves.

---

## `shapes::face` — a smiley and a ghost

```rust
face::smiley(1.0)          // Shape
face::ghost(1.0)           // Shape
face::smiley_recipe(1.0)   // Recipe, with the working
face::ghost_recipe(1.0)
```

The ghost's hem is `|sin 3πu|` along the bottom edge. The absolute value is the
whole trick — plain `sin` changes sign each lobe, so every other scallop would
bulge *upward* into the body and it would look like a games controller. There
is a test named after exactly that.

---

## `shapes::count` — tally marks

```rust
count::tally(7)     // Shape — seven marks, centred on the origin
count::width(7)     // f64   — how wide they come out
count::recipe(7)    // Recipe, one step per mark
```

Every fifth mark is the diagonal that strikes through the previous four.
`width` is what you want when centring a tally under something, and it is
tested to equal the width actually drawn — otherwise the two would drift apart
and everything would sit slightly off.

---

## `shapes::glyph` — punctuation, and shapes that are their own definition

```rust
glyph::plus()            glyph::plus_recipe()
glyph::equals()          glyph::equals_recipe()
glyph::question()        glyph::question_recipe()
                         glyph::circle_recipe()
                         glyph::square_recipe()
                         glyph::ngon_recipe(6)
```

The `n`-gon is worth reading. Its corners are the **`n`-th roots of unity** —
the `n` solutions of `z^n = 1`, which are `e^{i·2πk/n}`. There is no polygon
code, only that. The square likewise: one corner, multiplied by `i` three
times, closing up because `i⁴ = 1`.

```bash
cargo run -p shapes -- hexagon --steps    # watch the roots appear, then join up
```

---

## `shapes::wave` — `a sin(kx + φ)`, and adding them

```rust
let w = Wave::new(1.0, 2.0, 0.0);   // amplitude, frequency, phase

w.at(x)        // f64 — the height
w.arrow(x)     // Cx  — the rotating arrow it is the shadow of
w.phasor()     // Cx  — amplitude and phase in one number (the arrow at x = 0)

wave::total(&ws, x)     // f64      — the height of a whole stack
wave::chain(&ws, x)     // Vec<Cx>  — the arrows laid tip to tail
wave::combine(&ws)      // Option<Wave>
wave::next(&ws)         // Wave     — the next harmonic, continuing the pattern
```

`next` reads the frequencies rather than counting the waves. All odd → the
next odd one, so a square wave stays square; an even harmonic is symmetric
about the half-period where a square wave is antisymmetric, so it would undo
the flatness the odd terms built. Amplitude `1/k`, because coefficients
falling off that slowly are exactly what a waveform with a **jump** needs —
anything faster sums to something smooth, which can never have a vertical
edge.

Everything rests on one identity:

```text
    a sin(kx + φ)  =  Im( a e^{i(kx+φ)} )
```

The wave is the *shadow* of the arrow. Taking a shadow is linear, so
`Im(A) + Im(B) = Im(A+B)` — adding waves is adding arrows, and the head-to-tail
picture is not an illustration of the addition, it **is** the addition.

`combine` pulls the common rotation out when every frequency agrees:

```text
    Σ a_j sin(kx + φ_j)  =  |A| sin(kx + arg A),   A = Σ a_j e^{iφ_j}
```

One complex addition and you have the answer. Every sum-to-product identity in
a trigonometry textbook is that line written out in real numbers so that it
looks hard.

**And it returns `None` when the frequencies differ**, because then no single
sine is the answer and saying otherwise would be a lie. That refusal is where
Fourier series begin — it is the reason `fourier` exists.

```rust
combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 1.0, PI)])   // Some(amplitude 0)
combine(&[Wave::new(1.0, 1.0, 0.0), Wave::new(1.0, 2.0, 0.0)])  // None
```

```bash
cargo run -p studio --release --bin waves    # all of the above, moving
```

---

## `shapes::recipe` — a shape plus how it was built

```rust
Recipe::new("name", "the one line of maths it comes down to")
    .step("what you would say while drawing this bit", some_shape)
    .step("and then this bit", another_shape)
```

```rust
r.shape()      // the finished Shape
r.upto(2)      // just the first two construction lines
r.steps        // Vec<Step> { says, shape, colour }
r.maths        // the one-line summary
```

The finished shape **is** the last step of the recipe — there is only one of
them — so the picture can never disagree with its own working.

### Adding your own shape

1. Write `fn thing_recipe() -> Recipe`, built about the origin, in the module
   it belongs to (or a new one).
2. Add `fn thing() -> Shape { thing_recipe().shape() }` if you want the bare
   version.
3. Add a name for it in `shapes::find`, and to `shapes::catalogue`.

That third step is what earns you `cargo run -p shapes -- thing --steps`, and
it also enrols the shape in two tests that run over the whole catalogue: that
it is centred on its origin, and that it is near unit size. If either fails,
placing it would land it somewhere unexpected or dwarf its neighbours.

---

## Things worth knowing

**Sizing then placing is not the same as placing then sizing.** The API keeps
them in the order you would say them out loud: *a smiley, twice as big, over
there.*

```rust
shape.sized(2.0).at(z)   // usually what you want
shape.at(z).sized(2.0)   // also scales the position — occasionally what you want
```

**`Shape::map` is deferred.** `Graph` and `Implicit` shapes cannot be turned
into points until the view is known, because the sampling range *is* the
visible window. So `.map` wraps rather than transforming, and applies at draw
time. One implementation therefore works on every kind of shape.

**Anything can be a `.map`.** The named transforms are one-liners over it:

```rust
.shift(b)              // z ↦ z + b
.scaled(k)             // z ↦ kz
.rotated(θ)            // z ↦ e^{iθ} z
.rotated_about(c, θ)   // z ↦ c + e^{iθ}(z − c)
.affine(a, b)          // z ↦ az + b   — every similarity of the plane, in one line
.map(|z| z.conj())     // a reflection, which none of the above cover
.map(|z| z * z)        // and here you are off the map entirely
```

---

## Where things live

| | |
|---|---|
| `plotkit/` | `Cx`, `View`, `Shape`, `Frame`, `plot`, `pen`, `raster`, `expr`, `script` |
| `shapes/` | this document |
| `shapes/src/bin/shape.rs` | the terminal drawer |
| `studio/src/lib.rs` | `Graph`, `Keys`, and the prelude |
| `studio/src/bin/sketch.rs` | **a blank page — start here** |
| `studio/src/bin/stage.rs` | grouping and motion, in one screen |
| `studio/src/main.rs` | the maths game |
| `studio/src/bin/waves.rs` | adding sine waves |
| `document/HowItWorks.md` | why the library is shaped the way it is |
| `scripts/playground.rec` | the typed-text front door, hot-reloading |
