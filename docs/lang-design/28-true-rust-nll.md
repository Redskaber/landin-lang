# Stage 15.67 — True Rust NLL (Reject GAP-1 Compromise)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.192.0 → v0.193.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"
> **Supersedes**: `docs/lang-design/24-gap1-reconciliation.md` (Option B compromise — REJECTED)

## 1. Problem Statement

Stage 15.39 adopted **Option B** from the GAP-1 reconciliation doc — a
compromise that preserves lexical lifetimes (rejecting valid NLL programs)
to avoid fixing the `&mut self` method-call false positive. This violates
**§1.0 原則 9 "正确 > 妥协"** (added in v3.24).

The compromise:
- `kill_expired_borrows_dataflow` has an `ever_read` guard: if a borrow's
  `ref_local` was never read, the borrow is NOT killed (stays as a "stray"
  until scope end).
- This causes valid NLL programs like `let r1 = &mut x; let r2 = &mut x;`
  (r1 never read) to be REJECTED (r1's borrow lingers, conflicts with r2).

**The user's directive**: "正确 > 妥协，不能因为困难而选择妥协" — implement
real Rust NLL (Option A), not the Option B compromise.

## 2. Root Cause Analysis

### 2.1 The "ever_read" guard is the compromise

In `kill_expired_borrows_dataflow` (src/borrowck/mod.rs:333-354):
```rust
.filter(|local| {
    // GAP-1 preservation: never kill a borrow whose ref_local
    // was never read.
    if !ever_read.contains(local) {
        return false; // do NOT kill
    }
    // Kill if the ref_local's last read is at the current point.
    if let Some(last_use) = last_use_map.get(local) {
        *last_use == current_point
    } else {
        false
    }
})
```

The `ever_read` guard preserves GAP-1 by keeping never-read borrows alive.
This is lexical lifetimes, NOT NLL.

### 2.2 The `&mut self` false positive (the reason for the compromise)

The 1 DATAFLOW-STRICTER case (`e2e-runok-132-state-machine.lin`) has:
```rust
let mut m = Machine::new();
m.start();       // &mut self borrow on m (via temp)
m.tick();        // &mut self borrow on m (via temp)
```

The MIR lowering creates a temp for the `&mut self` argument:
```text
tmp = &mut m         (borrow created, ref_local = tmp)
call start(tmp)      (tmp read — last use)
tmp = &mut m         (tmp re-assigned — old borrow should be killed)
call tick(tmp)       (tmp read — last use)
```

The false positive occurs because the liveness-based kill doesn't kill the
old borrow before the new `tmp = &mut m` assignment. The `kill_borrows_on_redefinition`
(Stage 15.40) was added to fix this, but it may not cover all cases.

### 2.3 The real fix

For true NLL:
1. **Remove the `ever_read` guard** — kill borrows based on liveness, not
   on "was ever read". A never-read local is dead immediately after its
   definition, so its borrow should be killed immediately.
2. **Fix the `&mut self` false positive** — ensure `kill_borrows_on_redefinition`
   correctly kills the old borrow when a temp is re-assigned. This is the
   real fix, not the `ever_read` workaround.
3. **Flip the 79 GAP-1 conformance tests** from `compile_error` to `compile_ok`
   — they are valid NLL programs.

## 3. Implementation Plan

### 3.1 Remove the `ever_read` guard

In `kill_expired_borrows_dataflow`, remove the `ever_read` check. Kill a
borrow when its `ref_local` is not live after the current point (true NLL).

**Before** (Option B compromise):
```rust
.filter(|local| {
    if !ever_read.contains(local) {
        return false; // GAP-1: do NOT kill
    }
    if let Some(last_use) = last_use_map.get(local) {
        *last_use == current_point
    } else {
        false
    }
})
```

**After** (true NLL):
```rust
.filter(|local| {
    // True NLL: kill if the ref_local is not live after this point.
    // No "ever_read" guard — a never-read local is dead immediately,
    // so its borrow is killed immediately.
    !live_after.contains(local)
})
```

This requires switching from `last_use_map` to `live_after` (the fixpoint
liveness result). The `compute_live_after_point` helper already exists.

### 3.2 Fix the `&mut self` false positive

The `kill_borrows_on_redefinition` (Stage 15.40) should already handle the
`tmp = &mut m` re-assignment case. If the false positive persists after
removing the `ever_read` guard, investigate:

1. Is `kill_borrows_on_redefinition` called BEFORE the new borrow is created?
2. Does it correctly identify `tmp` as the LHS of the Assign?
3. Does it kill ALL borrows whose `ref_local` is `tmp`?

If needed, add explicit handling for the `&mut self` method-call pattern.

### 3.3 Flip the GAP-1 conformance tests

Change the 79 GAP-1 tests from `EXPECTED: compile_error` to `EXPECTED: compile_ok`.
These are valid NLL programs that should be accepted.

## 4. Verification

### 4.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 4.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ all PASS
- `python3 tests/conformance/run_all.py` — ✅ all PASS (including flipped GAP-1 tests)

## 5. §1.0 原則 9 "正确 > 妥协" Compliance

This stage implements the **correct** solution (true NLL) rather than the
**compromise** (Option B / ever_read guard). The compromise was adopted in
Stage 15.39 to avoid fixing the `&mut self` false positive. Per §1.0 原則 9,
this is unacceptable — the correct fix is to implement true NLL and fix the
false positive properly.

The GAP-1 reconciliation doc (Option B) is hereby **REJECTED** as a design
decision. It remains as a historical record of the compromise that was
considered and rejected.
