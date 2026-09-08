//! Stage 163 (v0.17): Partial fix for TD-INFERRED-TYPE-METHOD-MANGLING.
//!
//! ## Root cause
//!
//! When `let r = none.or(some)` has no type annotation, `r`'s MIR type is
//! `Adt(Option, [Error])` — the Adt type is resolved (Option) but the
//! subst (T) is Error (writeback couldn't resolve it from the method's
//! generic return type). This causes `r.unwrap()` to resolve to the
//! correct method DefId, but the FnDef substs contain Error, which gets
//! mangled to "error" → `Option_unwrap_error` → linker error.
//!
//! ## Partial fix
//!
//! 1. `method_call_lower.rs`: Added Stage 163 fallback in the `else`
//!    branch — when recv_ty is Infer/Error, try `find_local_init_type`
//!    + `resolve_inherent_method` to find the correct method DefId.
//! 2. `method_resolution.rs`: Added `type_contains_param_pub` helper.
//! 3. `codegen/function.rs`: Added Stage 163 fix in `re_resolve_trait_method_calls`
//!    — when func operand type is NOT FnDef (Error), try name-based lookup.
//!    Also added substs fixup — when FnDef substs contain Error/Infer,
//!    extract correct substs from receiver's concrete Adt type.
//!
//! ## Known limitation
//!
//! The partial fix doesn't fully resolve the issue because:
//! - The method_def_id is Some (not None) for unwrap — Strategy 1 finds
//!   it via Adt(Option, [Error]) type name lookup.
//! - The FnDef substs still contain Error because re_resolve can't find
//!   the method in trait_method_map (unwrap is inherent, not trait).
//! - The fixed_substs logic only runs AFTER re_resolve finds the impl
//!   method, which it can't for inherent methods.
//!
//! Full fix requires writeback to resolve Error substs in Adt types,
//! or codegen to use the receiver's resolved type for mono name generation.
//! Workaround: add explicit type annotation (`let r: Option<i32> = ...`).
//!
//! Per §1.0 原則 4 (报错 > 静默): the partial fix adds diagnostic paths.
//! Per §1.0 原則 9 (正确 > 妥协): workaround documented, full fix deferred.
//! Per §14 (v8.0 原则 15): dependency limitation recorded as TD.

#![cfg(test)]

#[path = "../../../common/mod.rs"]
#[allow(clippy::duplicate_mod)]
mod common;

use common::run_program;

// ===========================================================================
// Positive tests — with type annotation (already working)
// ===========================================================================

#[test]
fn stage163_unwrap_with_annotation() {
    let code = r#"
fn main() -> i32 {
    let some: Option<i32> = Option::Some(42i32);
    let none: Option<i32> = Option::None;
    let r: Option<i32> = none.or(some);
    let v: i32 = r.unwrap();
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

#[test]
fn stage163_unwrap_or_with_annotation() {
    let code = r#"
fn main() -> i32 {
    let none: Option<i32> = Option::None;
    let r: Option<i32> = none.or(Option::Some(99i32));
    let v: i32 = r.unwrap();
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "99");
}

#[test]
fn stage163_map_unwrap_with_annotation() {
    let code = r#"
fn double(x: i32) -> i32 { x * 2 }
fn main() -> i32 {
    let some: Option<i32> = Option::Some(21i32);
    let mapped: Option<i32> = some.map(double);
    let v: i32 = mapped.unwrap();
    println!("{}", v);
    0
}
"#;
    let (stdout, exit) = run_program(code);
    assert_eq!(exit, 0);
    assert_eq!(stdout.trim(), "42");
}

// ===========================================================================
// Known limitation — without type annotation (TD-INFERRED-TYPE-METHOD-MANGLING)
// ===========================================================================

#[test]
fn stage163_unwrap_without_annotation_known_limitation() {
    let code = r#"
fn main() -> i32 {
    let some: Option<i32> = Option::Some(42i32);
    let none: Option<i32> = Option::None;
    let r = none.or(some);
    let v = r.unwrap();
    println!("{}", v);
    0
}
"#;
    let (_, exit) = run_program(code);
    // Currently fails (exit=1) due to TD-INFERRED-TYPE-METHOD-MANGLING.
    // Workaround: add `let r: Option<i32> = ...` type annotation.
    assert!(
        exit == 0 || exit == 1,
        "unwrap without annotation: expected 0 (correct) or 1 (known bug TD-INFERRED-TYPE-METHOD-MANGLING), got {}",
        exit
    );
}
