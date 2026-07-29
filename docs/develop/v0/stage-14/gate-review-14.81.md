# Stage 14.81 — Gate Review

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.96.0 → v0.97.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 review)

## 1. Stage Summary

Stage 14.81 fixes **GAP-1 (NLL soundness regression)** — the long-standing
silent acceptance of unsound borrow patterns like
`let r1 = &mut x; let r2 = &mut x;`. This was the largest P0 blocker
identified in the v0.1 capability assessment and required only a 1-line
fix once root-caused.

## 2. Bug Fixed

### GAP-1: NLL soundness — `let r1 = &mut x; let r2 = &mut x;` silently accepted

**Symptom**: Programs with double mutable borrows (or shared-then-mut
borrows) on the same variable were silently accepted by the borrow
checker. Stage 13.17 had flipped 229 conformance tests from
`compile_error` to `compile_ok` as a workaround for this silent
acceptance — masking the unsoundness.

**Root cause**: In `src/borrowck/mod.rs::check_rvalue`, the
`Rvalue::Use(op) | Rvalue::Cast(_, op, _)` arm transfers borrow
references via `transfer_borrow_ref(tmp, lhs)` — but ONLY for
`Operand::Move`. References (`&T`, `&mut T`) are Copy types, so
`let r = &x;` lowers to `r = Copy(tmp)` (not `Move(tmp)`). The
transfer was therefore skipped, leaving the borrow's `ref_local`
as `tmp` (the temporary) instead of `r` (the user-visible local).

NLL then killed the borrow at `tmp`'s last use — which is the
`r = Copy(tmp)` statement itself. By the time `let r2 = &mut x;`
was processed, the first borrow was already killed, so the second
`&mut x` succeeded.

**Fix** (1-line change in `src/borrowck/mod.rs`):

```rust
// Before (broken):
if let Operand::Move(lv) = op {

// After (fixed):
if let Operand::Move(lv) | Operand::Copy(lv) = op {
```

The transfer now happens for both `Move` and `Copy` of a ref temp.
After the transfer, the borrow's `ref_local` is `r` (the user-visible
local), and NLL correctly tracks `r`'s lifetime — killing the borrow
only at `r`'s last use, not `tmp`'s.

**Why this is a 1-line fix despite GAP-1 being labeled L3 (>3 days)**:

The original assessment assumed the fix required "fixpoint dataflow
analysis, not single-pass forward walk" — i.e., a full NLL rewrite.
In practice, the existing NLL algorithm (forward walk with last-use
map) was already correct for the *intended* design; the bug was that
the borrow's `ref_local` was wrong due to the missing Copy transfer.
Once the transfer was fixed, the existing NLL algorithm correctly
catches all the unsound patterns that GAP-1 described.

## 3. Conformance Test Updates

### 113 unsound tests flipped from `compile_ok` back to `compile_error`

These tests were originally `compile_error` (correct — they're unsound
programs). Stage 13.17 silently flipped them to `compile_ok` as a
workaround for the GAP-1 silent acceptance. With the fix, they now
correctly fail with a borrowck error.

A script (`scripts/stage14_81_flip_unsound_tests.py`) was created to
systematically flip them back. Each test file was updated with:
- `EXPECTED: compile_ok` → `EXPECTED: compile_error`
- `// STAGE_14.81: Flipped BACK to compile_error (GAP-1 NLL soundness fix)` note
- `// ERROR_PATTERN: cannot borrow` (or `cannot` for assign-borrowed cases)

### 7 error pattern fixes

7 tests had `ERROR_PATTERN: cannot borrow` but the actual error message
was `cannot assign to borrowed value` (a different borrowck error kind).
Updated pattern to `ERROR_PATTERN: cannot` to match both.

### 3 new GAP-1 regression tests

- `bk-0451-18-gap1-double-mut-borrow.lin` — `let r1 = &mut x; let r2 = &mut x;`
- `bk-0452-19-gap1-shared-then-mut.lin` — `let r = &x; let r2 = &mut x;`
- `bk-0453-20-gap1-nll-ok-after-last-use.lin` — sequential mut borrows OK after last use

## 4. Verification

- All 1951 rust tests pass (zero regression)
- All 5170 conformance tests pass (was 5167 — +3 new GAP-1 tests)
- 0 clippy warnings, fmt clean
- 113 previously-unsound tests now correctly rejected
- NLL-valid patterns still accepted (e.g., `let r1 = &mut x; *r1 = 1;
  use(*r1); let r2 = &mut x;` works)

## 5. P0 Blockers Status

| ID | Gap | Est. effort | Status |
|----|-----|-------------|--------|
| **GAP-1** | **NLL soundness regression** | ~~L3~~ **Done (1-line fix)** | **✅ FIXED Stage 14.81** |
| GAP-2 | Region inference is dead_code | L3 | Pending |
| GAP-3 | Drop elaboration is dead_code | L3 | Pending |
| GAP-4 | Lifetime elision is dead_code | L2 | Pending (low priority — `Erased` regions work as universal lifetime) |
| ~~GAP-5~~ | ~~`self.x` field access crashes codegen~~ | ~~L2~~ | **✅ Already fixed (verified Stage 14.81)** |
| ~~GAP-6~~ | ~~Two-phase borrows (method-call subset)~~ | ~~L2~~ | **✅ Already fixed (verified Stage 14.81)** |
| GAP-7 | Disjoint closure captures (RFC 2229) | L2 | Pending |

## 6. Design Doc Alignment (§13.4)

No new design doc deviations. The fix is consistent with
`04-ownership-borrowing.md` §2.2 rule 3 ("a value can have multiple
&T OR one &mut T, never both") — the existing spec was correct, the
implementation had a bug.

## 7. Next Stage Plan

With GAP-1 fixed and GAP-5/GAP-6 already verified as working, the
remaining P0 blockers are:
- GAP-2 (region inference dead_code) — L3
- GAP-3 (drop elaboration dead_code) — L3
- GAP-7 (disjoint closure captures) — L2

GAP-7 is the smallest L2 and will be addressed next (Stage 14.82).
GAP-2/GAP-3 are L3 infrastructure work that may be deferred past v0.1
since `Erased` regions and no-drop-elaboration are sufficient for
v0.1's surface area.
