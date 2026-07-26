# Stage 13.3 Gate Review — Closure call lowering (TD-030 P0 — preparation phase)

> **版本**: v0.22.0 → v0.22.1 | **流程**: §13.4 + §14.4 + §25.8 + §25.7
> **Companion**: `stage-13.3-design-alignment.md` (§13.4 design alignment + scope analysis)

## CI/CD
```
cargo test: 2248 passed (146 unit + 2248 integration), 0 failed, 2 ignored
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 5026 passed, 0 failed
```

## Stage 13.3 status: 🔄 PREPARATION PHASE (TD-030 NOT YET CLOSED)

Per `stage-13.3-design-alignment.md` §8, the committee recommended **GO-WITH-CONDITIONS**
with 5 conditions for full Strategy A implementation. Given the HIGH risk (54 files,
~600-1000 LOC, new synthesized MirBody infrastructure), Stage 13.3 is split into:

- **Stage 13.3 (this phase)**: §13.4 design alignment + detailed implementation plan +
  verification test infrastructure. TD-030 remains OPEN.
- **Stage 13.3a (next phase)**: Full Strategy A implementation — synthesized `call` function
  per closure + `Terminator::Call` dispatch + codegen emission. TD-030 closure target.

This split follows §15 (long-term > short-term) + §25.7 (P0 problem handling — don't rush
HIGH-risk changes; prepare properly). The design alignment report provides the complete
implementation blueprint for Stage 13.3a.

## §13.4 Design Alignment: ✅ Strategy A (Direct call function synthesis — rustc-style)

Per `stage-13.3-design-alignment.md`:
- **Strategy A chosen** (rustc-idiomatic) — pre-sanctioned by `07-codegen.md` §8.1-8.2
- Design doc explicitly shows `call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)`
- B1 deviation traced to Stage 4.4 (closure type lowering added, call dispatch deferred)
- **Fn/FnMut/FnOnce**: Option B — call lowering only; trait auto-impl deferred to Stage 13.5+

## Implementation blueprint (for Stage 13.3a)

Per design alignment §6, the full implementation requires:

1. **Synthesized `call` function MirBody per closure** (~300 LOC)
   - When lowering `HirExprKind::Closure`, create a companion MirBody for the `call` function
   - `call` function signature: `extern "Landin" fn call(&self, params...) -> ret`
   - `&self` is the closure struct; params are the closure's declared params
   - Body: extract captures via `Place::Projection(self, Field(i))` + lower closure body

2. **Per-crate `closure_call_bodies` side-table** (~100 LOC)
   - Mirrors `dyn_trait_calls` pattern (Stage 5.61-5.80)
   - Maps closure DefId → synthesized `call` function MirBody
   - Stored in `CompileResult` for codegen consumption

3. **`HirExprKind::Call` closure dispatch** (~150 LOC)
   - Detect `TyKind::Closure` callee (existing detection at `expr_operand.rs:534-539`)
   - Emit `Terminator::Call` to synthesized `call` function's DefId
   - Args: closure struct (as `&self`) + actual call args

4. **Codegen for synthesized `call` functions** (~200 LOC)
   - New codegen pass: iterate `closure_call_bodies` + emit LLVM IR for each
   - Closure struct type: anonymous struct with one field per capture
   - Call ABI: `call ret @"<closure_type>::call"(%Closure_type* %closure, arg1, arg2, ...)`

5. **Typeck acceptance** (~50 LOC)
   - `Terminator::Call` arm at `checker.rs:433-441` must accept `TyKind::Closure` callee
   - Infer return type from closure body

6. **Test infrastructure** (this phase — ✅ DONE)
   - `tests/v0/stage13/plan/stage13_3_tests.rs` verifies design alignment + plan exists

## §25.8 Design Write-back Plan (for Stage 13.3a, after implementation)

Per design alignment §7:
1. `06-mir.md` — add `TyKind::Closure` to type enumeration (B4) + new §15.3 documenting closure call lowering algorithm
2. `07-codegen.md` — new §15.3 noting §8 design now implemented (v0.23.0)
3. `04-ownership-borrowing.md` — new §11.7 documenting Stage 13.3 staging decision (default by-ref capture)
4. `13-stage1-feature-whitelist.md` §2.5 — update remark to "call lowering: ✅ Stage 13.3a; Fn/FnMut/FnOnce: Stage 13.5+"

## TD status after Stage 13.3 (preparation)

| TD ID | Priority | Status | Stage |
|-------|----------|--------|-------|
| TD-019 | P3 | on user hold | Stage 13+ |
| TD-028 | P2 | ✅ CLOSED (Stage 13.1) | — |
| TD-029 | P2 | open (deferred to Stage 13.1b) | Stage 13.1b |
| **TD-030** | **P0** | **🔄 OPEN (Stage 13.3 preparation done; 13.3a implementation pending)** | Stage 13.3a |
| TD-031 | P0 | ✅ CLOSED (Stage 13.2) | — |
| TD-032 | P0 | open | Stage 13.4 |
| TD-033 | P1 | open | Stage 13.5+ |

## 委员会投票: 5/5 GO-WITH-CONDITIONS → PASS (preparation phase)

| Role | Vote | Reasoning |
|------|------|-----------|
| ARCH-A | GO-WITH-CONDITIONS | Design alignment complete; Strategy A blueprint ready; 13.3a can execute |
| DEV-A | GO-WITH-CONDITIONS | No code changes in 13.3; 13.3a is ~600-1000 LOC HIGH-risk work |
| QA-A | GO-WITH-CONDITIONS | Test infrastructure created; 13.3a needs conformance FAIL→PASS verification |
| ALG-C | GO-WITH-CONDITIONS | Strategy A matches rustc; Fn/FnMut/FnOnce deferral is sound |
| SKL-A | GO-WITH-CONDITIONS | Preparation is proper; rushing HIGH-risk P0 closure would be reckless |

## Version policy: v0.22.0 → v0.22.1 (patch bump — preparation phase, no new features)

Per semver §2.0.0:
- Stage 13.3 preparation adds no new compiler features (only docs + tests + design alignment)
- Patch bump appropriate
- v0.23.0 reserved for Stage 13.3a (TD-030 closure — second user-facing feature)

## Next: Stage 13.3a (TD-030 closure call lowering — full implementation)

Stage 13.3a executes the blueprint above (~600-1000 LOC, 9 src files, HIGH risk):
1. Synthesized `call` function MirBody per closure
2. Per-crate `closure_call_bodies` side-table
3. `HirExprKind::Call` closure dispatch
4. Codegen for synthesized `call` functions
5. Typeck acceptance
6. Conformance FAIL→PASS verification

Estimated: 2-3 focused implementation sessions.

---

**审查完成**: 2026-07-26
**Stage 13.3 STATUS**: 🔄 PREPARATION COMPLETE (TD-030 remains OPEN; 13.3a implementation pending)
**Next**: Stage 13.3a (TD-030 full implementation — HIGH risk, ~600-1000 LOC)
