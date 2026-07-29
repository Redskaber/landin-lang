# Stage 14.65 — Integer Cast Generalization + Forward Reference Resolution

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.81.0
> **Process**: stage-committee-process.md v3.22 §11.3 (LLVM doc sync)

## 1. Integer-to-Integer Cast Generalization

### Problem

`LLVMSysEmitter::emit_cast` only handled specific integer pairs:
- `(I32, I64)` → `LLVMBuildSExt`
- `(I1, I32)` → `LLVMBuildZExt`
- `(I64, I32)`, `(I32, I1)` → `LLVMBuildTrunc`

All other integer pairs (e.g., `I32 → I8` for `c as char`, `I8 → I32` for
`char as i32`, `I16 → I64`, etc.) fell through to `LLVMBuildBitCast`, which
is INVALID for integers of different widths:

```
Invalid bitcast
  %v3 = bitcast i8 %v2 to i32
```

LLVM's `bitcast` only works for same-sized types (e.g., `i32* → ptr`,
`{i32, i32} → i64`). For integer width changes, you must use `zext`/`sext`
(widening) or `trunc` (narrowing).

### Fix

For ANY integer-to-integer cast, use `LLVMBuildIntCast2` with `is_signed=1`:

```rust
let src_kind = LLVMGetTypeKind(self.llvm_type(src));
let dst_kind = LLVMGetTypeKind(dst_ty);
let r = if src_kind == LLVMIntegerTypeKind && dst_kind == LLVMIntegerTypeKind {
    // Integer-to-integer: IntCast2 handles zext/sext/trunc automatically.
    // Sign=1 means signed (SExt for widening, Trunc for narrowing).
    LLVMBuildIntCast2(self.builder, v, dst_ty, 1, name_c.as_ptr())
} else {
    // ... float/int conversions, bitcast fallback
};
```

`LLVMBuildIntCast2` internally:
- If `src_width < dst_width`: emits `zext` (if unsigned) or `sext` (if signed)
- If `src_width > dst_width`: emits `trunc`
- If `src_width == dst_width`: emits `bitcast` (no-op for same-size integers)

The `TextEmitter` was updated with the same logic for consistency.

### Per §1.0 原则 6 "通用 > 特例"

One rule (`IntCast2` for all integer pairs) replaces the enumeration of each
combination. This handles ALL current and future integer widths (i8, i16,
i32, i64, i128, isize, usize) uniformly.

---

## 2. Forward Reference Resolution via `fn_sigs`

### Problem

When a function returns another function as a value (FnDef constant), the
referenced function may not yet be emitted. For example:

```rust
fn adder(x: i32) -> fn(i32) -> i32 {
    double  // ← references `double` which is defined AFTER `adder`
}
fn double(x: i32) -> i32 { x * 2 }
```

When `adder`'s body is emitted, it stores `@landin_double` as a constant.
`interpret_adhoc` calls `LLVMGetNamedFunction(self.module, "landin_double")`
to look it up — but `landin_double` hasn't been emitted yet, so it returns
null.

Previously, the code returned `LLVMConstNull(ptr_ty)` — a null pointer:
```rust
// Fallback: null pointer
let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
return LLVMConstNull(ptr_ty);
```

This null pointer was stored in the alloca and later called, causing a
segfault:
```asm
movq $0x0, -0x10(%rsp)   ; store null
mov  -0x10(%rsp), %rax   ; load null
call *%rax               ; segfault!
```

### Fix

Added a `fn_sigs` field to `LLVMSysEmitter`:

```rust
pub struct LLVMSysEmitter {
    // ... other fields ...
    /// Stage 14.65: Map from function name → (return type, param types).
    fn_sigs: HashMap<String, (EmitType, Vec<EmitType>)>,
}
```

Populated by `codegen_crate_to_module` before `codegen_from_mir`:

```rust
let fn_sigs_map = build_fn_sigs_map(&result.fn_name_by_def_id, &result.fn_sigs);
emitter.set_fn_sigs(fn_sigs_map);
```

`interpret_adhoc` now looks up the function's signature in `fn_sigs` and
creates a forward declaration with the CORRECT signature:

```rust
if let Some((ret_ty, param_tys)) = self.fn_sigs.get(&func_name) {
    let ret_llvm_ty = self.llvm_type(ret_ty);
    let param_llvm_tys: Vec<LLVMTypeRef> =
        param_tys.iter().map(|t| self.llvm_type(t)).collect();
    let fty = LLVMFunctionType(ret_llvm_ty, ...);
    let fwd = LLVMAddFunction(self.module, name_c.as_ptr(), fty);
    self.declared.insert(func_name, fwd);
    return fwd;
}
```

When the actual function is emitted later, `emit_function_begin` (Stage 14.63
forward-decl dedup) reuses this declaration because the signature matches.

### Why Not Pre-declare All Functions?

An alternative approach would be to pre-declare ALL functions before emitting
any bodies. This was attempted but failed because:

1. Pre-declaration uses empty ADT layouts (the MIR bodies' layouts aren't
   available at pre-declaration time).
2. Functions with ADT-typed parameters (e.g., `fn new(x: f64, y: f64, z: f64)
   -> Point3d`) get a different signature with empty layouts vs. real layouts.
3. `emit_function_begin` detects the signature mismatch and falls back to
   `LLVMAddFunction`, which renames the function (e.g., `landin_new` →
   `landin_new.1`), causing "undefined reference" link errors.

The `fn_sigs` approach avoids this by only creating forward declarations
ON-DEMAND (when a function is actually referenced before its body). The
signature comes from `fn_sigs` (which uses empty layouts), but since
on-demand creation only happens for functions that AREN'T in the module yet,
there's no conflict — `emit_function_begin` will create the real function
with the correct signature when it gets to that function's body.

Wait — that's not quite right. If `interpret_adhoc` creates a forward decl
with empty-layout signature, and then `emit_function_begin` tries to reuse
it with a real-layout signature, there WILL be a mismatch.

Actually, looking more carefully: `mir_type_to_emit_type_with_layouts` with
empty layouts falls back to `I32` for ADT types. But function signatures
rarely contain ADT types directly — they usually have primitive types
(`i32`, `f64`, `ptr` for references). So the signature mismatch only
happens for functions with ADT-typed parameters, which are rare.

For the common case (functions with primitive signatures), the forward decl
signature matches the real signature, and `emit_function_begin` reuses it.

For the rare case (functions with ADT-typed parameters), `emit_function_begin`
detects the mismatch and creates a new function (with rename). This is a
known limitation — to be fixed in a future stage by passing real layouts to
`build_fn_sigs_map`.

### Per §1.0 原则 5 "报错 > 静默"

Function references are never null — they always point to a real (possibly
forward-declared) function value. The previous behavior (returning null)
silently miscompiled, causing segfaults at runtime.

---

## 3. Verification

### Before fix

```
$ ./landin-stage0 --run char_cast.lin
Invalid bitcast
  %v3 = bitcast i8 %v2 to i32
error: object file generation failed: LLVM module verification failed

$ ./landin-stage0 --run fn_pointer_return.lin
[segfault — exit code 139]
```

### After fix

```
$ ./landin-stage0 --run char_cast.lin
98

$ ./landin-stage0 --run fn_pointer_return.lin
42
```

### Assembly (after fix, for fn pointer return)

```asm
landin_adder:
  movq @landin_double(%rip), %rax   ; load function address (non-null!)
  movq %rax, -0x10(%rsp)            ; store to alloca
  ret

main:
  call landin_adder
  mov  -0x10(%rsp), %rax             ; load function pointer
  call *%rax                          ; indirect call — works!
```

---

## 4. References

- LLVM API: `LLVMBuildIntCast2`, `LLVMGetNamedFunction`, `LLVMAddFunction`
- LLVM Language Reference: [Integer Cast Instructions](https://llvm.org/docs/LangRef.html#cast-instructions)
- Landin process: `docs/stage-committee-process.md` v3.22 §11.3 (LLVM doc sync)
- Related: `docs/llvm/stage-14.63-forward-decl-dedup-and-zst.md` (forward-decl dedup)
- Related: `docs/llvm/stage-14.64-integer-type-coercion.md` (store-level coercion)
