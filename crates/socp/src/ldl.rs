//! Sparse LDLᵀ factorization of a symmetric quasi-definite matrix, after
//! QDLDL (Stellato, Goulart et al.): an elimination tree computed once
//! from the pattern, then numeric factorizations that allocate nothing.
//! A minimum-degree ordering, also computed once, keeps the factor sparse.

/// Minimum-degree ordering of a symmetric pattern given as adjacency lists
/// (no self loops). Returns `perm` with `perm[new] = old`.
///
/// Plain minimum degree on an explicit elimination graph: eliminating a
/// node joins its neighbours into a clique. Quadratic in the worst case;
/// run once per pattern, before any iteration.
pub fn min_degree(adj: &[Vec<usize>]) -> Vec<usize> {
    let n = adj.len();
    let mut nb: Vec<Vec<usize>> = adj
        .iter()
        .map(|a| {
            let mut a = a.clone();
            a.sort_unstable();
            a.dedup();
            a
        })
        .collect();
    let mut done = vec![false; n];
    let mut perm = Vec::with_capacity(n);
    let mut mark = vec![usize::MAX; n];
    let mut tag = 0usize;
    for _ in 0..n {
        let mut best = usize::MAX;
        let mut best_deg = usize::MAX;
        for v in 0..n {
            if !done[v] && nb[v].len() < best_deg {
                best_deg = nb[v].len();
                best = v;
                if best_deg == 0 {
                    break;
                }
            }
        }
        let v = best;
        done[v] = true;
        perm.push(v);
        let list = std::mem::take(&mut nb[v]);
        // Remove v from its neighbours; join them into a clique.
        for &a in &list {
            nb[a].retain(|&x| x != v);
        }
        for &a in &list {
            for &x in &nb[a] {
                mark[x] = tag;
            }
            mark[a] = tag;
            for &b in &list {
                if mark[b] != tag {
                    mark[b] = tag;
                    nb[a].push(b);
                }
            }
            tag += 1;
        }
    }
    perm
}

/// Symbolic and numeric LDLᵀ of an `n × n` matrix given by its upper
/// triangle in CSC form (diagonal present in every column).
pub struct Ldl {
    n: usize,
    etree: Vec<usize>,
    lnz: Vec<usize>,
    pub lp: Vec<usize>,
    li: Vec<usize>,
    lx: Vec<f64>,
    d: Vec<f64>,
    dinv: Vec<f64>,
    // Work.
    marker: Vec<bool>,
    y_idx: Vec<usize>,
    elim: Vec<usize>,
    next_space: Vec<usize>,
    y_vals: Vec<f64>,
}

const NONE: usize = usize::MAX;

impl Ldl {
    /// Elimination tree and column counts of the factor. Panics if the
    /// pattern has entries below the diagonal.
    pub fn new(n: usize, ap: &[usize], ai: &[usize]) -> Ldl {
        let mut work = vec![0usize; n];
        let mut lnz = vec![0usize; n];
        let mut etree = vec![NONE; n];
        for j in 0..n {
            work[j] = j;
            for &i0 in &ai[ap[j]..ap[j + 1]] {
                let mut i = i0;
                assert!(
                    i <= j,
                    "lower-triangular entry in an upper-triangular pattern"
                );
                while work[i] != j {
                    if etree[i] == NONE {
                        etree[i] = j;
                    }
                    lnz[i] += 1;
                    work[i] = j;
                    i = etree[i];
                }
            }
        }
        let total: usize = lnz.iter().sum();
        let mut lp = vec![0usize; n + 1];
        for i in 0..n {
            lp[i + 1] = lp[i] + lnz[i];
        }
        Ldl {
            n,
            etree,
            lnz,
            lp,
            li: vec![0; total],
            lx: vec![0.0; total],
            d: vec![0.0; n],
            dinv: vec![0.0; n],
            marker: vec![false; n],
            y_idx: vec![0; n],
            elim: vec![0; n],
            next_space: vec![0; n],
            y_vals: vec![0.0; n],
        }
    }

    /// Nonzeros in L (strictly below the diagonal).
    pub fn nnz(&self) -> usize {
        self.lx.len()
    }

    /// Numeric factorization. `sign[k]` is the expected sign of pivot `k`
    /// (+1 or −1, the matrix being quasi-definite); a pivot of the wrong
    /// sign or smaller than `eps` is replaced by `sign · delta` (dynamic
    /// regularization). Returns how many pivots were replaced.
    pub fn factor(
        &mut self,
        ap: &[usize],
        ai: &[usize],
        ax: &[f64],
        sign: &[f64],
        eps: f64,
        delta: f64,
    ) -> usize {
        let n = self.n;
        let mut bumped = 0;
        for i in 0..n {
            self.marker[i] = false;
            self.y_vals[i] = 0.0;
            self.d[i] = 0.0;
            self.next_space[i] = self.lp[i];
        }
        for k in 0..n {
            let mut nnz_y = 0;
            for p in ap[k]..ap[k + 1] {
                let b = ai[p];
                if b == k {
                    self.d[k] = ax[p];
                    continue;
                }
                self.y_vals[b] = ax[p];
                if !self.marker[b] {
                    self.marker[b] = true;
                    self.elim[0] = b;
                    let mut nnz_e = 1;
                    let mut next = self.etree[b];
                    while next != NONE && next < k {
                        if self.marker[next] {
                            break;
                        }
                        self.marker[next] = true;
                        self.elim[nnz_e] = next;
                        nnz_e += 1;
                        next = self.etree[next];
                    }
                    while nnz_e > 0 {
                        nnz_e -= 1;
                        self.y_idx[nnz_y] = self.elim[nnz_e];
                        nnz_y += 1;
                    }
                }
            }
            for i in (0..nnz_y).rev() {
                let c = self.y_idx[i];
                let end = self.next_space[c];
                let yc = self.y_vals[c];
                for j in self.lp[c]..end {
                    self.y_vals[self.li[j]] -= self.lx[j] * yc;
                }
                self.li[end] = k;
                let l = yc * self.dinv[c];
                self.lx[end] = l;
                self.d[k] -= yc * l;
                self.next_space[c] += 1;
                self.y_vals[c] = 0.0;
                self.marker[c] = false;
            }
            if self.d[k] * sign[k] < eps {
                self.d[k] = sign[k] * delta;
                bumped += 1;
            }
            self.dinv[k] = 1.0 / self.d[k];
        }
        debug_assert!((0..n).all(|i| self.next_space[i] == self.lp[i] + self.lnz[i]));
        bumped
    }

    /// Solve L D Lᵀ x = b in place.
    pub fn solve(&self, x: &mut [f64]) {
        let n = self.n;
        for i in 0..n {
            let xi = x[i];
            for j in self.lp[i]..self.lp[i + 1] {
                x[self.li[j]] -= self.lx[j] * xi;
            }
        }
        for i in 0..n {
            x[i] *= self.dinv[i];
        }
        for i in (0..n).rev() {
            let mut xi = x[i];
            for j in self.lp[i]..self.lp[i + 1] {
                xi -= self.lx[j] * x[self.li[j]];
            }
            x[i] = xi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small quasi-definite matrix, upper triangle, factored and solved.
    #[test]
    fn solves_a_quasi_definite_system() {
        // K = [[4, 1, 2], [1, 3, 0], [2, 0, -5]]
        let ap = [0, 1, 3, 5];
        let ai = [0, 0, 1, 0, 2];
        let ax = [4.0, 1.0, 3.0, 2.0, -5.0];
        let mut f = Ldl::new(3, &ap, &ai);
        let bumped = f.factor(&ap, &ai, &ax, &[1.0, 1.0, -1.0], 1e-12, 1e-8);
        assert_eq!(bumped, 0);
        let mut x = [1.0, 2.0, 3.0];
        f.solve(&mut x);
        let k = [[4.0, 1.0, 2.0], [1.0, 3.0, 0.0], [2.0, 0.0, -5.0]];
        for r in 0..3 {
            let v: f64 = (0..3).map(|c| k[r][c] * x[c]).sum();
            assert!((v - [1.0, 2.0, 3.0][r]).abs() < 1e-12);
        }
    }

    #[test]
    fn min_degree_is_a_permutation() {
        // A star around node 0: minimum degree eliminates the leaves first.
        let adj = vec![vec![1, 2, 3, 4], vec![0], vec![0], vec![0], vec![0]];
        let p = min_degree(&adj);
        let mut s = p.clone();
        s.sort_unstable();
        assert_eq!(s, vec![0, 1, 2, 3, 4]);
        assert_ne!(p[0], 0);
    }
}
