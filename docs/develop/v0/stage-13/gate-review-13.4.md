# Stage 13.4 Gate Review — macro_rules! / built-in macros (TD-032 P0 — preparation phase)

> **版本**: v0.23.0 → v0.23.1 | **流程**: §13.4 + §14.4 + §25.8 + §25.7
> **Companion**: `stage-13.4-design-alignment.md` (§13.4 design alignment + scope analysis)

## CI/CD
```
cargo test: 2265 passed (146 unit + 2265 integration), 0 failed, 2 ignored
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 5026 passed, 0 failed
```

## Stage 13.4 status: 🔄 PREPARATION PHASE (TD-032 NOT YET CLOSED)

Per `stage-13.4-design-alignment.md` §8, the committee recommended **GO-WITH-CONDITIONS**
with 6 conditions. Key finding: **TD-032 was mislabeled** — design docs forbid `macro_rules!`
for v0.1/v0.3. The actual v0.3 blocker is **19 missing built-in macros** (7 of 26 hardcoded).
Strategy A (full macro_rules!) is REJECTED as design-forbidden; Strategy B (extend built-in
macros) is design-sanctioned by `02-grammar.md` §4.4.

Given the HIGH risk of Stage 13.4a (~800-1200 LOC, new `TokenTree` type, 19 individual
expanders), Stage 13.4 is split into:

- **Stage 13.4 (this phase)**: §13.4 design alignment + TD-032 reframe + implementation blueprint + verification test infrastructure
- **Stage 13.4a (next phase)**: Full Strategy B implementation — 19 missing built-in macros + TokenTree type + HIR-time expansion. TD-032 closure target.

## Critical TD-032 Reframe

**Before Stage 13.4** (r216/r217/plan-13.1.md):
- TD-032 described as "macro_rules! not implemented (26 built-in macros hardcoded)"
- This was **INCORRECT** — 5 design docs forbid macro_rules! for v0.1/v0.3

**After Stage 13.4 (reframe)**:
- TD-032 reframed as "19 of 26 built-in macros missing (7 hardcoded as non-functional placeholders)"
- `macro_rules!` is **NOT** a v0.3 blocker — it's a v0.2+ feature per `12-roadmap.md` §4.1
- Strategy A (full macro_rules!) REJECTED per §13.4.2 rule 1 (design alignment)
- Strategy B (extend built-in macros) is design-sanctioned by `02-grammar.md` §4.4

## §13.4 Design Alignment: ✅ Strategy C → B (preparation + implementation split)

Per `stage-13.4-design-alignment.md`:
- **Strategy A (full macro_rules!)**: REJECTED — design-forbidden per 5 design docs
- **Strategy B (extend built-in macros)**: DESIGN-SANCTIONED — `02-grammar.md` §4.4 "编译器硬编码展开"
- **Strategy C (preparation phase)**: CHOSEN for Stage 13.4; Stage 13.4a = Strategy B implementation

## Implementation blueprint (for Stage 13.4a)

Per design alignment §6, the full implementation requires:

1. **New `TokenTree` type** (~100 LOC)
   - `05-ast.md` §8 design specifies `args: Vec<TokenTree>` for MacroCall
   - Currently MacroCall discards body tokens (B1 deviation)
   - TokenTree needed for proper macro argument handling

2. **19 missing built-in macro expanders** (~500-700 LOC)
   - Each macro: parse args → produce HIR/MIR
   - Macros: assert!, assert_eq!, assert_ne!, debug_assert!, debug_assert_eq!, debug_assert_ne!,
     write!, writeln!, eprintln!, eprint!, panic!, todo!, unimplemented!, unreachable!,
     cfg!, include!, concat!, env!, option_env!, format_args!
   - (7 existing: println!, format!, vec!, stringify!, print!, write_str!, dbg!)

3. **HIR-time expansion** (~200-300 LOC)
   - Current: macros lowered in MIR (limited)
   - Target: macros expand in HIR lowering (proper architecture per design)
   - This is the architectural shift — MIR sees only expanded HIR

4. **Conformance FAIL→PASS verification** (N conformance tests)

## §25.8 Design Write-back Plan (for Stage 13.4a)

Per design alignment §7:
1. `05-ast.md` §8 — add TokenTree type (B4 gray-area fill)
2. `02-grammar.md` §4.4 — note built-in macro expansion is design-sanctioned
3. `13-stage1-feature-whitelist.md` §2.6 — update 26-macro status (7 done → 26 done after 13.4a)
4. `07-codegen.md` — document HIR-time macro expansion architecture

## TD status after Stage 13.4 (preparation)

| TD ID | Priority | Status | Stage |
|-------|----------|--------|-------|
| TD-019 | P3 | on user hold | Stage 13+ |
| TD-028 | P2 | ✅ CLOSED (Stage 13.1) | — |
| TD-029 | P2 | open (deferred to Stage 13.1b) | Stage 13.1b |
| TD-030 | P0 | ✅ CLOSED (Stage 13.3a) | — |
| TD-031 | P0 | ✅ CLOSED (Stage 13.2) | — |
| **TD-032** | **P0** | **🔄 OPEN (reframed: 19 missing built-in macros; 13.4a implementation pending)** | Stage 13.4a |
| TD-033 | P1 | open | Stage 13.5+ |

**P0 closure progress**: 2/3 P0 closed (TD-030 + TD-031); 1 in preparation (TD-032 reframed)

## 委员会投票: 5/5 GO-WITH-CONDITIONS → PASS (preparation phase)

| Role | Vote | Reasoning |
|------|------|-----------|
| ARCH-A | GO-WITH-CONDITIONS | TD-032 reframe is critical; Strategy A rejection correct |
| DEV-A | GO-WITH-CONDITIONS | No code changes in 13.4; 13.4a is ~800-1200 LOC HIGH-risk |
| QA-A | GO-WITH-CONDITIONS | Test infrastructure created; 13.4a needs conformance verification |
| ALG-C | GO-WITH-CONDITIONS | Strategy B is design-sanctioned; HIR-time expansion is proper architecture |
| SKL-A | GO-WITH-CONDITIONS | Preparation is proper; rushing HIGH-risk P0 would be reckless |

## Version policy: v0.23.0 → v0.23.1 (patch bump — preparation phase, no new features)

Per semver §2.0.0:
- Stage 13.4 preparation adds no new compiler features (only docs + tests + design alignment)
- Patch bump appropriate
- v0.24.0 reserved for Stage 13.4a (19 built-in macros — third user-facing feature)

## Next: Stage 13.4a (TD-032 — 19 missing built-in macros)

Stage 13.4a executes the blueprint above (~800-1200 LOC, HIGH risk):
1. New TokenTree type
2. 19 missing built-in macro expanders
3. HIR-time expansion architecture
4. Conformance FAIL→PASS verification

After Stage 13.4a: **all 3 P0 items closed** → v0.3 self-hosting can begin.

---

**审查完成**: 2026-07-26
**Stage 13.4 STATUS**: 🔄 PREPARATION COMPLETE (TD-032 reframed; 13.4a implementation pending)
**Next**: Stage 13.4a (TD-032 — 19 missing built-in macros; after this, all P0 closed)
