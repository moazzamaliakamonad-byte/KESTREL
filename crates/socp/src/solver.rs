//! The interior-point iteration. See the crate documentation.

use crate::cones;
use crate::csc::Csc;
use crate::ldl::{self, Ldl};

/// One block of rows of `A x + s = b`, in row order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cone {
    /// `s = 0`: equality rows.
    Zero(usize),
    /// `s ≥ 0`, elementwise.
    Nonneg(usize),
    /// `s₀ ≥ ‖s₁..‖`, this many rows.
    Soc(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Settings {
    /// Most iterations; with the fixed work per iteration this bounds the run time.
    pub max_iter: usize,
    /// Feasibility tolerance (scaled residuals).
    pub tol_feas: f64,
    /// Duality gap tolerance, absolute and relative.
    pub tol_gap_abs: f64,
    pub tol_gap_rel: f64,
    /// Static regularization added to the KKT matrix's diagonal.
    pub static_reg: f64,
    /// Pivots below this are replaced by `dynamic_reg` (right sign).
    pub dynamic_eps: f64,
    pub dynamic_reg: f64,
    /// Iterative refinement steps per KKT solve.
    pub refine: usize,
    /// Fraction of the way to the cone boundary a step may go.
    pub step: f64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            max_iter: 60,
            tol_feas: 1e-8,
            tol_gap_abs: 1e-8,
            tol_gap_rel: 1e-8,
            static_reg: 1e-8,
            dynamic_eps: 1e-13,
            dynamic_reg: 1e-7,
            refine: 3,
            step: 0.99,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Unsolved,
    Solved,
    /// The iteration cap was reached first.
    MaxIterations,
    /// The iterates grew without bound: most likely infeasible.
    Diverged,
    /// A step of zero length or non-finite numbers.
    NumericalError,
}

#[derive(Debug, Clone, Copy)]
struct Block {
    /// First row in the cone rows.
    start: usize,
    dim: usize,
    soc: bool,
    /// Offset of this block's d×d matrices in the dense buffers (SOC).
    dense: usize,
}

/// Where each input entry lives in the permuted KKT matrix's values.
struct KktMap {
    p_diag: Vec<usize>,
    eq_diag: Vec<usize>,
    /// Per entry of `A` (CSC order).
    a: Vec<usize>,
    /// Per cone row: its diagonal (nonnegative rows) or, for SOC blocks,
    /// the upper triangle of the block, row-major over (r ≤ c).
    cone: Vec<usize>,
}

pub struct Solver {
    settings: Settings,
    n: usize,
    /// Equality rows (p) and cone rows (m), as indices into A's rows.
    a: Csc,
    row_kind: Vec<(bool, usize)>,
    p_eq: usize,
    m_cone: usize,
    blocks: Vec<Block>,
    /// Degree of the cone (nonnegative rows + SOC blocks).
    nu: f64,
    p_diag: Vec<f64>,
    q: Vec<f64>,
    b_eq: Vec<f64>,
    h: Vec<f64>,
    // KKT.
    kkt_n: usize,
    perm: Vec<usize>,
    kp: Vec<usize>,
    ki: Vec<usize>,
    kx: Vec<f64>,
    sign: Vec<f64>,
    map: KktMap,
    ldl: Ldl,
    // Iterates.
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    pub z: Vec<f64>,
    pub s: Vec<f64>,
    // Scaling.
    w_nn: Vec<f64>,
    w: Vec<f64>,
    w2: Vec<f64>,
    winv: Vec<f64>,
    wbar: Vec<f64>,
    lambda: Vec<f64>,
    // Work.
    rx: Vec<f64>,
    ry: Vec<f64>,
    rz: Vec<f64>,
    rhs: Vec<f64>,
    sol: Vec<f64>,
    res: Vec<f64>,
    tmp: Vec<f64>,
    tmp2: Vec<f64>,
    perm_buf: Vec<f64>,
    ds: Vec<f64>,
    dx_a: Vec<f64>,
    dz_a: Vec<f64>,
    ds_a: Vec<f64>,
    dx: Vec<f64>,
    dy: Vec<f64>,
    dz: Vec<f64>,
    dsv: Vec<f64>,
    status: Status,
    iterations: usize,
}

fn norm(v: &[f64]) -> f64 {
    v.iter().map(|c| c * c).sum::<f64>().sqrt()
}

fn dot(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}

impl Solver {
    /// Analyse the pattern, order and allocate everything. `p_diag` is the
    /// diagonal of P (length n).
    pub fn new(
        p_diag: &[f64],
        q: &[f64],
        a: &Csc,
        b: &[f64],
        cones: &[Cone],
        settings: Settings,
    ) -> Result<Solver, &'static str> {
        let n = a.n;
        if p_diag.len() != n || q.len() != n || b.len() != a.m {
            return Err("dimension mismatch");
        }
        let rows: usize = cones
            .iter()
            .map(|c| match c {
                Cone::Zero(k) | Cone::Nonneg(k) | Cone::Soc(k) => *k,
            })
            .sum();
        if rows != a.m {
            return Err("cone sizes do not add up to the rows of A");
        }
        // Split rows.
        let mut row_kind = Vec::with_capacity(a.m);
        let mut blocks = Vec::new();
        let (mut p_eq, mut m_cone, mut dense) = (0, 0, 0);
        let mut nu = 0.0;
        for c in cones {
            match *c {
                Cone::Zero(k) => {
                    for _ in 0..k {
                        row_kind.push((true, p_eq));
                        p_eq += 1;
                    }
                }
                Cone::Nonneg(k) => {
                    if k > 0 {
                        blocks.push(Block {
                            start: m_cone,
                            dim: k,
                            soc: false,
                            dense: 0,
                        });
                    }
                    for _ in 0..k {
                        row_kind.push((false, m_cone));
                        m_cone += 1;
                    }
                    nu += k as f64;
                }
                Cone::Soc(k) => {
                    if k == 0 {
                        continue;
                    }
                    blocks.push(Block {
                        start: m_cone,
                        dim: k,
                        soc: true,
                        dense,
                    });
                    dense += k * k;
                    for _ in 0..k {
                        row_kind.push((false, m_cone));
                        m_cone += 1;
                    }
                    nu += 1.0;
                }
            }
        }
        let kkt_n = n + p_eq + m_cone;
        // KKT upper-triangle entries in the original order, each tagged with
        // where its value comes from.
        #[derive(Clone, Copy)]
        enum Src {
            P(usize),
            Eq(usize),
            A(usize),
            Cone(usize),
        }
        let mut entries: Vec<(usize, usize, Src)> = Vec::new();
        for j in 0..n {
            entries.push((j, j, Src::P(j)));
        }
        for e in 0..p_eq {
            entries.push((n + e, n + e, Src::Eq(e)));
        }
        for col in 0..n {
            for k in a.colptr[col]..a.colptr[col + 1] {
                let (is_eq, i) = row_kind[a.rowval[k]];
                let r = if is_eq { n + i } else { n + p_eq + i };
                entries.push((col, r, Src::A(k)));
            }
        }
        let mut cone_count = 0;
        for blk in &blocks {
            if blk.soc {
                for r in 0..blk.dim {
                    for c in r..blk.dim {
                        let (gr, gc) = (n + p_eq + blk.start + r, n + p_eq + blk.start + c);
                        entries.push((gr, gc, Src::Cone(cone_count)));
                        cone_count += 1;
                    }
                }
            } else {
                for r in 0..blk.dim {
                    let g = n + p_eq + blk.start + r;
                    entries.push((g, g, Src::Cone(cone_count)));
                    cone_count += 1;
                }
            }
        }
        // Ordering on the symmetric pattern.
        let mut adj = vec![Vec::new(); kkt_n];
        for &(r, c, _) in &entries {
            if r != c {
                adj[r].push(c);
                adj[c].push(r);
            }
        }
        let perm = ldl::min_degree(&adj);
        let mut pinv = vec![0; kkt_n];
        for (new, &old) in perm.iter().enumerate() {
            pinv[old] = new;
        }
        // Permuted upper triangle, CSC, with the source of each value.
        let mut placed: Vec<(usize, usize, usize)> = entries
            .iter()
            .enumerate()
            .map(|(id, &(r, c, _))| {
                let (a1, b1) = (pinv[r], pinv[c]);
                (a1.min(b1), a1.max(b1), id)
            })
            .collect();
        placed.sort_unstable_by_key(|&(r, c, _)| (c, r));
        let mut kp = vec![0; kkt_n + 1];
        let mut ki = Vec::with_capacity(placed.len());
        let mut pos = vec![0; entries.len()];
        for (k, &(r, c, id)) in placed.iter().enumerate() {
            kp[c + 1] += 1;
            ki.push(r);
            pos[id] = k;
        }
        for c in 0..kkt_n {
            kp[c + 1] += kp[c];
        }
        let mut map = KktMap {
            p_diag: vec![0; n],
            eq_diag: vec![0; p_eq],
            a: vec![0; a.nzval.len()],
            cone: vec![0; cone_count],
        };
        for (id, &(_, _, src)) in entries.iter().enumerate() {
            match src {
                Src::P(j) => map.p_diag[j] = pos[id],
                Src::Eq(e) => map.eq_diag[e] = pos[id],
                Src::A(k) => map.a[k] = pos[id],
                Src::Cone(k) => map.cone[k] = pos[id],
            }
        }
        let sign = perm
            .iter()
            .map(|&old| if old < n { 1.0 } else { -1.0 })
            .collect();
        let ldl = Ldl::new(kkt_n, &kp, &ki);
        let nnz = ki.len();
        let mut solver = Solver {
            settings,
            n,
            a: a.clone(),
            row_kind,
            p_eq,
            m_cone,
            blocks,
            nu,
            p_diag: p_diag.to_vec(),
            q: q.to_vec(),
            b_eq: vec![0.0; p_eq],
            h: vec![0.0; m_cone],
            kkt_n,
            perm,
            kp,
            ki,
            kx: vec![0.0; nnz],
            sign,
            map,
            ldl,
            x: vec![0.0; n],
            y: vec![0.0; p_eq],
            z: vec![0.0; m_cone],
            s: vec![0.0; m_cone],
            w_nn: vec![0.0; m_cone],
            w: vec![0.0; dense],
            w2: vec![0.0; dense],
            winv: vec![0.0; dense],
            wbar: vec![0.0; m_cone],
            lambda: vec![0.0; m_cone],
            rx: vec![0.0; n],
            ry: vec![0.0; p_eq],
            rz: vec![0.0; m_cone],
            rhs: vec![0.0; kkt_n],
            sol: vec![0.0; kkt_n],
            res: vec![0.0; kkt_n],
            tmp: vec![0.0; m_cone],
            tmp2: vec![0.0; m_cone],
            perm_buf: vec![0.0; kkt_n],
            ds: vec![0.0; m_cone],
            dx_a: vec![0.0; n],
            dz_a: vec![0.0; m_cone],
            ds_a: vec![0.0; m_cone],
            dx: vec![0.0; n],
            dy: vec![0.0; p_eq],
            dz: vec![0.0; m_cone],
            dsv: vec![0.0; m_cone],
            status: Status::Unsolved,
            iterations: 0,
        };
        solver.split_b(b);
        Ok(solver)
    }

    fn split_b(&mut self, b: &[f64]) {
        for (row, &(is_eq, i)) in self.row_kind.iter().enumerate() {
            if is_eq {
                self.b_eq[i] = b[row];
            } else {
                self.h[i] = b[row];
            }
        }
    }

    /// New numbers for the same pattern (no allocation).
    pub fn update(
        &mut self,
        p_diag: &[f64],
        q: &[f64],
        a: &Csc,
        b: &[f64],
    ) -> Result<(), &'static str> {
        if !self.a.same_pattern(a) || p_diag.len() != self.n || q.len() != self.n || b.len() != a.m
        {
            return Err("pattern changed");
        }
        self.a.nzval.copy_from_slice(&a.nzval);
        self.p_diag.copy_from_slice(p_diag);
        self.q.copy_from_slice(q);
        self.split_b(b);
        self.status = Status::Unsolved;
        Ok(())
    }

    /// Same pattern as `a` (so `update` will accept it).
    pub fn accepts(&self, a: &Csc) -> bool {
        self.a.same_pattern(a)
    }

    pub fn status(&self) -> Status {
        self.status
    }
    pub fn iterations(&self) -> usize {
        self.iterations
    }
    /// Nonzeros in the KKT matrix (upper triangle) and in its factor.
    pub fn sizes(&self) -> (usize, usize, usize) {
        (self.kkt_n, self.ki.len(), self.ldl.nnz())
    }

    /// Gx (cone rows) and Ax (equality rows) of `x`, added into `eq`, `cone`.
    fn mul_a(&self, x: &[f64], eq: &mut [f64], cone: &mut [f64]) {
        let a = &self.a;
        for c in 0..a.n {
            let xc = x[c];
            if xc == 0.0 {
                continue;
            }
            for k in a.colptr[c]..a.colptr[c + 1] {
                let (is_eq, i) = self.row_kind[a.rowval[k]];
                if is_eq {
                    eq[i] += a.nzval[k] * xc;
                } else {
                    cone[i] += a.nzval[k] * xc;
                }
            }
        }
    }

    /// Aᵀy + Gᵀz added into `out`.
    fn tmul_a(&self, y: &[f64], z: &[f64], out: &mut [f64]) {
        let a = &self.a;
        for c in 0..a.n {
            let mut acc = 0.0;
            for k in a.colptr[c]..a.colptr[c + 1] {
                let (is_eq, i) = self.row_kind[a.rowval[k]];
                acc += a.nzval[k] * if is_eq { y[i] } else { z[i] };
            }
            out[c] += acc;
        }
    }

    /// out = W² v over the cone rows (current scaling).
    fn mul_w2(&self, v: &[f64], out: &mut [f64]) {
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                let d = blk.dim;
                cones::dense_mul(
                    &self.w2[blk.dense..blk.dense + d * d],
                    &v[r.clone()],
                    &mut out[r],
                );
            } else {
                for i in r {
                    out[i] = self.w_nn[i] * self.w_nn[i] * v[i];
                }
            }
        }
    }

    /// out = W v (`inverse`: W⁻¹ v) over the cone rows.
    fn mul_w(&self, v: &[f64], out: &mut [f64], inverse: bool) {
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                let d = blk.dim;
                let m = if inverse { &self.winv } else { &self.w };
                cones::dense_mul(&m[blk.dense..blk.dense + d * d], &v[r.clone()], &mut out[r]);
            } else {
                for i in r {
                    out[i] = if inverse {
                        v[i] / self.w_nn[i]
                    } else {
                        self.w_nn[i] * v[i]
                    };
                }
            }
        }
    }

    /// Fill the KKT values: P + δ, −δ, A, −(W² + δ).
    fn assemble(&mut self, identity_scaling: bool) {
        let delta = self.settings.static_reg;
        for j in 0..self.n {
            self.kx[self.map.p_diag[j]] = self.p_diag[j] + delta;
        }
        for e in 0..self.p_eq {
            self.kx[self.map.eq_diag[e]] = -delta;
        }
        for k in 0..self.a.nzval.len() {
            self.kx[self.map.a[k]] = self.a.nzval[k];
        }
        let mut c = 0;
        for bi in 0..self.blocks.len() {
            let blk = self.blocks[bi];
            if blk.soc {
                let d = blk.dim;
                for r in 0..d {
                    for col in r..d {
                        let v = if identity_scaling {
                            if r == col { 1.0 } else { 0.0 }
                        } else {
                            self.w2[blk.dense + r * d + col]
                        };
                        let reg = if r == col { delta } else { 0.0 };
                        self.kx[self.map.cone[c]] = -(v + reg);
                        c += 1;
                    }
                }
            } else {
                for r in 0..blk.dim {
                    let i = blk.start + r;
                    let v = if identity_scaling {
                        1.0
                    } else {
                        self.w_nn[i] * self.w_nn[i]
                    };
                    self.kx[self.map.cone[c]] = -(v + delta);
                    c += 1;
                }
            }
        }
    }

    fn factor(&mut self) {
        let s = self.settings;
        self.ldl.factor(
            &self.kp,
            &self.ki,
            &self.kx,
            &self.sign,
            s.dynamic_eps,
            s.dynamic_reg,
        );
    }

    /// K v for the unregularized KKT matrix (W² or I in the cone block).
    fn kkt_mul(&self, v: &[f64], out: &mut [f64], identity_scaling: bool) {
        let (n, p) = (self.n, self.p_eq);
        let (vx, rest) = v.split_at(n);
        let (vy, vz) = rest.split_at(p);
        let (ox, rest) = out.split_at_mut(n);
        let (oy, oz) = rest.split_at_mut(p);
        for j in 0..n {
            ox[j] = self.p_diag[j] * vx[j];
        }
        self.tmul_a(vy, vz, ox);
        oy.fill(0.0);
        if identity_scaling {
            for i in 0..oz.len() {
                oz[i] = -vz[i];
            }
        } else {
            self.mul_w2(vz, oz);
            for c in oz.iter_mut() {
                *c = -*c;
            }
        }
        self.mul_a(vx, oy, oz);
    }

    /// Solve K sol = rhs with the current factor and iterative refinement.
    fn kkt_solve(&mut self, identity_scaling: bool) {
        let nk = self.kkt_n;
        for i in 0..nk {
            self.perm_buf[i] = self.rhs[self.perm[i]];
        }
        self.ldl.solve(&mut self.perm_buf);
        for i in 0..nk {
            self.sol[self.perm[i]] = self.perm_buf[i];
        }
        for _ in 0..self.settings.refine {
            let mut res = std::mem::take(&mut self.res);
            self.kkt_mul(&self.sol, &mut res, identity_scaling);
            let mut worst: f64 = 0.0;
            for i in 0..nk {
                res[i] = self.rhs[i] - res[i];
                worst = worst.max(res[i].abs());
            }
            if worst < 1e-14 {
                self.res = res;
                break;
            }
            for i in 0..nk {
                self.perm_buf[i] = res[self.perm[i]];
            }
            self.res = res;
            self.ldl.solve(&mut self.perm_buf);
            for i in 0..nk {
                self.sol[self.perm[i]] += self.perm_buf[i];
            }
        }
    }

    /// Smallest margin of `v` inside the cone over all blocks.
    fn min_margin(&self, v: &[f64]) -> f64 {
        let mut m = f64::INFINITY;
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                m = m.min(cones::soc_margin(&v[r]));
            } else {
                for i in r {
                    m = m.min(v[i]);
                }
            }
        }
        m
    }

    /// v += a · e (e: the cone's identity).
    fn add_identity(&self, v: &mut [f64], a: f64) {
        for blk in &self.blocks {
            if blk.soc {
                v[blk.start] += a;
            } else {
                for i in blk.start..blk.start + blk.dim {
                    v[i] += a;
                }
            }
        }
    }

    /// Largest step along (ds, dz) keeping s and z in their cones.
    fn max_step(&self, ds: &[f64], dz: &[f64]) -> f64 {
        let mut a = f64::INFINITY;
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                a = a.min(cones::soc_max_step(&self.s[r.clone()], &ds[r.clone()]));
                a = a.min(cones::soc_max_step(&self.z[r.clone()], &dz[r]));
            } else {
                for i in r {
                    if ds[i] < 0.0 {
                        a = a.min(-self.s[i] / ds[i]);
                    }
                    if dz[i] < 0.0 {
                        a = a.min(-self.z[i] / dz[i]);
                    }
                }
            }
        }
        a
    }

    /// NT scaling at the current (s, z) and λ = W z.
    fn scale(&mut self) {
        for bi in 0..self.blocks.len() {
            let blk = self.blocks[bi];
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                let d = blk.dim;
                let dr = blk.dense..blk.dense + d * d;
                cones::soc_scaling(
                    &self.s[r.clone()],
                    &self.z[r.clone()],
                    &mut self.w[dr.clone()],
                    &mut self.w2[dr.clone()],
                    &mut self.winv[dr.clone()],
                    &mut self.wbar[r.clone()],
                );
                cones::dense_mul(&self.w[dr], &self.z[r.clone()], &mut self.lambda[r]);
            } else {
                for i in r {
                    self.w_nn[i] = (self.s[i] / self.z[i]).sqrt();
                    self.lambda[i] = (self.s[i] * self.z[i]).sqrt();
                }
            }
        }
    }

    /// out = λ \ d (Jordan division) over the cone rows.
    fn lambda_div(&self, d: &[f64], out: &mut [f64]) {
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                cones::soc_div(&self.lambda[r.clone()], &d[r.clone()], &mut out[r]);
            } else {
                for i in r {
                    out[i] = d[i] / self.lambda[i];
                }
            }
        }
    }

    /// out = u ∘ v over the cone rows.
    fn jordan(&self, u: &[f64], v: &[f64], out: &mut [f64]) {
        for blk in &self.blocks {
            let r = blk.start..blk.start + blk.dim;
            if blk.soc {
                cones::soc_prod(&u[r.clone()], &v[r.clone()], &mut out[r]);
            } else {
                for i in r {
                    out[i] = u[i] * v[i];
                }
            }
        }
    }

    /// One Newton direction for complementarity right-hand side `self.ds`
    /// (`λ ∘ (W⁻ᵀΔs + WΔz) = ds`): fills dx, dy, dz, dsv.
    fn direction(&mut self) {
        let (n, p) = (self.n, self.p_eq);
        // tmp = λ \ ds; tmp2 = W tmp
        let mut t = std::mem::take(&mut self.tmp);
        let mut t2 = std::mem::take(&mut self.tmp2);
        self.lambda_div(&self.ds, &mut t);
        self.mul_w(&t, &mut t2, false);
        for j in 0..n {
            self.rhs[j] = -self.rx[j];
        }
        for e in 0..p {
            self.rhs[n + e] = -self.ry[e];
        }
        for i in 0..self.m_cone {
            self.rhs[n + p + i] = -self.rz[i] - t2[i];
        }
        self.kkt_solve(false);
        self.dx.copy_from_slice(&self.sol[..n]);
        self.dy.copy_from_slice(&self.sol[n..n + p]);
        self.dz.copy_from_slice(&self.sol[n + p..]);
        // Δs = W (λ\ds − W Δz) = t2 − W² Δz
        let mut w2dz = std::mem::take(&mut self.dsv);
        self.mul_w2(&self.dz, &mut w2dz);
        for i in 0..self.m_cone {
            w2dz[i] = t2[i] - w2dz[i];
        }
        self.dsv = w2dz;
        self.tmp = t;
        self.tmp2 = t2;
    }

    fn residuals(&mut self) {
        let n = self.n;
        // r_x = P x + q + Aᵀy + Gᵀz
        for j in 0..n {
            self.rx[j] = self.p_diag[j] * self.x[j] + self.q[j];
        }
        let mut rx = std::mem::take(&mut self.rx);
        self.tmul_a(&self.y, &self.z, &mut rx);
        self.rx = rx;
        // r_y = A x − b, r_z = G x + s − h
        for e in 0..self.p_eq {
            self.ry[e] = -self.b_eq[e];
        }
        for i in 0..self.m_cone {
            self.rz[i] = self.s[i] - self.h[i];
        }
        let mut ry = std::mem::take(&mut self.ry);
        let mut rz = std::mem::take(&mut self.rz);
        self.mul_a(&self.x, &mut ry, &mut rz);
        self.ry = ry;
        self.rz = rz;
    }

    /// Run the iteration from a fresh start. Allocates nothing.
    pub fn solve(&mut self) -> Status {
        let (n, p, m) = (self.n, self.p_eq, self.m_cone);
        let st = self.settings;
        // Start: the KKT system with W = I gives x, y and z = G x − h;
        // s = −z; then both shifted into their cones.
        self.assemble(true);
        self.factor();
        for j in 0..n {
            self.rhs[j] = -self.q[j];
        }
        for e in 0..p {
            self.rhs[n + e] = self.b_eq[e];
        }
        for i in 0..m {
            self.rhs[n + p + i] = self.h[i];
        }
        self.kkt_solve(true);
        self.x.copy_from_slice(&self.sol[..n]);
        self.y.copy_from_slice(&self.sol[n..n + p]);
        for i in 0..m {
            self.z[i] = self.sol[n + p + i];
            self.s[i] = -self.sol[n + p + i];
        }
        let mut s = std::mem::take(&mut self.s);
        let ap = -self.min_margin(&s);
        if ap >= -1e-8 {
            self.add_identity(&mut s, 1.0 + ap);
        }
        self.s = s;
        let mut z = std::mem::take(&mut self.z);
        let ad = -self.min_margin(&z);
        if ad >= -1e-8 {
            self.add_identity(&mut z, 1.0 + ad);
        }
        self.z = z;

        let norm_q = norm(&self.q).max(1.0);
        let norm_bh = norm(&self.b_eq).max(norm(&self.h)).max(1.0);
        self.status = Status::MaxIterations;
        self.iterations = 0;
        for it in 0..=st.max_iter {
            self.residuals();
            let gap = dot(&self.s, &self.z);
            let mu = gap / self.nu.max(1.0);
            let pcost = 0.5
                * (0..n)
                    .map(|j| self.p_diag[j] * self.x[j] * self.x[j])
                    .sum::<f64>()
                + dot(&self.q, &self.x);
            let pres = (norm(&self.ry) / norm_bh).max(norm(&self.rz) / norm_bh);
            let dres = norm(&self.rx) / norm_q;
            let relgap = gap / pcost.abs().max(1e-12);
            if !(pres.is_finite() && dres.is_finite() && gap.is_finite()) {
                self.status = Status::NumericalError;
                break;
            }
            if pres <= st.tol_feas
                && dres <= st.tol_feas
                && (gap <= st.tol_gap_abs || relgap <= st.tol_gap_rel)
            {
                self.status = Status::Solved;
                break;
            }
            if norm(&self.x) > 1e12 || norm(&self.z) > 1e12 {
                self.status = Status::Diverged;
                break;
            }
            if it == st.max_iter {
                break;
            }
            self.iterations = it + 1;
            self.scale();
            self.assemble(false);
            self.factor();
            // Predictor: ds = −λ∘λ.
            let mut ds = std::mem::take(&mut self.ds);
            self.jordan(&self.lambda, &self.lambda, &mut ds);
            for c in ds.iter_mut() {
                *c = -*c;
            }
            self.ds = ds;
            self.direction();
            self.dx_a.copy_from_slice(&self.dx);
            self.dz_a.copy_from_slice(&self.dz);
            self.ds_a.copy_from_slice(&self.dsv);
            let alpha_a = self.max_step(&self.ds_a, &self.dz_a).min(1.0);
            let sigma = (1.0 - alpha_a).clamp(0.0, 1.0).powi(3);
            // Corrector: ds = −λ∘λ − (W⁻ᵀΔs_a)∘(WΔz_a) + σμe.
            let mut a1 = std::mem::take(&mut self.tmp);
            let mut a2 = std::mem::take(&mut self.tmp2);
            self.mul_w(&self.ds_a, &mut a1, true);
            self.mul_w(&self.dz_a, &mut a2, false);
            let mut ds = std::mem::take(&mut self.ds);
            self.jordan(&a1, &a2, &mut ds);
            self.tmp = a1;
            self.tmp2 = a2;
            let mut ll = std::mem::take(&mut self.dsv);
            self.jordan(&self.lambda, &self.lambda, &mut ll);
            for i in 0..m {
                ds[i] = -ll[i] - ds[i];
            }
            self.dsv = ll;
            self.add_identity(&mut ds, sigma * mu);
            self.ds = ds;
            self.direction();
            let alpha = (st.step * self.max_step(&self.dsv, &self.dz)).min(1.0);
            if !(alpha > 0.0) || !alpha.is_finite() {
                self.status = Status::NumericalError;
                break;
            }
            for j in 0..n {
                self.x[j] += alpha * self.dx[j];
            }
            for e in 0..p {
                self.y[e] += alpha * self.dy[e];
            }
            for i in 0..m {
                self.s[i] += alpha * self.dsv[i];
                self.z[i] += alpha * self.dz[i];
            }
        }
        self.status
    }

    /// The solution in the input's row order: (x, s, dual of each row).
    pub fn x(&self) -> &[f64] {
        &self.x
    }
}
