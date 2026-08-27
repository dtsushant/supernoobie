//! Renders the system to a self-contained **animated SVG** (SMIL), so it can
//! be opened in any browser with no GUI toolkit, no GPU and no dependencies.
//!
//! Note what is static and what moves. Because the gear centres never move,
//! the two wrapped arcs and the straight run between them are FIXED - only
//! the gear rotation and the two hanging lengths change. That is the rope
//! constraint showing up as a rendering optimisation.

use crate::complex::Cx;
use crate::pulley::Config;
use std::f64::consts::PI;

const W: f64 = 1000.0;
const H: f64 = 780.0;
const ORX: f64 = 500.0;
const ORY: f64 = 250.0;

fn sx(p: Cx) -> f64 {
    ORX + p.re
}
fn sy(p: Cx) -> f64 {
    ORY - p.im // SVG y points DOWN; world y points UP
}

fn arc_points(centre: Cx, r: f64, a0: f64, a1: f64, steps: usize) -> String {
    (0..=steps)
        .map(|k| {
            let t = a0 + (a1 - a0) * k as f64 / steps as f64;
            let p = centre + Cx::expi(t).scale(r);
            format!("{:.2},{:.2}", sx(p), sy(p))
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Build the animated SVG from a scripted sine sweep (iteration 1 - the crank
/// angle is imposed, nothing causes it).
pub fn render(cfg: &Config, frames: usize) -> String {
    let tm = cfg.theta_max();
    let thetas: Vec<f64> = (0..=frames)
        .map(|k| tm * (k as f64 / frames as f64 * 2.0 * PI).sin())
        .collect();
    render_trajectory(cfg, &thetas, 7.0, "scripted sweep (iteration 1)")
}

/// Build the animated SVG from an actual simulated trajectory (iteration 2 -
/// the angles came out of an ODE, not a formula).
pub fn render_trajectory(cfg: &Config, thetas: &[f64], dur_s: f64, caption: &str) -> String {
    let dur = format!("{dur_s}s");
    let states: Vec<_> = thetas.iter().map(|&t| cfg.solve(t)).collect();
    // Frame 0 is what gets DRAWN; the animations are offsets from it. With a
    // single frame there is nothing to animate and this is a static picture.
    let base = *states.first().expect("render needs at least one frame");
    let animate = states.len() > 1;

    // ---- animation value lists -------------------------------------------
    // Rotations are RELATIVE to frame 0, because frame 0 is already drawn at
    // its own angle. (SVG turns clockwise, world turns anticlockwise: negate.)
    let a_rot = states
        .iter()
        .map(|s| {
            format!("{:.3} {:.2} {:.2}", -(s.theta - base.theta).to_degrees(), sx(s.a), sy(s.a))
        })
        .collect::<Vec<_>>()
        .join(";");
    let b_rot = states
        .iter()
        .map(|s| {
            format!(
                "{:.3} {:.2} {:.2}",
                -(s.gear_b_angle - base.gear_b_angle).to_degrees(),
                sx(s.b),
                sy(s.b)
            )
        })
        .collect::<Vec<_>>()
        .join(";");
    let y1_vals = states
        .iter()
        .map(|s| format!("{:.2}", sy(s.w1)))
        .collect::<Vec<_>>()
        .join(";");
    let y2_vals = states
        .iter()
        .map(|s| format!("{:.2}", sy(s.w2)))
        .collect::<Vec<_>>()
        .join(";");
    let w1_tr = states
        .iter()
        .map(|s| format!("0 {:.2}", sy(s.w1) - sy(base.w1)))
        .collect::<Vec<_>>()
        .join(";");
    let w2_tr = states
        .iter()
        .map(|s| format!("0 {:.2}", sy(s.w2) - sy(base.w2)))
        .collect::<Vec<_>>()
        .join(";");
    let th_txt = states
        .iter()
        .map(|s| format!("theta = {:+.2} rad", s.theta))
        .collect::<Vec<_>>()
        .join(";");

    let anim = |attr: &str, vals: &str| {
        format!(
            r#"<animate attributeName="{attr}" values="{vals}" dur="{dur}" repeatCount="indefinite" calcMode="linear"/>"#
        )
    };
    let anim_tr = |vals: &str| {
        format!(
            r#"<animateTransform attributeName="transform" type="{}" values="{vals}" dur="{dur}" repeatCount="indefinite" calcMode="linear"/>"#,
            "translate"
        )
    };
    let anim_rot = |vals: &str| {
        format!(
            r#"<animateTransform attributeName="transform" type="rotate" values="{vals}" dur="{dur}" repeatCount="indefinite" calcMode="linear"/>"#
        )
    };

    // ---- static geometry --------------------------------------------------
    let wrap_a = arc_points(base.a, cfg.r_a, PI, base.tangent_angle, 48);
    let wrap_b = arc_points(base.b, cfg.r_b, base.tangent_angle, 0.0, 48);
    let rim_a = arc_points(base.a, cfg.r_a, 0.0, 2.0 * PI, 96);
    let rim_b = arc_points(base.b, cfg.r_b, 0.0, 2.0 * PI, 96);

    let teeth_a = teeth_svg(base.a, cfg.r_a, cfg.teeth, base.theta);
    let n_b = ((cfg.teeth as f64 * cfg.r_b / cfg.r_a).round() as usize).max(6);
    let teeth_b = teeth_svg(base.b, cfg.r_b, n_b, base.gear_b_angle);

    // the extended line y = m x + c
    let line = if base.slope.is_finite() {
        let x0 = -ORX;
        let x1 = W - ORX;
        let p0 = Cx::new(x0, base.slope * x0 + base.intercept);
        let p1 = Cx::new(x1, base.slope * x1 + base.intercept);
        format!(
            r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="#8C99A4" stroke-width="1.2" stroke-dasharray="4 7"/>"##,
            sx(p0), sy(p0), sx(p1), sy(p1)
        )
    } else {
        String::new()
    };

    let eq = if base.slope.is_finite() {
        format!("y = {:.3} x {} {:.1}", base.slope,
                if base.intercept >= 0.0 { "+" } else { "-" }, base.intercept.abs())
    } else {
        "x = const".into()
    };

    format!(
r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {W} {H}" width="{W}" height="{H}" font-family="Georgia, serif">
  <rect width="{W}" height="{H}" fill="#0d141b"/>

  <!-- real and imaginary axes -->
  <line x1="0" y1="{ORY}" x2="{W}" y2="{ORY}" stroke="#E0A44A" stroke-width="1.4" stroke-dasharray="7 6" opacity="0.6"/>
  <line x1="{ORX}" y1="0" x2="{ORX}" y2="{H}" stroke="#4FBCD4" stroke-width="1.4" stroke-dasharray="7 6" opacity="0.6"/>
  {line}

  <!-- rope: the fixed part (wrap A, straight run, wrap B) -->
  <polyline points="{wrap_a}" fill="none" stroke="#8CA0B3" stroke-width="3.2" stroke-linecap="round"/>
  <line x1="{tax:.2}" y1="{tay:.2}" x2="{tbx:.2}" y2="{tby:.2}" stroke="#8CA0B3" stroke-width="3.2" stroke-linecap="round"/>
  <polyline points="{wrap_b}" fill="none" stroke="#8CA0B3" stroke-width="3.2" stroke-linecap="round"/>

  <!-- rope: the two hanging parts, whose LENGTH is what the crank changes -->
  <line x1="{pax:.2}" y1="{pay:.2}" x2="{pax:.2}" y2="{w1y:.2}" stroke="#8CA0B3" stroke-width="3.2" stroke-linecap="round">
    {hang1}
  </line>
  <line x1="{pbx:.2}" y1="{pby:.2}" x2="{pbx:.2}" y2="{w2y:.2}" stroke="#8CA0B3" stroke-width="3.2" stroke-linecap="round">
    {hang2}
  </line>

  <!-- gear A: rim static, teeth rotate -->
  <polyline points="{rim_a}" fill="none" stroke="#E3E9EF" stroke-width="2.2"/>
  <g>{teeth_a}{rot_a}</g>
  <circle cx="{ax:.2}" cy="{ay:.2}" r="3.5" fill="#E3E9EF"/>

  <!-- gear B -->
  <polyline points="{rim_b}" fill="none" stroke="#E3E9EF" stroke-width="2.2"/>
  <g>{teeth_b}{rot_b}</g>
  <circle cx="{bx:.2}" cy="{by:.2}" r="3.5" fill="#E3E9EF"/>

  <!-- tangent points -->
  <circle cx="{tax:.2}" cy="{tay:.2}" r="4.5" fill="#0d141b" stroke="#4FBCD4" stroke-width="2.2"/>
  <circle cx="{tbx:.2}" cy="{tby:.2}" r="4.5" fill="#0d141b" stroke="#4FBCD4" stroke-width="2.2"/>
  <line x1="{ax:.2}" y1="{ay:.2}" x2="{tax:.2}" y2="{tay:.2}" stroke="#4FBCD4" stroke-width="1.3" stroke-dasharray="3 5"/>

  <!-- weights -->
  <g>
    <rect x="{w1x:.2}" y="{w1y:.2}" width="{w1w:.1}" height="{w1h:.1}" fill="#131c25" stroke="#E0A44A" stroke-width="2.2"/>
    <text x="{w1cx:.2}" y="{w1ty:.2}" fill="#E0A44A" font-size="14" text-anchor="middle">m1 {m1:.1}kg</text>
    {tr1}
  </g>
  <g>
    <rect x="{w2x:.2}" y="{w2y:.2}" width="{w2w:.1}" height="{w2h:.1}" fill="#131c25" stroke="#E585AC" stroke-width="2.2"/>
    <text x="{w2cx:.2}" y="{w2ty:.2}" fill="#E585AC" font-size="14" text-anchor="middle">m2 {m2:.1}kg</text>
    {tr2}
  </g>

  <!-- labels -->
  <text x="{alx:.2}" y="{aly:.2}" fill="#E3E9EF" font-size="18" font-style="italic">A</text>
  <text x="{blx:.2}" y="{bly:.2}" fill="#E3E9EF" font-size="18" font-style="italic">B</text>

  <text x="20" y="34" fill="#E3E9EF" font-size="17">Recursion I  -  pulley on the complex plane</text>
  <text x="20" y="56" fill="#8C99A4" font-size="13">{eq}   |   rope L = {rope:.0}   |   fixed path = {fixed:.1}   |   teeth = e^(i(theta + 2*pi*k/N))</text>
  <text x="20" y="76" fill="#8C99A4" font-size="13">{caption}</text>
  <text x="20" y="{ttxt}" fill="#E585AC" font-size="15">theta = {th0:+.3} rad{thanim}</text>
</svg>
"##,
        W = W, H = H, ORX = ORX, ORY = ORY,
        line = line,
        wrap_a = wrap_a, wrap_b = wrap_b, rim_a = rim_a, rim_b = rim_b,
        teeth_a = teeth_a, teeth_b = teeth_b,
        rot_a = if animate { anim_rot(&a_rot) } else { String::new() },
        rot_b = if animate { anim_rot(&b_rot) } else { String::new() },
        hang1 = if animate { anim("y2", &y1_vals) } else { String::new() },
        hang2 = if animate { anim("y2", &y2_vals) } else { String::new() },
        tr1 = if animate { anim_tr(&w1_tr) } else { String::new() },
        tr2 = if animate { anim_tr(&w2_tr) } else { String::new() },
        thanim = if animate {
            format!(r#"<animate attributeName="textContent" values="{th_txt}" dur="{dur}" repeatCount="indefinite"/>"#)
        } else {
            String::new()
        },
        ax = sx(base.a), ay = sy(base.a), bx = sx(base.b), by = sy(base.b),
        tax = sx(base.ta), tay = sy(base.ta), tbx = sx(base.tb), tby = sy(base.tb),
        pax = sx(base.pa), pay = sy(base.pa), pbx = sx(base.pb), pby = sy(base.pb),
        w1y = sy(base.w1), w2y = sy(base.w2),
        w1x = sx(base.w1) - (30.0 + cfg.m1 * 4.2) / 2.0,
        w2x = sx(base.w2) - (30.0 + cfg.m2 * 4.2) / 2.0,
        w1w = 30.0 + cfg.m1 * 4.2, w1h = 22.0 + cfg.m1 * 2.6,
        w2w = 30.0 + cfg.m2 * 4.2, w2h = 22.0 + cfg.m2 * 2.6,
        w1cx = sx(base.w1), w1ty = sy(base.w1) + (22.0 + cfg.m1 * 2.6) / 2.0 + 5.0,
        w2cx = sx(base.w2), w2ty = sy(base.w2) + (22.0 + cfg.m2 * 2.6) / 2.0 + 5.0,
        m1 = cfg.m1, m2 = cfg.m2,
        alx = sx(base.a) - 26.0, aly = sy(base.a) - 12.0,
        blx = sx(base.b) + 12.0, bly = sy(base.b) - 12.0,
        eq = eq, rope = cfg.rope_len, fixed = base.fixed,
        caption = caption,
        th0 = base.theta,
        ttxt = H - 22.0,
    )
}

/// A single still frame - what the HTMX console swaps in on every tick.
pub fn render_static(cfg: &Config, theta: f64, caption: &str) -> String {
    render_trajectory(cfg, &[theta], 1.0, caption)
}

fn teeth_svg(centre: Cx, r: f64, n: usize, angle: f64) -> String {
    // Both ends of every tooth come from the model's roots-of-unity helper -
    // the drawing code does not get its own copy of the mathematics.
    let inner = Config::teeth_of(centre, r * 0.98, angle, n);
    let outer = Config::teeth_of(centre, r * 1.16, angle, n);
    inner
        .iter()
        .zip(outer.iter())
        .enumerate()
        .map(|(k, (&p0, &p1))| {
            let colour = if k == 0 { "#E585AC" } else { "#E3E9EF" };
            let wdt = if k == 0 { 3.6 } else { 3.0 };
            format!(
                r#"<line x1="{:.2}" y1="{:.2}" x2="{:.2}" y2="{:.2}" stroke="{}" stroke-width="{}" stroke-linecap="round"/>"#,
                sx(p0), sy(p0), sx(p1), sy(p1), colour, wdt
            )
        })
        .collect::<Vec<_>>()
        .join("")
}
