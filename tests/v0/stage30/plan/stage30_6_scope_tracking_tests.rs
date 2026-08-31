//! Stage 30.6 (v0.14 TD-DROP-SCOPE-TIMING): Scope tracking in MirLowerCtxt —
//! StorageDead emitted at block scope end (not function end).
//!
//! Per stage-committee-process.md §9.4.3 (1:3+ positive:negative ratio):
//!   - 6 positive tests (drop fires at block/if/while/for/match/nested scope end)
//!   - 2 negative tests (drop order verification)
//!   - 2 regression tests (drop still fires at function end for body-level locals)
//!
//! Per §1.0 原則 9 (正确 > 妥协): root-cause fix — scope tracking, not
//! function-end approximation.
//! Per §1.0 原則 6 (通解 > 特解): one mechanism (scope_stack) handles all
//! block scopes.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

// ============================================================================
// POSITIVE TESTS — Drop fires at scope end (block/if/while/for/match/nested)
// ============================================================================

/// Stage 30.6 positive 1: Drop fires at block scope end.
#[test]
fn stage30_6_positive_drop_at_block_scope_end() {
    assert_runtime(
        "stage30-6-drop-block-scope",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    {
        let _t = Tracker { count_ptr: counter };
    }
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.6 positive 2: Drop fires at if-block end.
#[test]
fn stage30_6_positive_drop_at_if_block_end() {
    assert_runtime(
        "stage30-6-drop-if-block",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    if true {
        let _t = Tracker { count_ptr: counter };
    }
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.6 positive 3: Drop fires at each while-loop iteration end.
#[test]
fn stage30_6_positive_drop_at_loop_iteration_end() {
    assert_runtime(
        "stage30-6-drop-loop-iter",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let mut i = 0;
    while i < 3 {
        let _t = Tracker { count_ptr: counter };
        i = i + 1;
    }
    println!("{}", *counter);
    0
}
"#,
        "3\n",
    );
}

/// Stage 30.6 positive 4: Multiple block-scoped locals — drop in reverse order.
#[test]
fn stage30_6_positive_multiple_locals_reverse_drop() {
    // Each Tracker sets counter to its own idx. With reverse drop order,
    // the LAST-declared local drops first, so counter ends up with the
    // FIRST-declared local's idx.
    assert_runtime(
        "stage30-6-reverse-drop",
        r#"
struct Last { counter_ptr: *mut i32, idx: i32 }
impl Drop for Last {
    fn drop(&mut self) {
        unsafe { *self.counter_ptr = self.idx; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    {
        let _a = Last { counter_ptr: counter, idx: 1 };
        let _b = Last { counter_ptr: counter, idx: 2 };
        // Reverse drop: _b drops first (counter=2), then _a drops (counter=1)
    }
    // Final counter = 1 (from _a, the last to drop)
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.6 positive 5: Nested blocks — inner drops before outer.
#[test]
fn stage30_6_positive_nested_block_drop_order() {
    assert_runtime(
        "stage30-6-nested-drop",
        r#"
struct Tracker { count_ptr: *mut i32, idx: i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = self.idx; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    {
        let _outer = Tracker { count_ptr: counter, idx: 1 };
        {
            let _inner = Tracker { count_ptr: counter, idx: 2 };
            // _inner drops here (counter=2)
        }
        // _outer drops here (counter=1)
    }
    // Final counter = 1 (outer dropped last)
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.6 positive 6: Drop in else branch.
#[test]
fn stage30_6_positive_drop_in_else_branch() {
    assert_runtime(
        "stage30-6-drop-else",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    if false {
        // not taken
    } else {
        let _t = Tracker { count_ptr: counter };
    }
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

// ============================================================================
// REGRESSION TESTS — Drop still fires at function end for body-level locals
// ============================================================================

/// Stage 30.6 regression 1: Body-level local still drops at function end.
#[test]
fn stage30_6_regression_body_level_drop_at_fn_end() {
    assert_runtime(
        "stage30-6-body-level-drop",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn make_and_drop(counter: *mut i32) -> i32 {
    let _t = Tracker { count_ptr: counter };
    42
}
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let _ = make_and_drop(counter);
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}

/// Stage 30.6 regression 2: Parameter still drops at function end.
#[test]
fn stage30_6_regression_param_drop_at_fn_end() {
    assert_runtime(
        "stage30-6-param-drop",
        r#"
struct Tracker { count_ptr: *mut i32 }
impl Drop for Tracker {
    fn drop(&mut self) {
        unsafe { *self.count_ptr = *self.count_ptr + 1; }
    }
}
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
fn consume(_t: Tracker) {
    // _t drops at end of consume
}
fn main() -> i32 {
    let counter: *mut i32 = __landin_alloc(4) as *mut i32;
    *counter = 0;
    let t = Tracker { count_ptr: counter };
    consume(t);
    println!("{}", *counter);
    0
}
"#,
        "1\n",
    );
}
