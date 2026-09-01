# How It Works

> **The goal.** A piece of paper I can compute on. I want to write down a
> theorem the way I would by hand — *let R be rotation by θ; apply R to the
> square* — and have the picture appear, with no drawing code anywhere in the
> maths. Something as elegant on screen as the derivative of a sigmoid is on
> paper.
>
> Phase 1 is the plane. Phase 2 is the natural extension to 3D and quaternions.

---

## The one rule

**Maths goes in. Pictures come out. The maths never mentions pixels.**

Everything below exists to hold that line. If a function that computes
something also has to know a colour or a screen size, the line has broken.

---

## The layers

```
  YOUR FILE          pure maths + a scene description        <- you write this
      |
  shape              geometry as VALUES you can .map()
  frame              a layer of styled shapes
      |
  plot / pen         curves, graph paper, dimensions         (world units)
      |
  view               world -> screen. origin anywhere, y counts UP
      |
  raster             pixels. origin top-left, y counts DOWN
```

`view` is the only place the two coordinate systems meet. Above it everything
is "a circle of radius 1 at the origin". Below it everything is a `u32` in a
`Vec`. Nothing skips a layer.

`expr` and `script` sit off to the side: they turn *typed text* into the same
shapes, which is what `scripts/playground.rec` uses. Same pictures, different
front door.

---

## 1. The paper — `View`

Your original `RenderGraph()` is `View`. It is four numbers and a mapping:

```rust
let v = View::centred(1100, 700, 52.0);   // 52 pixels per unit, origin in the middle
```

```
    screen_x = origin_x + scale * world_x
    screen_y = origin_y - scale * world_y        <- the minus is the whole trick
```

That minus sign is why `y` counts up for you and down for the framebuffer.
It is written once, here, and never again.

**Zoom.** You wanted each unit to fold into 10 on zoom-in. Pure ×10 subdivision
*pops*: at some zoom the lines are 4 px apart and unreadable, one notch later
they are 40 px apart and sparse. Real graph paper uses the **1-2-5 sequence** —
1, 2, 5, 10, 20, 50, 100 — so the spacing never strays far from comfortable:

```rust
plot::nice_step(scale, target_px)   // picks 1, 2 or 5 x a power of ten
```

Your ×10 instinct survives where it belongs: minor lines are the major step
divided by 5, so you still get the fold-into-subsections feel, but the *labelled*
lines stay legible at every zoom.

**Two views on one canvas is legal and useful.** A view is just a mapping — nothing
stops you from making one for the phasor panel and another for the wave strip
and drawing both onto the same pixels. `studio/src/bin/waves.rs` does not need
this, but the option is there.

---

## 2. Geometry as values — `Shape`

This is the correction that mattered most, and it comes straight from your own
sentence: *"as devoid of the render logic as possible and be pure math."*

On paper you do not say **draw a rotated square**. You say *let R be rotation
by θ*, and then *apply R to the square*. Two things: a shape, and a map. So
the API has exactly those two things:

```rust
let sq = Shape::unit_square();
let r  = Cx::expi(theta);
let turned = sq.map(move |z| r * z);      // apply R to the square
```

Not `draw_rotated_square(...)`. The rotation is a **value** — an ordinary
`Cx` — and `.map` is function application. Composition is composition:

```rust
shape.map(f).map(g)       // g ∘ f, and it reads left to right like paper
```

The convenience names are all one-liners over `map`, and their bodies *are*
their definitions:

| written | means |
|---|---|
| `.shift(b)` | `z ↦ z + b` |
| `.scaled(k)` | `z ↦ kz` |
| `.rotated(θ)` | `z ↦ e^{iθ} z` |
| `.rotated_about(c, θ)` | `z ↦ c + e^{iθ}(z − c)` |
| `.affine(a, b)` | `z ↦ az + b` — *every* similarity of the plane, in one line |

`.affine(a, b)` is worth staring at. One complex multiply and one complex add
covers rotation, uniform scale, translation and any composition of them. A
2×2-matrix-and-a-vector version of this library would need three types and a
page of code to say the same thing.

### The forms a shape can take

There is **one** curve concept, with three ways to pin it down — the three ways
a curve is ever specified in a maths text:

```rust
Shape::param(|t| Cx::expi(t), 0.0, TAU, 512)   // parametric   z = γ(t)
Shape::graph(|x| x.sin())                      // explicit     y = f(x)
Shape::implicit(|x, y| x*x + y*y, 1.0)         // implicit     F(x,y) = c
```

plus `points`, `path`, `polygon`, `group`.

> **`Sin` and `Cos` are not plot types.** Your draft had `Sin(...)` and
> `Cos(...)` as separate kinds of plot. That road has no end — you would need
> `Tan`, `Exp`, `Ln`, `Sinh`, a new type per named function forever, and none
> of them would compose. `Shape::graph(f)` covers all of them and every function
> nobody has named yet.

### Where `+` actually belongs

Your line

```
Sin(Cx1) + Sin(Cx2) + Cos(Cx1) + Cos(Cx2)
```

is the single best instinct in the original document, and it is pointing at
something real. But the `+` does not belong on *plots*. It belongs on
**functions**:

```rust
let f = |x: f64| 1.0*(x).sin() + 0.5*(3.0*x).sin() + 0.33*(5.0*x).sin();
frame.add(Shape::graph(f));
```

Adding two *plots* would only mean "draw both". Adding two *functions* makes a
third function that neither one is — and that is the Fourier series. Sum
enough sines and you get a square wave, a sawtooth, the outline of a duck, or
the digit **7**. The game in `studio/src/main.rs` is built on precisely this,
which is the proof the correction was the right one.

`studio/src/bin/waves.rs` is the small version: two sines, their sum, and the
phasor picture that explains why the sum comes out the way it does.

### `.map` is deferred, and that is deliberate

`Shape::graph` and `Shape::implicit` cannot be turned into points until the
view is known — the sampling range *is* the visible window. So `.map` does not
transform anything when you call it. It wraps:

```rust
Shape::Mapped(Box<Shape>, Arc<dyn Fn(Cx) -> Cx>)
```

and applies at draw time, after the range is known. One `map` implementation
therefore works on every shape, view-dependent or not. `Implicit` marches its
grid in the *original* space and transforms the resulting segments, so
`.rotated(0.3)` on an implicit curve rotates the curve rather than rotating the
grid and producing a staircase.

---

## 3. A layer of shapes — `Frame`

```rust
let mut f = Frame::new();
f.add(Shape::circle(Cx::ZERO, 1.0));
f.add(turned).color(0xE0A44A).width(3);
f.label(Cx::new(1.1, 0.0), "R(square)", 0x9AA7B4, 2);
```

`add` returns the style so it can be adjusted where the shape is added, and
takes the next palette colour if you say nothing — so a scene of bare `add`
calls is already readable.

> **A `Frame` is a *layer*, not a time step.** Your draft had `RunFrame(Frame...)`
> taking a list of frames to play through. Make `Frame` a moment in time and
> you need a second concept for "things drawn together", and a third for
> scrubbing. Make it a layer instead and animation is just a function:
>
> ```rust
> fn scene(t: f64) -> Frame { ... }
> ```
>
> A still picture is `scene(0.0)`. A film is `scene(t)` in a loop. Scrubbing,
> stepping, exporting a PNG at any instant — all free, no new API.
>
> And `RunFrame` did come back, once there was enough evidence for what it
> should be: [`studio::Graph`] owns the canvas, the view, the window and the
> loop, so `Graph::new("x").animate(scene)` is the whole program. It is a
> *convenience over* `f(t) -> Frame`, not a replacement for it — which is why
> `Graph::png` and `Graph::print` can run the same scene with no window at all.

`merge` lets a scene be built out of parts that each know their own colours,
so `fn digit(d, at) -> Frame` and `fn sticks(n, at) -> Frame` compose without
either knowing about the other.

---

## 4. What you write

A studio file is two things and nothing else:

```rust
// ---- THE MATHEMATICS ------------------------------------------------
// plain functions. no Canvas, no View, no colours. testable on their own.
fn fourier(coeffs: &[(i32, Cx)], t: f64) -> Cx { ... }

// ---- THE SCENE ------------------------------------------------------
fn scene(t: f64, state: &State) -> Frame { ... }

// ---- THE WINDOW -----------------------------------------------------
// twenty boring lines. the same twenty in every file.
```

The top section is the paper. If you deleted the window and kept the maths,
nothing of value would be lost — the functions would still be correct and
still be testable. That is the test for whether the line has held.

---

## 5. Phase 2 — 3D and quaternions

> *"My visual intuition tells me quaternions are just stacking the same 2D
> space in an additional space."*

Half right, and the half that is wrong is the interesting half.

Right: **ℍ contains ℂ.** Fix any unit imaginary direction — `i`, or `j`, or
`(i+j+k)/√3` — and the span of `{1, that}` is a copy of the complex plane,
with the same `e^{iθ} = cos θ + i sin θ`. Every rotation is a rotation *in some
plane*, and inside that plane it is the 2D story you already know. All the
work on `Cx` transfers directly.

Wrong: **the planes are not independent.** `ij = k` but `ji = −k`. Turning in
the `i`-plane changes what the `j`-plane means. That non-commutativity *is* the
coupling between the planes — it is the content, not an inconvenience.
Genuinely independent stacked planes would be Euler angles, and Euler angles
gimbal-lock: at 90° pitch two of the three axes collapse onto each other and a
degree of freedom vanishes. Quaternions do not, and the reason they do not is
exactly the thing your intuition currently subtracts.

The other half-turn to expect: the sandwich `q v q*` applies rotation by θ from
a quaternion built with **θ/2**. Two units map to each rotation, `q` and `−q`.
The double cover you already met chasing `e^{i2π} = 1` in the plane shows up
again here, and it is the same phenomenon.

Concretely the extension is small, because the design above does not care about
dimension:

- `Shape` becomes generic over its point type, or gains a `Shape3`.
- `.map` needs no change at all — it was always "apply this function".
- `View` gains a projection step: world → camera → clip → screen.
- `.affine(a, b)` becomes `q v q* · s + b`.
- `Frame` needs a depth sort or the z-buffer already in `render3.rs`.

Nothing in the maths files changes, because none of them ever knew there was a
screen.

---

## Where things are

| | |
|---|---|
| `plotkit/` | the library. zero dependencies. `Cx`, `View`, `Shape`, `Frame`, `plot`, `pen`, `raster`, `expr`, `script` |
| `shapes/` | things to draw — digits, faces, tallies, glyphs, waves. depends on `plotkit` only, so it has no window and can draw in a terminal. **[Shapes.md](Shapes.md) is the cookbook** |
| `studio/` | the applications. depends on `shapes` + a window, and holds no geometry at all |
| `shapes/src/bin/shape.rs` | `cargo run -p shapes -- seven --steps` — any shape drawn in the terminal, one construction line at a time |
| `studio/src/bin/sketch.rs` | **a blank page.** `Graph` owns the window and the loop, so a sketch is a scene function and nothing else |
| `studio/src/bin/waves.rs` | any number of sines and their sum, with the phasors that explain it |
| `studio/src/main.rs` | the maths game — digits drawn as sums of sine waves |
| `scripts/playground.rec` | the typed-text front door. hot-reloads on save |
| `src/` | the physics ladder — rigid bodies, cloth, fluid, quaternions, rasteriser |
