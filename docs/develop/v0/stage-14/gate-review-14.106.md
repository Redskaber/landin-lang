# Stage 14.106 — Gate Review: Phase 2 Architecture Audit + HP-1 Infrastructure

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.119.0 → v0.120.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.106 launches Phase 2 of the deep architecture audit — a comprehensive
assessment of architectural readiness for v0.2. The audit covered all 9
pipeline stages (95 files, ~41,769 LOC) and produced:

- Per-stage architectural verdicts
- Top 10 hidden problems ranked by risk × fix cost
- v0.2 readiness assessment with 3 mandatory pre-v0.2 fixes
- Performance assessment with 8 optimization recommendations
- v0.2 roadmap (~16 weeks / 4 months)

Additionally, this stage implemented the HP-1 infrastructure (sound Copy
detection in BorrowChecker) though activation is deferred to v0.2.

## 2. Phase 2 Architecture Audit Results

### Per-Stage Verdict

| Stage | Verdict | Key Issues |
|-------|---------|-----------|
| Lexer | ✅ READY | (none blocking) |
| Parser | ✅ READY | (none blocking) |
| HIR | ✅ READY | HP-4/5 sentinel region/tyvar |
| Resolve | ⚠️ NEEDS WORK | HP-15/16 by-name lookup; O(n²) scope clone; stub visibility |
| MIR | ⚠️ NEEDS WORK | HP-3/19/20/21/22/23; 8 O(n²) HIR reach-ins; expr_operand.rs 3,392 LOC |
| TypeCheck | ⚠️ NEEDS WORK | HP-6/13/14; 6 driver writeback passes; deprecated entry points |
| BorrowCheck | ❌ NOT READY | HP-1 unsound ty_is_copy; HP-10 NLL loop bug; HP-12 Drop inert |
| Codegen | ⚠️ NEEDS WORK | HP-B4 String bridge; HP-B5 type check discarded; HP-B6/7 leaks |
| Driver | ⚠️ NEEDS WORK | 8 writeback passes; HP-B12 quadratic; HP-B10 no incremental |

### 3 Mandatory Pre-v0.2 Fixes (from audit)

1. **HP-1/HP-11**: Thread TraitResolver into BorrowChecker (soundness)
2. **HP-19/HP-21**: Add span to BasicBlock and Terminator (debug info prep)
3. **HP-B11**: Consolidate 8 driver writeback passes into 1 (maintainability)

### v0.2 Roadmap

- Phase 1 (weeks 1-4): Architectural debt payment (EmitValue bridge, impl index, type-aware registry, closures)
- Phase 2 (weeks 5-8): Soundness closures (fixpoint NLL, drop elaboration, region allocation)
- Phase 3 (weeks 9-12): Feature work (monomorphization, lifetimes, Drop, dyn Trait)
- Phase 4 (weeks 13-16): Polish + incremental

## 3. HP-1 Infrastructure (Implemented, Activation Deferred)

### What Was Implemented

- `BorrowChecker` struct now has lifetime parameter `'a` and optional
  `resolver: Option<&'a TraitResolver>` + `interner: Option<&'a Rodeo>` fields
- `BorrowChecker::with_resolver(resolver, interner)` constructor for sound Copy
- `BorrowChecker::is_copy(ty)` method that uses `ty_is_copy_with_resolver` when
  resolver is available, falls back to unsound `ty_is_copy` otherwise
- `check_mir_body` call site in driver updated (but uses `new()` — deferred)

### Why Activation Is Deferred

`ty_is_copy_with_resolver` returns `false` for ALL user-defined structs because
v0.1 has no `#[derive(Copy)]` support and users don't write `impl Copy for Type`
blocks. This causes 223 test failures — v0.1 tests expect structs with all-Copy
fields to be Copy (matching Rust's `#[derive(Copy)]` rules).

The correct v0.2 fix is field-level Copy detection: a struct is Copy if ALL its
fields are Copy. This requires field type lookup infrastructure that doesn't
exist in v0.1. The infrastructure (BorrowChecker lifetime + with_resolver) is
ready for v0.2 activation.

## 4. Test Count

| Suite | Count | Pass rate |
|-------|-------|-----------|
| Rust tests | 1951 | 100% |
| Conformance tests | 5216 | 100% |

No new tests (infrastructure-only change).

## 5. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 6. Stage Verdict

**PASS** — Phase 2 architecture audit complete. HP-1 infrastructure implemented
(activation deferred to v0.2). All tests pass. No regressions.

v0.120.0: minor bump (Phase 2 audit + HP-1 infrastructure)
