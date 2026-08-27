# Recursion I — a pulley machine, from complex numbers to dynamics

A two-gear pulley system where **every position is a complex number** and the
motion comes out of an ODE. Zero dependencies, so `cargo test` is the whole
toolchain.

```bash
cargo test                                    # 37 tests - the mathematics, verified
cargo run                                     # tables + writes pulley.svg, pulley_sim.svg
cargo run --example play_complex              # scratchpad: edit the numbers, re-run
cargo run --release --features play --bin play    # interactive window, CPU-rendered
cargo run --features serve --bin serve        # live console at http://127.0.0.1:3000
```

Nothing to install: the maths core is `std`-only, and the two web crates are
behind a feature flag.

## Reading order

| File | What it holds | Read it |
|---|---|---|
| `src/complex.rs` | the number type: `i`, Euler, multiply, divide, `arg` | **first** |
| `src/pulley.rs` | iteration 1 — geometry and the rope constraint | second |
| `src/dynamics.rs` | iteration 2 — the equation of motion and four integrators | third |
| `src/svg.rs` | drawing only | skip while learning |
| `src/main.rs` | the CLI report | last |
| `src/bin/serve.rs` | Axum + Tokio + HTMX console | when you want to poke it |

## Rigid bodies — `src/rigid.rs`

```
cargo run --release --features play --bin bodies
```

**The mathematics is in one file and nothing else is.** `rigid.rs` never
draws, never reads a key, never opens a window — so it can be read and argued
with on its own. `bin/bodies.rs` is only keys-in, pixels-out.

Every body is a disc. Not a limitation of the method — the impulse mathematics
is identical for any shape — it just keeps collision *detection* to two lines
so collision *response* is not buried under it.

**The whole method is one line.** For a contact with normal `n`:

```text
v_n = relative velocity at the contact, along n
K   = inv_m_a + inv_m_b + inv_i_a (ra x n)^2 + inv_i_b (rb x n)^2
j   = -(1 + e) v_n / K
```

`K` is the effective inverse mass *at that contact, along that normal* — the
direct analogue of `M_eff` in the pulley. It grows when the contact is off
centre, because some of the impulse goes into spin instead of translation.

Three places the complex type does real work:

| | |
|---|---|
| `perp(r) = i * r` | a point offset `r` on a body spinning at `omega` moves at `omega * perp(r)` |
| `a.dot(b) = Re(conj(a) * b)` | both plane products fall out of **one** multiplication… |
| `a.cross(b) = Im(conj(a) * b)` | …the dot is its real part, the 2-D cross its imaginary part |
| `gravity * e^(i*tilt)` | tilting the arena is one multiplication |

**Keys:** arrows tilt gravity · ↓ reset it · S spawn · Tab scene · N reseed ·
1–4 solver iterations · C contacts · space pause.

### What the tests pin down

Momentum conserved by every collision; energy conserved at `e = 1` and lost at
`e < 1` **while momentum still is** — that pair is the point. Off-centre hits
create spin, centred hits create none. Stacks stay stacked, resting contacts
do not sink, friction slows a sliding disc and frictionless does not.

And a proper **Newton's cradle**: five touching discs, strike the end, the
striker stops dead, the middle three never move, the far one leaves at exactly
the incoming speed — momentum and energy conserved to machine precision.

Two things I assumed and the tests disproved, both now recorded as tests:

* A single solver iteration does **not** smear the cradle. The impulse
  propagates one contact per *timestep*, so iteration count barely matters
  there.
* Where iterations do matter is **stacks** — many contacts that must hold
  simultaneously against gravity. One sweep sags, forty holds.

## The interactive window — no GPU anywhere

```
cargo run --release --features play --bin play
```

| key | |
|---|---|
| **← / →** (or A / D) | hold to apply torque to the crank |
| space | pause |
| R | reset the preset |
| Tab | next preset |
| 1 2 3 4 | explicit Euler · semi-implicit · Verlet · RK4 |
| − / = | slow down / speed up time |
| Esc | quit |

Every pixel is written by the CPU — `src/raster.rs` is a `Vec<u32>` plus
Bresenham lines, midpoint circles, and a 5×7 bitmap font in 320 bytes. No
shader, no graphics API, no GPU. `minifb` only supplies the window and the
keyboard.

**Two ideas make it work.**

*Input is one term.* Making the machine interactive meant adding
`input_torque` to the equation of motion and changing nothing else:

```
M_eff · θ̈ = gravity − k·θ − c·θ̇ + INPUT
```

*The physics step never varies.* A loop that steps by "however long the last
frame took" behaves differently on different hardware, and one hitch can
tunnel a weight through its end stop. Instead, real time is accumulated and
spent in fixed chunks:

```rust
acc += frame_time * time_scale;
while acc >= DT { sim.step(DT, integrator); acc -= DT; }
```

`--snapshot N` renders one frame of preset N straight to PNG and exits — no
display needed, which makes the renderer checkable on a headless box. The PNG
writer is hand-rolled too (`write_png`): stored deflate blocks, CRC-32 and
Adler-32, about sixty lines. A PNG turns out to be mostly bookkeeping.

## The live console

```
cargo run --features serve --bin serve   ->   http://127.0.0.1:3000
```

**Where the work happens.** A Tokio task advances the simulation on a fixed
16 ms wall-clock tick at a 0.5 ms physics step, whether or not a browser is
attached. HTMX polls `/frame`; the SVG is rendered **server-side in Rust** and
swapped in. There is no physics in JavaScript — no `requestAnimationFrame`, no
client-side state. The Rust process owns the truth and the page is a window
onto it, which is why `curl` sees exactly the same machine:

```bash
curl -X POST localhost:3000/preset/oscillator
curl -X POST localhost:3000/integrator/euler
curl -s localhost:3000/frame | grep -o 'peak drift.*'
```

The polling is **self-terminating**: paused, the fragment comes back with no
`hx-trigger`, so the browser stops asking. Pressing Run returns one that has it.

**Endpoints**

| | |
|---|---|
| `GET /` | the page |
| `GET /frame` | the polled fragment: picture, readouts, transport |
| `POST /toggle` `/reset` `/nudge/{up\|down}` | transport |
| `POST /integrator/{euler\|semi\|verlet\|rk4}` | swap scheme mid-flight |
| `POST /preset/{atwood\|oscillator\|damped\|overdamped}` | jump to a regime |
| `POST /config` | slider changes, `key=value&…` |

**What to do first.** Click *undamped spring*, then cycle the integrators and
watch the **peak drift** readout. Measured live over 6 seconds each:

```
explicit Euler        24.83%      <- manufacturing energy
semi-implicit Euler    0.54%
velocity Verlet        0.00%
Runge-Kutta 4          0.00%
```

Then click through *undamped → underdamped → overdamped* and watch `lambda`:

```
oscillator    -0.000 + 9.737i     decay x rotation
damped        -2.844 + 9.312i     decay x rotation
overdamped    real                rotation gone
```

The imaginary part *vanishing* is what "overdamped" means.

Each file opens with a doc comment that derives its mathematics before any code
appears. The derivations are the point; the code is the check.

## The three places Euler's formula does real work

1. **Gear teeth** are rotated roots of unity — `c + r·e^(i(θ + 2πk/N))`.
   Turning a gear is *adding to the exponent*. There is no rotation matrix in
   this crate.
2. **The tangent offset** is a multiplication by `i`. With equal radii the
   offset angle is `acos(0) = π/2` exactly, so the offset direction *is*
   `i · d̂`. Unequal radii tilt it off 90° by just the right amount — one
   `acos` covers both cases.
3. **Rope paid out** is `r·θ`, an arc length. True only in radians — the same
   reason `e^{iθ}` has period `2π` and not `360`.

And a fourth, in iteration 2: **the solution of the machine's motion** is
`e^{λt}` with `λ = −ζωₙ + iωd`. Decay times rotation. The C1 spiral, as physics.

## What each iteration means

**Iteration 1 — kinematics.** `theta` is *imposed*. The rope constraint

```
L = h₁ + arc_A + straight_run + arc_B + h₂
```

holds exactly (asserted at 121 angles), and cranking trades `h₁` against `h₂`
one for one.

**Iteration 2 — dynamics.** `theta` becomes state. The Lagrangian gives

```
M_eff · θ̈  =  (m₁ − m₂)·g·r_a  −  k·θ  −  c·θ̇
               gravity            spring    damping

M_eff = (m₁ + m₂)·r_a²  +  I_a  +  I_b·(r_a/r_b)²
```

Note the gear ratio enters **squared** — a small fast gear costs far more than
its mass suggests.

## The numerical lesson

Same problem, same step size, four integrators, graded against the closed-form
solution:

```
dt = 0.02, t = 4 s          theta(T)      error      energy drift
explicit Euler             +11.882424     1.18e1    +170970.57%   <- exploded
semi-implicit Euler         +0.065356     6.14e-2        -3.07%   symplectic
velocity Verlet             +0.103120     2.36e-2        -0.89%   symplectic
Runge-Kutta 4               +0.126933     1.65e-4        -0.02%
EXACT                       +0.126768
```

Over 200 seconds, explicit Euler ends with **600,000×** its starting energy.
Semi-implicit Euler ends with 1.006×.

**The two schemes differ by the order of two lines:**

```rust
// explicit — both updates read the OLD state
self.theta = th + w * dt;
self.omega = w + a * dt;

// semi-implicit — velocity first, then position uses the NEW velocity
self.omega += a * dt;
self.theta += self.omega * dt;
```

Same cost, same line count. One manufactures energy from nothing; the other is
stable forever. This is why game engines use Verlet and not the obvious thing.

## A note on units

Lengths are in *drawing units* (the gears have radius ~64), while masses are in
kg and `g = 9.81`. The system is internally consistent but not physically
scaled — treating a 64-unit radius as 64 metres makes `M_eff` large and the
machine slow. Iteration 3 should introduce an explicit metres-per-unit scale.

## What is still missing

| | Add | Where it goes |
|---|---|---|
| iteration 3 | rope stretch — the rope becomes a spring, so the system gains a second degree of freedom | C7 |
| | Coulomb (dry) friction and stiction — no longer a smooth ODE | C7 |
| | proper unit scaling | here |
| iteration 4 | many gears, chains, a general constraint solver | engine track |
| | port the renderer to `macroquad` for real-time interaction | engine track |

The physics core needs no changes for a `macroquad` port — `dynamics.rs` and
`pulley.rs` already know nothing about drawing.
