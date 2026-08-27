//! # Iteration 2 - dynamics
//!
//! In iteration 1 the crank angle `theta` was a number you *passed in*. Here it
//! becomes **state that evolves**: the masses pull, the gears resist, and we
//! integrate forward in time.
//!
//! ## Deriving the equation of motion (Lagrangian, because the sign errors
//! ## in a free-body diagram are brutal)
//!
//! Take `theta` (gear A's angle) as the single generalised coordinate.
//! Positive `theta` pays rope out on the left, so **m1 descends**.
//!
//! **Kinetic energy.** Both masses move at rope speed `v = r_a * theta_dot`.
//! Gear A spins at `theta_dot`; gear B, driven by the same rope, spins at
//! `theta_dot * r_a / r_b`. A solid disc has `I = m r^2 / 2`.
//!
//! ```text
//! T = 1/2 (m1 + m2) (r_a w)^2  +  1/2 I_a w^2  +  1/2 I_b (w r_a/r_b)^2
//!   = 1/2 * M_eff * w^2,      where w = theta_dot and
//!
//! M_eff = (m1 + m2) r_a^2  +  I_a  +  I_b (r_a/r_b)^2
//! ```
//!
//! `M_eff` is the **effective rotational inertia** - everything the crank has
//! to shift, expressed as one number. Note the gear-ratio term is *squared*:
//! a small fast gear costs disproportionately more than its mass suggests.
//!
//! **Potential energy.** m1 falls by `r_a*theta` while m2 rises by the same, so
//! gravity contributes `(m2 - m1) g r_a theta`. A torsional return spring at
//! the axle adds `1/2 k theta^2`.
//!
//! ```text
//! U = (m2 - m1) g r_a theta  +  1/2 k theta^2
//! ```
//!
//! **Euler-Lagrange** `d/dt(dL/dw) - dL/dtheta = 0`, plus a viscous damping
//! torque `-c*w` (which is not conservative, so it enters as a force, not
//! through U):
//!
//! ```text
//! M_eff * theta_ddot  =  (m1 - m2) g r_a  -  k theta  -  c theta_dot
//!                        \______________/    \_____/    \__________/
//!                         gravity (const)     spring      damping
//! ```
//!
//! ## Why this is secretly the complex plane
//!
//! Shift to `phi = theta - theta_eq` and divide by `M_eff`:
//!
//! ```text
//! phi_ddot + 2 zeta wn phi_dot + wn^2 phi = 0
//!     wn   = sqrt(k / M_eff)             natural frequency
//!     zeta = c / (2 sqrt(k M_eff))       damping ratio
//! ```
//!
//! That is the damped harmonic oscillator, and its solution is
//!
//! ```text
//! phi(t) = Re{ A e^(lambda t) },   lambda = -zeta*wn + i*wn*sqrt(1 - zeta^2)
//! ```
//!
//! **`e^(lambda t)` is decay TIMES rotation** - exactly the spiral from C1.
//! `|e^(lambda t)| < 1` settles, `= 1` oscillates forever, `> 1` blows up. The
//! stability rule you found by dragging a slider in Desmos is this machine's
//! equation of motion.
//!
//! Because we have that closed form, `Sim::exact` can grade every numerical
//! integrator against the truth.

use crate::complex::Cx;
use crate::pulley::{Config, G};

/// Physical properties that iteration 1 ignored.
#[derive(Clone, Copy, Debug)]
pub struct Physics {
    /// Mass of gear A, treated as a solid disc (`I = m r^2 / 2`).
    pub gear_mass_a: f64,
    pub gear_mass_b: f64,
    /// Torsional return spring at gear A's axle. `0.0` = no spring (pure
    /// Atwood, constant acceleration).
    pub spring_k: f64,
    /// Viscous damping torque coefficient, `torque = -c * omega`.
    pub damping_c: f64,
    /// Bounce factor when a weight reaches its travel limit.
    /// `0.0` = dead stop, `1.0` = perfectly elastic.
    pub restitution: f64,
}

impl Default for Physics {
    fn default() -> Self {
        Physics {
            gear_mass_a: 1.5,
            gear_mass_b: 0.8,
            spring_k: 0.0,
            damping_c: 0.0,
            restitution: 0.35,
        }
    }
}

/// Which numerical scheme steps the state forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Integrator {
    /// `theta += w*dt` then `w += a*dt`, both using the OLD state.
    /// First order, and **not** symplectic - it pumps energy into an
    /// oscillator until it explodes. This is why naive game physics blows up.
    ExplicitEuler,
    /// `w += a*dt` FIRST, then `theta += w_new*dt`. One line reordered.
    /// Still first order, but symplectic: energy stays bounded forever.
    SemiImplicitEuler,
    /// Position from a Taylor step, velocity from the average of the old and
    /// new accelerations. Second order and symplectic - the workhorse.
    Verlet,
    /// Fourth-order Runge-Kutta. Superb short-term accuracy, but NOT
    /// symplectic: over very long runs it drifts (usually decaying).
    Rk4,
}

impl Integrator {
    pub const ALL: [Integrator; 4] = [
        Integrator::ExplicitEuler,
        Integrator::SemiImplicitEuler,
        Integrator::Verlet,
        Integrator::Rk4,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Integrator::ExplicitEuler => "explicit Euler",
            Integrator::SemiImplicitEuler => "semi-implicit Euler",
            Integrator::Verlet => "velocity Verlet",
            Integrator::Rk4 => "Runge-Kutta 4",
        }
    }
    /// Symplectic schemes conserve a nearby "shadow" energy exactly, so their
    /// error oscillates instead of accumulating.
    pub fn is_symplectic(self) -> bool {
        matches!(self, Integrator::SemiImplicitEuler | Integrator::Verlet)
    }
}

/// The evolving state of the machine.
#[derive(Clone, Copy, Debug)]
pub struct Sim {
    pub cfg: Config,
    pub phys: Physics,
    pub theta: f64,
    pub omega: f64,
    pub t: f64,
    /// Torque applied by whoever is holding the crank. Not a property of the
    /// machine, so it lives here rather than in `Physics` - it changes every
    /// frame. Zero unless something is driving it.
    ///
    /// This is the ONLY thing that had to change to make the simulation
    /// interactive: one more term in `torque()`.
    pub input_torque: f64,
    /// Initial conditions, kept so `exact` can be evaluated at any time.
    theta0: f64,
    omega0: f64,
}

impl Sim {
    pub fn new(cfg: Config, phys: Physics, theta0: f64, omega0: f64) -> Self {
        Sim { cfg, phys, theta: theta0, omega: omega0, t: 0.0, input_torque: 0.0, theta0, omega0 }
    }

    // ---- the physical constants of this particular machine ----------------

    /// `I = m r^2 / 2` for a solid disc.
    fn inertia_a(&self) -> f64 {
        0.5 * self.phys.gear_mass_a * self.cfg.r_a * self.cfg.r_a
    }
    fn inertia_b(&self) -> f64 {
        0.5 * self.phys.gear_mass_b * self.cfg.r_b * self.cfg.r_b
    }

    /// Everything the crank must accelerate, as one number.
    /// Note the **squared** gear ratio on gear B's contribution.
    pub fn m_eff(&self) -> f64 {
        let ratio = self.cfg.r_a / self.cfg.r_b;
        (self.cfg.m1 + self.cfg.m2) * self.cfg.r_a * self.cfg.r_a
            + self.inertia_a()
            + self.inertia_b() * ratio * ratio
    }

    /// The constant gravitational torque, positive when m1 is heavier.
    pub fn gravity_torque(&self) -> f64 {
        (self.cfg.m1 - self.cfg.m2) * G * self.cfg.r_a
    }

    /// Net torque at a given state. This IS the equation of motion.
    pub fn torque(&self, theta: f64, omega: f64) -> f64 {
        self.gravity_torque() - self.phys.spring_k * theta - self.phys.damping_c * omega
            + self.input_torque
    }

    /// Angular acceleration `theta_ddot`.
    pub fn accel(&self, theta: f64, omega: f64) -> f64 {
        self.torque(theta, omega) / self.m_eff()
    }

    /// Where the machine comes to rest (spring balances gravity).
    /// Undefined without a spring - a pure Atwood machine has no equilibrium.
    pub fn equilibrium(&self) -> Option<f64> {
        if self.phys.spring_k > 0.0 {
            Some(self.gravity_torque() / self.phys.spring_k)
        } else {
            None
        }
    }

    /// Natural frequency `wn = sqrt(k / M_eff)`.
    pub fn omega_n(&self) -> f64 {
        (self.phys.spring_k / self.m_eff()).sqrt()
    }

    /// Damping ratio `zeta`. Below 1 oscillates, 1 is critical, above 1 crawls.
    pub fn zeta(&self) -> f64 {
        if self.phys.spring_k <= 0.0 {
            return f64::INFINITY;
        }
        self.phys.damping_c / (2.0 * (self.phys.spring_k * self.m_eff()).sqrt())
    }

    /// The characteristic root `lambda = -zeta*wn + i*wd`, as a complex number.
    /// `Some` only when the motion is genuinely oscillatory (`zeta < 1`).
    pub fn lambda(&self) -> Option<Cx> {
        let z = self.zeta();
        if self.phys.spring_k <= 0.0 || z >= 1.0 {
            return None;
        }
        let wn = self.omega_n();
        Some(Cx::new(-z * wn, wn * (1.0 - z * z).sqrt()))
    }

    // ---- energy -----------------------------------------------------------

    /// Potential energy. Differentiating gives back the conservative torques,
    /// which is worth checking by hand: `-dU/dtheta = (m1-m2) g r_a - k theta`.
    pub fn potential(&self, theta: f64) -> f64 {
        (self.cfg.m2 - self.cfg.m1) * G * self.cfg.r_a * theta
            + 0.5 * self.phys.spring_k * theta * theta
    }
    pub fn kinetic(&self, omega: f64) -> f64 {
        0.5 * self.m_eff() * omega * omega
    }
    /// With `damping_c = 0` and no bouncing, this must stay constant. Watching
    /// it drift is the cleanest way to judge an integrator.
    pub fn energy(&self) -> f64 {
        self.kinetic(self.omega) + self.potential(self.theta)
    }

    // ---- the exact solution ----------------------------------------------

    /// Closed-form `theta(t)`, for grading the numerical schemes.
    ///
    /// Three regimes, and the interesting one is built from `Cx::expi`:
    /// * no spring  -> constant acceleration, a parabola in `t`
    /// * `zeta < 1` -> `Re{ A e^(lambda t) }`, decay times rotation
    /// * `zeta >= 1`-> two real exponentials, no rotation left
    ///
    /// (Ignores the travel limits, so compare only over intervals where the
    /// machine never hits an end stop.)
    pub fn exact(&self, t: f64) -> f64 {
        // --- no spring: uniformly accelerated, unless damping is present ---
        if self.phys.spring_k <= 0.0 {
            let a = self.gravity_torque() / self.m_eff();
            let c = self.phys.damping_c / self.m_eff();
            if c <= 0.0 {
                // theta = theta0 + w0 t + 1/2 a t^2
                return self.theta0 + self.omega0 * t + 0.5 * a * t * t;
            }
            // omega relaxes exponentially toward the terminal rate a/c
            let term = a / c;
            let w = term + (self.omega0 - term) * (-c * t).exp();
            let _ = w;
            return self.theta0 + term * t + (self.omega0 - term) / c * (1.0 - (-c * t).exp());
        }

        let eq = self.equilibrium().unwrap();
        let p0 = self.theta0 - eq; // displacement from rest
        let wn = self.omega_n();
        let z = self.zeta();

        if z < 1.0 {
            // ---- underdamped: THE complex-exponential case ----------------
            let lam = self.lambda().unwrap();
            let wd = lam.im; // damped frequency
            let sigma = lam.re; // decay rate (negative)

            // phi(t) = Re{ A e^(lambda t) } with A fixed by phi(0), phi'(0).
            // Writing A = p + qi and expanding gives p = phi0 and
            // q = -(w0 - sigma*phi0)/wd.
            let a_cx = Cx::new(p0, -(self.omega0 - sigma * p0) / wd);

            // e^(lambda t) = e^(sigma t) * e^(i wd t)  <- decay TIMES rotation
            let rotation = Cx::expi(wd * t);
            let phi = (a_cx * rotation).re * (sigma * t).exp();
            eq + phi
        } else if (z - 1.0).abs() < 1e-12 {
            // ---- critically damped: the repeated-root case ----------------
            let phi = (p0 + (self.omega0 + wn * p0) * t) * (-wn * t).exp();
            eq + phi
        } else {
            // ---- overdamped: two real roots, no oscillation ---------------
            let s = wn * (z * z - 1.0).sqrt();
            let r1 = -z * wn + s;
            let r2 = -z * wn - s;
            let c1 = (self.omega0 - r2 * p0) / (r1 - r2);
            let c2 = p0 - c1;
            eq + c1 * (r1 * t).exp() + c2 * (r2 * t).exp()
        }
    }

    // ---- stepping ---------------------------------------------------------

    /// Advance one time step with the chosen scheme.
    pub fn step(&mut self, dt: f64, method: Integrator) {
        match method {
            // ---------------------------------------------------------------
            // Position updated from the OLD velocity, velocity from the OLD
            // acceleration. Simple, intuitive, and wrong in a specific way:
            // it consistently overshoots outward on a curved trajectory, so
            // an oscillator gains energy every cycle.
            Integrator::ExplicitEuler => {
                let a = self.accel(self.theta, self.omega);
                let th = self.theta;
                let w = self.omega;
                self.theta = th + w * dt;
                self.omega = w + a * dt;
            }

            // ---------------------------------------------------------------
            // The SAME two lines, in the other order. Velocity is updated
            // first, then position uses the NEW velocity. That single swap
            // makes it symplectic: energy error oscillates but never grows.
            Integrator::SemiImplicitEuler => {
                let a = self.accel(self.theta, self.omega);
                self.omega += a * dt;
                self.theta += self.omega * dt;
            }

            // ---------------------------------------------------------------
            // Velocity Verlet. Position gets the full second-order Taylor
            // term, velocity uses the average of the accelerations at both
            // ends. Because our force depends on velocity (damping), we
            // predict the new velocity first, then correct.
            Integrator::Verlet => {
                let a0 = self.accel(self.theta, self.omega);
                let new_theta = self.theta + self.omega * dt + 0.5 * a0 * dt * dt;
                let w_pred = self.omega + a0 * dt;
                let a1 = self.accel(new_theta, w_pred);
                self.theta = new_theta;
                self.omega += 0.5 * (a0 + a1) * dt;
            }

            // ---------------------------------------------------------------
            // Classic RK4 on the state vector y = (theta, omega), where
            // y' = (omega, accel). Four probes, weighted 1:2:2:1.
            Integrator::Rk4 => {
                let f = |th: f64, w: f64| (w, self.accel(th, w));
                let (k1t, k1w) = f(self.theta, self.omega);
                let (k2t, k2w) = f(self.theta + 0.5 * dt * k1t, self.omega + 0.5 * dt * k1w);
                let (k3t, k3w) = f(self.theta + 0.5 * dt * k2t, self.omega + 0.5 * dt * k2w);
                let (k4t, k4w) = f(self.theta + dt * k3t, self.omega + dt * k3w);
                self.theta += dt / 6.0 * (k1t + 2.0 * k2t + 2.0 * k3t + k4t);
                self.omega += dt / 6.0 * (k1w + 2.0 * k2w + 2.0 * k3w + k4w);
            }
        }

        self.t += dt;
        self.apply_limits();
    }

    /// A weight cannot pass through its gear. On contact, reverse the motion
    /// and keep `restitution` of the speed - so `0.0` thuds, `1.0` bounces
    /// forever.
    fn apply_limits(&mut self) {
        let lim = self.cfg.theta_max();
        if self.theta > lim {
            self.theta = lim;
            if self.omega > 0.0 {
                self.omega = -self.omega * self.phys.restitution;
            }
        } else if self.theta < -lim {
            self.theta = -lim;
            if self.omega < 0.0 {
                self.omega = -self.omega * self.phys.restitution;
            }
        }
    }

    /// Run for `steps` steps, returning `theta` at each one.
    pub fn run(&mut self, dt: f64, steps: usize, method: Integrator) -> Vec<f64> {
        let mut out = Vec::with_capacity(steps + 1);
        out.push(self.theta);
        for _ in 0..steps {
            self.step(dt, method);
            out.push(self.theta);
        }
        out
    }
}

// ===========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, tol: f64) -> bool {
        (a - b).abs() < tol
    }

    /// Strip out the gears and the spring and we must recover the textbook
    /// Atwood result: linear acceleration `g (m1 - m2) / (m1 + m2)`.
    #[test]
    fn reduces_to_the_textbook_atwood_machine() {
        let cfg = Config { m1: 3.0, m2: 1.0, ..Config::default() };
        let phys = Physics { gear_mass_a: 0.0, gear_mass_b: 0.0, ..Physics::default() };
        let s = Sim::new(cfg, phys, 0.0, 0.0);
        let linear = s.accel(0.0, 0.0) * cfg.r_a;
        assert!(close(linear, G * (3.0 - 1.0) / (3.0 + 1.0), 1e-9));
    }

    /// Equal masses, no spring: nothing happens, forever.
    #[test]
    fn balanced_masses_do_not_move() {
        let cfg = Config { m1: 2.5, m2: 2.5, ..Config::default() };
        let mut s = Sim::new(cfg, Physics::default(), 0.0, 0.0);
        s.run(0.01, 500, Integrator::Verlet);
        assert!(close(s.theta, 0.0, 1e-12));
        assert!(close(s.omega, 0.0, 1e-12));
    }

    /// Heavy gears must slow the machine down - inertia is inertia.
    #[test]
    fn gear_inertia_reduces_acceleration() {
        let cfg = Config::default();
        let light = Sim::new(cfg, Physics { gear_mass_a: 0.0, gear_mass_b: 0.0, ..Physics::default() }, 0.0, 0.0);
        let heavy = Sim::new(cfg, Physics { gear_mass_a: 40.0, gear_mass_b: 40.0, ..Physics::default() }, 0.0, 0.0);
        assert!(heavy.m_eff() > light.m_eff());
        assert!(heavy.accel(0.0, 0.0).abs() < light.accel(0.0, 0.0).abs());
    }

    /// Every integrator should track the exact parabola of constant
    /// acceleration closely over a short run.
    #[test]
    fn all_integrators_track_constant_acceleration() {
        let cfg = Config { rope_len: 40_000.0, ..Config::default() }; // no end stops
        let phys = Physics { restitution: 0.0, ..Physics::default() };
        for m in Integrator::ALL {
            let mut s = Sim::new(cfg, phys, 0.0, 0.0);
            let dt = 1e-4;
            let n = 2000;
            s.run(dt, n, m);
            let want = s.exact(dt * n as f64);
            assert!(close(s.theta, want, 1e-3), "{}: {} vs {}", m.name(), s.theta, want);
        }
    }

    /// With a spring and no damping the motion is a pure oscillation, and
    /// Verlet and RK4 should match the closed-form solution tightly.
    #[test]
    fn accurate_integrators_match_the_exact_oscillation() {
        let cfg = Config { rope_len: 40_000.0, m1: 3.0, m2: 1.0, ..Config::default() };
        let phys = Physics { spring_k: 2.0e6, damping_c: 0.0, restitution: 0.0, ..Physics::default() };
        for m in [Integrator::Verlet, Integrator::Rk4] {
            let mut s = Sim::new(cfg, phys, 0.4, 0.0);
            let dt = 1e-4;
            let n = 20_000;
            s.run(dt, n, m);
            let want = s.exact(dt * n as f64);
            assert!(close(s.theta, want, 1e-4), "{}: {} vs {}", m.name(), s.theta, want);
        }
    }

    /// The headline numerical lesson. Same problem, same step size:
    /// explicit Euler MANUFACTURES energy, the symplectic pair does not.
    #[test]
    fn explicit_euler_gains_energy_while_symplectic_schemes_do_not() {
        let cfg = Config { rope_len: 40_000.0, m1: 2.0, m2: 2.0, ..Config::default() };
        let phys = Physics { spring_k: 2.0e6, damping_c: 0.0, restitution: 0.0, ..Physics::default() };
        let dt = 2e-3;
        let n = 20_000;

        let mut bad = Sim::new(cfg, phys, 0.5, 0.0);
        let e0 = bad.energy();
        bad.run(dt, n, Integrator::ExplicitEuler);
        assert!(bad.energy() > e0 * 1.5, "explicit Euler should blow up, got {}", bad.energy() / e0);

        for m in [Integrator::SemiImplicitEuler, Integrator::Verlet] {
            let mut good = Sim::new(cfg, phys, 0.5, 0.0);
            let e0 = good.energy();
            good.run(dt, n, m);
            let drift = (good.energy() - e0).abs() / e0;
            assert!(drift < 0.10, "{} drifted {:.3}", m.name(), drift);
        }
    }

    /// Damping must remove energy, and the machine must end up at rest at the
    /// point where the spring balances gravity.
    #[test]
    fn damped_system_settles_at_equilibrium() {
        let cfg = Config { rope_len: 40_000.0, m1: 3.0, m2: 1.0, ..Config::default() };
        let phys = Physics { spring_k: 2.0e6, damping_c: 290_000.0, restitution: 0.0, ..Physics::default() };
        let mut s = Sim::new(cfg, phys, 0.0, 0.0);
        s.run(1e-3, 20_000, Integrator::Verlet);
        let eq = s.equilibrium().unwrap();
        assert!(close(s.theta, eq, 1e-4), "{} vs {}", s.theta, eq);
        assert!(close(s.omega, 0.0, 1e-4));
    }

    /// `zeta` classifies the three regimes, and `lambda` exists only when the
    /// motion actually rotates in the complex plane.
    #[test]
    fn damping_ratio_classifies_the_regimes() {
        let cfg = Config { rope_len: 40_000.0, ..Config::default() };
        let base = Physics { spring_k: 2.0e6, restitution: 0.0, ..Physics::default() };
        let m_eff = Sim::new(cfg, base, 0.0, 0.0).m_eff();
        let c_crit = 2.0 * (2.0e6f64 * m_eff).sqrt();

        let under = Sim::new(cfg, Physics { damping_c: 0.3 * c_crit, ..base }, 0.0, 0.0);
        let over = Sim::new(cfg, Physics { damping_c: 2.0 * c_crit, ..base }, 0.0, 0.0);
        assert!(under.zeta() < 1.0 && under.lambda().is_some());
        assert!(over.zeta() > 1.0 && over.lambda().is_none());

        // an underdamped root really is "decay times rotation"
        let lam = under.lambda().unwrap();
        assert!(lam.re < 0.0 && lam.im > 0.0);
    }

    /// A weight must never pass through its gear, however hard it is driven.
    #[test]
    fn end_stops_are_respected() {
        let cfg = Config::default();
        let phys = Physics { restitution: 0.5, ..Physics::default() };
        let mut s = Sim::new(cfg, phys, 0.0, 40.0);
        s.run(1e-3, 20_000, Integrator::Verlet);
        let lim = cfg.theta_max();
        assert!(s.theta.abs() <= lim + 1e-9);
    }

    /// Bouncing with restitution < 1 must lose energy every impact.
    #[test]
    fn restitution_removes_energy_on_impact() {
        let cfg = Config::default();
        let phys = Physics { restitution: 0.4, ..Physics::default() };
        let mut s = Sim::new(cfg, phys, 0.0, 6.0);
        let e0 = s.energy();
        s.run(1e-3, 30_000, Integrator::Verlet);
        assert!(s.energy() < e0);
    }
}


