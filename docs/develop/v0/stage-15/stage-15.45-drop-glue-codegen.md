# Stage 15.45 — Drop Glue Codegen (Non-Noop `TerminatorKind::Drop`)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.170.0 → v0.171.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 8 (step 4 of 6)**: Wire up drop elaboration (HP-12)
> **Design doc**: `docs/lang-design/25-drop-elaboration.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.44-elaborate-drops.md`

## 1. Executive Summary

Stage 15.45 makes `TerminatorKind::Drop` codegen non-noop. Previously
(Stage 14.103), the `Drop` terminator was a no-op (just branched to
target without calling any destructor). Now it computes the place's
address, determines the drop glue function name, and emits a call to
`drop_adt_<DefId>` (for ADT types) or `drop_generic` (for other types).

**Key results**:
- `TerminatorKind::Drop` codegen now emits a `call void @drop_adt_<N>(...)`.
- All 226 lib + 5216 conformance tests pass (zero regression).
- The code path is not yet exercised by existing tests (no `Drop`
  terminators are generated because `elaborate_drops` is a no-op until
  `impl Drop` support is added in Stage 15.46).

## 2. What Was Done

### 2.1 Modified `TerminatorKind::Drop` codegen (`src/codegen/terminator.rs`)

**Before** (Stage 14.103 — no-op):
```rust
TerminatorKind::Drop { place, target, .. } => {
    let _ = place; // v0.1: no Drop impls exist, so nothing to call
    emitter.emit_br(&format!("bb{}", target.0));
}
```

**After** (Stage 15.45):
```rust
TerminatorKind::Drop { place, target, .. } => {
    // 1. Compute the place's address.
    let place_addr = compute_place_address(emitter, mir, place, interner, layouts);
    // 2. Get the place's LLVM type.
    let place_ty = detect_place_type(mir, place, layouts);
    // 3. Determine the drop glue function name.
    let drop_fn_name = match &mir_ty {
        Some(TyKind::Adt(def_id, _)) => format!("drop_adt_{}", def_id.0),
        _ => "drop_generic".to_string(),
    };
    // 4. Call the drop glue function.
    emitter.emit_call(&drop_fn_name, &[(place_ty, &place_addr)], &EmitType::Void);
    // 5. Branch to target.
    emitter.emit_br(&format!("bb{}", target.0));
}
```

### 2.2 Drop glue function naming

The drop glue function name uses the ADT's DefId:
- ADT types: `drop_adt_<DefId>` (e.g., `drop_adt_3` for `DefId(3)`).
- Other types (tuples, arrays, etc.): `drop_generic`.

This is a simple, unambiguous naming scheme. The actual drop glue
function emission (`emit_drop_glue`) will be implemented in Stage 15.46.

## 3. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `drop_adt_<N>` | `drop_<noun>_<id>` (drop glue function name) | ✅ |
| `drop_generic` | `drop_<noun>` (fallback drop glue function name) | ✅ |

Per §23: function name follows `drop_<noun>` pattern.
Per §1.0 原則 3 "显式 > 隐式": the drop call is explicit.

## 4. Testing

No new tests were added in this stage because the code path is not
exercised (no `Drop` terminators are generated). When Stage 15.46 adds
`impl Drop` support, tests will verify the drop call is emitted.

All existing tests pass (zero regression) — the codegen change doesn't
affect any existing code path.

## 5. Migration Plan (Stages 15.42-15.47) — Updated

| Stage | Status | Description |
|-------|--------|-------------|
| 15.42 | ✅ DONE (v0.168.0) | Design doc |
| 15.43 | ✅ DONE (v0.169.0) | `ty_needs_drop` analysis |
| 15.44 | ✅ DONE (v0.170.0) | `elaborate_drops` pass |
| **15.45** | **✅ DONE (v0.171.0)** | **Drop glue codegen (this stage)** |
| 15.46 | ⏳ NEXT | Integration + `impl Drop` support + conformance tests |
| 15.47 | ⏳ PLANNED | Gate review + deep review |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo fmt --check` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
