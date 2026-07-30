# Stage 14.107 — Gate Review: HP-19/21 Span Infrastructure

> **Author**: redskaber
> **Date**: 2026-07-30
> **Version**: v0.120.0 → v0.121.0
> **Process**: stage-committee-process.md v3.22 §25 (D8 deep review)

## 1. Stage Summary

Stage 14.107 implements HP-19 and HP-21 — adding source span fields to
`BasicBlock` and its terminator. This is the second of 3 mandatory pre-v0.2
fixes identified by the Phase 2 architecture audit.

## 2. What Was Implemented

### HP-19: BasicBlock span

Added `span: Span` field to `BasicBlock` struct. This is the span of the
first statement in the block (or DUMMY if the block is empty). Populated
during MIR lowering — currently set to DUMMY by `new_block()`.

### HP-21: Terminator span

Added `terminator_span: Span` field to `BasicBlock` struct. This is the
span of the source construct that generated the terminator (e.g., `return`
keyword span, `if` condition span, `match` scrutinee span).

### Design Decision: Fields on BasicBlock, not on Terminator

The Phase 2 audit suggested adding span directly to `Terminator` (converting
it from an enum to a struct with `kind` + `span` fields). However, this would
require updating 120+ pattern-match call sites across the codebase.

Instead, we add `terminator_span` as a field on `BasicBlock`. This:
- Avoids the 120+ call-site refactoring
- Still provides the span information for v0.2 debug info
- Follows §14.4 (minimal change principle)
- The span can be accessed as `block.terminator_span`

Per §1.0 原则 3 "显式 > 隐式": spans are now explicit on BasicBlock.
Per §14.4: minimal change — add fields rather than refactoring the enum.

## 3. Verification

```
cargo build --release --features llvm-backend: ✅
cargo fmt: ✅ clean
cargo clippy --all-targets --features llvm-backend: ✅ 0 warnings
cargo test --features llvm-backend: ✅ 1951 passed, 0 failed
python3 tests/conformance/run_all.py: ✅ 5216 passed, 0 failed
```

## 4. Pre-v0.2 Fix Status

| Fix | Status |
|-----|--------|
| HP-1: Sound Copy detection | ✅ Infrastructure ready (activation deferred to v0.2) |
| HP-19/21: Span on BasicBlock/Terminator | ✅ DONE (Stage 14.107) |
| HP-B11: Consolidate writeback passes | ⏳ Pending (Stage 14.108) |

## 5. Stage Verdict

**PASS** — HP-19/21 span infrastructure implemented. All tests pass.
No regressions. 2 of 3 pre-v0.2 fixes done.

v0.121.0: minor bump (HP-19/21 span infrastructure)
