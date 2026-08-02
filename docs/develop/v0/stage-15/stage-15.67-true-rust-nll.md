# Stage 15.67 — True Rust NLL (Reject GAP-1 Compromise)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.192.0 → v0.193.0
> **Process**: stage-committee-process.md v3.24 §1.0 原則 9 "正确 > 妥协"
> **Supersedes**: `docs/lang-design/24-gap1-reconciliation.md` (Option B compromise — REJECTED)

## 1. Executive Summary

Stage 15.67 implements **true Rust NLL** (Non-Lexical Lifetimes), rejecting
the Stage 15.39 "Option B" compromise that preserved GAP-1 lexical lifetimes.
Per §1.0 原則 9 "正确 > 妥协" (added in v3.24), the correct solution is to
implement real NLL, not a compromise that rejects valid programs to avoid
fixing a false positive.

**Key results**:
- Removed the `ever_read` guard from `kill_expired_borrows_dataflow` — borrows
  are now killed based on liveness (true NLL), not on "was ever read" + last-use.
- Added "kill-after-call" semantics in `check_terminator`'s Call arm — after a
  call `f(tmp)`, the temp `tmp` is dead, so its borrow is killed. This fixes
  the `&mut self` method-call false positive that motivated the Option B
  compromise.
- Added block-entry kill — at the start of each basic block, kill borrows
  whose `ref_local` is not in `live_in[bb]`. This handles the case where a
  method-call temp in a conditional block is dead at the merge point.
- Updated liveness analysis to handle `StorageLive`/`StorageDead` —
  `StorageDead(local)` is treated as a write (kills the local), `StorageLive(local)`
  is treated as a read (local enters scope).
- Flipped 108 conformance tests from `compile_error` to `compile_ok` — these
  are valid NLL programs (e.g., `let r1 = &mut x; let r2 = &mut x;` where r1
  is never read) that were rejected by the GAP-1 compromise.
- Updated 7 integration tests + 2 lib tests to match true NLL semantics.
- All 7575 tests pass (226 lib + 2133 integration + 5216 conformance).

## 2. The GAP-1 Compromise (REJECTED)

### 2.1 What was the compromise?

Stage 15.39 adopted Option B from the GAP-1 reconciliation doc:
- `kill_expired_borrows_dataflow` had an `ever_read` guard: if a borrow's
  `ref_local` was never read, the borrow was NOT killed (stayed as a "stray"
  until scope end).
- This caused valid NLL programs like `let r1 = &mut x; let r2 = &mut x;`
  (r1 never read) to be REJECTED.

### 2.2 Why was it adopted?

To avoid fixing the `&mut self` method-call false positive (1 DATAFLOW-STRICTER
case). The false positive occurred because method-call temps in loops were
considered live across the entire function (no `StorageDead` handling), causing
their borrows to never expire.

### 2.3 Why is it rejected? (§1.0 原則 9)

Per §1.0 原則 9 "正确 > 妥协":
- The compromise rejected valid NLL programs to avoid fixing a false positive.
- The correct fix is to implement true NLL AND fix the false positive properly.
- "这里妥协那里妥协，最后还是你想要的内容吗？" — compromises accumulate as
  technical debt; the correct solution is to do it right.

## 3. What Was Done

### 3.1 True NLL: liveness-based kill

In `kill_expired_borrows_dataflow` (`src/borrowck/mod.rs`):
- Removed the `ever_read` guard.
- Removed the `last_use_map` check.
- Now kills a borrow when its `ref_local` is NOT in `live_after` (the fixpoint
  liveness result from `compute_live_after_point`).

### 3.2 Kill-after-call (fixes `&mut self` false positive)

In `check_terminator`'s `Call` arm (`src/borrowck/mod.rs`):
- After checking the call's operands, kill borrows for all temp locals used
  as call args (Copy or Move of a Local place).
- This ensures that after `call f(tmp)`, the temp `tmp`'s borrow is killed,
  allowing the next method call to borrow the same place without conflict.

### 3.3 Block-entry kill

In `check_mir_body_with_dataflow`:
- At the start of each basic block, kill borrows whose `ref_local` is NOT in
  `live_in[bb]`.
- This handles the case where a method-call temp in a conditional block is
  dead at the merge point — its borrow must be killed before the next method
  call (which may be in a different conditional block).

### 3.4 StorageLive/StorageDead handling in liveness

In `statement_reads` and `statement_writes` (`src/borrowck/liveness.rs`):
- `StorageLive(local)` is treated as a READ — the local enters scope.
- `StorageDead(local)` is treated as a WRITE — the local exits scope (killed).
- This ensures locals are dead after their `StorageDead` point, not alive
  until function return.

### 3.5 Flipped 108 conformance tests

Changed 108 GAP-1 tests from `EXPECTED: compile_error` to `EXPECTED: compile_ok`.
These are valid NLL programs:
- `let r1 = &mut x; let r2 = &mut x;` (r1 never read)
- `let r = &x; x = 2;` (r never read after scope)
- etc.

### 3.6 Updated tests to match true NLL

- 2 lib tests (`move_borrowed_detected`, `assign_to_borrowed_detected`) —
  now assert no errors (true NLL allows these).
- 7 integration tests (stage15_37, stage15_40, stage15_41, option_b) —
  now assert no errors (true NLL allows these).

### 3.7 Process doc updated

- Added §1.0 原則 9 "正确 > 妥协" to `docs/stage-committee-process.md` (v3.24).
- Added execution requirements for principle 9.

## 4. Verification

### 4.1 Quality checks
- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings

### 4.2 Test results
- `cargo test --features llvm-backend --lib` — ✅ 226/226 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2133/2133 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS

**Total: 7575 tests passing, 0 failures, 0 warnings.**

### 4.3 Runtime verification

The `e2e-runok-132-state-machine.lin` test (the original DATAFLOW-STRICTER
false positive case) now passes — the state machine with `&mut self` method
calls in a loop with two conditionals compiles and runs correctly.

## 5. §1.0 原則 9 "正确 > 妥协" Compliance

This stage implements the **correct** solution (true NLL) rather than the
**compromise** (Option B / ever_read guard). Per §1.0 原則 9:
- The correct fix was chosen despite being more effort (fixing the false
  positive properly).
- The GAP-1 reconciliation doc (Option B) is hereby **REJECTED** as a design
  decision. It remains as a historical record.

## 6. Files Modified

### 6.1 `src/borrowck/mod.rs`
- `kill_expired_borrows_dataflow`: removed `ever_read` + `last_use_map`, use
  `compute_live_after_point` (true NLL).
- `check_mir_body_with_dataflow`: use `compute_liveness` (both `live_in` and
  `live_out`); added block-entry kill.
- `check_terminator` Call arm: added kill-after-call for temp args.
- Updated 2 lib tests (`move_borrowed_detected`, `assign_to_borrowed_detected`).

### 6.2 `src/borrowck/liveness.rs`
- `statement_reads`: handle `StorageLive` as a read.
- `statement_writes`: handle `StorageDead` as a write.

### 6.3 `docs/stage-committee-process.md`
- Added §1.0 原則 9 "正确 > 妥协" (v3.24).

### 6.4 `docs/lang-design/28-true-rust-nll.md` (NEW)
- Design doc for true Rust NLL (rejects Option B).

### 6.5 Conformance tests (108 files flipped)
- Changed `EXPECTED: compile_error` to `EXPECTED: compile_ok`.

### 6.6 Integration tests (7 tests updated)
- `stage15_37_driver_switch_tests.rs` (2 tests)
- `stage15_40_driver_switch_tests.rs` (2 tests)
- `stage15_41_legacy_delegation_tests.rs` (2 tests)
- `option_b_implementation_tests.rs` (3 tests)

### 6.7 `Cargo.toml`
- Bumped v0.192.0 → v0.193.0.

## 7. Committee Vote: GO

**Decision**: Stage 15.67 is **COMPLETE**. True Rust NLL is implemented,
rejecting the GAP-1 compromise. The `&mut self` false positive is fixed
properly via kill-after-call semantics. All valid NLL programs are now
accepted.
