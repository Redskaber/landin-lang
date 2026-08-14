# Stage 18.98 — Adt Substs Soundness Fix (types_match_loose)

> **Author**: redskaber
> **Date**: 2026-08-11
> **Version**: v0.365.0 → v0.366.0
> **Status**: Active

## 1. 设计文档对齐（§13.1）

### 1.1 对应设计文档

- `docs/develop/v0/v0.1-capability-boundaries.md` §2 Type System — "Param unify unsound"
- `docs/develop/v0/task-11-monomorphization-design.md` — monomorphization infra (complete)
- `docs/stage-committee-process.md` §2.0 原则 9 "正确 > 妥协" — soundness is a correctness property

### 1.2 设计意图摘要

v0.1 capability boundaries documents the "Param unify unsound" limitation:
generic type params unify with any type, so `Vec<i32> = Vec<bool>` is accepted.
The v0.2 P0 roadmap lists "Monomorphization" as the fix — but investigation
reveals the monomorphization infrastructure is already complete (Task 11
Phases 1-4c, Stage 16.59). The actual soundness hole is narrower and more
fixable than the roadmap suggested.

### 1.3 Root Cause Analysis

**Symptom**: `let v3: Vec<i32> = v2;` (where `v2: Vec<bool>`) compiles without error.

**MIR state** (verified via debug output):
- `v2` local decl: `Adt(DefId(0), [Bool])` ✅ correct
- `v3` local decl: `Adt(DefId(0), [Int(I32)])` ✅ correct
- Assignment `v3 = v2` should fail because substs differ ([Bool] vs [I32])

**Bug location**: `src/typeck/checker.rs:1545` in `types_match_loose`:
```rust
// Adt with same DefId (generic substs may differ in representation)
(TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) if a_def == b_def => true,
```

This loose match accepts ANY two Adts with the same DefId, **ignoring substs
entirely**. The comment says "generic substs may differ in representation" —
but this was a workaround for the pre-Stage-16.52 era when substs weren't
propagated. Now that substs ARE propagated (Task 11 Phase 1c complete), this
loose match is the soundness hole.

**Why post_check_statement uses types_match_loose**: Stage 18.71 added
`post_check_statement` as a "Phase 5.5" post-pass to catch mismatches that
Phase 1 (MIR lower) missed. To avoid false positives on generic/unresolved
types, it uses `types_match_loose` as a "should I suppress this error?" check.
The Adt case in `types_match_loose` was too permissive.

### 1.4 已实现 / 偏差 / 未实现

| Item | Status |
|------|--------|
| Monomorphization infra (collect_mono_items, MonoLayoutMap) | ✅ Stage 16.59 |
| Substs propagation into TyKind::Adt | ✅ Stage 16.52 |
| Substs propagation into AggregateKind::Adt | ✅ Stage 16.52 |
| Adt unify in unify.rs (checks substs when both non-empty) | ✅ Stage 16.52 |
| **types_match_loose Adt case (ignores substs)** | ❌ **THIS STAGE FIXES** |
| GAT Phase 4 (monomorphization of associated types) | ⏳ Deferred (P1) |

## 2. 任务拆分（MUV）

| ID | Task | Acceptance |
|----|------|------------|
| 18.98.1 | Fix `types_match_loose` Adt case | Recursive substs comparison, empty-substs still loose-match (inference case) |
| 18.98.2 | Add positive test: `Vec<i32> = Vec<bool>` now rejected | Compile error with type mismatch |
| 18.98.3 | Add negative test: `Vec<i32> = Vec<i32>` still accepted | Compiles OK |
| 18.98.4 | Add negative test: empty-substs inference still works | `let x: Vec<i32> = Vec::new();` compiles |
| 18.98.5 | Run full test suite, fix any regressions | 0 regressions |

## 3. Fix Design

### 3.1 Current (Buggy) Code

```rust
// Adt with same DefId (generic substs may differ in representation)
(TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) if a_def == b_def => true,
```

### 3.2 Fixed Code

```rust
// Stage 18.98: Adt with same DefId — check substs recursively.
// Per §2.0 原则 9 "正确 > 妥协": Vec<i32> != Vec<bool> (soundness).
// Empty substs (inference case) still loose-match — they represent
// "unknown, to be inferred" and unify with anything per unify.rs.
(TyKind::Adt(a_def, a_substs), TyKind::Adt(b_def, b_substs)) => {
    if a_def != b_def {
        return false;
    }
    // Empty substs = inference case (unknown instantiation) → loose match
    if a_substs.is_empty() || b_substs.is_empty() {
        return true;
    }
    // Both have substs — must match in length and element-wise (loose)
    if a_substs.len() != b_substs.len() {
        return false;
    }
    a_substs.iter().zip(b_substs.iter()).all(|(a, b)| types_match_loose(a, b))
}
```

### 3.3 API Naming Compliance (§10)

No new API — fix is internal to `types_match_loose`. The function name
already follows §23 convention (it's a predicate, not a public API entry).

### 3.4 Interface Isolation (§11)

No cross-stage changes — fix is within typeck module. The fix makes typeck
correctly reject unsound assignments, which is its responsibility per §11.

## 4. Test Strategy (§9)

### 4.1 New Tests

```rust
// Stage 18.98 positive: Vec<i32> = Vec<bool> must be rejected (soundness)
#[test]
fn stage18_98_adt_substs_mismatch_rejected() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<bool> = Vec { data: true, len: 1 };
    let v3: Vec<i32> = v2;  // ERROR: type mismatch
}
"#;
    let result = compile(src);
    assert!(result.has_errors(), "Vec<i32> = Vec<bool> must be rejected");
}

// Stage 18.98 negative 1: Vec<i32> = Vec<i32> still accepted
#[test]
fn stage18_98_adt_substs_match_accepted() {
    let src = r#"
struct Vec<T> { data: T, len: i32 }
fn main() {
    let v1: Vec<i32> = Vec { data: 42, len: 1 };
    let v2: Vec<i32> = v1;  // OK: same substs
}
"#;
    let result = compile(src);
    assert!(!result.has_errors(), "Vec<i32> = Vec<i32> should be accepted");
}

// Stage 18.98 negative 2: empty substs inference still works
#[test]
fn stage18_98_adt_empty_substs_inference() {
    let src = r#"
struct Wrapper<T> { inner: T }
fn make<T>(x: T) -> Wrapper<T> { Wrapper { inner: x } }
fn main() {
    let w: Wrapper<i32> = make(42);  // OK: inference fills substs
}
"#;
    let result = compile(src);
    assert!(!result.has_errors(), "empty-substs inference should work");
}
```

### 4.2 Per §9.4.3: 1 positive + 2 negative (1:2 ratio)

## 5. Risk & Rollback

### 5.1 Risk

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| False positives on valid generic code | Medium | High | Empty-substs fallback preserves inference; full test suite catches regressions |
| Existing tests relied on unsound behavior | Low | Medium | Search for `Vec<` in tests, verify they use same-type assignments |

### 5.2 Rollback

Single-line revert: restore `if a_def == b_def => true` (the old loose match).

## 6. 验收标准（§3.2）

- [ ] `cargo build --features llvm-backend` 成功
- [ ] `cargo fmt --check` exit 0
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` 0 warnings
- [ ] `cargo test --features llvm-backend --lib` 全绿
- [ ] `cargo test --features llvm-backend --tests` (skip runtime) 全绿
- [ ] New soundness test passes (Vec<i32> = Vec<bool> rejected)
- [ ] No regressions in existing tests
