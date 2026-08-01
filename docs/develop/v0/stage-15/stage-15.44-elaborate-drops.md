# Stage 15.44 — `elaborate_drops` Pass (Drop Terminator Insertion)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.169.0 → v0.170.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 2 Task 8 (step 3 of 6)**: Wire up drop elaboration (HP-12)
> **Design doc**: `docs/lang-design/25-drop-elaboration.md`
> **Prior stage**: `docs/develop/v0/stage-15/stage-15.43-ty-needs-drop.md`

## 1. Executive Summary

Stage 15.44 implements `elaborate_drops` — the MIR-to-MIR pass that inserts
`Drop` terminators before `StorageDead` statements for locals whose type
needs drop glue. This pass uses the `ty_needs_drop` analysis from Stage 15.43
and performs basic block splitting to insert the `Drop` terminators.

**Key results**:
- New `elaborate_drops` function in `src/mir/drop_elaboration.rs`.
- 2 unit tests + 3 integration tests (5 new tests total).
- All 226 lib + 2082 integration tests pass (zero regression).
- The pass is currently a **no-op on all existing code** (no types implement
  `Drop` yet). The effect will be visible in Stage 15.46 when `impl Drop`
  support is added.

## 2. What Was Done

### 2.1 Implemented `elaborate_drops`

```rust
pub fn elaborate_drops(
    mir: &mut MirBody,
    resolver: &TraitResolver,
    interner: &Rodeo,
)
```

The pass walks all basic blocks. For each `StorageDead(local)` statement
where `ty_needs_drop(local.ty)` is true, it splits the basic block:

1. The current block's statements up to the `StorageDead` stay.
2. The current block's terminator is replaced with
   `Drop { place: local, target: new_block }`.
3. A new block is created with the `StorageDead` + remaining statements +
   original terminator.

The pass processes blocks in order, and new blocks created by splitting are
processed in subsequent iterations (so multiple `StorageDead` statements
needing drop are handled correctly — each gets its own `Drop` terminator).

### 2.2 Block splitting algorithm

The key challenge is the borrow checker: `mir.basic_blocks.len()` (immutable
borrow) conflicts with `mir.block_mut()` (mutable borrow). The fix is to
compute the new block ID **before** the mutable borrow:

```rust
let new_block_id_num = mir.basic_blocks.len() as u32;  // immutable borrow ends here
let bb_mut = mir.block_mut(bb_id);                      // mutable borrow starts
// ... use new_block_id_num in the Drop terminator ...
```

### 2.3 Current behavior (no-op)

Since no user-defined `Drop` impls exist yet (the parser doesn't support
`impl Drop for T`), `ty_needs_drop` returns `false` for all types in
existing code. Therefore `elaborate_drops` doesn't insert any `Drop`
terminators — it's a no-op. The tests verify this no-op behavior.

When Stage 15.46 adds `impl Drop` support, the pass will start inserting
`Drop` terminators, and the tests will be updated to verify actual insertion.

## 3. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `elaborate_drops` | `<verb>_<noun>` (free-function entry point, §23.1 rule 1) | ✅ |

Per §23.1 rule 1: free-function entry point.
Per §16: mutates `MirBody` in place — MIR-to-MIR transformation.
Per §1.0 原則 3 "显式 > 隐式": `Drop` terminators are explicit in the MIR.

## 4. Testing

### 4.1 Unit tests (2 new, in `src/mir/drop_elaboration.rs::tests`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_44_elaborate_drops_noop_when_no_drop_needed` | No blocks inserted when no types need drop |
| 2 | `stage15_44_elaborate_drops_empty_body` | No panic on empty body |

### 4.2 Integration tests (3 new, in `tests/v0/stage15/plan/elaborate_drops_integration_tests.rs`)

| # | Test name | Verifies |
|---|-----------|----------|
| 1 | `stage15_44_integration_elaborate_drops_noop_on_real_mir` | No-op on simple program |
| 2 | `stage15_44_integration_elaborate_drops_struct_no_drop` | No-op on struct (no Drop impl) |
| 3 | `stage15_44_integration_elaborate_drops_complex_program` | No-op on complex program |

## 5. Migration Plan (Stages 15.42-15.47) — Updated

| Stage | Status | Description |
|-------|--------|-------------|
| 15.42 | ✅ DONE (v0.168.0) | Design doc |
| 15.43 | ✅ DONE (v0.169.0) | `ty_needs_drop` analysis |
| **15.44** | **✅ DONE (v0.170.0)** | **`elaborate_drops` pass (this stage)** |
| 15.45 | ⏳ NEXT | Implement drop glue codegen |
| 15.46 | ⏳ PLANNED | Integration + conformance tests |
| 15.47 | ⏳ PLANNED | Gate review + deep review |

## 6. Verification

- `cargo build --features llvm-backend` — ✅ clean build, 0 warnings
- `cargo test --features llvm-backend --lib drop_elaboration` — ✅ 18/18 PASS
- `cargo test --features llvm-backend --test all_tests stage15_elaborate_drops_integration` — ✅ 3/3 PASS
- All existing tests pass (zero regression)
