# Stage 15.42 — Test Plan: Drop Elaboration Design Doc

> **Date**: 2026-08-01
> **Version**: v0.167.0 → v0.168.0
> **Process**: stage-committee-process.md v3.23 §17 + §13.4
> **Develop doc**: `docs/develop/v0/stage-15/stage-15.42-drop-elaboration-design.md`

## 1. Test Scope

Stage 15.42 is a **design-only stage** — no code changes. The test plan
verifies that the design document exists and covers the required topics.

| Area | Test type | Count |
|------|-----------|-------|
| Design doc existence + coverage | Manual review | 1 (checklist) |
| Regression (existing tests) | All | 208 lib + 2076 integration + 5216 conformance |

## 2. Design Doc Coverage Checklist

The design doc (`docs/lang-design/25-drop-elaboration.md`) must cover:

- [x] **Problem statement**: What's broken (no user-defined Drop).
- [x] **Current state**: `TerminatorKind::Drop` is a no-op, `drop_elaboration` removed.
- [x] **Design**: `needs_drop` analysis, drop insertion, drop glue codegen, drop order.
- [x] **Migration strategy**: 6 stages (15.42-15.47).
- [x] **Dependencies**: Task 7 (NLL) — COMPLETE, Task 1 (Ty interning) — COMPLETE, TraitResolver — EXISTS.
- [x] **Testing strategy**: unit + integration + conformance.
- [x] **API naming compliance (§23)**: `ty_needs_drop`, `elaborate_drops`, `emit_drop_glue`.
- [x] **Open questions**: field type traversal, block splitting, naming, `move` interaction.
- [x] **Effort estimate**: 3-5 days.

## 3. Regression Test Strategy

### 3.1 No regression expected

Stage 15.42 adds a design doc only — no code changes. All 208 lib tests +
2076 integration tests + 5216 conformance tests must pass unchanged.

### 3.2 Verification

```bash
# Build (should be unchanged)
cargo build --features llvm-backend

# Run tests (should be unchanged)
cargo test --features llvm-backend

# Run conformance (should be unchanged)
python3 tests/conformance/run_all.py
```

## 4. Expected Results

- **Lib tests**: 208/208 PASS (zero regression)
- **Integration tests**: 2076/2076 PASS (zero regression)
- **Conformance tests**: 5216/5216 PASS (zero regression)
- **Clippy**: 0 warnings
- **Fmt**: clean
- **Design doc**: exists and covers all required topics
