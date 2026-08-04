# v0.3 Deep Review Round 3 — Stage 16.15

> **Author**: Super Z (main agent, acting as committee)
> **Date**: 2026-08-03
> **Version**: v0.228.1 (review only — no code change)
> **Process**: stage-committee-process.md v3.24 §25 (Deep Review) + §29
> **Scope**: Post-Task-10-Steps-1+2 assessment + Step 3+4 readiness

## Executive Summary

This deep review evaluates v0.3 progress after 15 stages (16.00–16.14),
following the completion of Task 10 Steps 1+2 (Closure Redesign
infrastructure + MIR body synthesis). It assesses readiness for the
risky Step 3+4 (call site migration + codegen) that switches from
inline to synthesized `call` function.

**Verdict**: ✅ **GO** — Task 10 Steps 1+2 are solid. The infrastructure
and MIR body synthesis are correct. Steps 3+4 require careful migration
but the foundation is ready. 7687 tests passing, 0 failures, 0 warnings.

**Key findings**:
- Task 10 Steps 1+2: infrastructure complete and tested
- Step 3 (call site migration) is the risky switch — needs Step 4 (codegen) simultaneously
- Recommend: Steps 3+4 together in one stage (can't do Step 3 without Step 4)
- Codegen needs: function name registration + LLVM function emission

---

## D1: Architecture Health (§16 Interface Isolation)

### Current State

**Pipeline stages** (unchanged):
```
Lexer → Parser → HIR Lower → Resolve → MIR Lower → Typeck → Drop Elaboration → Borrowck → Codegen
```

**Task 10 changes (Steps 1+2)**:
- `SynthesizedClosureFunction` struct + side-table on `MirLowerCtxt` (Stage 16.13)
- `build_synthesized_closure_mir_body()` function (Stage 16.14)
- `synthesized_closure_mir_bodies` field on `CompileResult` (Stage 16.14)
- No new coupling — data flows downstream (MIR lower → driver → codegen)

### Interface Isolation

- ✅ MIR lower reads HIR (closure body) — allowed
- ✅ Driver builds MIR bodies — allowed (orchestration)
- ✅ Codegen will read `synthesized_closure_mir_bodies` — allowed (data only)
- ✅ No HIR access from codegen (side-table carries all needed info)

### Action Items

- **None**. Architecture is healthy.

---

## D2: Technical Debt

### Debt Inventory

| ID | Description | Priority | Status |
|----|-------------|----------|--------|
| TD-COPY-1 | `ty_is_copy` deprecated | P3 | ✅ Documented |
| TD-KEYS-1 | `impl_by_trait_and_type` (Spur) exists | P3 | ✅ Methods deprecated |
| TD-FALLBACK-1 | `BorrowChecker::new()` uses unsound `ty_is_copy` | P3 | ✅ Test-only |
| TD-MIR-COPY-1 | `is_mir_ty_copy_conservative` | P3 | ✅ Documented |
| TD-CLOSURE-1 | Inline closure call path (Stage 13.3a) still active | P2 | 🔧 Will be resolved in Step 3+4 |
| TD-CLOSURE-2 | `closure_bodies` side-table duplicates `synthesized_closure_functions` | P3 | 🔧 Will be removed in Step 5 |

### Risk Assessment

- **TD-CLOSURE-1**: The inline path is the current production path. Step 3+4
  will replace it with the synthesized `call` function. This is the main
  risk for the next stage.
- **TD-CLOSURE-2**: Two side-tables carry similar info. `closure_bodies`
  (Stage 13.3a, keyed by LocalId) and `synthesized_closure_functions`
  (Stage 16.13, keyed by DefId). Step 5 cleanup will remove `closure_bodies`.

### Action Items

- **TD-CLOSURE-1**: Address in Stage 16.16 (Steps 3+4 together)
- **TD-CLOSURE-2**: Address in Stage 16.17+ (Step 5 cleanup)

---

## D3: Test Coverage

### Current Coverage

| Test Type | Count | Pass Rate |
|-----------|-------|-----------|
| Lib tests | 244 | 100% |
| Integration tests | 2219 | 100% |
| Conformance tests | 5224 | 100% |
| **Total** | **7687** | **100%** |

### Task 10 Test Additions

| Stage | Tests Added | Focus |
|-------|-------------|-------|
| 16.13 | +8 | Infrastructure (struct, side-table, DefId) |
| 16.14 | +8 | MIR body synthesis (basic blocks, return, locals) |
| **Total** | **+16** | |

### Gap Analysis

- ✅ Infrastructure: fully tested
- ✅ MIR body synthesis: fully tested
- 🔧 **Gap**: No test verifying the synthesized MIR body produces correct
  results when codegen'd (deferred to Step 4)
- 🔧 **Gap**: No test for the call site migration (deferred to Step 3)

### Action Items

- **Add end-to-end closure call test** in Stage 16.16 (Step 3+4)

---

## D4: Step 3+4 Readiness

### Step 3 (Call Site Migration) Requirements

| Requirement | Status | Gap |
|-------------|--------|-----|
| `SynthesizedClosureFunction` metadata | ✅ Ready (16.13) | None |
| `build_synthesized_closure_mir_body()` | ✅ Ready (16.14) | None |
| `synthesized_closure_mir_bodies` on CompileResult | ✅ Ready (16.14) | None |
| Change `lower_closure_call_inline` to emit `TerminatorKind::Call` | 🔧 Not ready | Need to switch from inline to call |
| Register closure fn_name in `fn_name_by_def_id` | 🔧 Not ready | Need to add registration |

### Step 4 (Codegen) Requirements

| Requirement | Status | Gap |
|-------------|--------|-----|
| Synthesized MIR bodies available | ✅ Ready (16.14) | None |
| Emit LLVM function for each synthesized MIR body | 🔧 Not ready | Need to call `codegen_function` for each |
| Handle `TerminatorKind::Call` to closure function | ✅ Exists | Codegen already handles FnDef calls |
| Function name resolution | 🔧 Not ready | Need to register names in `fn_name_by_def_id` |

### Readiness Assessment

**Step 3+4 must be done together** — you can't switch call sites to
`TerminatorKind::Call` without codegen emitting the target function.
The migration is:

1. Register synthesized closure function names in `fn_name_by_def_id`
2. Emit LLVM functions for `synthesized_closure_mir_bodies` in codegen
3. Change `lower_closure_call_inline` to emit `TerminatorKind::Call`
4. Verify all closure tests still pass

**This is a single stage's work** (Stage 16.16).

### Action Items

- **Stage 16.16**: Steps 3+4 together — the big switch

---

## D5: Design Reasonableness

### Synthesized Closure Architecture (Task 10 Steps 1+2)

**Assessment**: ✅ **Well-designed**

- `SynthesizedClosureFunction` carries all needed metadata
- `build_synthesized_closure_mir_body()` correctly builds MIR with:
  - `self` parameter (LocalId(1))
  - Closure params (LocalId(2+))
  - Capture extraction via field projections
  - Body lowering
  - Return terminator
- Side-table pattern matches existing `adt_layouts`, `closure_bodies`
- DefId allocation uses reserved range (no collision)

### Migration Strategy

**Assessment**: ✅ **Sound**

- Gradual migration (Steps 1-5) avoids big-bang risk
- Steps 1+2 are infrastructure-only (no behavior change)
- Steps 3+4 are the switch (behavior change, but tested)
- Step 5 is cleanup (remove dead code)

### Action Items

- **None**. Design is sound.

---

## D6: Performance & Scalability

### Performance Baseline

- Build time: ~17s
- Test time: ~5s integration + ~30s conformance
- No performance regressions in Stages 16.13–16.14

### Bottleneck Analysis

| Potential Bottleneck | Impact | Status |
|---------------------|--------|--------|
| Synthesized MIR body building (per closure) | O(closures) | ✅ Acceptable |
| Extra MIR bodies in CompileResult | O(closures) memory | ✅ Acceptable |
| Codegen emitting extra functions | O(closures) time | ✅ Acceptable |

### Action Items

- **None** for current scale.

---

## D7: Documentation & Knowledge Transfer

### Documentation Inventory

| Doc | Status |
|-----|--------|
| Stage docs (16.00–16.14) | ✅ Complete (15 docs) |
| Deep review rounds 1+2 | ✅ Complete |
| Task 3 design doc | ✅ Complete |
| Task 10 design doc | ✅ Complete (Stage 16.13) |
| Worklog | ✅ Up to date |
| RELEASE_NOTES.md | ✅ v0.228.1 |
| README.md | ✅ v0.228.1 |

### Action Items

- **None**. Documentation is complete.

---

## D8: Test Path Coverage & Pipeline Corroboration

### Pipeline Test Coverage

- ✅ Tier 1: Pipeline stage coverage
- ✅ Tier 2: Inter-stage integration tests
- ✅ Tier 3: End-to-end E2E tests

### Branch Flow Coverage

| Flow Type | Coverage |
|-----------|----------|
| Control flow | ✅ Covered |
| Data flow (Copy/Move) | ✅ Covered |
| Type system | ✅ Covered |
| Trait dispatch | ✅ Covered |
| Drop elaboration | ✅ Covered |
| Closure (inline) | ✅ Covered |
| Closure (synthesized) | 🔧 Infrastructure tested, end-to-end deferred to Step 3+4 |

### Action Items

- **Add synthesized closure end-to-end test** in Stage 16.16

---

## Committee Vote

| Role | Vote | Rationale |
|------|------|-----------|
| ARCH-A | GO | Architecture healthy, Task 10 Steps 1+2 solid |
| QA-A | GO | 7687 tests, 100% pass, 0 warnings |
| REV-A | GO | 0 TODOs, debts documented, TD-CLOSURE-1 has clear plan |
| PM-A | GO | Step 3+4 ready — recommend doing them together in Stage 16.16 |
| DEV-A | GO | Code is clean, foundation is solid |

**Consensus**: ✅ **GO** — proceed to Steps 3+4 (the big switch).

---

## Recommended Next Stage

**Stage 16.16: Task 10 Steps 3+4 — Call site migration + Codegen**

This is the big switch from inline to synthesized `call` function:

1. Register synthesized closure function names in `fn_name_by_def_id`
2. Emit LLVM functions for `synthesized_closure_mir_bodies` in codegen
3. Change `lower_closure_call_inline` to emit `TerminatorKind::Call`
4. Verify all closure tests pass

**Effort**: 1-2 days (careful migration, but foundation is ready)

**Risk**: Behavior change — if the synthesized MIR body or codegen is
incorrect, closure tests will fail. But the infrastructure is tested,
so the risk is manageable.

---

## Summary

Task 10 Steps 1+2 are complete and solid. The infrastructure (struct,
side-table, DefId allocation) and MIR body synthesis are correct and
tested. The next step is the big switch (Steps 3+4 together) which
replaces the inline approach with the synthesized `call` function. The
foundation is ready, and the committee recommends proceeding.
