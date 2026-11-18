//! In-flight wind estimate (M4.1, D-046).
//!
//! A two-state extended Kalman filter on the horizontal wind `w`. The
//! accelerometers say what force acted on the vehicle; take away gravity
//! and the engines' thrust (engine telemetry) and what is left is the air's
//! force. The shared aerodynamic model says what that force would be for a
//! given wind (it depends on the airspeed `v − w`), so the filter finds the
//! wind that explains it, and how sure it can be: at high dynamic pressure
//! a metre per second of crosswind is a large force, near hover it is
//! almost none, and the filter's gain follows on its own.
//!
//! Inputs are what the vehicle senses (navigation estimate, accelerometers,
//! engine telemetry, surface positions), never the truth. Simulation only.

use model::aero::{self, FLAPS};
use model::dynamics::{self, M, R, State};
use model::quat::{self, Quat, Vec3};
use model::{Scenario, VehicleKind};

/// 1σ of the wind before any measurement, m/s (no forecast).
pub const INITIAL_SIGMA: f64 = 15.0;
/// Random-walk strength of the wind, m/s per √s: how fast the filter
/// believes the wind can change (lets it follow a 4 s gust).
pub const WALK: f64 = 3.0;
/// Engine telemetry's thrust error, as a fraction of thrust (*illustrative*).
pub const THRUST_TELEMETRY: f64 = 0.01;
/// Unmodelled force (model error, vibration), N, 1σ per axis.
pub const FORCE_FLOOR: f64 = 2e3;
/// The estimate never leaves ±LIMIT m/s per axis.
pub const LIMIT: f64 = 60.0;
/// Innovations this unlikely (normalized, 3 degrees of freedom; 25 is far
/// beyond the 99.9 % point, 16.3) are set aside as a reading glitch...
const GATE: f64 = 25.0;
/// ...unless they persist this long, s: then the wind really changed.
const PERSIST: f64 = 0.3;

/// What the vehicle sensed over one control step.
pub struct WindInputs<'a> {
    pub scn: &'a Scenario,
    /// Navigation estimate at the end of the step (mass, position,
    /// velocity, attitude).
    pub x: &'a State,
    /// The accelerometers' velocity change over the step, m/s (gravity
    /// included, as the navigation filter uses it).
    pub dv: Vec3,
    /// Step length, s.
    pub h: f64,
    /// Engine telemetry: the thrust delivered during the step, body axes, N.
    pub thrust_body: Vec3,
    /// Flap exposures (ship) or fin deflections (booster) during the step.
    pub surfaces: [f64; FLAPS],
    /// 1σ of the attitude estimate, rad, and of the velocity estimate, m/s.
    pub att_sigma: f64,
    pub vel_sigma: f64,
}

/// The filter's answer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindEstimate {
    /// Horizontal wind (x, y), m/s: the air's velocity, blowing toward.
    pub w: [f64; 2],
    /// 1σ per axis, m/s.
    pub sigma: [f64; 2],
    /// Measured minus predicted air force at the last update, N.
    pub innovation: Vec3,
}

#[derive(Debug, Clone)]
pub struct WindFilter {
    w: [f64; 2],
    p: [[f64; 2]; 2],
    innovation: Vec3,
    /// How long innovations have been set aside in a row, s.
    rejected_for: f64,
}

impl Default for WindFilter {
    fn default() -> Self {
        Self::new()
    }
}

impl WindFilter {
    pub fn new() -> WindFilter {
        Self::with_prior([0.0; 2], INITIAL_SIGMA)
    }

    /// Start from a known wind with 1σ `sigma` (a forecast, or a test).
    pub fn with_prior(w: [f64; 2], sigma: f64) -> WindFilter {
        WindFilter {
            w,
            p: [[sigma * sigma, 0.0], [0.0, sigma * sigma]],
            innovation: [0.0; 3],
            rejected_for: 0.0,
        }
    }

    pub fn estimate(&self) -> WindEstimate {
        WindEstimate {
            w: self.w,
            sigma: [self.p[0][0].sqrt(), self.p[1][1].sqrt()],
            innovation: self.innovation,
        }
    }

    /// The estimate as an inertial vector (z = 0).
    pub fn wind(&self) -> Vec3 {
        [self.w[0], self.w[1], 0.0]
    }

    pub fn covariance(&self) -> [[f64; 2]; 2] {
        self.p
    }

    pub fn update(&mut self, i: &WindInputs) {
        if !(i.h.is_finite() && i.h > 0.0) {
            return;
        }
        // The wind may have wandered since the last step.
        let walk = WALK * WALK * i.h;
        self.p[0][0] += walk;
        self.p[1][1] += walk;

        let x = i.x;
        let rho = i.scn.density(x[R + 2]);
        if rho.is_nan() || rho <= 1e-9 || !i.dv.iter().all(|c| c.is_finite()) {
            return;
        }
        let m = x[M];
        let (v, q) = (dynamics::velocity(x), dynamics::attitude(x));
        let g = i.scn.planet.gravity();
        let z = [
            m * i.dv[0] / i.h,
            m * i.dv[1] / i.h,
            m * (i.dv[2] / i.h + g),
        ];
        let model = |w: [f64; 2], q: Quat, v: Vec3| force(i, rho, w, q, v);
        let pred = model(self.w, q, v);
        let innov = quat::sub(z, pred);

        // ∂(force)/∂w, 3×2.
        const DW: f64 = 0.5;
        let mut hm = [[0.0; 2]; 3];
        for j in 0..2 {
            let (mut a, mut b) = (self.w, self.w);
            a[j] += DW;
            b[j] -= DW;
            let d = quat::sub(model(a, q, v), model(b, q, v));
            for (r, row) in hm.iter_mut().enumerate() {
                row[j] = d[r] / (2.0 * DW);
            }
        }

        // Measurement noise: what the readings' own errors do to the force.
        let mut rm = [[0.0; 3]; 3];
        let mut add_outer = |col: Vec3, var: f64| {
            for r in 0..3 {
                for c in 0..3 {
                    rm[r][c] += var * col[r] * col[c];
                }
            }
        };
        const DQ: f64 = 1e-3;
        for axis in [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]] {
            let qa = quat::mul(q, quat::from_axis_angle(axis, DQ));
            let qb = quat::mul(q, quat::from_axis_angle(axis, -DQ));
            let d = quat::scale(
                quat::sub(model(self.w, qa, v), model(self.w, qb, v)),
                0.5 / DQ,
            );
            add_outer(d, i.att_sigma * i.att_sigma);
        }
        const DV: f64 = 0.1;
        for k in 0..3 {
            let (mut va, mut vb) = (v, v);
            va[k] += DV;
            vb[k] -= DV;
            let d = quat::scale(
                quat::sub(model(self.w, q, va), model(self.w, q, vb)),
                0.5 / DV,
            );
            add_outer(d, i.vel_sigma * i.vel_sigma);
        }
        let thrust = quat::rotate(q, i.thrust_body);
        add_outer(thrust, THRUST_TELEMETRY * THRUST_TELEMETRY);
        for (k, row) in rm.iter_mut().enumerate() {
            row[k] += FORCE_FLOOR * FORCE_FLOOR;
        }

        // S = H P Hᵀ + R.
        let ph: [[f64; 3]; 2] = std::array::from_fn(|a| {
            std::array::from_fn(|r| (0..2).map(|b| self.p[a][b] * hm[r][b]).sum())
        });
        let s: [[f64; 3]; 3] = std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..2).map(|a| hm[r][a] * ph[a][c]).sum::<f64>() + rm[r][c])
        });
        let Some(si) = inverse3(&s) else {
            return;
        };
        self.innovation = innov;
        let nis: f64 = (0..3)
            .map(|r| (0..3).map(|c| innov[r] * si[r][c] * innov[c]).sum::<f64>())
            .sum();
        if nis > GATE && self.rejected_for < PERSIST {
            self.rejected_for += i.h;
            return;
        }
        self.rejected_for = 0.0;

        // K = P Hᵀ S⁻¹ (2×3).
        let k: [[f64; 3]; 2] = std::array::from_fn(|a| {
            std::array::from_fn(|c| (0..3).map(|r| ph[a][r] * si[r][c]).sum())
        });
        for (a, row) in k.iter().enumerate() {
            self.w[a] += (0..3).map(|r| row[r] * innov[r]).sum::<f64>();
            self.w[a] = self.w[a].clamp(-LIMIT, LIMIT);
        }
        // Joseph form: P = (I − KH) P (I − KH)ᵀ + K R Kᵀ.
        let a: [[f64; 2]; 2] = std::array::from_fn(|r| {
            std::array::from_fn(|c| {
                let kh: f64 = (0..3).map(|j| k[r][j] * hm[j][c]).sum();
                if r == c { 1.0 - kh } else { -kh }
            })
        });
        let ap: [[f64; 2]; 2] = std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..2).map(|j| a[r][j] * self.p[j][c]).sum())
        });
        let mut p: [[f64; 2]; 2] = std::array::from_fn(|r| {
            std::array::from_fn(|c| (0..2).map(|j| ap[r][j] * a[c][j]).sum())
        });
        for (r, pr) in p.iter_mut().enumerate() {
            for (c, pc) in pr.iter_mut().enumerate() {
                *pc += (0..3)
                    .map(|j| (0..3).map(|l| k[r][j] * rm[j][l] * k[c][l]).sum::<f64>())
                    .sum::<f64>();
            }
        }
        let off = 0.5 * (p[0][1] + p[1][0]);
        p[0][1] = off;
        p[1][0] = off;
        // Never certain: keep a little uncertainty so it can still learn.
        p[0][0] = p[0][0].max(1e-4);
        p[1][1] = p[1][1].max(1e-4);
        self.p = p;
    }
}

/// The model's total non-gravity force (air on body and surfaces, plus
/// thrust), inertial, N, for wind `w` at attitude `q` and velocity `v`.
fn force(i: &WindInputs, rho: f64, w: [f64; 2], q: Quat, v: Vec3) -> Vec3 {
    let ship = &i.scn.ship;
    let air = [v[0] - w[0], v[1] - w[1], v[2]];
    let body = aero::body_force(ship, rho, q, air).0;
    let surf = match ship.kind {
        VehicleKind::Ship => aero::flap_force(ship, rho, q, air, i.surfaces).0,
        VehicleKind::Booster => aero::fin_force(ship, rho, q, air, i.surfaces).0,
    };
    quat::add(quat::add(body, surf), quat::rotate(q, i.thrust_body))
}

fn inverse3(m: &[[f64; 3]; 3]) -> Option<[[f64; 3]; 3]> {
    let c = |r: usize, k: usize| {
        let (r1, r2) = ((r + 1) % 3, (r + 2) % 3);
        let (k1, k2) = ((k + 1) % 3, (k + 2) % 3);
        m[r1][k1] * m[r2][k2] - m[r1][k2] * m[r2][k1]
    };
    let det = m[0][0] * c(0, 0) + m[0][1] * c(0, 1) + m[0][2] * c(0, 2);
    if !det.is_finite() || det.abs() <= 1e-300 {
        return None;
    }
    // inverse = adjugate / det; adjugate[k][r] = cofactor(r, k).
    Some(std::array::from_fn(|k| {
        std::array::from_fn(|r| c(r, k) / det)
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ship_control;
    use model::aero;
    use model::dynamics::{self, Extra, M, R, V};
    use model::quat::{self, Vec3};
    use model::{Scenario, VehicleKind};

    const H: f64 = 0.01;

    /// The shared model as the truth: one Euler step of `h` in `wind`, with
    /// the given thrust and surfaces. Returns the next state and the
    /// accelerometers' velocity change over the step.
    fn truth_step(
        scn: &Scenario,
        x: &State,
        thrust: Vec3,
        surfaces: [f64; aero::FLAPS],
        wind: Vec3,
    ) -> (State, Vec3) {
        let q = dynamics::attitude(x);
        let rho = scn.density(x[R + 2]);
        let air = quat::sub(dynamics::velocity(x), wind);
        let (ff, ft) = match scn.ship.kind {
            VehicleKind::Ship => aero::flap_force(&scn.ship, rho, q, air, surfaces),
            VehicleKind::Booster => aero::fin_force(&scn.ship, rho, q, air, surfaces),
        };
        let extra = Extra {
            force: ff,
            torque: ft,
            wind,
        };
        let d = dynamics::f(scn, x, thrust, &extra);
        let mut next = *x;
        for i in 0..next.len() {
            next[i] += H * d[i];
        }
        let qn = quat::normalize(dynamics::attitude(&next));
        next[dynamics::Q..dynamics::Q + 4].copy_from_slice(&qn);
        (next, [H * d[V], H * d[V + 1], H * d[V + 2]])
    }

    fn inputs<'a>(
        scn: &'a Scenario,
        x: &'a State,
        dv: Vec3,
        thrust: Vec3,
        surfaces: [f64; aero::FLAPS],
    ) -> WindInputs<'a> {
        WindInputs {
            scn,
            x,
            dv,
            h: H,
            thrust_body: thrust,
            surfaces,
            att_sigma: 0.0,
            vel_sigma: 0.0,
        }
    }

    /// Fall engines-off for `seconds` in `wind(t)`, updating `f` each step.
    fn fall(
        scn: &Scenario,
        mut x: State,
        f: &mut WindFilter,
        seconds: f64,
        t0: f64,
        wind: impl Fn(f64) -> Vec3,
    ) -> State {
        let surfaces = [scn.ship.surface_trim(); aero::FLAPS];
        for k in 0..(seconds / H).round() as usize {
            let t = t0 + k as f64 * H;
            let (next, dv) = truth_step(scn, &x, [0.0; 3], surfaces, wind(t));
            x = next;
            f.update(&inputs(scn, &x, dv, [0.0; 3], surfaces));
        }
        x
    }

    fn error(f: &WindFilter, truth: Vec3) -> f64 {
        let w = f.estimate().w;
        (w[0] - truth[0]).hypot(w[1] - truth[1])
    }

    fn booster_start() -> (Scenario, State) {
        let scn = Scenario::booster();
        let v = [-30.0, 0.0, -420.0];
        let x = dynamics::state(
            scn.start_mass,
            [900.0, 0.0, 14_000.0],
            v,
            ship_control::descent_attitude(v),
            [0.0; 3],
        );
        (scn, x)
    }

    fn ship_start() -> (Scenario, State) {
        let scn = Scenario::earth();
        let x = dynamics::state(
            scn.start_mass,
            [700.0, 0.0, 5000.0],
            [-25.0, 0.0, -75.0],
            ship_control::belly_attitude(0.0),
            [0.0; 3],
        );
        (scn, x)
    }

    #[test]
    fn starts_at_zero_with_the_prior_uncertainty() {
        let e = WindFilter::new().estimate();
        assert_eq!(e.w, [0.0, 0.0]);
        assert_eq!(e.sigma, [INITIAL_SIGMA, INITIAL_SIGMA]);
    }

    #[test]
    fn converges_in_a_steady_crosswind() {
        let (scn, x) = booster_start();
        let truth = [0.0, -15.0, 0.0];
        let mut f = WindFilter::new();
        fall(&scn, x, &mut f, 10.0, 0.0, |_| truth);
        let e = f.estimate();
        eprintln!("booster: {:?} ± {:?}", e.w, e.sigma);
        assert!(error(&f, truth) < 1.0, "error {}", error(&f, truth));
        assert!(e.sigma[0] < 2.0 && e.sigma[1] < 2.0, "sigma {:?}", e.sigma);
    }

    #[test]
    fn ship_belly_flop_converges() {
        let (scn, x) = ship_start();
        let truth = [10.0, 5.0, 0.0];
        let mut f = WindFilter::new();
        fall(&scn, x, &mut f, 10.0, 0.0, |_| truth);
        eprintln!("ship: {:?} ± {:?}", f.estimate().w, f.estimate().sigma);
        assert!(error(&f, truth) < 1.0, "error {}", error(&f, truth));
    }

    #[test]
    fn follows_a_step_change() {
        let (scn, x) = booster_start();
        let mut f = WindFilter::new();
        let wind = |t: f64| if t < 5.0 { [0.0; 3] } else { [-10.0, 0.0, 0.0] };
        let x = fall(&scn, x, &mut f, 5.0, 0.0, wind);
        assert!(error(&f, [0.0; 3]) < 1.0);
        fall(&scn, x, &mut f, 4.0, 5.0, wind);
        eprintln!("after the step: {:?}", f.estimate().w);
        assert!(
            error(&f, [-10.0, 0.0, 0.0]) < 3.0,
            "error {}",
            error(&f, [-10.0, 0.0, 0.0])
        );
    }

    /// Drifting with the air (no airspeed) the air says nothing about the
    /// wind: noisy readings must not move a settled estimate.
    #[test]
    fn coasts_when_the_air_is_still_relative() {
        let scn = Scenario::booster();
        let w = [8.0, 0.0, 0.0];
        let g = scn.planet.gravity();
        let m = scn.dry_mass + 20e3;
        let mut f = WindFilter::with_prior([8.0, 0.0], 1.0);
        let mut rng = 12345u64;
        let mut noise = || {
            // xorshift, mapped to [-1, 1]
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng as f64 / u64::MAX as f64) * 2.0 - 1.0
        };
        for _ in 0..3000 {
            let x = dynamics::state(m, [0.0, 0.0, 1000.0], w, [1.0, 0.0, 0.0, 0.0], [0.0; 3]);
            let thrust = [0.0, 0.0, m * g];
            // The readings: 1 % thrust error, a little lateral jitter.
            let dv = [
                H * 0.05 * noise(),
                H * 0.05 * noise(),
                H * 0.01 * g * noise(),
            ];
            let mut i = inputs(&scn, &x, dv, thrust, [0.0; aero::FLAPS]);
            i.att_sigma = 0.0035;
            i.vel_sigma = 0.8;
            f.update(&i);
        }
        let moved = error(&f, w);
        eprintln!(
            "coasting moved {moved:.3} m/s, sigma {:?}",
            f.estimate().sigma
        );
        assert!(moved < 0.5, "moved {moved}");
    }

    /// One step whose telemetry says 30 % more thrust than acted (a reading
    /// glitch) must not throw a settled estimate.
    #[test]
    fn thrust_step_does_not_throw_the_estimate() {
        let (scn, mut x) = booster_start();
        let truth = [5.0, 0.0, 0.0];
        let mut f = WindFilter::with_prior([5.0, 0.0], 1.0);
        // Upright and braking at 1 km.
        x[R + 2] = 1000.0;
        x[V..V + 3].copy_from_slice(&[0.0, 0.0, -150.0]);
        x[dynamics::Q..dynamics::Q + 4].copy_from_slice(&[1.0, 0.0, 0.0, 0.0]);
        x[M] = scn.dry_mass + 60e3;
        let thrust = [0.0, 0.0, 8e6];
        let surfaces = [0.0; aero::FLAPS];
        let (next, dv) = truth_step(&scn, &x, [0.0, 0.0, 0.7 * thrust[2]], surfaces, truth);
        f.update(&inputs(&scn, &next, dv, thrust, surfaces));
        let moved = error(&f, truth);
        eprintln!("glitch moved {moved:.3} m/s");
        assert!(moved < 2.0, "moved {moved}");
    }

    #[test]
    fn covariance_stays_symmetric_positive() {
        let (scn, x) = booster_start();
        let mut f = WindFilter::new();
        let mut s = 99u64;
        let mut r = || {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            (s as f64 / u64::MAX as f64) * 2.0 - 1.0
        };
        for _ in 0..10_000 {
            let dv = [H * 30.0 * r(), H * 30.0 * r(), H * 30.0 * r()];
            let mut i = inputs(
                &scn,
                &x,
                dv,
                [0.0, 0.0, 5e6 * r().abs()],
                [0.1 * r(); aero::FLAPS],
            );
            i.att_sigma = 0.0035 * r().abs();
            i.vel_sigma = r().abs();
            f.update(&i);
            let p = f.covariance();
            assert!((p[0][1] - p[1][0]).abs() < 1e-9);
            let (tr, det) = (p[0][0] + p[1][1], p[0][0] * p[1][1] - p[0][1] * p[1][0]);
            assert!(tr > 0.0 && det > 0.0, "P {p:?}");
            let w = f.estimate().w;
            assert!(w[0].abs() <= LIMIT && w[1].abs() <= LIMIT);
        }
    }
}
