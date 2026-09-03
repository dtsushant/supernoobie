//! # A tiny expression language over complex numbers
//!
//! Maths and parsing only — nothing here draws. It turns text like
//!
//! ```text
//! a = 0 + 0i
//! b = 1 + 2i
//! polygon(a, b)
//! ```
//!
//! into values and a list of drawing *commands*. Something else decides what a
//! command looks like on screen.
//!
//! ---
//!
//! ## Why complex numbers make this small
//!
//! Every point is one value. There is no vector type, no `(x, y)` pair, no
//! separate arithmetic for points and scalars — `a + b` means the same thing
//! whether the operands are positions, offsets or plain numbers, because in
//! the plane they are all the same object.
//!
//! Rotation comes free: `z * e^(i t)` turns `z`. So does scaling: `2z`. A
//! whole affine transform is `a*z + b`, written exactly like that.
//!
//! ## The grammar
//!
//! ```text
//! line     := IDENT '=' expr            a binding
//!           | IDENT '(' args ')'        a command
//!           | '#' ...                   a comment
//!
//! expr     := term (('+' | '-') term)*
//! term     := power (('*' | '/') power)*   and implicit multiplication
//! power    := unary ('^' power)?           right-associative
//! unary    := '-' unary | atom
//! atom     := NUMBER | IDENT | IDENT '(' args ')' | '(' expr ')'
//! ```
//!
//! **Implicit multiplication** is supported, as in Desmos: `2i`, `3x`,
//! `2(1+i)` all mean what they look like. The one ambiguity is `f(x)` — a call
//! or a product? Resolved by name: if `f` is a known function it is a call,
//! otherwise it is a multiplication.
//!
//! ## Deferred expressions
//!
//! `plot`, `param` and `implicit` take an expression that is **not evaluated
//! at parse time** — it has a free variable (`x`, `t`, or `x` and `y`) that
//! the renderer supplies, once per sample. So those commands carry an [`Expr`]
//! tree rather than a number.

use crate::complex::Cx;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// tokens
// ---------------------------------------------------------------------------

/// The name `at[3]` means: `at3`.
///
/// Rounded, and negatives keep their sign — `at[-1]` is `at-1`, which is a
/// name nobody can type by accident and so cannot collide with one somebody
/// meant.
pub fn indexed(name: &str, k: f64) -> String {
    format!("{name}{}", k.round() as i64)
}

/// How close two numbers have to be to count as the same.
///
/// Exact equality between floats is a trap — `0.1 + 0.2 == 0.3` is false — and
/// a language for drawings and games should not make anybody learn that before
/// they can ask a question. Also what counts as "true": anything not within a
/// hair of zero.
pub const NEAR: f64 = 1e-9;

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    Open,
    Close,
    Comma,
    Eq,
    /// A comparison: `==`, `!=`, `<`, `<=`, `>`, `>=`.
    Cmp(&'static str),
    OpenSquare,
    CloseSquare,
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < b.len() {
        let c = b[k];
        if c.is_whitespace() {
            k += 1;
        } else if c == '0' && k + 1 < b.len() && (b[k + 1] == 'x' || b[k + 1] == 'X') {
            // Hexadecimal, for colours. `color(14722122)` is a number nobody
            // can read or check; `color(0xE0A44A)` is the same colour written
            // the way every other tool writes it.
            k += 2;
            let start = k;
            while k < b.len() && b[k].is_ascii_hexdigit() {
                k += 1;
            }
            let t: String = b[start..k].iter().collect();
            let v = u32::from_str_radix(&t, 16).map_err(|_| format!("bad hex number '0x{t}'"))?;
            out.push(Tok::Num(f64::from(v)));
        } else if c.is_ascii_digit() || (c == '.' && k + 1 < b.len() && b[k + 1].is_ascii_digit()) {
            let start = k;
            while k < b.len() && (b[k].is_ascii_digit() || b[k] == '.') {
                k += 1;
            }
            let t: String = b[start..k].iter().collect();
            out.push(Tok::Num(t.parse().map_err(|_| format!("bad number '{t}'"))?));
        } else if c.is_alphabetic() || c == '_' {
            let start = k;
            while k < b.len() && (b[k].is_alphanumeric() || b[k] == '_') {
                k += 1;
            }
            out.push(Tok::Ident(b[start..k].iter().collect()));
        } else if matches!(c, '<' | '>' | '=' | '!') {
            // Two characters where there are two, one where there is one. `=`
            // on its own is still a binding, so `a = 3` is unchanged and
            // `a == 3` is a question.
            let next = b.get(k + 1).copied();
            let two = matches!((c, next), ('<', Some('=')) | ('>', Some('=')) | ('=', Some('=')) | ('!', Some('=')));
            if two {
                out.push(Tok::Cmp(match c {
                    '<' => "<=",
                    '>' => ">=",
                    '=' => "==",
                    _ => "!=",
                }));
                k += 2;
            } else if c == '<' || c == '>' {
                out.push(Tok::Cmp(if c == '<' { "<" } else { ">" }));
                k += 1;
            } else if c == '=' {
                out.push(Tok::Eq);
                k += 1;
            } else {
                return Err("a lone '!' means nothing; did you mean '!='?".into());
            }
        } else {
            k += 1;
            match c {
                '(' => out.push(Tok::Open),
                '[' => out.push(Tok::OpenSquare),
                ']' => out.push(Tok::CloseSquare),
                ')' => out.push(Tok::Close),
                ',' => out.push(Tok::Comma),
                '+' | '-' | '*' | '/' | '^' => out.push(Tok::Op(c)),
                _ => return Err(format!("unexpected character '{c}'")),
            }
        }
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// the tree
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub enum Expr {
    /// `at[k]` — the variable **named** `at` followed by the number `k`.
    ///
    /// Not an array. There is no second kind of value anywhere in this
    /// language, and adding one would mean two of everything: two ways to
    /// bind, two ways to save, two ways to be wrong. `at[3]` is spelling for
    /// the name `at3`, and `at[k]` works out `k` first.
    Index(String, Box<Expr>),
    /// A comparison, giving `1` for true and `0` for false — because every
    /// value here is a number and inventing a second kind would mean two of
    /// everything.
    Cmp(&'static str, Box<Expr>, Box<Expr>),
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

/// Names that parse as function calls rather than implicit multiplication.
pub const FUNCS: [&str; 30] = [
    "exp", "ln", "sin", "cos", "tan", "sqrt", "abs", "arg", "conj", "re", "im", "polar", "pow",
    // Whole numbers. A language with no way to say "the integer part" cannot
    // say "a number between 1 and 9", which is most of what a counting game
    // needs.
    "floor", "round", "mod", "max", "min",
    // Asking questions. `if`, `and`, `or` and `pick` are decided before
    // their arguments are worked out; see `Expr::eval`.
    "if", "and", "or", "not", "pick",
    // Where a token stands on a board of squares. See `crate::ludo`.
    "ludox", "ludoy",
    // A die in mid-throw. See `crate::dice`. All five take the same four
    // things -- the game’s seed, which throw this is, how long ago it left the
    // hand, and the half-width of the board it is thrown across -- so a whole
    // die is five rows that differ only in the name at the front.
    "diex", "diey", "dieturn", "dieface", "diedone",
];

/// Names that draw something.
pub const COMMANDS: [&str; 9] = [
    "point", "line", "polygon", "circle", "ngon", "plot", "param", "implicit", "color",
];

impl Expr {
    /// Evaluate with the given bindings. Free variables are an error, which is
    /// how a typo gets reported rather than silently becoming zero.
    pub fn eval(&self, env: &HashMap<String, Cx>) -> Result<Cx, String> {
        Ok(match self {
            Expr::Num(v) => Cx::new(*v, 0.0),
            Expr::Var(n) => *env
                .get(n.as_str())
                .ok_or_else(|| format!("unknown name '{n}'"))?,
            Expr::Neg(a) => -a.eval(env)?,
            Expr::Bin(op, a, b) => {
                let (x, y) = (a.eval(env)?, b.eval(env)?);
                match op {
                    '+' => x + y,
                    '-' => x - y,
                    '*' => x * y,
                    '/' => {
                        if y.abs() < 1e-300 {
                            return Err("division by zero".into());
                        }
                        x / y
                    }
                    '^' => cpow(x, y),
                    _ => return Err(format!("bad operator '{op}'")),
                }
            }
            Expr::Index(name, k) => {
                let full = indexed(name, k.eval(env)?.re);
                *env.get(full.as_str()).ok_or_else(|| format!("unknown name '{full}'"))?
            }
            Expr::Cmp(op, a, b) => {
                let (x, y) = (a.eval(env)?, b.eval(env)?);
                let yes = match *op {
                    // Equality with a hair of slack. Exact equality between
                    // floats is a trap: `0.1 + 0.2 == 0.3` is false, and a
                    // language for drawings and games should not make anybody
                    // learn that before they can ask a question.
                    "==" => (x - y).abs() < NEAR,
                    "!=" => (x - y).abs() >= NEAR,
                    // Complex numbers are not ordered, so an order test is on
                    // the **real part**, and says so here rather than
                    // pretending otherwise.
                    "<" => x.re < y.re,
                    "<=" => x.re <= y.re,
                    ">" => x.re > y.re,
                    _ => x.re >= y.re,
                };
                Cx::new(if yes { 1.0 } else { 0.0 }, 0.0)
            }
            // `if`, `and`, `or` are decided **before** their arguments are
            // worked out, because the whole point is not to work them all out.
            // `if(x == 0, 0, 1/x)` must not divide by nothing on the way to
            // deciding it should not divide by nothing.
            Expr::Call(f, args) if f == "if" => {
                if args.len() != 3 {
                    return Err("if takes a question and two answers".into());
                }
                let yes = args[0].eval(env)?.re.abs() > NEAR;
                return args[if yes { 1 } else { 2 }].eval(env);
            }
            Expr::Call(f, args) if f == "and" || f == "or" => {
                if args.len() != 2 {
                    return Err(format!("{f} takes two"));
                }
                let first = args[0].eval(env)?.re.abs() > NEAR;
                let stop = if f == "and" { !first } else { first };
                if stop {
                    return Ok(Cx::new(if first { 1.0 } else { 0.0 }, 0.0));
                }
                let second = args[1].eval(env)?.re.abs() > NEAR;
                return Ok(Cx::new(if second { 1.0 } else { 0.0 }, 0.0));
            }
            // `pick` gives you an indexed read without a second kind of value:
            // `pick(k, a, b, c)` is the k-th of them. Only the one chosen is
            // worked out, so the others may be nonsense at this moment.
            Expr::Call(f, args) if f == "pick" => {
                if args.len() < 2 {
                    return Err("pick takes an index and at least one value".into());
                }
                let k = args[0].eval(env)?.re.round();
                let n = (args.len() - 1) as f64;
                if !(0.0..n).contains(&k) {
                    return Err(format!("pick: {k} is outside 0..{}", n - 1.0));
                }
                return args[1 + k as usize].eval(env);
            }
            Expr::Call(f, args) => {
                let a: Result<Vec<Cx>, String> = args.iter().map(|e| e.eval(env)).collect();
                let a = a?;
                let one = |n: usize| -> Result<Cx, String> {
                    a.get(n).copied().ok_or_else(|| format!("'{f}' needs more arguments"))
                };
                match f.as_str() {
                    "max" | "min" => {
                        if a.len() != 2 {
                            return Err(format!("'{f}' takes two numbers"));
                        }
                        let (x, y) = (a[0].re, a[1].re);
                        Cx::new(if f == "max" { x.max(y) } else { x.min(y) }, 0.0)
                    }
                    // `ludox(seat, step)` and `ludoy(...)`. Two functions
                    // rather than one giving a point, because a mark is placed
                    // by two expressions -- and two expressions is what makes
                    // it text, which is what saves.
                    "ludox" | "ludoy" => {
                        if a.len() != 2 {
                            return Err(format!("'{f}' takes a seat and a step"));
                        }
                        let seat = a[0].re.round().rem_euclid(4.0) as usize;
                        let step = a[1].re.round();
                        let at = if step < 0.0 {
                            crate::ludo::waiting(seat, (-step - 1.0).max(0.0) as usize)
                        } else {
                            crate::ludo::place(seat, (step as usize).min(crate::ludo::FINISH))
                        };
                        Cx::new(if f == "ludox" { at.re } else { at.im }, 0.0)
                    }
                    // A die in mid-throw. Five functions rather than one giving
                    // a die, for the same reason `ludox` and `ludoy` are two:
                    // a row holds one number, and numbers are what save.
                    "diex" | "diey" | "dieturn" | "dieface" | "diedone" => {
                        if a.len() != 4 {
                            return Err(format!("'{f}' takes a seed, a throw, an age and a span"));
                        }
                        let r = crate::dice::thrown(a[0].re, a[1].re, a[2].re, a[3].re);
                        Cx::new(
                            match f.as_str() {
                                "diex" => r.at.re,
                                "diey" => r.at.im,
                                "dieturn" => r.turn,
                                "dieface" => r.face as f64,
                                _ => r.done as u8 as f64,
                            },
                            0.0,
                        )
                    }
                    "not" => Cx::new(if one(0)?.re.abs() > NEAR { 0.0 } else { 1.0 }, 0.0),
                    "floor" => Cx::new(one(0)?.re.floor(), 0.0),
                    "round" => Cx::new(one(0)?.re.round(), 0.0),
                    "mod" => {
                        if args.len() != 2 {
                            return Err("mod takes two numbers".into());
                        }
                        let (a, b) = (args[0].eval(env)?.re, args[1].eval(env)?.re);
                        if b.abs() < 1e-300 {
                            return Err("mod by nothing".into());
                        }
                        // Euclidean, so `mod(-1, 9)` is 8 rather than -1. A
                        // counting game that stepped past zero into negative
                        // answers would be a strange kind of counting game.
                        Cx::new(a.rem_euclid(b), 0.0)
                    }
                    "exp" => cexp(one(0)?),
                    "ln" => cln(one(0)?)?,
                    "sin" => csin(one(0)?),
                    "cos" => ccos(one(0)?),
                    "tan" => {
                        let z = one(0)?;
                        let c = ccos(z);
                        if c.abs() < 1e-300 {
                            return Err("tan is undefined here".into());
                        }
                        csin(z) / c
                    }
                    "sqrt" => cpow(one(0)?, Cx::new(0.5, 0.0)),
                    "abs" => Cx::new(one(0)?.abs(), 0.0),
                    // The SAME `arg` that `ln` uses. It was `Cx::arg`, which
                    // does not normalise negative zero -- so `ln(-1)` gave
                    // `+i pi` and `arg(-1)` gave `-pi`, two answers for one
                    // number, from one expression language. The trap is that
                    // `-1` parses as `Neg(1)`, and negating `0.0` gives `-0.0`,
                    // and `(-0.0).atan2(-1.0)` is `-pi`.
                    "arg" => Cx::new(principal_arg(one(0)?), 0.0),
                    "conj" => one(0)?.conj(),
                    "re" => Cx::new(one(0)?.re, 0.0),
                    "im" => Cx::new(one(0)?.im, 0.0),
                    "pow" => cpow(one(0)?, one(1)?),
                    // polar(r, theta) = r e^(i theta)
                    "polar" => Cx::expi(one(1)?.re).scale(one(0)?.re),
                    _ => return Err(format!("unknown function '{f}'")),
                }
            }
        })
    }
}

// ---- complex elementary functions ----------------------------------------

fn cexp(z: Cx) -> Cx {
    // e^(a+bi) = e^a (cos b + i sin b)
    Cx::expi(z.im).scale(z.re.exp())
}

/// `arg`, with **negative zero normalised away**.
///
/// A genuine trap. `-1` evaluated as `Neg(1)` gives `Cx { re: -1.0, im: -0.0 }`
/// — the imaginary part is *negative* zero, because negating `0.0` does that.
/// And `(-0.0).atan2(-1.0)` is `-pi`, not `+pi`.
///
/// So `sqrt(-1)` comes out as `-i` instead of `+i`: the negative zero lands you
/// on the far side of the branch cut along the negative real axis. IEEE says
/// that is correct and the sign of zero is meaningful there — but someone
/// typing `sqrt(-1)` means the principal root, so we normalise `-0.0` to `0.0`
/// first and always take the upper side.
fn principal_arg(z: Cx) -> f64 {
    // `-0.0 == 0.0` is true, so this replaces negative zero and nothing else
    let im = if z.im == 0.0 { 0.0 } else { z.im };
    im.atan2(z.re)
}

fn cln(z: Cx) -> Result<Cx, String> {
    if z.abs() < 1e-300 {
        return Err("ln(0) is undefined".into());
    }
    // the principal branch: ln|z| + i arg(z)
    Ok(Cx::new(z.abs().ln(), principal_arg(z)))
}

fn csin(z: Cx) -> Cx {
    Cx::new(z.re.sin() * z.im.cosh(), z.re.cos() * z.im.sinh())
}
fn ccos(z: Cx) -> Cx {
    Cx::new(z.re.cos() * z.im.cosh(), -z.re.sin() * z.im.sinh())
}

/// `z^w`. Whole-number powers are done by repeated multiplication so they are
/// exact; anything else goes through `exp(w ln z)` and takes the principal
/// branch, with all the multi-valuedness that implies.
fn cpow(z: Cx, w: Cx) -> Cx {
    if w.im.abs() < 1e-15 && (w.re - w.re.round()).abs() < 1e-12 && w.re.abs() <= 64.0 {
        let n = w.re.round() as i64;
        let mut acc = Cx::ONE;
        for _ in 0..n.abs() {
            acc = acc * z;
        }
        return if n < 0 { Cx::ONE / acc } else { acc };
    }
    if z.abs() < 1e-300 {
        return Cx::ZERO;
    }
    cexp(w * Cx::new(z.abs().ln(), principal_arg(z)))
}

// ---------------------------------------------------------------------------
// parser
// ---------------------------------------------------------------------------

struct P {
    t: Vec<Tok>,
    k: usize,
}

impl P {
    fn peek(&self) -> Option<&Tok> {
        self.t.get(self.k)
    }
    fn eat(&mut self, t: &Tok) -> bool {
        if self.peek() == Some(t) {
            self.k += 1;
            true
        } else {
            false
        }
    }

    /// `sum (CMP sum)?` — one comparison, and it binds **looser** than `+`, so
    /// `a + b == c` asks what it looks like it asks.
    ///
    /// Only one: `a < b < c` is a thing people write and almost never mean, so
    /// it is refused rather than quietly read as `(a < b) < c`.
    fn expr(&mut self) -> Result<Expr, String> {
        let a = self.sum()?;
        if let Some(Tok::Cmp(op)) = self.peek().cloned() {
            self.k += 1;
            let b = self.sum()?;
            if let Some(Tok::Cmp(_)) = self.peek() {
                return Err("two comparisons in a row: write and(a < b, b < c)".into());
            }
            return Ok(Expr::Cmp(op, Box::new(a), Box::new(b)));
        }
        Ok(a)
    }

    fn sum(&mut self) -> Result<Expr, String> {
        let mut a = self.term()?;
        while let Some(Tok::Op(c @ ('+' | '-'))) = self.peek().cloned() {
            self.k += 1;
            a = Expr::Bin(c, Box::new(a), Box::new(self.term()?));
        }
        Ok(a)
    }

    fn term(&mut self) -> Result<Expr, String> {
        let mut a = self.unary()?;
        loop {
            if let Some(Tok::Op(c @ ('*' | '/'))) = self.peek().cloned() {
                self.k += 1;
                a = Expr::Bin(c, Box::new(a), Box::new(self.unary()?));
                continue;
            }
            // implicit multiplication: 2i, 3x, 2(1+i)
            match self.peek() {
                Some(Tok::Num(_)) | Some(Tok::Ident(_)) | Some(Tok::Open) => {
                    a = Expr::Bin('*', Box::new(a), Box::new(self.power()?));
                }
                _ => break,
            }
        }
        Ok(a)
    }

    /// Unary minus binds *looser* than `^`, so `-2^2` is `-(2^2) = -4` and
    /// not `(-2)^2 = 4`. That is the ordinary mathematical convention, and the
    /// one Desmos uses.
    fn unary(&mut self) -> Result<Expr, String> {
        if self.eat(&Tok::Op('-')) {
            return Ok(Expr::Neg(Box::new(self.unary()?)));
        }
        let _ = self.eat(&Tok::Op('+'));
        self.power()
    }

    /// `atom ('^' unary)?` — right-associative, and the right-hand side goes
    /// back through `unary` so `2^-3` works.
    fn power(&mut self) -> Result<Expr, String> {
        let a = self.atom()?;
        if self.eat(&Tok::Op('^')) {
            return Ok(Expr::Bin('^', Box::new(a), Box::new(self.unary()?)));
        }
        Ok(a)
    }

    fn atom(&mut self) -> Result<Expr, String> {
        match self.peek().cloned() {
            Some(Tok::Num(v)) => {
                self.k += 1;
                Ok(Expr::Num(v))
            }
            Some(Tok::Ident(n)) if matches!(self.t.get(self.k + 1), Some(Tok::OpenSquare)) => {
                self.k += 2;
                let inside = self.expr()?;
                if !self.eat(&Tok::CloseSquare) {
                    return Err(format!("'{n}[' was never closed"));
                }
                Ok(Expr::Index(n, Box::new(inside)))
            }
            Some(Tok::Ident(n)) => {
                self.k += 1;
                // `f(` is a call only when f is a known function; otherwise it
                // is implicit multiplication, which is what `2(1+i)` needs.
                if FUNCS.contains(&n.as_str()) && self.peek() == Some(&Tok::Open) {
                    self.k += 1;
                    let args = self.args()?;
                    return Ok(Expr::Call(n, args));
                }
                Ok(Expr::Var(n))
            }
            Some(Tok::Open) => {
                self.k += 1;
                let e = self.expr()?;
                if !self.eat(&Tok::Close) {
                    return Err("expected ')'".into());
                }
                Ok(e)
            }
            other => Err(format!("unexpected {other:?}")),
        }
    }

    fn args(&mut self) -> Result<Vec<Expr>, String> {
        let mut out = Vec::new();
        if self.eat(&Tok::Close) {
            return Ok(out);
        }
        loop {
            out.push(self.expr()?);
            if self.eat(&Tok::Comma) {
                continue;
            }
            if self.eat(&Tok::Close) {
                return Ok(out);
            }
            return Err("expected ',' or ')'".into());
        }
    }
}

/// Parse a single expression. Useful on its own, and what the tests poke at.
pub fn parse(src: &str) -> Result<Expr, String> {
    let mut p = P { t: lex(src)?, k: 0 };
    let e = p.expr()?;
    if p.k != p.t.len() {
        return Err("trailing junk after expression".into());
    }
    Ok(e)
}

// ---------------------------------------------------------------------------
// commands
// ---------------------------------------------------------------------------

/// Something to draw. The renderer decides how; this only says what.
#[derive(Clone, Debug)]
pub enum Cmd {
    Point(Vec<Cx>),
    /// An open path.
    Line(Vec<Cx>),
    /// A closed path. Two points make a straight line, which is why
    /// `polygon(a, b)` does the obvious thing.
    Polygon(Vec<Cx>),
    Circle(Cx, f64),
    Ngon(Cx, f64, usize),
    /// `y = f(x)`, evaluated per sample with `x` bound.
    Plot(Expr),
    /// `t -> z(t)` over `[t0, t1]`, with `t` bound.
    Param(Expr, f64, f64),
    /// `F(x, y) = level`, with `x` and `y` bound.
    Implicit(Expr, f64),
    /// Sets the colour of everything after it.
    Color(u32),
}

#[derive(Debug)]
pub struct Program {
    pub cmds: Vec<Cmd>,
    /// `(line number, message)` — reported, never fatal, so one bad line does
    /// not blank the whole drawing.
    pub errors: Vec<(usize, String)>,
    /// Every binding, in case something wants to show them.
    pub vars: Vec<(String, Cx)>,
}

fn base_env() -> HashMap<String, Cx> {
    let mut m = HashMap::new();
    m.insert("i".to_string(), Cx::I);
    m.insert("pi".to_string(), Cx::new(std::f64::consts::PI, 0.0));
    m.insert("tau".to_string(), Cx::new(std::f64::consts::TAU, 0.0));
    m.insert("e".to_string(), Cx::new(std::f64::consts::E, 0.0));
    m
}

/// Run a script: bindings are evaluated in order, commands are collected.
///
/// Errors are per line and never stop the run — a script being edited live is
/// broken most of the time, and blanking the screen on every keystroke would
/// make it useless.
pub fn run(src: &str) -> Program {
    let mut env = base_env();
    let mut cmds = Vec::new();
    let mut errors = Vec::new();
    let mut vars = Vec::new();

    for (n, raw) in src.lines().enumerate() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Err(e) = run_line(line, &mut env, &mut cmds, &mut vars) {
            errors.push((n + 1, e));
        }
    }
    Program { cmds, errors, vars }
}

fn run_line(
    line: &str,
    env: &mut HashMap<String, Cx>,
    cmds: &mut Vec<Cmd>,
    vars: &mut Vec<(String, Cx)>,
) -> Result<(), String> {
    let toks = lex(line)?;
    // IDENT '=' ...   is a binding
    if toks.len() >= 2 && matches!(toks[0], Tok::Ident(_)) && toks[1] == Tok::Eq {
        let name = match &toks[0] {
            Tok::Ident(n) => n.clone(),
            _ => unreachable!(),
        };
        if FUNCS.contains(&name.as_str()) || COMMANDS.contains(&name.as_str()) {
            return Err(format!("'{name}' is reserved"));
        }
        let mut p = P { t: toks[2..].to_vec(), k: 0 };
        let e = p.expr()?;
        let v = e.eval(env)?;
        env.insert(name.clone(), v);
        vars.push((name, v));
        return Ok(());
    }

    // IDENT '(' args ')'   is a command
    if let (Some(Tok::Ident(name)), Some(Tok::Open)) = (toks.first(), toks.get(1)) {
        let name = name.clone();
        if !COMMANDS.contains(&name.as_str()) {
            return Err(format!("unknown command '{name}'"));
        }
        let mut p = P { t: toks[2..].to_vec(), k: 0 };
        let args = p.args()?;
        cmds.push(build(&name, &args, env)?);
        return Ok(());
    }

    Err("expected 'name = expression' or 'command(...)'".into())
}

fn build(name: &str, args: &[Expr], env: &HashMap<String, Cx>) -> Result<Cmd, String> {
    let need = |n: usize| -> Result<(), String> {
        if args.len() < n {
            Err(format!("'{name}' needs {n} argument(s), got {}", args.len()))
        } else {
            Ok(())
        }
    };
    let val = |k: usize| -> Result<Cx, String> { args[k].eval(env) };
    let pts = || -> Result<Vec<Cx>, String> { args.iter().map(|a| a.eval(env)).collect() };

    Ok(match name {
        "point" => {
            need(1)?;
            Cmd::Point(pts()?)
        }
        "line" => {
            need(2)?;
            Cmd::Line(pts()?)
        }
        "polygon" => {
            need(2)?;
            Cmd::Polygon(pts()?)
        }
        "circle" => {
            need(2)?;
            Cmd::Circle(val(0)?, val(1)?.re)
        }
        "ngon" => {
            need(3)?;
            let n = val(2)?.re.round();
            if !(3.0..=512.0).contains(&n) {
                return Err(format!("ngon needs 3..512 sides, got {n}"));
            }
            Cmd::Ngon(val(0)?, val(1)?.re, n as usize)
        }
        // deferred: the expression keeps a free variable
        "plot" => {
            need(1)?;
            Cmd::Plot(args[0].clone())
        }
        "param" => {
            need(3)?;
            Cmd::Param(args[0].clone(), val(1)?.re, val(2)?.re)
        }
        "implicit" => {
            need(1)?;
            let level = if args.len() > 1 { val(1)?.re } else { 0.0 };
            Cmd::Implicit(args[0].clone(), level)
        }
        "color" => {
            need(1)?;
            let v = val(0)?.re;
            if !(0.0..=16_777_215.0).contains(&v) {
                return Err("colour must be 0..16777215".into());
            }
            Cmd::Color(v as u32)
        }
        _ => return Err(format!("unknown command '{name}'")),
    })
}

/// Evaluate a deferred expression with one extra binding — what `plot` and
/// `param` need per sample.
pub fn eval_with(e: &Expr, name: &str, v: Cx, base: &HashMap<String, Cx>) -> Result<Cx, String> {
    let mut env = base.clone();
    env.insert(name.to_string(), v);
    e.eval(&env)
}

/// The environment a script ends with, so a renderer can re-evaluate deferred
/// expressions against the same bindings.
pub fn env_of(p: &Program) -> HashMap<String, Cx> {
    let mut env = base_env();
    for (k, v) in &p.vars {
        env.insert(k.clone(), *v);
    }
    env
}

// ===========================================================================
#[cfg(test)]
mod tests {
    /// ★ Where a token stands, from inside an expression — which is what lets
    /// a drawn mark follow a game's numbers.
    #[test]
    fn a_square_can_be_asked_for_by_seat_and_step() {
        let p = run("s = 1
k = 20
a = ludox(s, k)
b = ludoy(s, k)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let get = |n: &str| p.vars.iter().find(|(k, _)| k == n).expect(n).1.re;
        let want = crate::ludo::place(1, 20);
        assert!((get("a") - want.re).abs() < 1e-9);
        assert!((get("b") - want.im).abs() < 1e-9);
    }

    /// And a negative step is the yard, the same as everywhere else.
    #[test]
    fn a_negative_step_asks_for_the_yard() {
        let p = run("a = ludox(0, -1)
b = ludoy(0, -1)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let get = |n: &str| p.vars.iter().find(|(k, _)| k == n).expect(n).1.re;
        let want = crate::ludo::waiting(0, 0);
        assert!((get("a") - want.re).abs() < 1e-9 && (get("b") - want.im).abs() < 1e-9);
    }

    /// ★ `at[k]` is **spelling**, not an array. There is no second kind of
    /// value anywhere in this language, and adding one would mean two of
    /// everything: two ways to bind, two ways to save, two ways to be wrong.
    #[test]
    fn a_subscript_is_a_name_worked_out() {
        let p = run("at0 = 5\nat1 = 9\nk = 1\na = at[k]\nb = at[0]");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let get = |n: &str| p.vars.iter().find(|(k, _)| k == n).expect(n).1.re;
        assert_eq!(get("a"), 9.0);
        assert_eq!(get("b"), 5.0);
    }

    /// The index is worked out first, so it can be anything.
    #[test]
    fn the_index_can_be_a_sum() {
        let p = run("at2 = 7\nk = 1\na = at[k + 1]");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.vars.iter().find(|(n, _)| n == "a").expect("a").1.re, 7.0);
    }

    /// ★ A subscript that names nothing says so, exactly as a plain name does
    /// — rather than quietly becoming zero, which is how a typo turns into a
    /// drawing that is subtly wrong instead of visibly broken.
    #[test]
    fn a_subscript_that_names_nothing_says_so() {
        let p = run("at0 = 1\na = at[9]");
        assert!(!p.errors.is_empty());
        assert!(p.errors[0].1.contains("at9"), "it should say which name: {:?}", p.errors);
    }

    /// Negative subscripts keep their sign, so they cannot collide with a name
    /// anybody meant.
    #[test]
    fn a_negative_subscript_is_its_own_name() {
        assert_eq!(indexed("at", -1.0), "at-1");
        assert_eq!(indexed("at", 3.4), "at3");
        assert_ne!(indexed("at", -1.0), indexed("at", 1.0));
    }

    /// An unclosed subscript is a mistake with a message, not a silent
    /// misreading.
    #[test]
    fn an_unclosed_subscript_is_refused() {
        assert!(!run("a = at[1").errors.is_empty());
    }

    /// Read one binding out of a script.
    fn val(src: &str) -> f64 {
        let p = run(src);
        assert!(p.errors.is_empty(), "{src}: {:?}", p.errors);
        p.vars.iter().find(|(n, _)| n == "a").expect("a").1.re
    }

    /// ★ A comparison gives 1 or 0, because every value here is a number and
    /// inventing a second kind would mean two of everything — two sets of
    /// operators, two things a variable might hold, two ways to be wrong.
    #[test]
    fn a_question_is_answered_with_a_number() {
        assert_eq!(val("a = 3 == 3"), 1.0);
        assert_eq!(val("a = 3 == 4"), 0.0);
        assert_eq!(val("a = 3 != 4"), 1.0);
        assert_eq!(val("a = 2 < 3"), 1.0);
        assert_eq!(val("a = 3 <= 3"), 1.0);
        assert_eq!(val("a = 4 > 3"), 1.0);
        assert_eq!(val("a = 2 >= 3"), 0.0);
        // So an answer is arithmetic, which is the point.
        assert_eq!(val("a = 5 + 10*(3 == 3)"), 15.0);
    }

    /// ★ Equality has a hair of slack. Exact equality between floats is a trap
    /// — `0.1 + 0.2 == 0.3` is false — and a language for drawings and games
    /// should not make anybody learn that before they can ask a question.
    #[test]
    fn equality_does_not_make_you_learn_about_floats_first() {
        assert_eq!(val("a = 0.1 + 0.2 == 0.3"), 1.0);
        assert_eq!(val("a = 1 == 1.0000001"), 0.0, "but it is slack, not blind");
    }

    /// ★ Comparison binds **looser** than `+`, so `a + b == c` asks what it
    /// looks like it asks.
    #[test]
    fn a_comparison_binds_looser_than_arithmetic() {
        assert_eq!(val("a = 1 + 2 == 3"), 1.0);
        assert_eq!(val("a = 2*3 == 6"), 1.0);
        assert_eq!(val("a = 1 == 2 - 1"), 1.0);
    }

    /// And two in a row are refused rather than quietly read as `(a<b)<c`,
    /// which is a thing people write and almost never mean.
    #[test]
    fn two_comparisons_in_a_row_are_refused() {
        assert!(!run("a = 1 < 2 < 3").errors.is_empty());
    }

    /// A lone `=` is still a binding, so nothing that worked before changed.
    #[test]
    fn a_single_equals_is_still_a_binding() {
        assert_eq!(val("a = 3"), 3.0);
        let p = run("a = 1
b = a == 1");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.vars.iter().find(|(n, _)| n == "b").expect("b").1.re, 1.0);
    }

    /// ★ **`if` is decided before its answers are worked out.** The whole point
    /// is not to work them all out: `if(x == 0, 0, 1/x)` must not divide by
    /// nothing on the way to deciding it should not divide by nothing.
    #[test]
    fn if_does_not_evaluate_the_answer_it_did_not_choose() {
        assert_eq!(val("x = 0
a = if(x == 0, 7, 1/x)"), 7.0);
        assert_eq!(val("x = 4
a = if(x == 0, 7, 1/x)"), 0.25);
        // And the branch not taken may be outright nonsense.
        assert_eq!(val("a = if(1, 5, ln(0))"), 5.0);
    }

    /// ★ `and` and `or` stop as soon as they know, for the same reason.
    #[test]
    fn and_and_or_stop_as_soon_as_they_know() {
        assert_eq!(val("a = and(0, ln(0))"), 0.0, "false and anything is false");
        assert_eq!(val("a = or(1, ln(0))"), 1.0, "true or anything is true");
        assert_eq!(val("a = and(1, 1)"), 1.0);
        assert_eq!(val("a = or(0, 0)"), 0.0);
        assert_eq!(val("a = not(0)"), 1.0);
        assert_eq!(val("a = not(3)"), 0.0, "anything away from zero is true");
    }

    /// ★ `pick` gives an indexed read without a second kind of value — and
    /// only the one chosen is worked out, so the others may be nonsense at
    /// this moment.
    #[test]
    fn pick_reads_by_number_without_needing_arrays() {
        assert_eq!(val("a = pick(0, 10, 20, 30)"), 10.0);
        assert_eq!(val("a = pick(2, 10, 20, 30)"), 30.0);
        assert_eq!(val("k = 1
a = pick(k, 10, 20, 30)"), 20.0);
        assert_eq!(val("a = pick(0, 5, ln(0))"), 5.0, "the ones not picked are not worked out");
        assert!(!run("a = pick(9, 1, 2)").errors.is_empty(), "and out of range says so");
    }

    /// A worked example of the sort of thing this was added for: a rule that
    /// only fires under a condition, written as one expression.
    #[test]
    fn a_rule_can_now_ask_a_question() {
        // "come out of the yard only on a six"
        let out = |die: i32, at: i32| {
            let src = format!("die = {die}
at = {at}
a = if(and(die == 6, at < 0), 0, at)");
            val(&src)
        };
        assert_eq!(out(6, -1), 0.0, "a six brings it out");
        assert_eq!(out(3, -1), -1.0, "anything else leaves it in the yard");
        assert_eq!(out(6, 12), 12.0, "and one already out is left where it is");
    }

    /// ★ `arg` and `ln` must agree about the same number. They did not: `arg`
    /// used `Cx::arg`, which does not normalise negative zero, so `ln(-1)`
    /// gave `+i pi` and `arg(-1)` gave `-pi`. The trap is that `-1` parses as
    /// `Neg(1)`, negating `0.0` gives `-0.0`, and `(-0.0).atan2(-1.0)` is
    /// `-pi`.
    #[test]
    fn arg_and_ln_agree_about_the_same_number() {
        let val = |src: &str| {
            let p = run(&format!("a = {src}"));
            assert!(p.errors.is_empty(), "{src}: {:?}", p.errors);
            p.vars.iter().find(|(n, _)| n == "a").expect("a").1
        };
        let pi = std::f64::consts::PI;
        assert!((val("arg(-1)").re - pi).abs() < 1e-12, "arg(-1) should be +pi, got {}", val("arg(-1)").re);
        assert!((val("im(ln(-1))").re - pi).abs() < 1e-12, "and ln(-1) should agree");
        assert!(val("arg(1)").re.abs() < 1e-12);
        assert!((val("arg(i)").re - pi / 2.0).abs() < 1e-12);
    }

    /// ★ Whole numbers. Without a way to say "the integer part" the language
    /// cannot say "a number between 1 and 9", which is most of what a counting
    /// game needs.
    #[test]
    fn it_can_do_whole_numbers() {
        let val = |src: &str| {
            let p = run(&format!("a = {src}"));
            assert!(p.errors.is_empty(), "{src}: {:?}", p.errors);
            p.vars.iter().find(|(n, _)| n == "a").expect("a").1.re
        };
        assert_eq!(val("floor(3.7)"), 3.0);
        assert_eq!(val("round(3.7)"), 4.0);
        assert_eq!(val("mod(11, 9)"), 2.0);
        // Euclidean: a counting game that stepped past zero into negative
        // answers would be a strange kind of counting game.
        assert_eq!(val("mod(-1, 9)"), 8.0);
        // `max` is what makes a thing fade to nothing and stay there rather
        // than turning inside out: max(0, 1 - t).
        assert_eq!(val("max(0, 3 - 5)"), 0.0);
        assert_eq!(val("min(4, 9)"), 4.0);
        assert!(!run("a = mod(1, 0)").errors.is_empty(), "mod by nothing should say so");
    }

    /// ★ Hex numbers, for colours. `color(14722122)` is a number nobody can
    /// read or check against anything; `color(0xE0A44A)` is the same colour
    /// written the way every other tool writes it.
    #[test]
    fn hex_numbers_are_read_for_colours() {
        let p = run("color(0xE0A44A)
circle(0, 1)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert!(matches!(p.cmds.first(), Some(Cmd::Color(0xE0_A4_4A))), "{:?}", p.cmds.first());

        // And it does not swallow ordinary numbers starting with zero.
        let q = run("a = 0.5
b = 0");
        assert!(q.errors.is_empty(), "{:?}", q.errors);
        assert!((q.vars.iter().find(|(n, _)| n == "a").expect("a").1.re - 0.5).abs() < 1e-12);
    }

    use super::*;

    fn ev(s: &str) -> Cx {
        parse(s).unwrap().eval(&base_env()).unwrap()
    }
    fn close(a: Cx, b: Cx) -> bool {
        (a - b).abs() < 1e-9
    }

    /// The headline: a complex literal reads the way it is written.
    #[test]
    fn complex_literals_parse() {
        assert!(close(ev("0 + 0i"), Cx::ZERO));
        assert!(close(ev("1 + 2i"), Cx::new(1.0, 2.0)));
        assert!(close(ev("i"), Cx::I));
        assert!(close(ev("-3i"), Cx::new(0.0, -3.0)));
        assert!(close(ev("2.5"), Cx::new(2.5, 0.0)));
    }

    /// `2i` is implicit multiplication, exactly as in Desmos.
    #[test]
    fn implicit_multiplication_works() {
        assert!(close(ev("2i"), Cx::new(0.0, 2.0)));
        assert!(close(ev("3(1 + i)"), Cx::new(3.0, 3.0)));
        assert!(close(ev("2 3"), Cx::new(6.0, 0.0)));
        // and it binds tighter than +, as multiplication should
        assert!(close(ev("1 + 2i"), Cx::new(1.0, 2.0)));
    }

    #[test]
    fn precedence_and_associativity() {
        assert!(close(ev("1 + 2*3"), Cx::new(7.0, 0.0)));
        assert!(close(ev("(1 + 2)*3"), Cx::new(9.0, 0.0)));
        assert!(close(ev("-2^2"), Cx::new(-4.0, 0.0)), "unary minus is outside the power");
        assert!(close(ev("2^3^2"), Cx::new(512.0, 0.0)), "^ is right-associative");
    }

    /// Whole-number powers go through repeated multiplication, so they are
    /// exact - `(1+i)^8` must be exactly 16, not 15.999999.
    #[test]
    fn integer_powers_are_exact() {
        assert_eq!(ev("(1 + i)^2"), Cx::new(0.0, 2.0));
        assert_eq!(ev("(1 + i)^4"), Cx::new(-4.0, 0.0));
        assert_eq!(ev("(1 + i)^8"), Cx::new(16.0, 0.0));
        assert_eq!(ev("i^4"), Cx::ONE);
    }

    /// Euler's identity, typed in.
    #[test]
    fn euler_identity_evaluates() {
        assert!(close(ev("exp(i*pi)"), Cx::new(-1.0, 0.0)));
        assert!(close(ev("exp(i*tau)"), Cx::ONE));
        assert!(close(ev("polar(2, pi/2)"), Cx::new(0.0, 2.0)));
    }

    #[test]
    fn the_function_library_works() {
        assert!(close(ev("abs(3 + 4i)"), Cx::new(5.0, 0.0)));
        assert!(close(ev("conj(3 + 4i)"), Cx::new(3.0, -4.0)));
        assert!(close(ev("re(3 + 4i)"), Cx::new(3.0, 0.0)));
        assert!(close(ev("im(3 + 4i)"), Cx::new(4.0, 0.0)));
        assert!(close(ev("sqrt(-1)"), Cx::I));
        assert!(close(ev("ln(e)"), Cx::ONE));
        // sin of a real argument is the ordinary sine
        assert!((ev("sin(1)").re - 1f64.sin()).abs() < 1e-12);
    }

    /// ★ Negative zero must not flip the branch cut. `-1` evaluates to
    /// `re: -1.0, im: -0.0`, and `(-0.0).atan2(-1.0)` is `-pi` — which would
    /// quietly make `sqrt(-1)` come out as `-i`.
    #[test]
    fn negative_zero_does_not_flip_the_branch_cut() {
        assert!(close(ev("sqrt(-1)"), Cx::I), "got {}", ev("sqrt(-1)"));
        assert!(close(ev("ln(-1)"), Cx::new(0.0, std::f64::consts::PI)));
        // the same number written two ways must agree
        assert!(close(ev("sqrt(-1)"), ev("sqrt(0 - 1)")));
        assert!(close(ev("sqrt(-4)"), Cx::new(0.0, 2.0)));
    }

    /// ★ The example from the request, end to end.
    #[test]
    fn the_requested_script_produces_a_line() {
        let p = run("cx1 = 0 + 0i\ncx2 = 1 + 2i\npolygon(cx1, cx2)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.vars.len(), 2);
        assert_eq!(p.cmds.len(), 1);
        match &p.cmds[0] {
            Cmd::Polygon(v) => {
                assert_eq!(v.len(), 2);
                assert!(close(v[0], Cx::ZERO));
                assert!(close(v[1], Cx::new(1.0, 2.0)));
            }
            other => panic!("wrong command: {other:?}"),
        }
    }

    /// Bindings can refer to earlier bindings, and arithmetic between points
    /// is just arithmetic - no separate vector type.
    #[test]
    fn bindings_compose() {
        let p = run("a = 1 + 1i\nb = a * i\nc = a + b\npoint(c)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        // a*i rotates a quarter turn: (1+i)i = -1+i
        assert!(close(p.vars[1].1, Cx::new(-1.0, 1.0)));
        assert!(close(p.vars[2].1, Cx::new(0.0, 2.0)));
    }

    /// One bad line must not take the rest of the script with it.
    #[test]
    fn errors_are_reported_per_line_and_do_not_stop_the_run() {
        let p = run("a = 1\nb = nosuchthing + 1\npoint(a)\nc = 2\n");
        assert_eq!(p.errors.len(), 1);
        assert_eq!(p.errors[0].0, 2, "wrong line number");
        assert!(p.errors[0].1.contains("nosuchthing"), "{}", p.errors[0].1);
        assert_eq!(p.cmds.len(), 1, "the good command should still have run");
        assert_eq!(p.vars.len(), 2, "a and c should both be bound");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let p = run("# a comment\n\na = 2   # trailing comment\npoint(a)\n");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        assert_eq!(p.vars.len(), 1);
        assert_eq!(p.cmds.len(), 1);
    }

    /// Deferred expressions keep their free variable until the renderer binds
    /// it - that is what makes `plot` possible at all.
    #[test]
    fn plot_defers_its_expression() {
        let p = run("plot(sin(x))");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        let env = env_of(&p);
        match &p.cmds[0] {
            Cmd::Plot(e) => {
                // unbound, it fails...
                assert!(e.eval(&env).is_err());
                // ...bound, it is sin
                let y = eval_with(e, "x", Cx::new(0.5, 0.0), &env).unwrap();
                assert!((y.re - 0.5f64.sin()).abs() < 1e-12);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn param_and_implicit_carry_their_ranges() {
        let p = run("param(exp(i*t), 0, tau)\nimplicit(x*x + y*y, 4)");
        assert!(p.errors.is_empty(), "{:?}", p.errors);
        match &p.cmds[0] {
            Cmd::Param(_, a, b) => {
                assert_eq!(*a, 0.0);
                assert!((*b - std::f64::consts::TAU).abs() < 1e-12);
            }
            other => panic!("{other:?}"),
        }
        match &p.cmds[1] {
            Cmd::Implicit(e, lvl) => {
                assert_eq!(*lvl, 4.0);
                let env = env_of(&p);
                let mut m = env.clone();
                m.insert("x".into(), Cx::new(2.0, 0.0));
                m.insert("y".into(), Cx::ZERO);
                assert!(close(e.eval(&m).unwrap(), Cx::new(4.0, 0.0)));
            }
            other => panic!("{other:?}"),
        }
    }

    /// Reserved names cannot be shadowed, or `sin = 3` would break every
    /// script that came after it.
    #[test]
    fn reserved_names_are_protected() {
        let p = run("sin = 3\npolygon = 4");
        assert_eq!(p.errors.len(), 2);
        assert!(p.errors.iter().all(|(_, m)| m.contains("reserved")));
    }

    /// Malformed input must produce a message, never a panic.
    #[test]
    fn broken_input_never_panics() {
        for bad in [
            "a = ", "a = (1 + ", "point(", "polygon()", ")(", "a = 1 +* 2",
            "a = 1/0", "ln(0)", "@#$", "circle(0)", "ngon(0, 1, 2)", "= 5",
        ] {
            let p = run(bad);
            assert!(!p.errors.is_empty(), "'{bad}' should have errored");
        }
    }

    /// A script that is half-typed - which is most of the time, live - should
    /// still draw whatever is already valid.
    #[test]
    fn a_half_typed_script_still_draws_what_works() {
        let p = run("a = 1 + 1i\npolygon(a, 2a)\ncircle(a, ");
        assert_eq!(p.cmds.len(), 1);
        assert_eq!(p.errors.len(), 1);
    }
}
