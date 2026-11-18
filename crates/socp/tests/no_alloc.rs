//! `Solver::solve` and `Solver::update` allocate nothing: counted with a
//! global allocator that tallies every allocation in this test binary.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use socp::{Cone, Csc, Settings, Solver, Status};

struct Counting;

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[test]
fn solve_and_update_allocate_nothing() {
    // A small SOCP with every cone type: min t + ½‖x‖²·0.1 s.t. ‖(x1, x2)‖ ≤ t,
    // x1 + x2 = 1, x ≥ −1.
    let (rows, cols, vals) = (
        vec![0, 0, 1, 2, 3, 4, 5, 6],
        vec![1, 2, 1, 2, 0, 1, 2, 0],
        vec![1.0, 1.0, -1.0, -1.0, -1.0, -1.0, -1.0, -1.0],
    );
    let a = Csc::from_triplets(7, 3, &rows, &cols, &vals);
    let b = [1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 5.0];
    let cones = [
        Cone::Zero(1),
        Cone::Nonneg(2),
        Cone::Soc(3),
        Cone::Nonneg(1),
    ];
    let p = [0.1, 0.1, 0.1];
    let q = [1.0, 0.0, 0.0];
    let mut s = Solver::new(&p, &q, &a, &b, &cones, Settings::default()).unwrap();

    let before = ALLOCATIONS.load(Ordering::SeqCst);
    let st = s.solve();
    let q2 = [1.0, 0.2, -0.1];
    s.update(&p, &q2, &a, &b).unwrap();
    let st2 = s.solve();
    let after = ALLOCATIONS.load(Ordering::SeqCst);

    assert_eq!(st, Status::Solved);
    assert_eq!(st2, Status::Solved);
    assert_eq!(after - before, 0, "allocations during solve/update");
    let x = s.x();
    assert!((x[1] + x[2] - 1.0).abs() < 1e-7);
}
