# Agent Team Workflow

> **Author**: redskaber
> **Date**: 2026-07-19
> **Version**: v0.1
> **Status**: Active

## Development Pipeline

```text
Source Code
    │
    ▼
[Frontend Agents]     Lexer → Parser → AST
    │
    ▼
[HIR Agents]          AST → HIR Lowering → Name Resolution
    │
    ▼
[MIR Agents]          HIR → MIR Lowering
    │
    ▼
[Type System Agents]  MIR → Type Check (unification + inference)
    │
    ▼
[Borrow Check Agents] MIR → NLL Borrow Check
    │
    ▼
[Codegen Agents]      MIR → LLVM IR (Emitter trait → TextEmitter/InkwellEmitter)
    │
    ▼
[Testing Agents]      Integration tests + Negative cases + Deep inspection
    │
    ▼
[Process Agents]      Stage Committee review → Approval
```

## Sub-stage Workflow (per §3)

1. **Complexity Assessment** (§3.1): L1/L2/L3, baseline rounds
2. **Inner Loop** (§3): Develop → Test → Review → Fix → Repeat
3. **Gate Review** (§9.3): Independent audit before next stage
4. **Committee Vote** (§5): 5-role weighted voting, ≥95% to pass

## Document Flow

- Plans: `docs/develop/v0/stage-N/plan.md`
- Dev logs: `docs/develop/v0/stage-N/dev-log.md`
- Gate reviews: `docs/develop/v0/stage-N/gate-review-roundN.md`
- Language design: `docs/lang-design/NN-topic.md`
- Process: `docs/stage-committee-process.md`
