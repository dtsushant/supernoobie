//! Recursion I - iteration 1
//!
//!   cargo test          verify the mathematics
//!   cargo run           sweep the crank, print the table, write pulley.svg
//!
//! Read `complex.rs` first (the number type), then `pulley.rs` (the model).
//! `svg.rs` is only drawing and can be ignored while you are learning the maths.

use recursion1::complex::Cx;
use recursion1::dynamics::{Integrator, Physics, Sim};
use recursion1::pulley::{self, Config};
use recursion1::svg;

fn main() {
    let cfg = Config::default();
    let base = cfg.solve(0.0);

    println!("=====================================================================");
    println!(" RECURSION I - pulley system on the complex plane (iteration 1)");
    println!("=====================================================================");
    println!(
        " gear A  centre {}   radius {:.0}   teeth {}",
        base.a, cfg.r_a, cfg.teeth
    );
    println!(" gear B  centre {}   radius {:.0}", base.b, cfg.r_b);
    println!(" rope L = {:.0}   masses {:.1} kg / {:.1} kg", cfg.rope_len, cfg.m1, cfg.m2);

    // ---- the geometry, worked once ---------------------------------------
    let d = base.b - base.a;
    println!();
    println!("--- geometry (fixed: depends on WHERE the gears are, not how far turned)");
    println!(" centre-to-centre  d = {}          |d| = {:.2}", d, d.abs());
    println!(" tangent offset    u = {}   (a unit complex number)", base.tangent_dir);
    println!(
        "                     arg(u) = {:.4} rad = {:.2} deg",
        base.tangent_angle,
        base.tangent_angle.to_degrees()
    );
    if (cfg.r_a - cfg.r_b).abs() < 1e-9 {
        println!("   radii are equal, so u is EXACTLY i * d_hat - a 90 degree turn");
    } else {
        let perp = Cx::I * d.unit();
        println!(
            "   radii differ, so u tilts off i*d_hat = {} by {:.2} deg",
            perp,
            (base.tangent_angle - perp.arg()).to_degrees()
        );
    }
    println!(" straight run      {:.2}", base.seg_len);
    println!(
        " wrapped on A      {:.4} rad  ->  arc = r*theta = {:.2}",
        base.wrap_a,
        cfg.r_a * base.wrap_a.abs()
    );
    println!(
        " wrapped on B      {:.4} rad  ->  arc = r*theta = {:.2}",
        base.wrap_b,
        cfg.r_b * base.wrap_b.abs()
    );
    println!(" fixed path total  {:.2}", base.fixed);
    println!(
        " straight run as a line:   y = {:.4} x + {:.2}",
        base.slope, base.intercept
    );

    // ---- the crank sweep --------------------------------------------------
    println!();
    println!("--- cranking: theta trades h1 against h2, one for one");
    println!(
        "  theta(rad)   gear B     h1        h2      h1+h2+fixed    check"
    );
    println!(
        "  ----------  --------  --------  --------  -----------   -------"
    );
    let tm = cfg.theta_max();
    for k in -5..=5 {
        let th = tm * k as f64 / 5.0;
        let s = cfg.solve(th);
        let total = s.rope_total();
        let ok = if (total - cfg.rope_len).abs() < 1e-9 { "OK" } else { "BROKEN" };
        println!(
            "  {:+9.4}  {:+8.4}  {:8.2}  {:8.2}  {:11.4}   {}",
            s.theta, s.gear_b_angle, s.h1, s.h2, total, ok
        );
    }
    println!("  theta is clamped at +/-{:.4} rad, where a weight would reach its gear.", tm);

    let over = cfg.solve(tm * 2.0);
    println!(
        "  asking for theta = {:.2} gives back {:.4} (limit {:.4}, clamped = {}), min hang = {:.2} >= {:.0}",
        tm * 2.0, over.theta, over.theta_max, over.clamped, over.h1.min(over.h2), pulley::MIN_HANG
    );

    // =====================================================================
    // ITERATION 2 - the crank is no longer imposed. Let go and integrate.
    // =====================================================================
    println!();
    println!("=====================================================================");
    println!(" ITERATION 2 - dynamics");
    println!("=====================================================================");

    let phys = Physics::default();
    let s0 = Sim::new(cfg, phys, 0.0, 0.0);
    println!(" effective inertia  M_eff = {:.1}", s0.m_eff());
    println!(
        "   = (m1+m2)*r_a^2 [{:.1}]  +  I_a [{:.1}]  +  I_b*(r_a/r_b)^2 [{:.1}]",
        (cfg.m1 + cfg.m2) * cfg.r_a * cfg.r_a,
        0.5 * phys.gear_mass_a * cfg.r_a * cfg.r_a,
        0.5 * phys.gear_mass_b * cfg.r_b * cfg.r_b * (cfg.r_a / cfg.r_b).powi(2),
    );
    println!(" gravity torque           = {:.1}", s0.gravity_torque());
    println!(" angular accel at rest    = {:.5} rad/s^2", s0.accel(0.0, 0.0));
    println!(
        " ideal (massless) gears would give {:.5} rad/s^2 - the gears cost {:.1}%",
        s0.gravity_torque() / ((cfg.m1 + cfg.m2) * cfg.r_a * cfg.r_a),
        100.0 * (1.0 - s0.accel(0.0, 0.0) / (s0.gravity_torque() / ((cfg.m1 + cfg.m2) * cfg.r_a * cfg.r_a)))
    );

    // ---- release it and watch ---------------------------------------------
    println!();
    println!("--- released from rest, bouncing off the end stops (e = {:.2})", phys.restitution);
    println!("     t(s)     theta      omega       h1        h2      energy");
    println!("   -------  ---------  ---------  --------  --------  ---------");
    let mut sim = Sim::new(cfg, phys, 0.0, 0.0);
    let dt = 1e-3;
    for k in 0..=10 {
        if k > 0 {
            for _ in 0..2000 {
                sim.step(dt, Integrator::Verlet);
            }
        }
        let st = cfg.solve(sim.theta);
        println!(
            "   {:7.3}  {:+9.4}  {:+9.4}  {:8.2}  {:8.2}  {:9.1}",
            sim.t, sim.theta, sim.omega, st.h1, st.h2, sim.energy()
        );
    }

    // =====================================================================
    // Add a return spring: the machine becomes a damped oscillator, whose
    // exact solution is a COMPLEX EXPONENTIAL.
    // =====================================================================
    println!();
    println!("--- with a return spring: theta_ddot + 2*zeta*wn*theta_dot + wn^2*theta = const");
    let osc_cfg = Config { rope_len: 40_000.0, m1: 3.0, m2: 1.0, ..cfg }; // no end stops
    for (label, c) in [("undamped", 0.0), ("underdamped", 120_000.0), ("critical", 0.0), ("overdamped", 0.0)] {
        let base_phys = Physics { spring_k: 2.0e6, damping_c: c, restitution: 0.0, ..phys };
        let probe = Sim::new(osc_cfg, base_phys, 0.4, 0.0);
        let c_crit = 2.0 * (base_phys.spring_k * probe.m_eff()).sqrt();
        let c_use = match label {
            "critical" => c_crit,
            "overdamped" => 2.5 * c_crit,
            _ => c,
        };
        let p = Physics { damping_c: c_use, ..base_phys };
        let s = Sim::new(osc_cfg, p, 0.4, 0.0);
        print!(
            "  {:12}  c = {:8.0}  zeta = {:6.3}  wn = {:.4} rad/s",
            label, c_use, s.zeta(), s.omega_n()
        );
        match s.lambda() {
            Some(l) => println!("   lambda = {}   <- decay x rotation", l),
            None => println!("   lambda real - no rotation left"),
        }
    }

    // ---- integrator bake-off, graded against the exact solution -----------
    println!();
    println!("--- integrator accuracy: undamped spring, exact answer known");
    let osc_phys = Physics { spring_k: 2.0e6, damping_c: 0.0, restitution: 0.0, ..phys };
    let t_end = 4.0;
    for dt in [2e-2, 5e-3, 1e-3] {
        let steps = (t_end / dt) as usize;
        println!("  dt = {dt:<8}  ({steps} steps to t = {t_end})");
        println!("     integrator            theta(T)      error      energy drift");
        println!("     -------------------  ----------  ----------   ------------");
        for m in Integrator::ALL {
            let mut s = Sim::new(osc_cfg, osc_phys, 0.4, 0.0);
            let e0 = s.energy();
            s.run(dt, steps, m);
            let want = s.exact(s.t);
            let drift = (s.energy() - e0) / e0;
            println!(
                "     {:19}  {:+10.6}  {:10.2e}   {:+11.4}%  {}",
                m.name(),
                s.theta,
                (s.theta - want).abs(),
                drift * 100.0,
                if m.is_symplectic() { "symplectic" } else { "" }
            );
        }
        println!("     {:19}  {:+10.6}  {:>10}", "EXACT", Sim::new(osc_cfg, osc_phys, 0.4, 0.0).exact(t_end), "-");
        println!();
    }

    // ---- the long-run lesson ----------------------------------------------
    println!("--- long run (t = 200 s, dt = 2 ms): who is still trustworthy?");
    println!("     integrator            energy / start");
    println!("     -------------------  ---------------");
    for m in Integrator::ALL {
        let mut s = Sim::new(osc_cfg, osc_phys, 0.4, 0.0);
        let e0 = s.energy();
        s.run(2e-3, 100_000, m);
        let ratio = s.energy() / e0;
        let verdict = if ratio > 2.0 {
            "EXPLODING"
        } else if ratio < 0.5 {
            "dying out"
        } else {
            "stable"
        };
        println!("     {:19}  {:>12.4}   {}", m.name(), ratio, verdict);
    }
    println!("   Explicit Euler manufactures energy from nothing. Reordering its");
    println!("   two lines (semi-implicit) fixes it completely - same cost.");

    // ---- draw both ---------------------------------------------------------
    println!();
    let out1 = svg::render(&cfg, 96);
    write_svg("pulley.svg", &out1);

    let mut anim = Sim::new(cfg, phys, 0.0, 0.0);
    let mut frames = Vec::new();
    for k in 0..600 {
        for _ in 0..25 {
            anim.step(2e-3, Integrator::Verlet);
        }
        if k % 2 == 0 {
            frames.push(anim.theta);
        }
    }
    let out2 = svg::render_trajectory(&cfg, &frames, 7.2, "simulated: released from rest, Verlet, bouncing (iteration 2)");
    write_svg("pulley_sim.svg", &out2);
}

fn write_svg(name: &str, body: &str) {
    match std::fs::write(name, body) {
        Ok(_) => println!("wrote {name} ({} bytes) - open it in a browser", body.len()),
        Err(e) => eprintln!("could not write {name}: {e}"),
    }
}



