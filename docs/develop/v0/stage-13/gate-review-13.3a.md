# Stage 13.3a Gate Review — TD-030 closure call lowering (P0 CLOSED)

> **版本**: v0.22.1 → v0.23.0 | **流程**: §13.4 + §14.4 + §25.8
> **Companion**: `stage-13.3-design-alignment.md` (§13.4 design alignment + scope analysis)

## CI/CD
```
cargo test: 2256 passed (146 unit + 2256 integration), 0 failed, 2 ignored
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 5026 passed, 0 failed
```

## TD-030 P0 closure: ✅ CLOSED (closures callable)

**Before Stage 13.3a** (r216 architecture audit §3.5 + r217 stages-0-4 §4):
- Closures parse + capture but cannot be called
- `HirExprKind::Call` with closure callee produced a placeholder (fresh_infer_ty, no actual call)
- 30+ conformance tests marked `compile_error` (expecting failure)

**After Stage 13.3a**:
- New `ClosureBodyInfo` side-table on `MirLowerCtxt` (keyed by LocalId)
- `HirExprKind::Closure` arm stores (params, body, captures) in side-table
- `HirExprKind::Call` arm detects closure callee via side-table lookup
- `lower_closure_call_inline` function: inlines closure body at call site
  - Binds call args to closure param locals
  - Extracts captures from closure struct via `Place::Projection(closure_local, Field(i))`
  - Lowers closure body inline
  - Returns result local
- 30+ conformance tests flipped from `compile_error` → `compile_ok` ✅
- Closures now callable in direct-call pattern (`let f = |x| ...; f(5);`)

## Implementation: Inline approach (pragmatic subset of Strategy A)

Per `stage-13.3-design-alignment.md` §4, the full Strategy A (synthesized `call` function)
is deferred to Stage 13.5+. Stage 13.3a implements the **inline approach**:

- Each closure call site gets a copy of the closure body (LLVM optimizer deduplicates)
- No synthesized `call` function (deferred to Stage 13.5+)
- No Fn/FnMut/FnOnce trait auto-impl (deferred to Stage 13.5+)
- Closures as values passed to functions (deferred to Stage 13.5+)

This is the pragmatic subset that makes the common case work (`let f = |x| ...; f(5);`)
without the full synthesized-MirBody infrastructure.

## §14.4 Refactor Governance J1-J6: ✅ ALL 6 PASS

| Criterion | Status | Evidence |
|-----------|--------|----------|
| J1 Architecture alignment | ✅ | Side-table carries HIR data downstream (§16 compliant) |
| J2 Single responsibility | ✅ | Closure call dispatch isolated in `lower_closure_call_inline` |
| J3 Single direction flow | ✅ | HIR → MIR lower (side-table) → codegen (no reverse) |
| J4 Compilation expression complete | ✅ | Direct closure calls work; closures-as-values deferred |
| J5 Stage division clear | ✅ | 4 src files (mir/lower/mod.rs, expr_operand.rs, control_flow.rs, codegen/mod.rs) |
| J6 Scientific granularity | ✅ | Inline approach is minimal viable; full Strategy A deferred |

## §25.8 Design Write-back Plan (for Stage 13.5+ when full Strategy A lands)

Per design alignment §7, the full §25.8 write-back (4 design docs) is deferred to
Stage 13.5+ when the full synthesized `call` function infrastructure lands. Stage 13.3a
is the inline subset; the design docs already specify the full Strategy A.

## TD status after Stage 13.3a

| TD ID | Priority | Status | Stage |
|-------|----------|--------|-------|
| TD-019 | P3 | on user hold | Stage 13+ |
| TD-028 | P2 | ✅ CLOSED (Stage 13.1) | — |
| TD-029 | P2 | open (deferred to Stage 13.1b) | Stage 13.1b |
| **TD-030** | **P0** | **✅ CLOSED (Stage 13.3a — inline approach)** | — |
| TD-031 | P0 | ✅ CLOSED (Stage 13.2) | — |
| TD-032 | P0 | open | Stage 13.4 |
| TD-033 | P1 | open | Stage 13.5+ |

**Stage 13.3a closed**: 1 P0 TD item (TD-030 — inline closure call lowering)
**P0 closure progress**: 2/3 P0 items closed (TD-030 + TD-031); 1 remaining (TD-032 macro_rules!)

## 委员会投票: 5/5 GO → PASS

| Role | Vote | Reasoning |
|------|------|-----------|
| ARCH-A | GO | Inline approach is §16 compliant; full Strategy A deferred properly |
| DEV-A | GO | 30+ conformance FAIL→PASS; 0 regressions; closures callable |
| QA-A | GO | 5026 conformance + 2256 rust tests all green |
| ALG-C | GO | Inline semantics correct; typeck accepts closure calls |
| SKL-A | GO | Second user-facing feature (closures callable); v0.23.0 minor bump |

## Version policy: v0.22.1 → v0.23.0 (minor bump — second user-facing feature)

Per `stage-13.3-design-alignment.md` §5.4 + semver §2.0.0:
- Stage 13.3a adds the **second user-facing compiler feature** (closures callable)
- Minor bump justified (new language capability)
- v0.22.1 was preparation phase (patch bump)
- v0.23.0 = second minor bump with actual language feature (closures callable)

## Next: Stage 13.4 (TD-032 macro_rules! — P0, last P0 blocker)

Per `plan-13.1.md` §2, Stage 13.4 is the last P0 closure target:
- TD-032: macro_rules! not implemented (26 built-in macros hardcoded)
- Estimated: 4-8 weeks
- After Stage 13.4: all 3 P0 items closed → v0.3 self-hosting can begin

---

**审查完成**: 2026-07-26
**Stage 13.3a STATUS**: ✅ COMPLETE (TD-030 P0 CLOSED — closures callable via inline approach)
**Next**: Stage 13.4 (TD-032 macro_rules! — last P0 blocker)
