# Recursion I — a pulley machine, from complex numbers to dynamics

A two-gear pulley system where **every position is a complex number** and the
motion comes out of an ODE. Zero dependencies, so `cargo test` is the whole
toolchain.

```bash
cargo test                                     # 143 tests - the mathematics, verified
cargo run                                      # tables + pulley.svg, pulley_sim.svg
cargo run --example play_complex               # complex-number scratchpad

cargo run --release --features window --bin play     # the pulley, crank it
cargo run --release --features window --bin bodies   # rigid bodies
cargo run --release --features window --bin cloth    # cloth, rope, soft bodies
cargo run --release --features window --bin fluid    # SPH fluid
cargo run --release --features window --bin spin3d   # 3D rotation, quaternions
cargo run --release --features window --bin render   # lit, depth-buffered 3D
cargo run --release --features window --bin pca      # eigenvectors and PCA
cargo run --features serve --bin serve         # browser console on :3000
```

Every windowed binary also takes `--snapshot N [seconds]`, which renders one
frame to PNG and exits — no display required.

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

## Eigenvectors and PCA — `src/eigen.rs`

```
cargo run --release --features window --bin pca
```

A matrix moves every vector somewhere else, usually pointing a different way.
A few special directions come out pointing exactly as they went in, merely
stretched: `A v = lambda v`. Those are the transformation's own natural axes.

**You have met the eigenvalue rule three times already.** `A^n v = lambda^n v`,
so `|lambda|` decides the long run — under 1 decays, over 1 explodes, exactly 1
persists. That is the same `|z|` from `complex.rs`, the same `lambda` from the
damped oscillator in `dynamics.rs`, the same criterion that decides whether
explicit Euler gains energy. And it is not an analogy: write the damped
oscillator as a 2x2 system and its `lambda` *is* an eigenvalue. **Complex
eigenvalues mean rotation.**

**Symmetric matrices are the good case** — real eigenvalues, perpendicular
eigenvectors (the spectral theorem). And the two symmetric matrices worth
knowing are both already in this crate:

* the **inertia tensor** from `body3.rs` — eigenvectors are the principal axes
  Euler's equations are written in, eigenvalues are the moments of inertia;
* the **covariance matrix** of a point cloud — eigenvectors are the directions
  the data spreads along.

Same theorem. One tells a tumbling box which way it can spin cleanly; the other
tells a dataset which of its features are really one feature. There is a test
that decomposes a rotated inertia tensor and recovers both.

PCA is then three lines: centre, covariance, eigen-decompose. Two algorithms
are implemented — **power iteration** (multiply and renormalise; the dominant
eigenvalue wins by `(l1/l2)^n`, which is the C1 spiral in a space of
directions) and **Jacobi rotations** (zero the largest off-diagonal entry,
repeat — Gauss-Seidel again, exactly as in `rigid.rs` and `soft.rs`).

**The graphics payoff, measured:** on a tilted slab the oriented bounding box
built from the eigenvectors is **7x smaller** than the axis-aligned one, and a
single axis explains 91% of the variance. The demo also shows the honest
failure case — a ring, whose structure is real but is *not a direction*, so
PCA has nothing useful to say about it.

## The software renderer — `src/render3.rs`

```
cargo run --release --features window --bin render
```

Filled, depth-buffered, lit triangles. Still a `Vec<u32>` written by the CPU;
no GPU, no shader language, no graphics API.

**Perspective is a division.** That is the whole of it — things far away look
small because you divide by how far away they are. Everything awkward that
follows (clipping, the depth buffer, perspective-correct interpolation) is a
consequence of that one division.

**Which pixels are inside a triangle?** The *edge function*:

```text
E(a, b, p) = (b - a) x (p - a)
```

Positive one side, negative the other, zero on the line — a point is inside
when all three agree. That is the same scalar cross product from `complex.rs`
(`Im(conj(a)·b)`, the signed area) doing an entirely different job. Divide the
three by their sum and they become **barycentric coordinates**, the weights
that blend attributes across the face. One primitive gives you the inside
test, the interpolation, *and* backface culling — the sign of the total area
is the winding.

### The three toggles are the lesson

**P — perspective-correct interpolation.** Screen space is not linear in world
space, so what varies linearly across the screen is `1/z`:

```text
WRONG:  attr = w0*a0 + w1*a1 + w2*a2
RIGHT:  (w0*a0/z0 + w1*a1/z1 + w2*a2/z2) / (w0/z0 + w1/z1 + w2/z2)
```

Turn it off and the floor checker shears along each quad's diagonal — the
artefact in every PlayStation 1 game, whose hardware genuinely could not
afford the divide. A test measures the error on a receding edge.

**Z — the depth buffer.** Painting far-to-near fails the moment two objects
interpenetrate. Keep a depth per *pixel* instead and the result stops
depending on submission order at all — there is a test that renders the same
two triangles in both orders and demands identical buffers.

**C — backface culling.** Roughly half of every closed mesh faces away.

### Two sign bugs, both found by looking, both now pinned

* **The camera basis was mirrored.** `forward × up` gives `−right`, so the
  whole image was flipped and every winding reversed — culling then kept
  exactly the wrong half of every mesh. It is `up × forward`.
* **The culling test was itself wrong.** I built a "front-facing" triangle
  whose assigned normal said `−Z` while its winding said `+Z`, and the test
  cheerfully enshrined the wrong sign. The floor vanished. The test now
  *derives* the normal from the winding and asserts it faces the camera.

The second is the more useful lesson: a test can be confidently green and
still be measuring the wrong thing.

## Three dimensions — `src/quat.rs`, `src/body3.rs`, `src/vec3.rs`

```
cargo run --release --features window --bin spin3d
```

This is the sequel to `complex.rs`, and the reason the project started with
complex numbers rather than vectors.

A complex number rotates the **plane** in one multiplication. So what rotates
**space**? The obvious answer — three angles — is wrong, and not obviously
wrong until it bites. Hamilton spent thirteen years looking for a three-number
system that multiplies properly. There isn't one. In 1843 he realised you need
**four**, and that the multiplication cannot commute:

```text
i^2 = j^2 = k^2 = i j k = -1
```

Everything else follows, including `ij = k` but `ji = -k`. **Order matters** —
and it has to, because rotating about x then y genuinely differs from y then x.
Non-commutativity isn't an inconvenience of the algebra; it is the physics
being reported accurately.

```text
q  = cos(theta/2) + sin(theta/2) * axis      note the HALF angle
v' = q v q*                                  the sandwich product
```

The half angle is not a convention you could drop: the vector is multiplied on
*both* sides, so the rotation lands twice and the half compensates.

| | plane | space |
|---|---|---|
| rotate | `z' = e^(i t) z` | `v' = q v q*` |
| numbers | 2 | 4 |
| commutes | yes | **no** |
| stored angle | `theta` | `theta/2` |
| degenerates | never | never (Euler angles do) |

Restrict a quaternion to `w + z k` and you have `complex.rs` back, doing
exactly what it did. A complex number is a quaternion that has only heard of
one axis — there is a test that says so.

### Three things the demo shows

**Gimbal lock**, drawn as the three Euler axes really are. At pitch 90° the
yaw and roll axes become the *same line* — `YAW . ROLL = 0.9998` — so three
knobs reach only a two-dimensional set of orientations. Apollo 11's inertial
platform had the same problem in hardware.

**The Dzhanibekov effect.** A free box spun about its **middle** moment of
inertia flips end over end, forever, with no torque. Spun about the largest or
smallest it is stable. The cause is the gyroscopic term in Euler's equations:

```text
I omega_dot = torque - omega x (I omega)
```

Start at `omega = (0.03, 5.0, 0.03)` and 3.4 s later it reads
`(+0.06, -5.00, +0.07)` — completely inverted, while the angular momentum
arrow has not moved and the energy is unchanged. Cosmonaut Dzhanibekov filmed
a wingnut doing this in 1985 and the footage was classified for a decade.

**Slerp against lerp.** Same endpoints, same time, visibly different speed
through the middle — and slerp knows to take the short way round the double
cover.

### The double cover

`q` and `-q` are the *same rotation* (both signs cancel in the sandwich), so a
full 360° turn leaves the quaternion at `-1`, and **720°** is needed to come
home. That is not notation: it is why an electron must be turned twice to
return to itself, and why you can untwist your arm by rotating a held glass
through two full turns but not one. Try it.

## SPH fluid — `src/fluid.rs` + `src/grid.rs`

```
cargo run --release --features window --bin fluid     # release matters: ~20x
```

Fluid dynamics is written for **fields** — density and pressure defined at
every point. Particles are not a field, they are a scatter of dots. SPH is the
bridge:

> **Read a field off a scatter of particles by smearing each one into a soft
> blob and adding up the overlaps.**

That blob is the **kernel** `W(r,h)`: a bump, peaked at the particle, exactly
zero past `h`. Every field becomes a weighted sum over neighbours, and every
*derivative* becomes a sum over the derivative of the kernel — which is known
analytically. Calculus on a point cloud, with no mesh.

```text
rho_i = sum_j m_j W(|r_i - r_j|, h)              density
p_i   = k (rho_i - rho_0)   clamped at zero      pressure
f_press = -sum_j m_j (p_i + p_j)/(2 rho_j) grad W_spiky
f_visc  =  mu sum_j m_j (v_j - v_i)/rho_j  lap  W_visc
```

The pressure term is **symmetrised** (`(p_i + p_j)/2`) so that j pushes i
exactly as hard as i pushes j. An unsymmetric pair invents momentum from
nothing; there is a test that it does not.

**Three different kernels, and it is not fussiness:**

| kernel | for | why |
|---|---|---|
| poly6 | density | smooth, and a function of `r^2` so no square root |
| spiky | pressure | poly6's gradient **vanishes** at `r=0`, so coincident particles feel no push apart and clump. Spiky's is strongest there. |
| viscosity | the Laplacian | poly6's Laplacian goes negative near `h`, *adding* energy |

Each is chosen for how a *derivative* of it behaves. Get it wrong and you do
not get a slightly worse fluid, you get clumping or a detonation.

### `grid.rs` — the spatial hash

Every sum above is "over j within h", so SPH lives or dies on neighbour search.
Chop space into cells the size of the interaction radius; anything close enough
to matter is in your cell or the eight around it. Cost per particle then tracks
local **density**, not population — O(n) instead of O(n²).

One structure, three payoffs: neighbours for `fluid.rs`, a broadphase for
`rigid.rs` (still brute force today), and eventually self-collision for
`soft.rs`.

### Two bugs this rung produced, both now tests

* **`stable_dt` used the wrong sound speed.** With `p = k(rho - rho_0)` the
  wave speed is `sqrt(k)` — it does *not* involve density. My version returned
  **2.6 seconds** instead of ~1.4 ms, so the fluid detonated on the first step
  while the diagnostic said everything was fine.
* **Stiffness cannot be guessed.** It has to hold up a column of fluid:
  `k = g·depth/squash`, and the rest density cancels. My initial guess was off
  by four orders of magnitude. `tune_stiffness(depth, squash)` now derives it.

Also: the hash used to panic on overflow when the fluid exploded, so the *hash*
reported the failure instead of the fluid. It now clamps and files the rubbish,
leaving the caller's own diagnostics to say what really went wrong.

## Cloth, rope and soft bodies — `src/soft.rs`

```
cargo run --release --features window --bin cloth
```

Left-drag grabs a node, **right-drag is a knife**, arrows tilt gravity.

Everything so far has been *force-based*: forces → acceleration → velocity →
position. Cloth breaks that. A sheet is thousands of stiff springs, and stiff
springs need a tiny timestep or they explode — exactly as explicit Euler did on
the pulley.

So invert it. Work **directly with positions**, and describe the fabric not as
forces but as **constraints**: *"these two particles are 10 apart."* Move them
until it is true. No spring constant, nothing to blow up.

**Verlet — where the velocity went.** Store the *previous* position instead of
a velocity:

```text
next = p + (p - prev) * damping + a * dt^2
```

`(p - prev)` **is** the velocity. It is never stored, only inferred, and that
one choice carries the method:

> **If you move a particle, you have changed its velocity.**

A constraint that yanks a node sideways gives it sideways momentum for free.
Collisions become "put it back outside the wall" and the bounce falls out.
Dragging with the mouse genuinely *throws* the fabric.

**The distance constraint** — the entire physics of cloth:

```text
corr = d/dist * (dist - L) * k / (wa + wb)
pa  += corr * wa          pb -= corr * wb
```

Split by **inverse** mass, so a pinned node (`w = 0`) never moves and its
partner takes the whole correction. Sweep the list repeatedly — the same
Gauss-Seidel loop as the contact solver in `rigid.rs`. Stiffness is corrected
by `k' = 1 - (1-k)^(1/n)` so iterations buy *accuracy*, not stiffness.

**Which pairs you link is the material:** structural (N/S/E/W) holds length but
shears freely; diagonals resist shearing; skip-a-neighbour resists folding.

### Three things I got wrong, all now tests

* **Damping was per *step*.** A plausible-looking `0.995` at 600 Hz leaves
  `0.995^600 = 0.05` of the motion — cloth fell at a fifth of the right speed,
  suspended in treacle. It is now per *second* via `damping^dt`, which also
  makes it step-size independent. Two tests pin it.
* **A 2-D sheet cannot drape.** Draping is fabric buckling *out of plane*, and
  a flat world has none. Fully triangulated at full stiffness, a grid is a
  rigid truss that tumbles like a dinner tray. Hence soft diagonals — and hence
  the honest 2-D analogue of cloth-over-a-sphere being a **slack rope** over
  one, which produces a proper catenary at 0.5% strain.
* **There is no self-collision.** Blobs fall straight through the rope. That is
  the genuinely expensive half of cloth simulation (spatial hashing,
  particle-versus-edge, continuous detection) and it is not here.

## Rigid bodies — `src/rigid.rs`

```
cargo run --release --features window --bin bodies
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
cargo run --release --features window --bin play
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
