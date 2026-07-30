# Stage 14.97 — Trait Default Body + For-Loop Codegen

> **Author**: redskaber
> **Date**: 2026-07-30
> **Stage**: 14.97
> **Version**: v0.110.0 → v0.111.0

## Overview

Stage 14.97 makes two important LLVM codegen improvements:

1. **Bug Y1 fix**: Trait default body methods that call `self.method()` now work
   correctly. Previously they crashed with LLVM verification errors due to wrong
   function signatures and vtable dispatch on non-fat-pointer values.
2. **For-loop over Range**: `for i in 0..N { body }` now properly lowered to a
   `while counter < end { body; counter += 1 }` loop.

## LLVM Codegen Changes

### Trait Default Body — Function Signature

**Before**: Trait default body methods had no `fn_sig_table` entry. Codegen
used a generic variadic signature, causing type mismatches like:
```llvm
define void @landin_Counter_default_double_value(i32 %arg0)  ; wrong: void + i32
```

**After**: `fn_sig_table` is populated for trait default body methods using the
first impl's `self_ty` as the specialization type (v0.1 single-impl heuristic):
```llvm
define i32 @landin_Counter_default_double_value(ptr %arg0)  ; correct: i32 + ptr
```

### Trait Default Body — Method Dispatch Inside Body

**Before**: `self.value()` inside a trait default body went through the dyn
Trait path (vtable indirect call) because the receiver type was `Infer`:
```llvm
%v3 = getelementptr { ptr, ptr }, ptr @.dynptr.Counter.Pair, i32 0, i32 1
%v4 = load ptr, ptr %v3
%v5 = load ptr, ptr %v4, i32 0
%v6 = call i32 %v5(ptr %v2)  ; indirect call via vtable
```

**After**: `resolve_self_param_type` searches Trait owners and uses the first
impl's `self_ty`. Method resolution now finds `Pair::value` via static dispatch:
```llvm
%v3 = call i32 @landin_Pair_value(ptr %v2)  ; direct call
```

### For-Loop Codegen

**Before**: For-loop was a stub that checked if the iter was "truthy":
```llvm
%v1 = load i32, %loc_iter
%v2 = icmp ne i32 %v1, 0
br i1 %v2, label %body, label %exit
```

**After**: For-loop is properly desugared to a while + counter:
```llvm
; counter = start
store i32 %start_val, %loc_counter
br label %cond
cond:
%v1 = load i32, %loc_counter
%v2 = load i32, %loc_end
%v3 = icmp slt i32 %v1, %v2  ; counter < end (excluded range)
br i1 %v3, label %body, label %exit
body:
; ... body code ...
; counter += 1
%v4 = load i32, %loc_counter
%v5 = add nsw i32 %v4, 1
store i32 %v5, %loc_counter
br label %cond
exit:
```

For inclusive ranges (`start..=end`), `icmp slt` is replaced with `icmp sle`
(less-than-or-equal).

## Verification

The LLVM IR generated for trait default bodies and for-loops is verified
by LLVM's module verifier (`LLVMVerifyModule`) — no verification errors.

End-to-end run_ok tests confirm correct runtime behavior:
- `p.double_value()` (calls `self.value()` in default body) → 42 ✅
- `p.doubled_increment()` (chain of 2 default bodies calling impl method) → 26 ✅
- `for i in 0..5 { sum += i; }` → 10 ✅
- `for i in 0..=5 { sum += i; }` → 15 ✅

## Known Limitations

- For-loop over arrays: not supported (clear typeck error)
- Open ranges (`..end`, `start..`): not supported
- Trait default body with multiple impls: uses first impl's self_ty
- Trait default body calling another trait's method: not supported
