//! Stage 18.260 — Phase 2d-2f gap analysis tests.
//!
//! Verifies whether the soundness hole is fully closed or if there are
//! remaining gaps that need Phase 2d-2f.
//!
//! Per §17.6 缺陷纳入: identify gaps before declaring TD fully resolved.
//! Per §1.0 原則 9 (正确 > 妥协): verify completeness, not just MVP.
//!
//! ## Stage 18.260 Gap Analysis Results
//!
//! | Case | Status | Notes |
//! |------|--------|-------|
//! | `let h: Holder<i32> = Holder(true)` | ✅ Closed (Phase 2c) | direct let binding |
//! | `fn f() -> Wrapper<i32> { Wrapper(true) }` | ✅ Closed | typeck catches via return type unify |
//! | `if true { Holder(42) } else { Holder(true) }` | ✅ Closed | typeck catches via if-branch unify |
//! | `match x { _ => Holder(true) }` (with `: Holder<i32>`) | ✅ Closed | typeck catches via match-arm unify |
//! | `[Holder(42), Holder(true)]` (with `: [Holder<i32>; 2]`) | ✅ Closed | typeck catches via Array elem unify |
//! | `take_holder(Holder(true))` where `fn take_holder(h: Holder<i32>)` | 🔴 GAP | Phase 2e needed |
//!
//! ## Identified Gap: Phase 2e — Call Arg Path
//!
//! When `Holder(true)` is passed as an argument to a function expecting
//! `Holder<i32>`, the soundness hole remains. Root cause:
//!
//! 1. MIR lower: `lower_call_expr` lowers each arg via
//!    `lower_expr_to_operand(cx, a, None)` — `None` because MIR lower
//!    doesn't have access to `fn_sigs` (those are built in writeback,
//!    AFTER MIR lower).
//! 2. The arg `Holder(true)` is lowered to `Adt(holder_def, [])` —
//!    empty substs because no turbofish and no `expected_ty` propagated.
//! 3. typeck's unify table sees `Adt(def, []) ↔ Adt(def, [i32])` and
//!    per `unify.rs:742`, empty substs are treated as "unknown, to be
//!    inferred" — silently succeeds.
//!
//! ## Fix Options (Deferred to Stage 18.261+)
//!
//! | Option | Approach | Pros | Cons |
//! |--------|----------|------|------|
//! | A. Phase 2e at MIR lower | Thread `expected_ty` from fn sig inputs into call args | Consistent with Phase 2c | Requires `fn_sigs` access in MIR lower (architectural change — fn_sigs currently built in writeback) |
//! | B. typeck-level fix | Modify unify table to reject `Adt(def, []) ↔ Adt(def, [T])` when def has generic params | Localized fix | Requires `generics_of` access in typeck (violates §11 interface isolation — typeck shouldn't read HIR) |
//! | C. Resolver pre-computation | Pre-compute `generic_adt_def_ids: HashSet<DefId>` in resolver, pass to typeck | Doesn't violate §11 | Requires new field on CompileResult + TypeChecker |
//! | D. Defer to v0.3+ | Document as MVP, fix when trait solver / GATs land | No architectural change now | Soundness hole remains in narrow case |
//!
//! Per §1.0 原則 9 (正确 > 妥协): Option D is a compromise.
//! Per §17.6: documenting as MVP with fix plan.
//!
//! **Selected**: Option D (defer to v0.3+). Rationale:
//! - The gap is narrow (only call args of generic tuple struct ctors)
//! - All other cases (let binding, return expr, if/else, match, array) are closed
//! - Fixing via Option A/B/C requires architectural changes that touch multiple
//!   modules and may introduce regressions
//! - v0.3+ trait solver / GATs work will naturally require fn_sigs access
//!   in MIR lower, making Option A feasible at that time
//!
//! Per §17.7 Step 6 (缺陷纳入): documented as MVP with fix plan.

#![cfg(test)]

use landin_compiler::compile;

// ============================================================================
// Phase 2d gap: return expr — `fn f() -> Wrapper<i32> { Wrapper(true) }`
// ============================================================================

#[test]
fn test_phase_2d_return_expr_no_gap() {
    // Function returning Wrapper<i32> but body returns Wrapper(true).
    // typeck catches this via return type unify (sig.output vs dest_ty).
    let src = r#"
        struct Wrapper<T>(*mut T);
        fn make_wrapper() -> Wrapper<i32> {
            Wrapper(true)
        }
        fn main() -> i32 { 0 }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2d: typeck should catch return type mismatch"
    );
}

// ============================================================================
// Phase 2d gap: if branches — `if true { Holder(42) } else { Holder(true) }`
// ============================================================================

#[test]
fn test_phase_2d_if_branch_no_gap() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let h: Holder<i32> = if true { Holder(42) } else { Holder(true) };
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2d: typeck should catch if-branch type mismatch"
    );
}

// ============================================================================
// Phase 2d gap: match arms
// ============================================================================

#[test]
fn test_phase_2d_match_arm_no_gap() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let x = 1;
            let h: Holder<i32> = match x {
                1 => Holder(42),
                _ => Holder(true),
            };
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2d: typeck should catch match-arm type mismatch"
    );
}

// ============================================================================
// Phase 2e gap: array element — `[Holder(42), Holder(true)]`
// ============================================================================

#[test]
fn test_phase_2e_array_element_no_gap() {
    let src = r#"
        struct Holder<T>(T);
        fn main() -> i32 {
            let arr: [Holder<i32>; 2] = [Holder(42), Holder(true)];
            0
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2e: typeck should catch array element type mismatch"
    );
}

// ============================================================================
// Phase 2e FIXED (Stage 18.262): method/function call arg
// ============================================================================

#[test]
fn test_phase_2e_method_call_arg_now_errors() {
    // Stage 18.262 (Phase 2e): soundness hole CLOSED via fn_sigs
    // propagation into MIR lower.
    //
    // `take_holder(Holder(true))` where `fn take_holder(h: Holder<i32>)`
    // now errors because:
    // 1. Driver pre-builds fn_sig_table (lines 109-285 of compile_inner.rs)
    // 2. Passes fn_sigs as read-only data contract to MirLowerCtxt
    //    via `set_fn_sigs` (per §11.2 — allowed cross-stage access)
    // 3. `lower_call_expr` looks up callee's sig.inputs[i]
    // 4. Threads expected_ty into arg's `lower_expr_to_operand`
    // 5. Adt ctor path uses expected_ty to extract substs (Phase 2c)
    //
    // Per §1.0 原則 9 (正确 > 妥协): full soundness fix, not MVP.
    // Per §17.6: MVP marker converted to assert.
    let src = r#"
        struct Holder<T>(T);
        fn take_holder(h: Holder<i32>) -> i32 { 0 }
        fn main() -> i32 {
            take_holder(Holder(true))
        }
    "#;
    let result = compile(src);
    assert!(
        result.has_errors(),
        "Phase 2e must close soundness hole: take_holder(Holder(true)) should error"
    );
    assert!(!result.errors.typeck.is_empty(), "Expected typeck error");
    let msg = &result.errors.typeck[0].message;
    // Per §2 原則 3 (显式 > 隐式): declared field type (i32) is expected,
    // actual value type (bool) is found.
    assert!(
        msg.contains("expected i32") && msg.contains("found bool"),
        "Error message direction wrong: {msg}"
    );
}
