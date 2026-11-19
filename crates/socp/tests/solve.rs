//! Small problems with known answers, and random problems checked against
//! Clarabel.

#![allow(clippy::needless_range_loop)]

use socp::{Cone, Csc, Settings, Solver, Status};

fn solve(p: &[f64], q: &[f64], a: &Csc, b: &[f64], cones: &[Cone]) -> (Status, Vec<f64>) {
    let mut s = Solver::new(p, q, a, b, cones, Settings::default()).unwrap();
    let st = s.solve();
    (st, s.x().to_vec())
}

fn dense(m: usize, n: usize, rows: &[&[f64]]) -> Csc {
    let (mut r, mut c, mut v) = (vec![], vec![], vec![]);
    for i in 0..m {
        for j in 0..n {
            if rows[i][j] != 0.0 {
                r.push(i);
                c.push(j);
                v.push(rows[i][j]);
            }
        }
    }
    Csc::from_triplets(m, n, &r, &c, &v)
}

#[test]
fn linear_program() {
    // min −x − y  s.t.  x + 2y ≤ 4, 3x + y ≤ 6, x, y ≥ 0  →  (1.6, 1.2)
    let a = dense(
        4,
        2,
        &[&[1.0, 2.0], &[3.0, 1.0], &[-1.0, 0.0], &[0.0, -1.0]],
    );
    let (st, x) = solve(
        &[0.0, 0.0],
        &[-1.0, -1.0],
        &a,
        &[4.0, 6.0, 0.0, 0.0],
        &[Cone::Nonneg(4)],
    );
    assert_eq!(st, Status::Solved);
    assert!(
        (x[0] - 1.6).abs() < 1e-6 && (x[1] - 1.2).abs() < 1e-6,
        "{x:?}"
    );
}

#[test]
fn second_order_cone() {
    // min t  s.t.  ‖(u, v)‖ ≤ t, u = 1, v = 2  →  t = √5
    // rows: u = 1, v = 2 (zero cone); (t, u, v) ∈ SOC as s = (t, u, v) = −A x + b with b = 0.
    let a = dense(
        5,
        3,
        &[
            &[0.0, 1.0, 0.0],
            &[0.0, 0.0, 1.0],
            &[-1.0, 0.0, 0.0],
            &[0.0, -1.0, 0.0],
            &[0.0, 0.0, -1.0],
        ],
    );
    let (st, x) = solve(
        &[0.0; 3],
        &[1.0, 0.0, 0.0],
        &a,
        &[1.0, 2.0, 0.0, 0.0, 0.0],
        &[Cone::Zero(2), Cone::Soc(3)],
    );
    assert_eq!(st, Status::Solved);
    assert!((x[0] - 5f64.sqrt()).abs() < 1e-6, "{x:?}");
}

#[test]
fn quadratic_objective_with_equality() {
    // min ½(x² + y²) − x  s.t.  x + y = 1  →  (1, 0)
    let a = dense(1, 2, &[&[1.0, 1.0]]);
    let (st, x) = solve(&[1.0, 1.0], &[-1.0, 0.0], &a, &[1.0], &[Cone::Zero(1)]);
    assert_eq!(st, Status::Solved);
    assert!((x[0] - 1.0).abs() < 1e-6 && x[1].abs() < 1e-6, "{x:?}");
}

/// A tiny deterministic generator (SplitMix64).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 11) as f64 / (1u64 << 53) as f64
    }
    fn signed(&mut self) -> f64 {
        2.0 * self.next() - 1.0
    }
}

/// A random feasible, bounded problem: equality rows, nonnegative rows and
/// a few SOCs, a strictly positive diagonal P.
fn random_problem(seed: u64) -> (Vec<f64>, Vec<f64>, Csc, Vec<f64>, Vec<Cone>) {
    let mut r = Rng(seed);
    let n = 12 + (seed % 7) as usize;
    let (n_eq, n_nn, socs) = (3, 8, [3usize, 4, 3]);
    let m = n_eq + n_nn + socs.iter().sum::<usize>();
    let x_feas: Vec<f64> = (0..n).map(|_| r.signed()).collect();
    let (mut ri, mut ci, mut vv) = (vec![], vec![], vec![]);
    for i in 0..m {
        for j in 0..n {
            if r.next() < 0.35 {
                ri.push(i);
                ci.push(j);
                vv.push(r.signed());
            }
        }
    }
    let a = Csc::from_triplets(m, n, &ri, &ci, &vv);
    let mut ax = vec![0.0; m];
    a.mul_add(&x_feas, &mut ax);
    // b = A x + s with s strictly inside the cone.
    let mut b = ax.clone();
    for i in n_eq..n_eq + n_nn {
        b[i] += 0.1 + r.next();
    }
    let mut row = n_eq + n_nn;
    for &d in &socs {
        b[row] += 1.0 + r.next();
        for k in 1..d {
            b[row + k] += 0.3 * r.signed();
        }
        row += d;
    }
    let p: Vec<f64> = (0..n).map(|_| 0.1 + r.next()).collect();
    let q: Vec<f64> = (0..n).map(|_| r.signed()).collect();
    let mut cones = vec![Cone::Zero(n_eq), Cone::Nonneg(n_nn)];
    cones.extend(socs.iter().map(|&d| Cone::Soc(d)));
    (p, q, a, b, cones)
}

fn clarabel_solve(p: &[f64], q: &[f64], a: &Csc, b: &[f64], cones: &[Cone]) -> Vec<f64> {
    use clarabel::algebra::CscMatrix;
    use clarabel::solver::{DefaultSettings, DefaultSolver, IPSolver, SupportedConeT};
    let n = a.n;
    let idx: Vec<usize> = (0..n).collect();
    let pm = CscMatrix::new_from_triplets(n, n, idx.clone(), idx, p.to_vec());
    let am = CscMatrix::new(
        a.m,
        a.n,
        a.colptr.clone(),
        a.rowval.clone(),
        a.nzval.clone(),
    );
    let cl: Vec<SupportedConeT<f64>> = cones
        .iter()
        .map(|c| match *c {
            Cone::Zero(k) => SupportedConeT::ZeroConeT(k),
            Cone::Nonneg(k) => SupportedConeT::NonnegativeConeT(k),
            Cone::Soc(k) => SupportedConeT::SecondOrderConeT(k),
        })
        .collect();
    let settings = DefaultSettings {
        verbose: false,
        ..Default::default()
    };
    let mut s = DefaultSolver::new(&pm, q, &am, b, &cl, settings).unwrap();
    s.solve();
    s.solution.x.clone()
}

fn objective(p: &[f64], q: &[f64], x: &[f64]) -> f64 {
    (0..x.len())
        .map(|j| 0.5 * p[j] * x[j] * x[j] + q[j] * x[j])
        .sum()
}

#[test]
fn random_problems_match_clarabel() {
    for seed in 1..=40 {
        let (p, q, a, b, cones) = random_problem(seed);
        let (st, x) = solve(&p, &q, &a, &b, &cones);
        assert_eq!(st, Status::Solved, "seed {seed}");
        let xc = clarabel_solve(&p, &q, &a, &b, &cones);
        let (f, fc) = (objective(&p, &q, &x), objective(&p, &q, &xc));
        assert!(
            (f - fc).abs() <= 1e-6 * (1.0 + fc.abs()),
            "seed {seed}: {f} vs Clarabel {fc}"
        );
        // Strictly convex: the minimizer itself agrees.
        let dx = x
            .iter()
            .zip(&xc)
            .map(|(u, v)| (u - v).abs())
            .fold(0.0, f64::max);
        assert!(dx < 1e-4, "seed {seed}: x differs by {dx}");
    }
}

#[test]
fn update_reuses_the_pattern() {
    let (p, q, a, b, cones) = random_problem(3);
    let mut s = Solver::new(&p, &q, &a, &b, &cones, Settings::default()).unwrap();
    assert_eq!(s.solve(), Status::Solved);
    let (p2, q2, mut a2, b2, _) = random_problem(3);
    // Same pattern, different numbers.
    for v in &mut a2.nzval {
        *v *= 1.1;
    }
    let q2: Vec<f64> = q2.iter().map(|c| -c).collect();
    s.update(&p2, &q2, &a2, &b2).unwrap();
    assert_eq!(s.solve(), Status::Solved);
    let fresh = solve(&p2, &q2, &a2, &b2, &cones).1;
    let dx = s
        .x()
        .iter()
        .zip(&fresh)
        .map(|(u, v)| (u - v).abs())
        .fold(0.0, f64::max);
    assert!(dx < 1e-8, "{dx}");
}

#[test]
fn infeasible_linear_program_is_proved_infeasible() {
    // x ≥ 1 and x ≤ 0: no x. rows: 1 − x ≥ 0 reversed... s = b − A x:
    // s0 = −1 + x ≥ 0 (x ≥ 1), s1 = 0 − x ≥ 0 (x ≤ 0).
    let a = dense(2, 1, &[&[-1.0], &[1.0]]);
    let (st, _) = solve(&[0.0], &[1.0], &a, &[-1.0, 0.0], &[Cone::Nonneg(2)]);
    assert_eq!(st, Status::PrimalInfeasible);
}

#[test]
fn infeasible_cone_program_is_proved_infeasible() {
    // ‖(u, v)‖ ≤ t with t = 1, u = 2: no point. Variables (t, u, v).
    let a = dense(
        5,
        3,
        &[
            &[1.0, 0.0, 0.0],
            &[0.0, 1.0, 0.0],
            &[-1.0, 0.0, 0.0],
            &[0.0, -1.0, 0.0],
            &[0.0, 0.0, -1.0],
        ],
    );
    let (st, _) = solve(
        &[0.0, 0.0, 0.0],
        &[0.0, 0.0, 1.0],
        &a,
        &[1.0, 2.0, 0.0, 0.0, 0.0],
        &[Cone::Zero(2), Cone::Soc(3)],
    );
    assert_eq!(st, Status::PrimalInfeasible);
}

#[test]
fn a_cut_short_solve_returns_its_best_iterate_as_almost_solved() {
    // The linear program above, stopped before full accuracy: the best
    // iterate within the reduced tolerances comes back as AlmostSolved.
    let a = dense(
        4,
        2,
        &[&[1.0, 2.0], &[3.0, 1.0], &[-1.0, 0.0], &[0.0, -1.0]],
    );
    let mut almost = 0;
    for max_iter in 1..30 {
        let settings = Settings {
            max_iter,
            ..Settings::default()
        };
        let mut s = Solver::new(
            &[0.0, 0.0],
            &[-1.0, -1.0],
            &a,
            &[4.0, 6.0, 0.0, 0.0],
            &[Cone::Nonneg(4)],
            settings,
        )
        .unwrap();
        match s.solve() {
            Status::Solved => {}
            Status::AlmostSolved => {
                almost += 1;
                let x = s.x();
                assert!(
                    (x[0] - 1.6).abs() < 1e-3 && (x[1] - 1.2).abs() < 1e-3,
                    "max_iter {max_iter}: {x:?}"
                );
            }
            Status::MaxIterations => {}
            other => panic!("max_iter {max_iter}: {other:?}"),
        }
    }
    assert!(almost > 0, "never cut short within the reduced tolerances");
}
