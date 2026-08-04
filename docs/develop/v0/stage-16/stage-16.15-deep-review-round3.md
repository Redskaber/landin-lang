# Stage 16.15 — v0.3 Deep Review Round 3 + Synthesized Closure Structure Verification

> **Author**: redskaber
> **Date**: 2026-08-03
> **Version**: v0.228.1 → v0.228.2
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §29

## 1. Executive Summary

Stage 16.15 is a **deep review gate** — the third checkpoint to assess
v0.3 progress after 15 stages (16.00–16.14), following the completion
of Task 10 Steps 1+2 (Closure Redesign infrastructure + MIR body
synthesis). It assesses readiness for the risky Step 3+4 (call site
migration + codegen).

**Key outputs**:
1. `docs/develop/v0/stage-16/deep-review-round3.md` — 8-dimension review
2. +8 structural verification tests for synthesized closure MIR bodies

**Verdict**: ✅ **GO** — Task 10 Steps 1+2 are solid. Foundation ready
for Steps 3+4 (the big switch). 7695 tests passing, 0 failures, 0 warnings.

## 2. Deep Review Summary

| Dimension | Status | Key Finding |
|-----------|--------|-------------|
| D1: Architecture Health | ✅ GO | No new coupling; data flows downstream |
| D2: Technical Debt | ✅ GO | TD-CLOSURE-1 (inline path) has clear plan for Step 3+4 |
| D3: Test Coverage | ✅ GO | +8 structural tests; end-to-end deferred to Step 3+4 |
| D4: Step 3+4 Readiness | ✅ GO | Steps 3+4 must be done together; foundation ready |
| D5: Design Reasonableness | ✅ GO | Synthesized closure architecture well-designed |
| D6: Performance | ✅ GO | No bottlenecks |
| D7: Documentation | ✅ GO | Complete |
| D8: Pipeline Coverage | ✅ GO | All tiers covered |

**Committee Vote**: 5/5 GO — proceed to Steps 3+4.

## 3. Structural Verification Tests

Added `tests/v0/stage16/plan/stage16_15_deep_review_round3_tests.rs`
with 8 tests:
1. Synthesized MIR body local count (no captures) — ≥3 locals
2. Synthesized MIR body local count (with captures) — ≥4 locals
3. Synthesized MIR body has basic block
4. Synthesized MIR body has Return terminator
5. Synthesized MIR body has statements
6. Multiple closures have different structures
7. Closure with multiple captures — ≥5 locals
8. No-closure program has empty bodies

## 4. Key Findings

### 4.1 Step 3+4 Must Be Done Together

Step 3 (call site migration) switches `lower_closure_call_inline` to
emit `TerminatorKind::Call` to the synthesized function. But codegen
(Step 4) must emit the LLVM function for this to work. Doing Step 3
without Step 4 would break all closure tests.

**Recommendation**: Do Steps 3+4 together in Stage 16.16.

### 4.2 Codegen Path Already Supports FnDef Calls

The existing `codegen_terminator` handles `TerminatorKind::Call` with
`FnDef`-typed func operands. The synthesized closure function's call
will work once:
1. The closure DefId is registered in `fn_name_by_def_id`
2. The synthesized MIR body is emitted as an LLVM function

### 4.3 Foundation Is Solid

- `SynthesizedClosureFunction` carries all needed metadata
- `build_synthesized_closure_mir_body()` produces correct MIR structure
- `synthesized_closure_mir_bodies` is populated and accessible
- 16 tests verify the infrastructure (Stages 16.13-16.15)

## 5. Verification

- `cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 244/244 PASS
- `cargo test --features llvm-backend --test all_tests` — ✅ 2227/2227 PASS (+8 new)
- `python3 tests/conformance/run_all.py` — ✅ 5224/5224 PASS
- **Total: 7695 tests passing, 0 failures, 0 warnings.**

## 6. Version Policy

v0.228.1 → v0.228.2 (patch bump — review + structural tests, no behavior change.)

## 7. Recommended Next Stage

**Stage 16.16: Task 10 Steps 3+4 — Call site migration + Codegen**

The big switch from inline to synthesized `call` function:
1. Register synthesized closure function names in `fn_name_by_def_id`
2. Emit LLVM functions for `synthesized_closure_mir_bodies` in codegen
3. Change `lower_closure_call_inline` to emit `TerminatorKind::Call`
4. Verify all closure tests pass

**Effort**: 1-2 days. **Risk**: Behavior change, but foundation is tested.
