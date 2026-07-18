# Landin Stage Committee — Process & Voting Rules

> **Version**: 1.0 (effective from Stage 1.1)
> **Purpose**: Formalize the multi-round review process for stage progression
> with explicit voting mechanism, role responsibilities, and acceptance gates.

---

## 1. Stage Committee Members (5 voting roles)

Each role has a distinct review focus. A stage may only progress to the next
when ALL members vote APPROVED or APPROVED WITH MINOR CONCERNS, with at most
2 minor concerns total.

| # | Role | Focus Area | Key Questions |
| --- | ------ | ------------ | --------------- |
| 1 | **Compiler Engineer** | Parser/lexer/codegen correctness, AST/IR data flow, span tracking | Does the code compile clean? Are spans preserved? Any silent data loss? Any infinite-loop risks? |
| 2 | **Type System Theorist** | Type inference readiness, generics, lifetimes, trait resolution | Does the IR preserve all type info needed for Stage 2 typeck? Are generic args / where clauses / associated types captured? |
| 3 | **Soundness Reviewer** | Memory safety, `unsafe` tracking, borrow-check readiness | Can the IR mislead Stage 2 borrowck? Are `unsafe` blocks/fns/raw ptrs distinguishable? Any `panic!()` reachable from user input? |
| 4 | **Testing & QA Lead** | Test coverage, structural assertions, conformance, property tests | Are tests real structural walks (not smoke)? Are error-message-content + span-correctness tests present? Is conformance suite growing? |
| 5 | **Tooling & DX Lead** | CI, cargo fmt, cargo clippy, docs, build system, dev ergonomics | Is `cargo fmt --check` clean? Is `cargo clippy --all-targets -- -D warnings` clean? Are docs synchronized? Does CI cover push + PR? |

## 2. Voting Rules

Each member casts one of three votes:

- **APPROVED** — No concerns; the work is ready.
- **APPROVED WITH MINOR CONCERNS** — Acceptable, but with up to 2 specific
  minor issues that should be tracked (P2 level — cosmetic, doc drift, etc.).
- **NEEDS REVISION** — One or more P0/P1 issues block progression.

### Acceptance Gate

A stage may progress to the next stage iff:

1. **Zero** NEEDS REVISION votes (unanimous approval required), AND
2. **At most 2** APPROVED WITH MINOR CONCERNS votes (otherwise too many minor
   issues accumulate), AND
3. The minor concerns are documented in the worklog with a target resolution
   stage.

If the gate is NOT met, the work returns to the implementer for another
review-refine cycle (Round N+1) targeting the specific objections.

## 3. Per-Stage Process (mandatory rounds)

```
┌─────────────────────────────────────────────────────────────┐
│ Round 1: Internal task breakdown                            │
│   - Read prior stage's worklog                              │
│   - Decompose stage into 10-15 atomic tasks                 │
│   - Define acceptance criteria per task                     │
│   - Output: TODO list + task brief                          │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Rounds 2-N: Implementation                                  │
│   - Implement tasks in dependency order                    │
│   - Run cargo test + clippy + fmt after each batch         │
│   - Document deviations from plan                          │
│   - N is typically 3-4 rounds                              │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Round N+1: Self-critical review                             │
│   - Walk through each implemented task                     │
│   - Identify gaps, regressions, doc drift                  │
│   - Run all 5 reviewer perspectives in advance             │
│   - If gaps found → another implementation round           │
└─────────────────────────────────────────────────────────────┘
                            ↓
┌─────────────────────────────────────────────────────────────┐
│ Round N+2: Stage Committee review                           │
│   - Launch 5 parallel subagents (one per role)             │
│   - Each produces VERDICT + findings with severity         │
│   - Tally votes per §2 rules                               │
└─────────────────────────────────────────────────────────────┘
                            ↓
        ┌───────────────────┴───────────────────┐
        │                                       │
   [GATE MET]                             [GATE NOT MET]
        │                                       │
        ↓                                       ↓
┌─────────────────────┐            ┌─────────────────────────┐
│ Round N+3: Commit   │            │ Round N+3: Refine       │
│  - Bump version     │            │  - Fix P0/P1 issues     │
│  - Update worklog   │            │  - Add regression tests │
│  - git commit       │            │  - Re-run self-review   │
│  - Start next stage │            │  - Re-submit to Cmte    │
└─────────────────────┘            └─────────────────────────┘
```

### Mandatory minimum: 4 rounds, maximum: 7 rounds

- **Minimum 4 rounds**: 1 plan + 2 impl + 1 committee review
- **Maximum 7 rounds**: 1 plan + 4 impl/refine + 1 self-review + 1 committee
- If the gate is not met after 7 rounds, escalate to user (likely scope
  mismatch — the stage may need to be split smaller)

## 4. Worklog Protocol

Each round appends a section to `/home/z/my-project/worklog.md`:

```markdown
---
Task ID: stage-X.Y-round-N
Agent: <agent name / role>
Task: <what this round accomplished>

Work Log:
- <concrete step 1>
- <concrete step 2>

Stage Summary:
- <key results>
- <test count + warnings>
- <vote tally if committee round>
```

## 5. Stage Numbering

- **Stage 0.x** — Front-end (lexer/parser/AST) — COMPLETE at v0.1.4
- **Stage 1.x** — HIR + Name Resolution (this stage)
  - 1.1 — HIR data structures + deferred AST schema fixes
  - 1.2 — AST → HIR lowering
  - 1.3 — Module-level name resolution
  - 1.4 — Scope-based name resolution
- **Stage 2.x** — Type check + Borrow check (NLL on MIR)
- **Stage 3.x** — LLVM codegen
- **Stage 4.x** — Macro system + attributes
- **Stage 5.x** — mini-cargo + stdlib MVP

Each sub-stage (1.1, 1.2, etc.) goes through the full multi-round process
independently. A sub-stage passes the gate → bump patch version → start next.

## 6. Version Policy

- `v0.1.x` — Stage 0 (front-end)
- `v0.2.x` — Stage 1 (HIR + name resolution)
- `v0.3.x` — Stage 2 (typeck + borrowck)
- ...
- `v1.0` — All stages complete + conformance suite passing

Each sub-stage that passes the gate increments the patch version
(e.g. v0.2.0 → v0.2.1 after Stage 1.1).

---

**This document is the single source of truth for the Landin development
process. All agents (main + subagents) must follow it.**
