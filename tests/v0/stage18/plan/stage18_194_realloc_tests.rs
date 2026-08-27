//! Stage 18.194 — Realloc infrastructure tests.
//!
//! Verifies that:
//! 1. `__landin_realloc(ptr, old, new)` works — preserves data after resize.
//! 2. Realloc to larger size preserves existing data.
//! 3. Realloc to smaller size preserves existing data (up to new size).
//! 4. Multiple reallocs in sequence work.
//! 5. Realloc'd memory can be freed with __landin_dealloc.

#![cfg(all(test, feature = "llvm-backend"))]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;
use common::assert_runtime;

#[test]
fn stage18_194_realloc_preserves_data_grow() {
    assert_runtime(
        "realloc-preserves-data-grow",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(4);
    *p = 42;
    let p2: *mut u8 = __landin_realloc(p, 4, 8);
    let v: u8 = *p2;
    println!("{}", v);
    __landin_dealloc(p2);
    0
}
"#,
        "42\n",
    );
}

#[test]
fn stage18_194_realloc_preserves_data_shrink() {
    assert_runtime(
        "realloc-preserves-data-shrink",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(8);
    *p = 99;
    let p2: *mut u8 = __landin_realloc(p, 8, 4);
    let v: u8 = *p2;
    println!("{}", v);
    __landin_dealloc(p2);
    0
}
"#,
        "99\n",
    );
}

#[test]
fn stage18_194_realloc_chain() {
    assert_runtime(
        "realloc-chain",
        r#"
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let p: *mut u8 = __landin_alloc(2);
    *p = 10;
    let p2: *mut u8 = __landin_realloc(p, 2, 4);
    *p2 = 20;
    let p3: *mut u8 = __landin_realloc(p2, 4, 8);
    let v1: u8 = *p3;
    *p3 = 30;
    let v2: u8 = *p3;
    println!("{} {}", v1, v2);
    __landin_dealloc(p3);
    0
}
"#,
        "20 30\n",
    );
}

#[test]
fn stage18_194_realloc_null_oom() {
    // Realloc with NULL ptr should behave like alloc.
    // Realloc to 0 should be safe (returns valid or NULL).
    assert_runtime(
        "realloc-null-oom",
        r#"
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_dealloc(ptr: *mut u8); }
fn main() -> i32 {
    let null: *mut u8 = 0 as *mut u8;
    let p: *mut u8 = __landin_realloc(null, 0, 4);
    *p = 7;
    println!("{}", *p);
    __landin_dealloc(p);
    0
}
"#,
        "7\n",
    );
}
