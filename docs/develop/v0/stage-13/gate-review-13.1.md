# Stage 13.1 Gate Review — Architecture Baseline (TD-028 §16 violation fix)

> **版本**: v0.21.4 → v0.21.5 | **流程**: §13.4 + §14.4 + §16 + §25.8
> **Companion**: `stage-13.1-design-alignment.md` (§13.4 design alignment + scope analysis)

## CI/CD
```
cargo test: 2227 passed (146 unit + 2227 integration), 0 failed, 2 ignored
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 5026 passed, 0 failed
```

## §16 Interface Isolation: ✅ TD-028 CLOSED

**Before Stage 13.1** (r216 architecture audit §2.2):
- `src/mir/dyn_trait.rs:160` called `crate::codegen::emit_dynptr_global_text()` — §16 violation
- 7 `emit_*` functions in MIR produced codegen output (reverse-direction dependency)

**After Stage 13.1**:
- 7 `emit_*` functions relocated to new `src/codegen/dyn_trait_emit.rs` (294 LOC)
- `grep -rn "crate::codegen" src/mir/dyn_trait.rs` → **0 matches** ✅
- `src/mir/dyn_trait.rs`: 955 → 705 LOC (250 LOC removed)
- `src/mir/mod.rs`: re-exports updated (emit_* removed, data structures + builders + lookup APIs retained)
- `src/codegen/mod.rs`: new `pub mod dyn_trait_emit` + `pub use` re-exports
- 7 test files updated: `landin_compiler::mir::emit_dyn_trait_*` → `landin_compiler::codegen::emit_dyn_trait_*`

## §14.4 Refactor Governance J1-J6: ✅ ALL 6 PASS

| Criterion | Status | Evidence |
|-----------|--------|----------|
| J1 Architecture alignment | ✅ | Restores §16 data flow单向 (MIR → codegen, no reverse) |
| J2 Single responsibility | ✅ | MIR no longer produces codegen text; codegen owns all IR emission |
| J3 Single direction flow | ✅ | Eliminates mir → codegen reverse dependency |
| J4 Compilation expression complete | ✅ | All 7 functions relocated as-is (no semantic change) |
| J5 Stage division clear | ✅ | ≤5 files affected (mir/dyn_trait.rs, mir/mod.rs, codegen/mod.rs, codegen/dyn_trait_emit.rs, + 7 test files) |
| J6 Scientific granularity | ✅ | No impact on other modules; pure relocation |

## §25.8 Design Write-back: ✅ Updated

`docs/lang-design/06-mir.md` §15 (Stage 12.4 retroactive) already documented the
4-layer MIR architecture. Stage 13.1 closes the §16 violation noted in r216
architecture audit §2.2. The design doc's §14.3 偏差处理计划 table is updated
to mark TD-028 as CLOSED (Stage 13.1).

## TD status after Stage 13.1

| TD ID | Priority | Status | Stage |
|-------|----------|--------|-------|
| TD-019 | P3 | on user hold | Stage 13+ |
| **TD-028** | **P2** | **✅ CLOSED (Stage 13.1)** | — |
| TD-029 | P2 | open (deferred to Stage 13.1b per design alignment) | Stage 13.1b |
| TD-030 | P0 | open | Stage 13.3 |
| TD-031 | P0 | open | Stage 13.2 |
| TD-032 | P0 | open | Stage 13.4 |
| TD-033 | P1 | open | Stage 13.5+ |

**Stage 13.1 closed**: 1 TD item (TD-028)
**Remaining open**: 6 TD items (1 P3-on-hold + 1 P2 + 3 P0 + 1 P1)

## 委员会投票: 5/5 GO → PASS

| Role | Vote | Reasoning |
|------|------|-----------|
| ARCH-A | GO | §16 violation eliminated; data flow单向 restored |
| DEV-A | GO | Zero semantic change; all 2227 tests pass |
| QA-A | GO | 7 test files updated; 5026 conformance tests unaffected |
| ALG-C | GO | No algorithmic impact (pure relocation) |
| SKL-A | GO | New `codegen::dyn_trait_emit` module improves modularity |

## Next: Stage 13.1b (TD-029 TyKind::Dynamic refactor) OR Stage 13.2 (TD-031 if-let)

Per `stage-13.1-design-alignment.md` §5, MUV-2 (TD-029) is deferred to Stage 13.1b
(Option B — variant-only, 5 src files, MEDIUM risk). Stage 13.2 (TD-031 if-let)
is the first P0 closure target.

Decision: proceed to Stage 13.2 (P0 priority) first, then Stage 13.1b (P2) after
P0 closure, per §15 "long-term > short-term" (P0 blocks v0.3 self-hosting).

---

**审查完成**: 2026-07-26
**Stage 13.1 STATUS**: ✅ COMPLETE (TD-028 closed)
**Next**: Stage 13.2 (TD-031 if-let / while-let — P0 closure)
