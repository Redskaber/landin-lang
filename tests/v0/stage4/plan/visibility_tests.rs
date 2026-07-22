//! Stage 4.12: Visibility enforcement tests
//!
//! Tests that current_module tracking + visibility enforcement infrastructure
//! works correctly.
//!
//! Per stage-committee-process.md v3.18 §17.1, new tests are placed in
//! `tests/v0/stage4/plan/` (standardized directory structure).

use landin_compiler::compile;

#[test]
fn test_pub_visible_cross_module() {
    // pub fn in a nested module should be accessible from crate root.
    let result = compile("mod inner { pub fn public_fn() {} } fn main() { inner::public_fn(); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}

#[test]
fn test_private_visible_same_module() {
    // Private fn in same module (crate root) should be accessible.
    let result = compile("fn private_fn() {} fn main() { private_fn(); }");
    assert!(!result.mirs.is_empty(), "should produce MIR");
}
