# Stage 14.63 — LLVMSysEmitter Forward Declaration Deduplication + ZST Representation

> **Author**: redskaber
> **Date**: 2026-07-29
> **Version**: v0.79.0
> **Process**: stage-committee-process.md v3.22 §11.3 (LLVM doc sync)

## 1. Forward Declaration Deduplication

### Problem

When mutually recursive functions were emitted, `LLVMSysEmitter::emit_function_begin`
called `LLVMAddFunction` without checking whether a forward declaration already existed
in the module. The sequence was:

1. `emit_function_begin("is_even")` → `LLVMAddFunction("is_even", ...)` creates the function
2. Inside `is_even`'s body, `emit_call("is_odd")` → `get_or_declare_function("is_odd")`
   → `LLVMAddFunction("is_odd", ...)` creates a forward declaration
3. `is_even`'s body finishes
4. `emit_function_begin("is_odd")` → `LLVMAddFunction("is_odd", ...)` creates ANOTHER
   function with the same name. LLVM silently renames it to `is_odd.1`.

Result: `nm` on the object file shows:
```
                 U landin_is_odd         <-- undefined (called from is_even)
0000000000000070 T landin_is_odd.1       <-- the actual definition, renamed
```

The linker reports `undefined reference to 'landin_is_odd'`.

### Fix

`emit_function_begin` now checks both:
1. The `self.declared` cache (Rust-side HashMap)
2. `LLVMGetNamedFunction(self.module, name)` (LLVM-side lookup)

If either returns an existing function value, AND it's a `FunctionValueKind` with
matching type, we reuse that function value instead of calling `LLVMAddFunction`.

```rust
let existing = if let Some(v) = self.declared.get(name) {
    Some(*v)
} else {
    let v = LLVMGetNamedFunction(self.module, name_c.as_ptr());
    if !v.is_null() { Some(v) } else { None }
};
let fn_val = if let Some(existing) = existing {
    let existing_type = LLVMGlobalGetValueType(existing);
    let existing_kind = LLVMGetValueKind(existing);
    if existing_kind == llvm_sys::LLVMValueKind::LLVMFunctionValueKind
        && existing_type == fty
    {
        existing  // Reuse the existing forward declaration
    } else {
        LLVMAddFunction(self.module, name_c.as_ptr(), fty)  // Signature mismatch — fall back
    }
} else {
    LLVMAddFunction(self.module, name_c.as_ptr(), fty)  // New function
};
self.declared.insert(name.to_string(), fn_val);
```

### Why this works

LLVM allows appending basic blocks to an existing function value. The forward
declaration is just a function with no body — adding basic blocks turns it into
a definition, in-place. No rename, no duplicate symbol.

### Per §1.0 原则 5 "报错 > 静默"

Signature mismatch now falls back to `LLVMAddFunction` (which renames) rather than
silently miscompiling. This surfaces the issue rather than hiding it.

---

## 2. Zero-Size Type (ZST) Representation

### Problem

Zero-field structs (`struct Unit;`) were mapped to `EmitType::Void` in
`mir_type_to_emit_type_with_layouts`. This caused three cascading problems:

1. `landin_new` had signature `void @landin_new()` — no return value
   - LLVM's `void` return type means the function returns NOTHING; the call site
     cannot capture a return value.
2. The local `u` (type `Unit` → `Void`) was skipped in the alloca loop
   (`if ty == EmitType::Void { continue; }`) — no storage was allocated.
3. `u.value()` had no alloca to take `&u` from — codegen passed `i32 0` as `&self`.
   - The function expected `ptr` (the `&self` type), but received `i32 0`.
   - LLVM module verification failed: `Call parameter type does not match function
     signature! i32 0, ptr %v1 = call i32 @landin_value(i32 0)`

### Fix

Changed zero-field struct Adt case from `EmitType::Void` to `EmitType::Struct(vec![])`:

```rust
TyKind::Adt(def_id, _substs) => match layouts.get(def_id) {
    Some(AdtLayout::Struct { field_tys }) => {
        if field_tys.is_empty() {
            EmitType::Struct(vec![])  // Was: EmitType::Void
        } else {
            // ... non-empty struct handling
        }
    }
    // ... enum handling
}
```

LLVM's `LLVMStructTypeInContext(ctx, [], 0, 0)` produces `{}` — an empty struct
with size 0 but a real value type (NOT `void`). This means:

- `landin_new` returns `{}` (an empty struct value, valid to capture)
- The local `u` gets `alloca {}` (valid, zero-size allocation)
- `&u` is the alloca pointer (valid `ptr` to pass as `&self`)

### Why `Struct(vec![])` instead of `Void`?

LLVM's `void` type is special — it can ONLY appear as a function return type, and
a function returning `void` cannot have its return value captured. This makes it
unsuitable for representing ZSTs that flow through values (locals, arguments,
return values).

`{}` (empty struct) is a regular first-class type:
- Can be stored in allocas
- Can be passed as function arguments
- Can be returned from functions and captured at call sites
- Has size 0 (no storage cost)

This matches Rust's representation: ZSTs like `()` and `struct Unit;` are real
types with values, just zero-size. They're not "void" in the C sense.

### Why not change `()` (unit tuple) too?

The empty tuple `()` (TyKind::Tuple(vec![])) is also currently mapped to
`EmitType::Void`. Changing it would be a larger refactor because:

1. Functions returning `()` are everywhere — they all become `{} @f()` instead
   of `void @f()`, requiring call site updates.
2. The `if ty == EmitType::Void { continue; }` skip in the alloca loop would no
   longer skip `()` locals — they'd all get `alloca {}`.
3. Existing test assertions would need updating.

The asymmetry (Unit → `{}`, `()` → `void`) is acceptable because:
- `()` is never used as a value (you can't write `let x: () = foo(); x.method()`)
- `Unit` (user-defined ZST) IS used as a value (it has methods via `impl Unit`)

Future work: unify both to `Struct(vec![])` for consistency, but that's a
separate refactor with broader impact.

### Per §1.0 原则 6 "通用 > 特例"

Zero-field structs now use the same code path as non-empty structs (just with
zero fields), instead of a special `Void` case. This eliminates the special-case
handling that was causing the bug.

---

## 3. Verification

### Before fix

```
$ ./landin-stage0 --run test.lin
Call parameter type does not match function signature!
i32 0
 ptr  %v1 = call i32 @landin_value(i32 0)
error: object file generation failed: LLVM module verification failed
```

### After fix

```
$ ./landin-stage0 --run test.lin
info: object file written to /tmp/landin_run_xxx.o
info: executable written to /tmp/landin_run_xxx.out
info: running /tmp/landin_run_xxx.out
42
```

### LLVM IR (after fix)

```llvm
define {} @landin_new() {
  %loc_0 = alloca {}
  ...
  ret {} undef
}

define i32 @landin_value(ptr %arg0) {
  ...
  ret i32 42
}

define i32 @landin_main() {
  %loc_u = alloca {}
  ...
  call {} @landin_new()
  %v = call i32 @landin_value(ptr %loc_u)
  ...
}
```

Note: `landin_new` returns `{}` (not `void`), `%loc_u` is `alloca {}` (not skipped),
and `landin_value` receives `ptr %loc_u` (the alloca pointer, not `i32 0`).

---

## 4. References

- LLVM Language Reference: [Struct Type](https://llvm.org/docs/LangRef.html#structure-type)
- LLVM Language Reference: [Void Type](https://llvm.org/docs/LangRef.html#void-type)
- LLVM API: `LLVMStructTypeInContext`, `LLVMGetNamedFunction`, `LLVMGlobalGetValueType`
- Rust Reference: [Zero-Sized Types](https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts)
- Landin process: `docs/stage-committee-process.md` v3.22 §11.3 (LLVM doc sync)
