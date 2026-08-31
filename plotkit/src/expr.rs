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

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    Open,
    Close,
    Comma,
    Eq,
}

fn lex(s: &str) -> Result<Vec<Tok>, String> {
    let b: Vec<char> = s.chars().collect();
    let mut out = Vec::new();
    let mut k = 0;
    while k < b.len() {
        let c = b[k];
        if c.is_whitespace() {
            k += 1;
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
        } else {
            k += 1;
            match c {
                '(' => out.push(Tok::Open),
                ')' => out.push(Tok::Close),
                ',' => out.push(Tok::Comma),
                '=' => out.push(Tok::Eq),
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
    Num(f64),
    Var(String),
    Neg(Box<Expr>),
    Bin(char, Box<Expr>, Box<Expr>),
    Call(String, Vec<Expr>),
}

/// Names that parse as function calls rather than implicit multiplication.
pub const FUNCS: [&str; 13] = [
    "exp", "ln", "sin", "cos", "tan", "sqrt", "abs", "arg", "conj", "re", "im", "polar", "pow",
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
            Expr::Call(f, args) => {
                let a: Result<Vec<Cx>, String> = args.iter().map(|e| e.eval(env)).collect();
                let a = a?;
                let one = |n: usize| -> Result<Cx, String> {
                    a.get(n).copied().ok_or_else(|| format!("'{f}' needs more arguments"))
                };
                match f.as_str() {
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
                    "arg" => Cx::new(one(0)?.arg(), 0.0),
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

    fn expr(&mut self) -> Result<Expr, String> {
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
