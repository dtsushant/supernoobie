# The studio: a cookbook

Everything here was read off the code rather than remembered, and every claim
in it that can be checked **is** checked — `easel/tests/cookbook.rs` fails the
day one stops being true. A document that drifts from the code is worse than no
document: it is wrong with authority.

Writing it found three things I had stated wrongly and one genuine bug, all
noted in place below.

```text
cargo run -p studio --release --bin draw -- samples/dials.easel     a window
cargo run -p web     --release --          samples/dials.easel     a browser
cargo run -p studio --bin sample                                    writes samples/
```

---

## 1. Files

### What opens

| | |
|---|---|
| `something.easel` | a whole drawing — strokes, keys, groups, functions |
| `something.rec` | a plain script, one statement to a line |

A `.rec` is **imported**, not opened: its lines come in as rows and saving goes
to a `.easel` beside it. A script you are borrowing from is never overwritten.

There is no "save as". One window, one file, named on the command line; `save`
and `open` both use it. For a second drawing, start a second window.

### What a `.easel` file looks like

Text, and meant to be read. This is a whole file:

```text
easel 1
row on # move r and everything below follows
row on r = 3
row on circle(0, r)
row off ngon(0, r, 6)
mark quill 0.1400 0.0200 0.1600 taper 0.1200 colour E0A44A fill closed
group 1
p -1.6000 3.2000 -1.5000 3.3000 -1.4000 3.4000
p -1.3000 3.4000
track loop
key 0.0000 1.00000 0.00000 0.0000 0.0000 smooth
key 1.5000 1.00000 0.00000 2.6000 1.6000 smooth
act loop
do walk 1.6000 0.0000 2.0000
```

Line by line:

| line | means |
|---|---|
| `easel 1` | the format's version. First word, always. |
| `row on <text>` | one line of script. `off` keeps it but does not run it. |
| `mark <nib> taper T colour HEX fill\|line closed\|open` | a stroke begins |
| `group N` | which figure that stroke belongs to. `0`, or absent, is none. |
| `p x y x y …` | its points, eight to a line |
| `track loop\|once` | it has keyframes, and whether they repeat |
| `key t aRe aIm bRe bIm ease` | a pose at a moment. `ease` is `smooth`, `linear` or `hold`. |
| `act loop\|once` | it has verbs |
| `do <verb> n0 n1 seconds` | one verb. `seconds` may be `inf`. |

Nibs are written `round W`, `quill SLOW FAST PACE`, or `broad WIDTH ANGLE`.

A pose is `z ↦ az + b`: `aRe aIm` is the complex `a`, which carries turn and
scale together; `bRe bIm` is where it is moved to.

### What happens to a file it cannot read

Nothing is fatal.

* A line it does not understand is **skipped and counted**. The count is
  reported; nine good marks are not thrown away because the tenth has a typo.
* A **field** it does not know is stepped over with its value, so a file
  written by a later version still opens.
* A `key` with no `ease` named gets `smooth`, rather than being lost.

### Editing one by hand

Reasonable, and the format is arranged for it: the script rows come first, so
you see what a drawing is *made of* before several hundred lines of points.
Two rules —

* `p` lines belong to the `mark` above them. Move a `mark` line and its points
  must go with it.
* Points are written to four decimal places. That is the format's promise, and
  at the scale a hand draws at a ten-thousandth is far below a pixel.

---

## 2. The script

### Every value is a complex number

There is no vector type and no `(x, y)`. A point, an offset and a plain number
are the same kind of thing, so:

```text
a + b       move                    a * b       scale AND turn
2z          twice as far out        z * i       a quarter turn
a*z + b     any similarity, written exactly as it reads
```

### Rows

The script is a **list of rows**, joined into one program before running — so a
name bound in row 2 is available in row 9. A row switched off becomes a blank
line rather than disappearing, so error messages keep pointing at the row you
can see.

A row of the form `name = value` binds a name, and if the value is a plain real
number it gets a **slider**. Moving the slider rewrites the row: the text is the
only truth.

### Names that already exist

```text
i   pi   tau   e        the usual
time                    the studio's clock, in seconds
x                       inside plot(...)     — real
y                       inside implicit(...) — real
t                       inside param(...)    — real
```

`time` is bound by writing a line into the source, not by a special case, so it
behaves like any other name. It is deliberately **not** offered as a slider: it
has a clock of its own.

### Commands

| | arguments | |
|---|---|---|
| `point(a, …)` | 1+ | marks |
| `line(a, b, …)` | 2+ | an open path |
| `polygon(a, b, …)` | 2+ | a closed path — two points give a straight line |
| `circle(centre, r)` | 2 | parametric, so it stays smooth at any zoom |
| `ngon(centre, r, n)` | 3 | `n` is 3…512 |
| `plot(f)` | 1 | `y = f(x)` |
| `param(f, t0, t1)` | 3 | `t ↦ z(t)` |
| `implicit(F, level)` | 1–2 | `F(x, y) = level`; `level` defaults to 0 |
| `color(n)` | 1 | pins the colour from there on. `0xE0A44A` works. |

There is no `if` **statement** and no loop. `if` is a function that gives back
a value, which is all an expression language needs and is what keeps a row a
row.

And four the studio adds on top, which `plotkit::expr` has never heard of:

```text
digits(value, x, y, size)        a number, written out
smiley(x, y, size)               a face
ghost(x, y, size)                the other face
ludo()                           the board
token(seat, step, size)          a token on it. a NEGATIVE step is the yard
when tap N: name = expr, …       a rule set off by a tap on figure N
when question: name = expr, …    a rule set off when something becomes true
```

A rule waits on a **gesture** or on a **question**:

```text
when tap 2: score = score + 1, cheer = time
when die == 6: again = 1
```

**Became true, not is true.** A rule that fired while its question held would
fire sixty times a second, and `turn = turn + 1` would run a game to the end of
time inside one frame.

One rule can make another's question true, and the answer does not wait for the
next frame — but the asking is **bounded** at eight passes, because two rules
can each make the other true and that is a loop nobody would find by reading.

`when tap` with no figure is refused rather than read as a question about a
variable named `tap`: it is nearly always `when tap 3:` typed wrong, and a rule
that quietly never fires is the worst way to learn that.

### Watching versus editing

While a drawing is being **watched** — the `play` link, or `~` in the window —
the pen taps and does nothing else. No ink, no rubbing out, no moving, no
choosing. A drawing with no tools on screen invites a hand, and a hand that
leaves a line through it is the first thing anybody does.

A face is drawn **at a size**, so a size of nothing is a face that is not
there. That is the whole of showing and hiding one:

```text
smiley(3.4, 1.5, max(0, 1.0 - 1.4*(time - cheer)))
```

### Asking questions

Every value is a number, so a question is answered with `1` or `0` — which
means an answer is arithmetic:

```text
a = 5 + 10*(die == 6)          15 on a six, 5 otherwise
```

| | |
|---|---|
| `==` `!=` | equal, with a hair of slack — `0.1 + 0.2 == 0.3` is **true** |
| `<` `<=` `>` `>=` | on the **real part**, since complex numbers are not ordered |
| `and(a,b)` `or(a,b)` `not(a)` | anything away from zero is true |
| `if(question, yes, no)` | |
| `pick(k, v0, v1, …)` | the `k`-th value, chosen without working out the rest |

Comparison binds **looser** than `+`, so `a + b == c` asks what it looks like
it asks. Two in a row (`a < b < c`) are refused rather than quietly read as
`(a < b) < c`, which is a thing people write and almost never mean.

**`if`, `and`, `or` and `pick` are decided before their arguments are worked
out.** That is not an optimisation, it is the point:

```text
if(x == 0, 0, 1/x)             never divides by nothing
and(0, ln(0))                  never takes the log of nothing
pick(0, 5, ln(0))              only the one chosen is worked out
```

### Names worked out

`at[k]` is the variable **named** `at` followed by the number `k`. It is
spelling, not an array — there is no second kind of value anywhere in this
language, and adding one would mean two of everything: two ways to bind, two
ways to save, two ways to be wrong.

```text
at0 = 5
at1 = 9
k = 1
a = at[k]                      9
when tap 1: at[k] = -1         and a rule can write to one
```

The subscript is worked out **when the deed happens**, against everything the
deeds before it have already done — so `k = k + 1, at[k] = 0` writes to the
*new* `k`, which is what reading it left to right says.

`at[-1]` is the name `at-1`, which nobody can type by accident, so a negative
subscript cannot collide with a name you meant.

This is what lets a rule say *"send home whichever token is on this square"*.
Without it a rule can only change a number it already knows the name of, and a
game cannot be written.

### Saying how big a drawing is

```text
bounds(-9.5, -7.7, 9.5, 7.7)
```

A drawing that says this is **fitted to the window and left there** — no wheel,
no drag. A board has edges and is meant to be seen whole; scrolling a Ludo board
away is a way to lose it, not a way to look at it.

A drawing that says nothing goes on for ever and keeps both. That is the right
default for a graph, a plot or a sketch, which is most of what this is for — so
the bounded case is the one that has to declare itself.

It draws nothing, and the four numbers may be worked out like anything else in a
row. A box with no width, or one whose corners are the wrong way round, is
refused rather than believed: dividing by a width of nothing gives an empty
screen, which looks like the drawing having failed to load.

### House rules a drawing declares

A row ending `# rule: <words>` is a **house rule** — a number the players settle
before they start:

```text
opens = 6      # rule: what brings a token out
again6 = 1     # rule: a six earns another turn
mustcut = 0    # rule: no way home until you have cut somebody
```

Nothing in the studio knows what any particular game is. A house rule was
already just a number in a row; the comment only adds *"and I am one of the ones
worth asking about before we start"*, together with words a person can read. Any
game gets a setup screen by writing it.

If the page shows the setup screen and the start button does nothing, the cause
is almost certainly an ID selector: `#setup { display: grid }` beats the
browser's own `[hidden] { display: none }`, so setting `hidden` has no effect at
all. `#setup[hidden] { display: none }` puts it back.

A rule that is 0 or 1 is offered as a tick box and anything else as a number,
which is the only distinction a setup screen needs. Setting one rewrites the row
and **keeps the comment** — otherwise setting a rule would delete it from the
screen, which is the one thing guaranteed to happen to it.

### Where a Ludo seat starts

`plotkit::ludo::OFF` is **two**, and it has to be. A seat's home column runs
outward-to-inward along the middle row of its arm, and the outer square in line
with it is the last square of the *track* — the one you turn in from. Start a
seat there and its start square sits in the mouth of its own home column, which
is what it looked like and is not where any board has it.

Two squares on puts the start on the row beside, and leaves the last track
square directly next to the door. One or three land it diagonally opposite
instead, a gap of √2 — a token stepping into its home column through a corner.

### A star

```text
star(x, y, size)             the usual five-pointed one
star(x, y, size, 6)          six points
star(x, y, size, 6, 0.5)     …and a fatter waist
star(x, y, size, 6, 0.5, t)  …turning
```

Only the count is usually worth saying, so it is the first thing after the
size. The waist is `inner/outer` and is the whole character of it: about 0.4 is
a star, much above 0.6 is a cog, much below 0.25 is a set of needles that
vanish when small. A star takes the colour of the row, like `param` and
`circle`.

### A die

A thrown die is not a random number with a picture on it, and it is not
written in rows. It lives in `plotkit::dice`, so any drawing can have one:

```text
span = 6.4
age  = max(0, time - flung)
die     = dieface(seed, rolls, age, span)
settled = diedone(seed, rolls, age, span)
dice(die, diex(seed, rolls, age, span), diey(seed, rolls, age, span),
     0.78, dieturn(seed, rolls, age, span))
```

All five take the same four things — the game’s seed, which throw this is, how
long ago it left the hand, and the half-width of the square it is thrown across
— so they differ only in the name at the front.

| | |
|---|---|
| `dieface(…)` | the face it is showing, 1 to 6 |
| `diex(…)` `diey(…)` | where it is |
| `dieturn(…)` | how far round it has turned, in radians |
| `diesquash(…)` | how wide the face looks — 1 flat on, 0 edge-on |
| `diedone(…)` | whether it has stopped |
| `dice(face, x, y, size, turn, squash)` | draws it — body and pips |

`flung` is *when* it was thrown, so everything is a function of the clock and
nothing is stepped frame by frame. A throw can be replayed, scrubbed, or watched
at half speed and looks the same every time.

**It slides, bounces and slows with one decay.** The speed dies away as
`e^{-t/τ}`, so the distance covered approaches a limit and never passes it —
that is why a die thrown hard does not slide for ever. It folds off the walls,
which *is* the reflection at the angle it struck, exact rather than detected:
there is no frame on which the die is a little way through the wall and has to
be pushed back. And it lands square, on a right angle.

**No random number generator.** The throw is worked out from the seed and the
throw number, so it is unpredictable in play and exactly repeatable — which is
what lets a match be replayed and makes “he cheated” answerable.

**A die tumbles, it does not spin.** A square turning smoothly on the spot looks
like a plate on a stick. The giveaway in two dimensions is foreshortening: the
face narrows to nothing as the die goes over its edge, and the next one opens
out — which is what `diesquash` is, and the face changes exactly where the die
is edge-on and nobody can see it change.

**The face is drawn first and the tumble is arranged to finish on it.** This was
the other way round to begin with, and it made an unfair die: the face fell out
of how far the die had turned, `quarters` runs over fifteen values, and fifteen
does not divide by six. Ones and fours came up eleven per cent more often than
twos and threes — chi-square 38.8 against a 11.07 threshold. Physically the new
way round is backwards; it is also the only way to get a fair die out of a flat
drawing with one angle, and it is checked by `plotkit/tests/fair.rs`.

### Turning a mark by a number

`placex` and `placey` move a mark; `placea` **turns** it, in radians, about its
own anchor:

```text
placex diex(seed, rolls, age, span)
placey diey(seed, rolls, age, span)
placea dieturn(seed, rolls, age, span)
```

A thing that follows a number nearly always has to face somewhere too — a die
tumbling, a car on a track, the hand of a clock. Without it a mark can be moved
by the game but only ever sits the way it was drawn, which reads as sliding
rather than moving. It is read only when `placex`/`placey` are set.

### Repeating a deed

A rule can do the same thing to every subscript in a range:

```text
when tap 1: each j in 0..16 (at[j] = if(at[j] == here, -1, at[j]))
```

That is *"send home whichever token is standing on this square"*, for sixteen
tokens, in one line. Written a token at a time it was sixteen rules of fifteen
deeds — 240 chances to mistype a subscript — and it is the reason the Ludo
board had two tokens a seat before it had four.

| | |
|---|---|
| `each j in 0..16 (…)` | `j` takes 0, 1, … 15 |
| `each j in 16 (…)` | the same; a bare count starts at nought |
| `each j in 2..5 (…)` | 2, 3, 4 |

The body is **bracketed**, so a rule can carry on afterwards —
`each j in 0..4 (at[j] = 0), turn = 1` — and two loops in a rule are two loops.
They nest.

Deeds inside a loop still see each other, and still see the passes before them,
so a loop can **add things up**:

```text
when tap 1: total = 0, each j in 0..16 (total = total + at[j])
```

`j` wins over anything the game happens to have called `j`. A loop whose
counter could be captured by the score would be a horrible thing to find.

**The range is written down, not worked out.** `each j in 0..n` is refused, as
is a range longer than 512. A range the game could change is a loop whose
length the game could change, and a rule that fires once a frame with a length
nothing bounds is a hung frame rather than a wrong answer. Nesting is bounded
too: one rule makes at most 20 000 writes in a frame, however deep it goes.

**There is no loop in the expressions**, only in the deeds. A deed already
writes to a named slot, so a loop over `j` needs nothing new to write to; an
expression has no slots and would want a fold, an accumulator, and somewhere to
put it. Rows still spell their sums out.

### Implicit multiplication

`2i`, `3x`, `2(1+i)` all mean what they look like. The one ambiguity is `f(x)`
— a call or a product? Settled by name: if `f` is a known function it is a
call, otherwise a multiplication.

---

## 3. `implicit` — the one worth explaining

```text
implicit(x*x + y*y, 4)          the circle of radius 2
implicit(x*x - y*y, 1)          a hyperbola
implicit(sin(x) + sin(y))       level defaults to 0
```

### How it draws

**Marching squares.** The visible window is divided into a `140 × 140` grid.
`F` is evaluated at every corner, `level` subtracted, and wherever the sign
changes along a cell's edge, a point is put on that edge by linear
interpolation. The points in a cell are joined into short segments.

Three consequences worth knowing before you are surprised by them:

**A curve that only *touches* the level is unreliable.** The sign test is
`(a > 0) != (b > 0)`, so a corner sitting *exactly* at the level counts as the
non-positive side. That means a touching curve is drawn **if and only if the
grid happens to land on it** — which is worse than never being drawn, because
it depends on where you are looking.

```text
implicit(x*x, 0)                       draws the y-axis: the grid lands on x = 0
implicit((x-0.0137)*(x-0.0137), 0)     the same curve, a hair off the grid: nothing
```

If a curve flickers in and out as you pan, this is why. Arrange for a genuine
crossing — `implicit(x, 0)` rather than `implicit(x*x, 0)` — and it is drawn
every time.

**The grid follows the window.** The 140 divisions span whatever you are
looking at, so zooming in gives *more* detail, not a bigger version of the same
staircase. Panning re-samples. This is also why the browser has to tell the
server where it is looking.

**A cell where `F` is not finite is skipped.** `implicit(1/(x*y))` leaves gaps
along the axes rather than drawing nonsense or stopping. Holes are the
signature of a function going to infinity there.

### What it costs

140 × 140 is **19 600 evaluations of `F` per implicit row, per frame**. That is
nothing for `x*x + y*y` and quite a lot for something with three `sin`s and a
`pow` in it. If a drawing goes sticky and it has implicits in it, that is the
first thing to switch off with its tick.

By contrast `plot(f)` costs one evaluation per pixel of width — about 900 — and
`param(f, t0, t1)` costs exactly 320, wherever you are looking.

### `plot` versus `implicit`

Use `plot` when `y` is a function of `x`. Use `implicit` when it is not:

```text
plot(sqrt(1 - x*x))             the top half of a circle only
implicit(x*x + y*y, 1)          the whole circle
```

`plot` breaks its line where the value stops being finite, rather than drawing
a vertical stroke across a pole — so `plot(1/x)` is two curves, not two curves
joined by a line through the middle.

---

## 4. The reach of the functions

**These are complex functions, and that is the whole story of their reach.**

### `sin` and `cos`

```text
sin(x + iy) = sin x · cosh y  +  i · cos x · sinh y
```

Along the real axis they behave as you expect and never leave `[-1, 1]`. Step
**off** the real axis and `cosh` and `sinh` take over, and those grow like
`e^|y| / 2`:

| `Im z` | roughly `|sin z|` |
|---|---|
| 0 | ≤ 1 |
| 5 | 74 |
| 10 | 11 013 |
| 20 | 2.4 × 10⁸ |
| 100 | 1.3 × 10⁴³ |
| 710 | 1.7 × 10³⁰⁸ — still just fits |
| 710.5 | **infinity** — `f64` gives up |

So the hard reach is `|Im z| < 710.5`, and the *useful* reach is far smaller: past `|Im z| ≈ 20` a curve is already millions of units tall and off
any screen you will ever look at. If `param(sin(z), …)` seems to draw nothing,
it has almost certainly drawn something enormous.

### `tan`

`sin/cos`, with one guard: it **errors** when `|cos z| < 1e-300`, which is at
`z = π/2 + kπ` on the real axis. Near a pole it is huge but finite, so a plot
of it breaks into pieces rather than joining across.

### `exp`

Overflows to infinity when `Re z > 709`. `Im z` may be anything: it only turns.

### `ln` — a branch cut

The **principal** branch: `ln|z| + i·arg z`, with `arg` in `(−π, π]`. So

* `ln(0)` is an error, not infinity.
* There is a cut along the **negative real axis**. A curve crossing it jumps by
  `2πi`. If a parametric curve has an unexplained vertical leap of about 6.28,
  this is why.

`arg` normalises negative zero away, so `arg(-1)` is `+π` and not `−π`. The
trap: `-1` parses as `Neg(1)`, negating `0.0` gives `-0.0`, and
`(-0.0).atan2(-1.0)` is `−π`. Writing this cookbook found that `arg` and `ln`
disagreed about it — `ln(-1)` gave `+iπ` while `arg(-1)` gave `−π`, two answers
for one number. They now use the same function.

### `sqrt` and `pow`

`sqrt(z)` is `pow(z, 0.5)`, principal branch — so `sqrt(-4)` is `2i`, and there
is the same cut along the negative reals.

`pow` has **two different implementations**, and the boundary matters:

* a whole-number exponent with `|n| ≤ 64` is done by repeated multiplication,
  and is exact
* anything else goes through `exp(w · ln z)`, principal branch

So `pow(z, 64)` and `pow(z, 65)` are computed differently. The second is
correct but takes the principal branch, and for negative or complex `z` that is
not always the answer you meant.

### The ones that hand back a real number

`abs`, `arg`, `re`, `im`, `floor`, `round`, `mod`, `max`, `min` all return a
number with **imaginary part zero**. `floor`, `round`, `mod`, `max`, `min` also
*read* only the real part of what you give them — so `floor(1.7 + 9i)` is `1`,
and the `9i` is dropped without complaint.

`mod` is Euclidean: `mod(-1, 9)` is `8`, not `-1`.

`polar(r, theta)` reads the real part of both, and gives `r·e^{iθ}`.

### Making whole numbers, without a random generator

```text
a = 1 + floor(5*abs(sin(37*(score+1))))
```

`sin` of a large multiple of a whole number wanders across its range without
settling into a pattern anybody spots; `floor` makes it whole. Nothing here is
random, so **the same game replays exactly** — which is what lets a wrong
answer be looked at again instead of lost.

---

## 5. Recipes

**A shape that turns with the clock**

```text
param(2*exp(i*(t + time)), 0, tau)
```

**A shape everything else follows**

```text
r = 2
circle(0, r)
ngon(0, r, 6)
```

**A rose, and a dial for its petals**

```text
n = 5
param(3*cos(n*t)*exp(i*t), 0, tau)
```

**A curve with a hole in it, drawn honestly**

```text
plot(1/x)
```

**Something that answers a tap**

```text
score = 0
when tap 1: score = score + 1
digits(score, 5, 3, 0.6)
```

Press **play**, then tap the shape. `when tap N` names a *figure* — group the
strokes first, and `N` is the number the tree shows.

**Fade something in and out on an event**

```text
cheer = -9
smiley(0, 0, max(0, 1 - (time - cheer)))
when tap 1: cheer = time
```

---

## 6. Where things live

| | |
|---|---|
| `plotkit` | complex numbers, shapes, the expression language, a rasteriser |
| `shapes` | things to draw — digits, faces, waves, strokes, motion |
| `physics` | how things move — oscillators, falling, triggers |
| `sound` | tones, a mixer, a kit of noises |
| `easel` | what a drawing **is**: marks, script, rules, keyframes, the file |
| `studio` | a desktop window |
| `web` | a server, and a browser front end |
| `live` | outside the workspace: the same world with a sound card |

The rule the whole thing rests on: **`easel` and everything under it has no
window in it**, which is why 787 tests can check the drawing in the dark.
