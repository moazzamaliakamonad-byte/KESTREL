//! Compressed sparse column matrices.

/// An `m × n` matrix in compressed sparse column form, rows sorted within
/// each column, no duplicates.
#[derive(Debug, Clone, PartialEq)]
pub struct Csc {
    pub m: usize,
    pub n: usize,
    pub colptr: Vec<usize>,
    pub rowval: Vec<usize>,
    pub nzval: Vec<f64>,
}

impl Csc {
    /// From (row, column, value) triplets; duplicates are summed. Explicit
    /// zeros are kept (they hold a place in the pattern).
    pub fn from_triplets(m: usize, n: usize, rows: &[usize], cols: &[usize], vals: &[f64]) -> Csc {
        assert!(rows.len() == cols.len() && cols.len() == vals.len());
        let mut order: Vec<usize> = (0..rows.len()).collect();
        order.sort_by_key(|&k| (cols[k], rows[k]));
        let mut colptr = vec![0; n + 1];
        let mut rowval = Vec::with_capacity(rows.len());
        let mut nzval: Vec<f64> = Vec::with_capacity(rows.len());
        let mut last: Option<(usize, usize)> = None;
        for &k in &order {
            let (r, c) = (rows[k], cols[k]);
            assert!(r < m && c < n, "triplet out of range");
            if last == Some((r, c)) {
                *nzval.last_mut().expect("entry") += vals[k];
                continue;
            }
            last = Some((r, c));
            rowval.push(r);
            nzval.push(vals[k]);
            colptr[c + 1] += 1;
        }
        for c in 0..n {
            colptr[c + 1] += colptr[c];
        }
        Csc {
            m,
            n,
            colptr,
            rowval,
            nzval,
        }
    }

    /// Same shape and pattern (values may differ).
    pub fn same_pattern(&self, other: &Csc) -> bool {
        self.m == other.m
            && self.n == other.n
            && self.colptr == other.colptr
            && self.rowval == other.rowval
    }

    /// y += A x
    pub fn mul_add(&self, x: &[f64], y: &mut [f64]) {
        for c in 0..self.n {
            let xc = x[c];
            if xc == 0.0 {
                continue;
            }
            for p in self.colptr[c]..self.colptr[c + 1] {
                y[self.rowval[p]] += self.nzval[p] * xc;
            }
        }
    }

    /// y += Aᵀ x
    pub fn tmul_add(&self, x: &[f64], y: &mut [f64]) {
        for c in 0..self.n {
            let mut acc = 0.0;
            for p in self.colptr[c]..self.colptr[c + 1] {
                acc += self.nzval[p] * x[self.rowval[p]];
            }
            y[c] += acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triplets_sum_duplicates_and_sort() {
        let a = Csc::from_triplets(2, 2, &[1, 0, 1, 0], &[0, 1, 0, 0], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(a.colptr, vec![0, 2, 3]);
        assert_eq!(a.rowval, vec![0, 1, 0]);
        assert_eq!(a.nzval, vec![4.0, 4.0, 2.0]);
        let mut y = vec![0.0; 2];
        a.mul_add(&[1.0, 1.0], &mut y);
        assert_eq!(y, vec![6.0, 4.0]);
        let mut z = vec![0.0; 2];
        a.tmul_add(&[1.0, 1.0], &mut z);
        assert_eq!(z, vec![8.0, 2.0]);
    }
}
