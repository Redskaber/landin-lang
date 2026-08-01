# Stage 15.46 — Drop Elaboration Integration (Wired into Driver Pipeline)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.171.0 → v0.172.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 8 (step 5 of 6)**: Wire up drop elaboration (HP-12)
> **Design doc**: `docs/lang-design/25-drop-elaboration.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.45-drop-glue-codegen.md`

## 1. Executive Summary

Stage 15.46 wires `elaborate_drops` into the driver pipeline. The pass
now runs AFTER typeck (which writes resolved types into MIR) and BEFORE
borrowck (so the borrow checker sees the `Drop` terminators).

**Key results**:
- `elaborate_drops` is called in `src/driver.rs` between typeck and borrowck.
- 3 integration tests verify the pipeline works correctly.
- All 226 lib + 2085 integration + 5216 conformance tests pass (zero regression).
- The pass is currently a no-op (no types implement `Drop` yet).

## 2. What Was Done

### 2.1 Wired `elaborate_drops` into `src/driver.rs`

Added the `elaborate_drops` call between typeck and borrowck:

```rust
// After typeck:
tc.check_mir_body_with_tables(&mut mir, Some(&field_ty_table));
// ...

// Stage 15.46: Drop elaboration (insert Drop terminators).
crate::mir::drop_elaboration::elaborate_drops(
    &mut mir,
    &trait_resolver,
    &interner,
);

// Before borrowck:
let mut bc = borrowck::BorrowChecker::new();
bc.check_mir_body_with_dataflow(&mir);
```

### 2.2 Updated driver pipeline documentation

Updated the pipeline diagram in `src/driver.rs` to include the new stage:

```
6. typeck::check_mir_body    → mutates MIR (writes resolved types) + type errors
6.5. mir::drop_elaboration::elaborate_drops  → insert Drop terminators (Stage 15.46)
7. borrowck::check_mir_body_with_dataflow  → borrow/move errors
```

## 3. API Naming Compliance (§23)

No new public API symbols — `elaborate_drops` was already implemented in
Stage 15.44. This stage just wires it into the driver.

Per §16: `elaborate_drops` is called with `&mut mir` (MIR-to-MIR
transformation), `&trait_resolver` (read-only), and `&interner`
(read-only). No HIR lookup.

## 4. Testing

### 4.1 New integration tests (3)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_46_driver_pipeline_runs_elaborate_drops` | Simple program compiles with elaborate_drops in pipeline |
| 2 | `stage15_46_struct_no_drop_compiles` | Struct without Drop compiles cleanly |
| 3 | `stage15_46_complex_program_compiles` | Complex program (loops, method calls) compiles cleanly |

## 5. Migration Plan (Stages 15.42-15.47) — Updated

| Stage | Status | Description |
|-------|--------|-------------|
| 15.42 | ✅ DONE (v0.168.0) | Design doc |
| 15.43 | ✅ DONE (v0.169.0) | `ty_needs_drop` analysis |
| 15.44 | ✅ DONE (v0.170.0) | `elaborate_drops` pass |
| 15.45 | ✅ DONE (v0.171.0) | Drop glue codegen |
| **15.46** | **✅ DONE (v0.172.0)** | **Integration: wired into driver pipeline (this stage)** |
| 15.47 | ⏳ NEXT | Gate review + deep review |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests stage15_drop_elaboration_integration` — ✅ 3/3 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
