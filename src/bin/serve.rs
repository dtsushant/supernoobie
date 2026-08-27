//! # Live console â€” Axum + Tokio + HTMX
//!
//!   cargo run --features serve --bin serve
//!   -> http://127.0.0.1:3000
//!
//! ## How it works
//!
//! A **Tokio task** advances the simulation on a fixed 16 ms wall-clock tick,
//! independently of anything the browser does. The browser is only a viewer:
//! HTMX polls `/frame`, the server re-renders the SVG **server-side** from the
//! current state, and swaps it in. There is no simulation logic in JavaScript
//! at all â€” no `requestAnimationFrame`, no client-side physics.
//!
//! That split is the point. The Rust process owns the truth; the page is a
//! window onto it. Any other client (curl, a second browser tab, a test)
//! sees exactly the same machine.
//!
//! The polling is **self-terminating**: when the sim is paused the fragment
//! comes back without an `hx-trigger`, so the browser stops asking. Pressing
//! Run returns a fragment that has the trigger again.

use axum::{
    extract::{Path, State},
    response::Html,
    routing::{get, post},
    Router,
};
use recursion1::dynamics::{Integrator, Physics, Sim};
use recursion1::pulley::Config;
use recursion1::svg;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Everything the process knows. One mutex, because the state is small and the
/// tick is 16 ms â€” there is nothing here worth a lock-free scheme.
struct App {
    cfg: Config,
    phys: Physics,
    sim: Sim,
    integrator: Integrator,
    running: bool,
    /// Simulated seconds per wall-clock second.
    speed: f64,
    /// Angle the machine is released from. A spring-loaded machine sits at
    /// `theta_eq = gravity_torque / k`, which for a stiff spring is
    /// essentially zero - so releasing from rest at 0 gives an oscillation
    /// far too small to see. The presets pull it back first.
    start_theta: f64,
    /// Peak |energy - E0| seen since the last reset, so integrator drift is
    /// visible without staring at the number.
    drift_peak: f64,
    energy0: f64,
    /// Largest kinetic energy seen. Used as the DENOMINATOR for drift, because
    /// total energy is measured from `theta = 0` and so starts at exactly zero
    /// - a percentage of it would be meaningless.
    ke_peak: f64,
}

type Shared = Arc<Mutex<App>>;

impl App {
    fn new() -> Self {
        let cfg = Config::default();
        let phys = Physics::default();
        let sim = Sim::new(cfg, phys, 0.0, 0.0);
        let e0 = sim.energy();
        App {
            cfg,
            phys,
            sim,
            integrator: Integrator::Verlet,
            running: false,
            speed: 4.0,
            start_theta: 0.0,
            drift_peak: 0.0,
            energy0: e0,
            ke_peak: 0.0,
        }
    }

    /// Rebuild the simulation from the current config, released from
    /// `start_theta` at zero speed.
    fn reset(&mut self) {
        self.sim = Sim::new(self.cfg, self.phys, self.start_theta, 0.0);
        self.energy0 = self.sim.energy();
        self.drift_peak = 0.0;
        self.ke_peak = 0.0;
    }

    /// Push the current config/physics into the running sim without stopping
    /// it, so sliders take effect live.
    fn sync(&mut self) {
        self.sim.cfg = self.cfg;
        self.sim.phys = self.phys;
    }
}

#[tokio::main]
async fn main() {
    let state: Shared = Arc::new(Mutex::new(App::new()));

    // ---- the simulation loop: a Tokio task, not a browser timer -----------
    {
        let state = state.clone();
        tokio::spawn(async move {
            const TICK: Duration = Duration::from_millis(16);
            const DT: f64 = 5e-4; // fixed physics step
            let mut ticker = tokio::time::interval(TICK);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let mut a = state.lock().unwrap();
                if !a.running {
                    continue;
                }
                // Advance by however much simulated time this tick is worth.
                // Capped so a huge speed cannot stall the server.
                let want = TICK.as_secs_f64() * a.speed;
                let steps = ((want / DT) as usize).min(20_000);
                let method = a.integrator;
                for _ in 0..steps {
                    a.sim.step(DT, method);
                }
                let d = (a.sim.energy() - a.energy0).abs();
                if d > a.drift_peak {
                    a.drift_peak = d;
                }
                let ke = a.sim.kinetic(a.sim.omega);
                if ke > a.ke_peak {
                    a.ke_peak = ke;
                }
            }
        });
    }

    let app = Router::new()
        .route("/", get(page))
        .route("/frame", get(frame))
        .route("/toggle", post(toggle))
        .route("/reset", post(reset))
        .route("/nudge/{dir}", post(nudge))
        .route("/integrator/{name}", post(set_integrator))
        .route("/preset/{name}", post(preset))
        .route("/config", post(set_config))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await.unwrap();
    println!("Recursion I console -> http://127.0.0.1:3000");
    axum::serve(listener, app).await.unwrap();
}

// ===========================================================================
// handlers
// ===========================================================================

async fn page(State(s): State<Shared>) -> Html<String> {
    let a = s.lock().unwrap();
    Html(full_page(&a))
}

async fn frame(State(s): State<Shared>) -> Html<String> {
    let a = s.lock().unwrap();
    Html(stage(&a))
}

async fn toggle(State(s): State<Shared>) -> Html<String> {
    let mut a = s.lock().unwrap();
    a.running = !a.running;
    Html(stage(&a))
}

async fn reset(State(s): State<Shared>) -> Html<String> {
    let mut a = s.lock().unwrap();
    a.reset();
    a.running = false;
    Html(app_body(&a))
}

/// Kick the crank â€” an instantaneous change in angular velocity.
async fn nudge(State(s): State<Shared>, Path(dir): Path<String>) -> Html<String> {
    let mut a = s.lock().unwrap();
    let d = if dir == "up" { 1.0 } else { -1.0 };
    a.sim.omega += d * 0.35;
    Html(stage(&a))
}

async fn set_integrator(State(s): State<Shared>, Path(name): Path<String>) -> Html<String> {
    let mut a = s.lock().unwrap();
    a.integrator = match name.as_str() {
        "euler" => Integrator::ExplicitEuler,
        "semi" => Integrator::SemiImplicitEuler,
        "rk4" => Integrator::Rk4,
        _ => Integrator::Verlet,
    };
    Html(app_body(&a))
}

/// Named starting points, so the interesting regimes are one click away.
async fn preset(State(s): State<Shared>, Path(name): Path<String>) -> Html<String> {
    let mut a = s.lock().unwrap();
    let base = Config::default();
    match name.as_str() {
        // pure Atwood: no spring, heavy imbalance, bouncy end stops
        "atwood" => {
            a.cfg = Config { m1: 2.0, m2: 5.0, ..base };
            a.phys = Physics { spring_k: 0.0, damping_c: 0.0, restitution: 0.55, ..Physics::default() };
            a.speed = 6.0;
            a.start_theta = 0.0;
        }
        // undamped spring: oscillates forever, so integrator drift is exposed
        "oscillator" => {
            a.cfg = Config { m1: 3.0, m2: 1.0, ..base };
            a.phys = Physics { spring_k: 2.0e6, damping_c: 0.0, restitution: 0.0, ..Physics::default() };
            a.speed = 1.0;
            a.start_theta = 0.9; // pulled back, so the swing is visible
        }
        // underdamped: the complex lambda has both parts, decay AND rotation
        "damped" => {
            a.cfg = Config { m1: 3.0, m2: 1.0, ..base };
            a.phys = Physics { spring_k: 2.0e6, damping_c: 120_000.0, restitution: 0.0, ..Physics::default() };
            a.speed = 1.0;
            a.start_theta = 0.9;
        }
        // overdamped: lambda goes real, the rotation disappears entirely
        "overdamped" => {
            a.cfg = Config { m1: 3.0, m2: 1.0, ..base };
            a.phys = Physics { spring_k: 2.0e6, damping_c: 900_000.0, restitution: 0.0, ..Physics::default() };
            a.speed = 1.0;
            a.start_theta = 0.9;
        }
        _ => {
            a.cfg = base;
            a.phys = Physics::default();
            a.speed = 4.0;
            a.start_theta = 0.0;
        }
    }
    a.reset();
    a.running = true;
    Html(app_body(&a))
}

/// Slider changes. The body is `key=value&key=value` with numeric values, so a
/// four-line parser is enough â€” no serde, no percent decoding needed.
async fn set_config(State(s): State<Shared>, body: String) -> Html<String> {
    let mut a = s.lock().unwrap();
    for (k, v) in parse_form(&body) {
        let Ok(x) = v.parse::<f64>() else { continue };
        match k.as_str() {
            "m1" => a.cfg.m1 = x,
            "m2" => a.cfg.m2 = x,
            "r_a" => a.cfg.r_a = x,
            "r_b" => a.cfg.r_b = x,
            "sep_x" => a.cfg.sep_x = x,
            "sep_y" => a.cfg.sep_y = x,
            "rope" => a.cfg.rope_len = x,
            "teeth" => a.cfg.teeth = x as usize,
            "k" => a.phys.spring_k = x,
            "c" => a.phys.damping_c = x,
            "e" => a.phys.restitution = x,
            "ga" => a.phys.gear_mass_a = x,
            "gb" => a.phys.gear_mass_b = x,
            "speed" => a.speed = x,
            _ => {}
        }
    }
    a.sync();
    Html(stage(&a))
}

fn parse_form(body: &str) -> Vec<(String, String)> {
    body.split('&')
        .filter_map(|kv| kv.split_once('='))
        .map(|(k, v)| (k.to_string(), v.replace('+', " ")))
        .collect()
}

// ===========================================================================
// rendering
// ===========================================================================

/// The polled region: picture, readouts, transport. Carries its own polling
/// trigger only while the machine is running.
fn stage(a: &App) -> String {
    let st = a.cfg.solve(a.sim.theta);
    let trigger = if a.running {
        r#"hx-trigger="every 100ms""#
    } else {
        ""
    };
    let picture = svg::render_static(&a.cfg, a.sim.theta, "live â€” state owned by the Rust process");

    let lambda = match a.sim.lambda() {
        Some(l) => format!(
            r#"<span class="md">{:.3} + {:.3}i</span> <span class="dim">decay x rotation</span>"#,
            l.re, l.im
        ),
        None if a.phys.spring_k <= 0.0 => r#"<span class="dim">no spring - no oscillation</span>"#.into(),
        None => r#"<span class="warn">real</span> <span class="dim">- rotation gone (overdamped)</span>"#.into(),
    };
    let zeta = a.sim.zeta();
    let zeta_txt = if zeta.is_finite() {
        format!("{zeta:.3}")
    } else {
        "-".into()
    };
    // Relative to the peak kinetic energy - the only meaningful scale here.
    let drift_pct = if a.ke_peak > 1e-9 {
        format!("{:.2}%", 100.0 * a.drift_peak / a.ke_peak)
    } else {
        "-".to_string()
    };

    format!(
        r##"<div id="stage" hx-get="/frame" {trigger} hx-swap="outerHTML">
  <div class="pic">{picture}</div>
  <div class="transport">
    <button class="primary" hx-post="/toggle" hx-target="#stage" hx-swap="outerHTML">{play}</button>
    <button hx-post="/nudge/down" hx-target="#stage" hx-swap="outerHTML">&#8595; nudge</button>
    <button hx-post="/nudge/up" hx-target="#stage" hx-swap="outerHTML">&#8593; nudge</button>
    <button hx-post="/reset" hx-target="#app" hx-swap="outerHTML">reset</button>
    <span class="dim">{status}</span>
  </div>
  <div class="readout">
    <div><span>t</span><b>{t:.2} s</b></div>
    <div><span>theta</span><b>{th:+.4} rad</b></div>
    <div><span>omega</span><b>{om:+.4} rad/s</b></div>
    <div><span>h1</span><b>{h1:.1}</b></div>
    <div><span>h2</span><b>{h2:.1}</b></div>
    <div><span>energy</span><b>{en:.1}</b></div>
    <div><span>M_eff</span><b>{me:.0}</b></div>
    <div><span>wn</span><b>{wn:.3} rad/s</b></div>
    <div><span>zeta</span><b>{zt}</b></div>
    <div><span>peak drift</span><b>{drift}</b></div>
  </div>
  <div class="lam"><span>lambda =</span> {lambda}</div>
</div>"##,
        trigger = trigger,
        picture = picture,
        play = if a.running { "&#10074;&#10074; pause" } else { "&#9654; run" },
        status = format!("{} &middot; {:.1}x speed", a.integrator.name(), a.speed),
        t = a.sim.t,
        th = a.sim.theta,
        om = a.sim.omega,
        h1 = st.h1,
        h2 = st.h2,
        en = a.sim.energy(),
        me = a.sim.m_eff(),
        wn = a.sim.omega_n(),
        zt = zeta_txt,
        drift = drift_pct,
        lambda = lambda,
    )
}

fn sl(name: &str, label: &str, min: f64, max: f64, step: f64, val: f64, unit: &str) -> String {
    format!(
        r#"<label><span>{label}<b>{val:.3} {unit}</b></span>
<input type="range" name="{name}" min="{min}" max="{max}" step="{step}" value="{val}"></label>"#
    )
}

fn controls(a: &App) -> String {
    let btn = |slug: &str, label: &str, active: bool| {
        format!(
            r##"<button class="{}" hx-post="/integrator/{slug}" hx-target="#app" hx-swap="outerHTML">{label}</button>"##,
            if active { "chip on" } else { "chip" }
        )
    };
    let i = a.integrator;
    format!(
        r##"<div class="panel">
  <section>
    <h2>preset</h2>
    <div class="row">
      <button class="chip" hx-post="/preset/atwood" hx-target="#app" hx-swap="outerHTML">Atwood + bounce</button>
      <button class="chip" hx-post="/preset/oscillator" hx-target="#app" hx-swap="outerHTML">undamped spring</button>
      <button class="chip" hx-post="/preset/damped" hx-target="#app" hx-swap="outerHTML">underdamped</button>
      <button class="chip" hx-post="/preset/overdamped" hx-target="#app" hx-swap="outerHTML">overdamped</button>
    </div>
  </section>

  <section>
    <h2>integrator</h2>
    <div class="row">{b1}{b2}{b3}{b4}</div>
    <p class="dim">Run the undamped spring on <b>explicit Euler</b> and watch
    the peak-drift readout climb. Switch to semi-implicit: same cost, no drift.</p>
  </section>

  <form class="sliders" hx-post="/config" hx-trigger="input changed delay:60ms"
        hx-target="#stage" hx-swap="outerHTML">
    <section>
      <h2>masses &amp; geometry</h2>
      {s_m1}{s_m2}{s_ra}{s_rb}{s_sx}{s_sy}{s_rope}{s_teeth}
    </section>
    <section>
      <h2>physics</h2>
      {s_ga}{s_gb}{s_k}{s_c}{s_e}
    </section>
    <section>
      <h2>time</h2>
      {s_speed}
    </section>
  </form>
</div>"##,
        b1 = btn("euler", "explicit Euler", i == Integrator::ExplicitEuler),
        b2 = btn("semi", "semi-implicit", i == Integrator::SemiImplicitEuler),
        b3 = btn("verlet", "Verlet", i == Integrator::Verlet),
        b4 = btn("rk4", "RK4", i == Integrator::Rk4),
        s_m1 = sl("m1", "m&#8321; (left)", 0.5, 10.0, 0.1, a.cfg.m1, "kg"),
        s_m2 = sl("m2", "m&#8322; (right)", 0.5, 10.0, 0.1, a.cfg.m2, "kg"),
        s_ra = sl("r_a", "radius A", 26.0, 105.0, 1.0, a.cfg.r_a, ""),
        s_rb = sl("r_b", "radius B", 26.0, 105.0, 1.0, a.cfg.r_b, ""),
        s_sx = sl("sep_x", "separation &#916;x", 150.0, 520.0, 1.0, a.cfg.sep_x, ""),
        s_sy = sl("sep_y", "separation &#916;y", -160.0, 160.0, 1.0, a.cfg.sep_y, ""),
        s_rope = sl("rope", "rope length L", 620.0, 1500.0, 5.0, a.cfg.rope_len.min(1500.0), ""),
        s_teeth = sl("teeth", "teeth N", 6.0, 28.0, 1.0, a.cfg.teeth as f64, ""),
        s_ga = sl("ga", "gear A mass", 0.0, 20.0, 0.1, a.phys.gear_mass_a, "kg"),
        s_gb = sl("gb", "gear B mass", 0.0, 20.0, 0.1, a.phys.gear_mass_b, "kg"),
        s_k = sl("k", "spring k", 0.0, 4.0e6, 2.0e4, a.phys.spring_k, ""),
        s_c = sl("c", "damping c", 0.0, 1.0e6, 1.0e4, a.phys.damping_c, ""),
        s_e = sl("e", "restitution", 0.0, 1.0, 0.01, a.phys.restitution, ""),
        s_speed = sl("speed", "speed", 0.1, 20.0, 0.1, a.speed, "x"),
    )
}

fn app_body(a: &App) -> String {
    format!(
        r##"<div id="app">{stage}{controls}</div>"##,
        stage = stage(a),
        controls = controls(a)
    )
}

fn full_page(a: &App) -> String {
    format!(
        r##"<!doctype html>
<html lang="en"><head>
<meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Recursion I â€” live console</title>
<script src="https://unpkg.com/htmx.org@2.0.4"></script>
<style>
  :root{{
    --bg:#0B1017; --surface:#131C25; --sunk:#0E161E;
    --ink:#E3E9EF; --soft:#94A1AE; --faint:#6B7987; --line:#22303C;
    --real:#E0A44A; --imag:#4FBCD4; --mod:#E585AC;
    --mono:ui-monospace,'Cascadia Mono','Segoe UI Mono',Consolas,monospace;
  }}
  *{{box-sizing:border-box}}
  body{{margin:0;background:var(--bg);color:var(--ink);line-height:1.5;
    font-family:ui-sans-serif,system-ui,'Segoe UI',Roboto,Arial,sans-serif}}
  .wrap{{max-width:1400px;margin:0 auto;padding:clamp(12px,2vw,26px);
    display:flex;flex-direction:column;gap:14px}}
  header h1{{font-family:Georgia,serif;font-weight:600;margin:0;font-size:clamp(1.3rem,3vw,1.9rem)}}
  header p{{margin:4px 0 0;color:var(--soft);font-size:.9rem;max-width:80ch}}
  #app{{display:grid;gap:14px;grid-template-columns:1fr}}
  @media(min-width:1050px){{#app{{grid-template-columns:minmax(0,1.75fr) minmax(300px,1fr)}}}}
  #stage{{display:flex;flex-direction:column;gap:10px;min-width:0}}
  .pic{{background:var(--sunk);border:1px solid var(--line);border-radius:11px;padding:8px;overflow-x:auto}}
  .pic svg{{display:block;width:100%;height:auto}}
  .transport{{display:flex;flex-wrap:wrap;gap:7px;align-items:center}}
  button{{font:inherit;font-size:.83rem;font-weight:600;color:var(--ink);background:var(--surface);
    border:1px solid var(--line);border-radius:7px;padding:7px 13px;cursor:pointer}}
  button:hover{{border-color:var(--mod)}}
  button:focus-visible{{outline:2px solid var(--mod);outline-offset:2px}}
  button.primary{{background:var(--mod);border-color:var(--mod);color:#0B1017}}
  .chip{{font-size:.78rem;padding:6px 11px}}
  .chip.on{{background:var(--imag);border-color:var(--imag);color:#0B1017}}
  .readout{{display:grid;gap:6px;grid-template-columns:repeat(auto-fit,minmax(128px,1fr))}}
  .readout div{{background:var(--surface);border:1px solid var(--line);border-radius:8px;
    padding:6px 10px;display:flex;flex-direction:column}}
  .readout span{{font-size:.66rem;letter-spacing:.1em;text-transform:uppercase;color:var(--faint)}}
  .readout b{{font-family:var(--mono);font-size:.9rem;font-variant-numeric:tabular-nums}}
  .lam{{background:var(--surface);border:1px solid var(--line);border-radius:8px;padding:8px 12px;
    font-family:var(--mono);font-size:.85rem}}
  .lam span:first-child{{color:var(--faint)}}
  .md{{color:var(--mod)}} .warn{{color:var(--real)}} .dim{{color:var(--faint);font-size:.8rem}}
  .panel{{display:flex;flex-direction:column;gap:12px;min-width:0}}
  section{{background:var(--surface);border:1px solid var(--line);border-radius:11px;
    padding:12px 13px;display:flex;flex-direction:column;gap:9px}}
  h2{{font-family:Georgia,serif;font-size:.92rem;margin:0;font-weight:600}}
  .row{{display:flex;flex-wrap:wrap;gap:6px}}
  .sliders{{display:contents}}
  label{{display:flex;flex-direction:column;gap:2px}}
  label span{{display:flex;justify-content:space-between;font-size:.78rem;color:var(--soft)}}
  label b{{font-family:var(--mono);font-size:.75rem;color:var(--ink);font-variant-numeric:tabular-nums}}
  input[type=range]{{width:100%;accent-color:var(--mod);margin:0}}
  p{{margin:0}}
  footer{{border-top:1px solid var(--line);padding-top:12px;color:var(--faint);font-size:.79rem}}
  code{{font-family:var(--mono);background:var(--sunk);border:1px solid var(--line);
    border-radius:4px;padding:1px 5px;font-size:.85em}}
</style></head><body>
<div class="wrap">
  <header>
    <h1>Recursion I â€” live console</h1>
    <p>The simulation runs in a Tokio task at a fixed 0.5&nbsp;ms step. HTMX polls
    <code>/frame</code>; the SVG is rendered <b>server-side</b> in Rust and swapped in.
    No physics in the browser. Pause and the polling stops by itself.</p>
  </header>
  {body}
  <footer>
    <code>cargo run --features serve --bin serve</code> &nbsp;Â·&nbsp;
    the maths core (<code>complex.rs</code>, <code>pulley.rs</code>, <code>dynamics.rs</code>)
    still has zero dependencies â€” Axum and Tokio are behind the <code>serve</code> feature.
  </footer>
</div>
</body></html>"##,
        body = app_body(a)
    )
}

