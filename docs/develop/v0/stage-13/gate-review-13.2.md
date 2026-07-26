# Stage 13.2 Gate Review — if-let / while-let (TD-031 P0 closure)

> **版本**: v0.21.5 → v0.22.0 | **流程**: §13.4 + §14.4 + §25.8
> **Companion**: `stage-13.2-design-alignment.md` (§13.4 design alignment + scope analysis)

## CI/CD
```
cargo test: 2237 passed (146 unit + 2237 integration), 0 failed, 2 ignored
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 5026 passed, 0 failed
```

## TD-031 P0 closure: ✅ if-let / while-let fully supported

**Before Stage 13.2** (r216 architecture audit §3.5 + r217 stages-0-4 §3):
- `if let` / `while let` not in AST/HIR (deferred from Stage 0 to "Stage 1+")
- 11 conformance FAIL tests in `00-parse/02-control-flow/` (6 if-let + 5 while-let)
- Parser emitted soft errors: "not yet supported in Stage 0 (will be added in Stage 1)"

**After Stage 13.2**:
- New AST variants: `Expr::IfLet { pat, expr, then, else_, span }` + `Expr::WhileLet { pat, expr, body, span }`
- Parser fully supports `if let` / `while let` (no soft errors)
- HIR lowering desugars to existing `Match` / `Loop { Match }` (Strategy B — rustc-idiomatic)
  - `if let pat = expr { then } else { else_ }` → `Match(expr, [Arm(pat, then), Arm(_, else_ or unit)])`
  - `while let pat = expr { body }` → `Loop { Match(expr, [Arm(pat, body), Arm(_, Break)]) }`
- 11 conformance FAIL tests flipped to PASS ✅
- 2 Stage 0 regression tests updated (`test_regression_no_infinite_loop_on_if_let` / `_while_let`)
  — now assert 0 errors (was `!errors.is_empty()`)

## §13.4 Design Alignment: ✅ Strategy B (Desugar to Match)

Per `stage-13.2-design-alignment.md` §4:
- **Strategy B chosen** (rustc-idiomatic) over Strategy A (direct MIR lowering) / Strategy C (hybrid)
- Rationale: `05-ast.md` §12.4 explicitly prescribes `if let → match` and `while let → loop { match }`
- Reuses existing `lower_match` (188 LOC, 6+ gate reviews) + `HirExprKind::Loop` lowering (24 LOC)
- Zero new MIR lowering, typeck, or borrowck arms — these layers see only Match/Loop MIR (§16 compliant)

## §14.4 Refactor Governance J1-J6: ✅ ALL 6 PASS

| Criterion | Status | Evidence |
|-----------|--------|----------|
| J1 Architecture alignment | ✅ | Desugar reuses existing Match/Loop infrastructure (§16 compliant) |
| J2 Single responsibility | ✅ | AST has IfLet/WhileLet; HIR sees only Match/Loop (clean separation) |
| J3 Single direction flow | ✅ | AST → HIR desugar → MIR (no reverse dependencies) |
| J4 Compilation expression complete | ✅ | All if-let/while-let forms handled (basic, chain, else, struct, tuple, wildcard, nested, break, continue) |
| J5 Stage division clear | ✅ | 3 src files (ast/kinds.rs, parser/expr.rs, hir/lower/body.rs) + 2 span-helper updates |
| J6 Scientific granularity | ✅ | No impact on typeck/borrowck/codegen (they see only Match/Loop) |

## §25.8 Design Write-back: ✅ Updated

- `docs/lang-design/05-ast.md` §8 — `IfLet` / `WhileLet` variants added as implementation-as-fact (B4)
- `docs/lang-design/03-type-system.md` §13.4 (new) — if-let refinement scope auto-handled via Match pattern bindings
- `docs/lang-design/04-ownership-borrowing.md` §4 — if-let borrow scope = match-arm basic block (NLL auto-handles)

## TD status after Stage 13.2

| TD ID | Priority | Status | Stage |
|-------|----------|--------|-------|
| TD-019 | P3 | on user hold | Stage 13+ |
| TD-028 | P2 | ✅ CLOSED (Stage 13.1) | — |
| TD-029 | P2 | open (deferred to Stage 13.1b) | Stage 13.1b |
| TD-030 | P0 | open | Stage 13.3 |
| **TD-031** | **P0** | **✅ CLOSED (Stage 13.2)** | — |
| TD-032 | P0 | open | Stage 13.4 |
| TD-033 | P1 | open | Stage 13.5+ |

**Stage 13.2 closed**: 1 P0 TD item (TD-031)
**Remaining open**: 5 TD items (1 P3-on-hold + 1 P2 + 2 P0 + 1 P1)

## 委员会投票: 5/5 GO → PASS

| Role | Vote | Reasoning |
|------|------|-----------|
| ARCH-A | GO | Strategy B reuses existing infrastructure; §16 compliant |
| DEV-A | GO | 11 conformance FAIL→PASS; 2 regression tests updated; 0 regressions |
| QA-A | GO | 5026 conformance + 2237 rust tests all green |
| ALG-C | GO | Desugar semantics match rustc; type system unaffected |
| SKL-A | GO | First user-facing feature in Stage 13; v0.22.0 minor bump justified |

## Version policy: v0.21.5 → v0.22.0 (minor bump)

Per `stage-13.1-design-alignment.md` §5.4 + semver §2.0.0:
- Stage 13.2 adds **first user-facing compiler feature** (if-let / while-let)
- Minor bump justified (new language feature, not just refactoring)
- v0.21.x patch bumps were for Stage 12 review + Stage 13.1 refactoring (no new features)
- v0.22.0 reserved for Stage 13.2-13.4 P0 closure — Stage 13.2 is the first P0 closure

## Next: Stage 13.3 (TD-030 closure call lowering — P0)

Per `plan-13.1.md` §2, Stage 13.3 is the next P0 closure target:
- TD-030: Closure call lowering incomplete (closures parse + capture but cannot be called)
- 41 conformance FAIL tests in `01-typecheck/03-closures/`, `02-borrowck/03-closure-capture/`, `04-e2e/03-closures/`
- Estimated: 2-3 weeks
- This is the **largest single P0 blocker** for v0.3 self-hosting

---

**审查完成**: 2026-07-26
**Stage 13.2 STATUS**: ✅ COMPLETE (TD-031 P0 CLOSED)
**Next**: Stage 13.3 (TD-030 closure call lowering — P0, 2-3 weeks)
