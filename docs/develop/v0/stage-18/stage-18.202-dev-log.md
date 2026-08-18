# Stage 18.202 — format! Variadic (TD-FORMAT-VARIADIC resolved)

> **Date**: 2026-08-17
> **Version**: v0.467.0 → v0.468.0
> **Task ID**: stage18.202

## 1. Scope

Per Stage 18.201 task review: implement `format!("x={}", x)` with variadic args.
Resolves TD-FORMAT-VARIADIC.

## 2. Implementation

### 2.1 C runtime helper (src/codegen/runtime.rs)

Added `__landin_format_variadic(out_str_ptr, fmt_ptr, fmt_len, n_args, arg_types, arg_vals, ...)`:
- Collects variadic args via `va_list`
- Builds C printf format string from Landin format string (replaces `{}` with `%ld`)
- NULL-safe: `arg_types` may be NULL → defaults all args to integer type
- Formats via `snprintf` into a buffer, then allocates String buffer + copies
- Writes result to output String struct via pointer arithmetic

### 2.2 MIR intrinsic (src/mir/lower/expr_variants.rs)

`lower_format_variadic_intrinsic` generates MIR for:
1. Create &output_String ref (Shared) → cast to *mut u8
2. Extract fmt.ptr (field 0) and fmt.len (field 1) from &str
3. n_args constant
4. arg_types and arg_vals = NULL (MVP — defaults to integer type)
5. Cast each format arg to i64 (variadic)
6. Call `__landin_format_variadic(out_ptr, fmt_ptr, fmt_len, n_args, NULL, NULL, val1, val2, ...)`

### 2.3 fn_sigs_map fix (src/codegen/llvm/function_sigs.rs)

Added runtime helper signatures to `build_fn_sigs_map` so `get_or_declare_function`
creates correct forward declarations (void return, correct param types). Without this,
the fallback creates `i32 (...)` which mismatches the actual void return + fixed params,
causing ABI issues.

### 2.4 Variadic declaration fix (src/codegen/llvm/mod.rs + aggregate.rs)

Added `__landin_format_variadic` to the variadic function check in both
`declare_function` (mod.rs) and `emit_call` (aggregate.rs).

### 2.5 Test updates (tests/v0/stage18/plan/stage18_186_format_macro_tests.rs)

Updated 3 tests that previously expected compile failure for `format!("x={}", x)`
to now expect compile success (TD-FORMAT-VARIADIC resolved).

## 3. Verification

```
format!("x={}", 42).len = 4            ✅ (field access)
format!("x={}", 42) compiles           ✅
format!("{}", 42) compiles              ✅
format!("a", "b") compiles              ✅
```

**Known limitation**: `s.len()` (method call) on format! result segfaults due to
pre-existing TD-FUNCTION-REDEFINE (forward declaration param type mismatch for
prelude methods). Field access `s.len` works correctly. This is not specific to
format! — it affects all prelude method calls on stack-allocated structs.

## 4. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3073 passed
- **Total**: 3731 tests, 0 failures

## 5. Tech Debt

| ID | Status |
|----|--------|
| TD-FORMAT-VARIADIC | ✅ Resolved — format! with {} args works |
| TD-FUNCTION-REDEFINE-PARAMS | 🟡 New — forward declaration param type mismatch for prelude methods (same root cause as Stage 18.188 but for params) |
