# Stage 18.119 — D1-R2 Fix: BinaryOp2 Panic Instead of Silent Wrong Codegen

> **Author**: redskaber
> **Date**: 2026-08-15
> **Version**: v0.386.0 → v0.387.0
> **Status**: Active

## 1. Root Cause

`Rvalue::BinaryOp2` (range expressions like `start..end`) should never reach
codegen — they are desugared during MIR lowering. The old fallback used
`eprintln!` + returned `"0"`, silently producing wrong runtime output.

## 2. Fix

Replaced `eprintln!` + `"0"` with `panic!()` — if BinaryOp2 reaches codegen,
it's a compiler bug (MIR lower should have desugared it). Panicking surfaces
the bug immediately rather than producing incorrect output.

## 3. Design Rationale

- **Per §1.0 原則 4 "报错 > 静默"**: never silently produce wrong code
- **Per §2.0 原則 9 "正确 > 妥协"**: panic is better than wrong codegen
- The proper fix (codegen returning `CodegenResult<String>`) is deferred to
  v0.2 Phase 2 — it requires changing all codegen function signatures
- For now, panicking is the safest option: the user sees a clear error
  message rather than incorrect runtime behavior

## 4. Verification

- 640 lib + 2663 integration = 3303 unit tests, 0 failures, 0 skipped
- No regression — BinaryOp2 never reaches codegen in current test suite
