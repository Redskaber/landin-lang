# Stage 18.112 — S2 Fix: Method Monomorphization (Constant Func Operand)

> **Author**: redskaber
> **Date**: 2026-08-15
> **Version**: v0.379.0 → v0.380.0
> **Status**: Active

## 1. Root Cause

`writeback_fndef_substs` only handled `Copy/Move` func operands (regular function
calls). Method calls use `Operand::Constant(Const { ty: FnDef(def_id, []), ... })`
— the Constant path was skipped with `continue`.

## 2. Fix

Restructured `writeback_fndef_substs` to handle both operand types:
1. `Copy/Move(local)`: read FnDef type from `local_decls` (existing path)
2. `Constant(Const { ty: FnDef(def_id, substs) })`: read directly from Const's type (new path)

For Constant func operands, the inferred substs are written back to the
terminator's Constant (not to local_decls). This requires a separate
`terminator_changes` list and post-loop application.

## 3. Verification

- 640 lib + 2663 integration = 3303 unit tests, 0 failures, 0 skipped
- No regressions — method calls with non-generic methods work as before
- Generic method calls now get substs propagated to the Constant func operand

## 4. ALL Monomorphization Tech Debt Resolved

| Tech Debt | Status |
|-----------|--------|
| S2: method monomorphization | ✅ Stage 18.112 |
| S5: type_names pre-computed | ✅ Stage 18.104 |
| S6: nested Param return type | ✅ Stage 18.105 |
| S7: MonoItem collection skips Param/Error | ✅ Stage 18.106 |
| S8: call-site sig substitution | ✅ Stage 18.107 |
| S9: dest local type writeback | ✅ Stage 18.111 |
| S10: DivisionByZero assert skip | ✅ Stage 18.109 |
| S11: const_prop loop safety | ✅ Stage 18.110 |

**All monomorphization tech debt (S2-S11) is now resolved!**
