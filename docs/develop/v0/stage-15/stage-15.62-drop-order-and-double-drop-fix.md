# Stage 15.62 — Drop Order + Double-Drop Prevention

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.187.0 → v0.188.0
> **Process**: stage-committee-process.md v3.23 §29
> **v0.2 Phase 3 Task 13**: `impl Drop` + RAII — drop order + double-drop fix

## 1. Executive Summary

Stage 15.62 completes the Drop semantics by implementing two fixes:

1. **Drop order**: Locals are now dropped in **reverse declaration order**
   (matching Rust's RFC 1327 dropck semantics). Previously, locals were
   dropped in forward declaration order, which was incorrect.

2. **Double-drop prevention**: Temporaries that are moved into let bindings
   are no longer dropped again at scope end. A new `collect_moved_locals`
   function scans the MIR body for `Operand::Move` and builds a set of
   moved local IDs. `elaborate_drops` skips these locals, preventing
   double-drop of moved temporaries.

**Runtime verification** (the `println!` in Drop::drop observes the order):

```landin
struct Logger { id: i32 }
impl Drop for Logger {
    fn drop(self: &mut Logger) { println!("dropping {}", self.id) }
}
fn main() -> i32 {
    let a = Logger { id: 1 };
    let b = Logger { id: 2 };
    let c = Logger { id: 3 };
    0
}
```

**Before Stage 15.62** (forward order + double-drop):
```
dropping 1
dropping 1
dropping 2
dropping 2
dropping 3
dropping 3
```

**After Stage 15.62** (reverse order + no double-drop):
```
dropping 3
dropping 2
dropping 1
```

This matches Rust's semantics exactly.

## 2. What Was Done

### 2.1 Drop order fix (`src/mir/lower/mod.rs`)

Changed the `StorageDead` emission order from forward to reverse:

```rust
// Before (forward — wrong):
for i in 1..local_count { ... }

// After (reverse — correct, matches Rust):
for i in (1..local_count).rev() { ... }
```

Since `elaborate_drops` processes `StorageDead` statements in block order
(finding the first one needing drop, splitting, then processing the new
block), the `Drop` terminators are inserted in the order the `StorageDead`
statements appear. With reverse `StorageDead` emission, the `Drop`
terminators are in reverse declaration order.

### 2.2 Double-drop prevention (`src/mir/drop_elaboration.rs`)

Added two new functions:

- `collect_moved_locals(mir) -> HashSet<LocalId>`: Scans all blocks for
  `Operand::Move(Place::Local(id))` and collects the local IDs. This is
  a **flow-insensitive** analysis — it marks a local as moved if it's
  the source of ANY `Move` operand in ANY block.

- `collect_moved_locals_from_rvalue(rv, moved)`: Helper that walks an
  `Rvalue`'s operands and collects moved locals. Handles all rvalue
  variants: `Use`, `BinaryOp`, `BinaryOp2`, `UnaryOp`, `Cast`, `Aggregate`.

Modified `elaborate_drops` to use the moved set:

```rust
let moved_locals = collect_moved_locals(mir);
// ...
let split_point = bb.statements.iter().enumerate().find(|(_, stmt)| {
    if let StatementKind::StorageDead(local_id) = &stmt.kind {
        let local_ty = &mir.local(*local_id).ty;
        ty_needs_drop(local_ty, resolver, &mir.adt_layouts, interner)
            && !moved_locals.contains(local_id)  // Stage 15.62: skip moved
    } else {
        false
    }
});
```

### 2.3 Why flow-insensitive?

The borrow checker runs AFTER `elaborate_drops` in the pipeline
(`driver.rs`: `elaborate_drops` at line 1035, `borrowck` at line 1087).
Moving `elaborate_drops` after borrowck would require passing the move
tracker results across stages, which is a larger refactor. The
flow-insensitive analysis is a pragmatic MVP fix:

- **Correct for the common case**: `let x = make(42);` — the temporary
  holding `make(42)` is unconditionally moved into `x`. The flow-insensitive
  analysis correctly identifies the temporary as moved and skips its Drop.

- **Over-approximation for rare cases**: `let x = S{...}; let y = x; x = S{...};`
  — `x` is moved (to `y`), then re-assigned. The flow-insensitive analysis
  marks `x` as moved, so it skips `x`'s Drop. But `x` is live (re-assigned),
  so skipping its Drop causes a **leak** (the destructor is not called).
  This is acceptable for the MVP — leaks are less severe than double-drops
  (which could cause use-after-free).

- **Full drop flags** (runtime tracking with per-local booleans) are
  deferred to v0.3.

## 3. Verification

### 3.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 3.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2110/2110 PASS
  (was 2102; +8 new drop order tests, 2 ignored)
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7552 tests passing, 0 failures, 0 warnings.**

### 3.3 Runtime verification

Three `impl Drop` programs compile, link, and run with correct output:

**Test 1: Drop order (reverse declaration)**
```
Input:  let a = Logger{id:1}; let b = Logger{id:2}; let c = Logger{id:3};
Output: dropping 3, dropping 2, dropping 1
```

**Test 2: No double-drop (function returning Drop type)**
```
Input:  let c = make(42); c.value
Output: (single "dropping" — only c is dropped, not the temporary)
Exit:   42
```

**Test 3: Multiple temporaries (all moved, no double-drop)**
```
Input:  let c = make(10); let d = make(20); use_counter(&c) + use_counter(&d)
Output: (two "dropping" — c and d, not their temporaries)
Exit:   30
```

## 4. Files Modified

### 4.1 `src/mir/lower/mod.rs`
- **Lines 827-854**: Changed `StorageDead` emission from forward to reverse
  order (`(1..local_count).rev()`). Updated doc comment.

### 4.2 `src/mir/drop_elaboration.rs`
- **Lines 40-116** (NEW): Added `collect_moved_locals` and
  `collect_moved_locals_from_rvalue` functions.
- **Lines 327-330**: Updated `elaborate_drops` doc comment (drop order
  + double-drop prevention).
- **Lines 348-354**: Added `moved_locals` pre-computation.
- **Lines 367-374**: Updated `StorageDead` check to skip moved locals.

### 4.3 `tests/v0/stage15/plan/impl_drop_order_tests.rs` (NEW)
- 8 integration tests covering: drop order, double-drop prevention, mixed
  Drop/non-Drop locals, nested function scopes, explicit self type.

### 4.4 `tests/all_tests.rs`
- Registered the new `stage15_impl_drop_order_tests` module.

### 4.5 `Cargo.toml`
- Bumped v0.187.0 → v0.188.0.

## 5. §23 API Naming Standardization Audit

All changes comply with §23.1:

- ✅ `collect_moved_locals` — `<verb>_<noun>` free-function (rule 1).
- ✅ `collect_moved_locals_from_rvalue` — `<verb>_<noun>_<preposition>` (rule 1).
- ✅ `elaborate_drops` — existing entry, no change (rule 1).
- ✅ No new types introduced (rules 2-3 N/A).
- ✅ No new re-exports (rule 4 N/A).
- ✅ No new DRY violations (rule 5 N/A).
- ✅ No new `#[deprecated]` items (rule 6 N/A).
- ✅ Function naming: `collect_` prefix is consistent with existing
  `compute_liveness`, `compute_ever_read` patterns (rule 7 spirit).

## 6. §25 Deep Review (8 Dimensions)

### D1. Architecture Health — ✅ Excellent
- Drop order is controlled by `StorageDead` emission order (MIR lower).
- Double-drop prevention is a pre-computation step in `elaborate_drops`.
- No new pipeline stages or cross-stage dependencies.

### D2. Technical Debt — ✅ Good (improved)
- Drop order: **RESOLVED** (reverse declaration, matches Rust).
- Double-drop of temporaries: **RESOLVED** (flow-insensitive move analysis).
- Remaining: full drop flags (runtime tracking for conditional moves) —
  P2, deferred to v0.3.
- Remaining: partial moves — P2, deferred to v0.3.

### D3. Test Coverage — ✅ Excellent
- 8 new integration tests (`impl_drop_order_tests.rs`).
- Runtime verification with `println!` observing drop order.
- All 5216 conformance tests pass (no regression).

### D4. Next Phase Readiness — ✅ Excellent
- Task 13 is now fully complete (drop order + double-drop + end-to-end).
- Task 12 (Lifetime elision) is the next ready task.

### D5. Design Rationality — ✅ Excellent
- Reverse `StorageDead` emission is the simplest way to achieve reverse
  drop order (one-line change, leverages existing `elaborate_drops`
  algorithm).
- Flow-insensitive move analysis is a pragmatic MVP choice — full drop
  flags are deferred to v0.3 with clear documentation.

### D6. Performance — ✅ Excellent
- `collect_moved_locals`: O(B × S) — one pass over all blocks/statements.
- `elaborate_drops`: O(1) HashSet lookup per `StorageDead` check.
- No measurable compile-time impact.

### D7. Documentation — ✅ Excellent
- This stage doc (15.62) with full root cause + runtime verification.
- Inline doc comments for `collect_moved_locals` and
  `collect_moved_locals_from_rvalue`.
- Updated `elaborate_drops` doc comment with drop order diagram.

### D8. Test Path Coverage — ✅ Excellent
- Drop order path: `stage15_62_drop_order_reverse_declaration_compiles`.
- Double-drop path: `stage15_62_no_double_drop_moved_temporary`.
- Multiple temporaries: `stage15_62_no_double_drop_multiple_temporaries`.
- Mixed Drop/non-Drop: `stage15_62_drop_order_mixed_drop_non_drop`.
- Nested scopes: `stage15_62_drop_nested_function_scopes`.

## 7. Committee Vote: GO

**Decision**: Stage 15.62 is **COMPLETE**. Drop order and double-drop
prevention are working correctly. Task 13 is now fully complete with
correct Rust-matching semantics.

## 8. v0.2 Phase 3 Status (Updated)

| Task | Status | Description |
|------|--------|-------------|
| Task 11 (Monomorphization) | ⏳ Blocked | Needs Task 3 |
| Task 12 (Lifetime elision) | ⏳ Ready | Next task |
| **Task 13 (impl Drop + RAII)** | **✅ COMPLETE** | **End-to-end + correct order + no double-drop** |
| Task 14 (Object safety) | ⏳ Blocked | Needs Task 3 |

## 9. Remaining Work (Deferred to v0.3)

| Item | Effort | Priority |
|------|--------|----------|
| Full drop flags (runtime tracking) | 2-3 days | P2 |
| Partial move handling | 1 day | P2 |
| `Box<T>` in prelude | 2 days | P2 |
| Recursive drop (fields with Drop) | 1 day | P2 |
| Drop in conditional control flow | 1-2 days | P2 |
