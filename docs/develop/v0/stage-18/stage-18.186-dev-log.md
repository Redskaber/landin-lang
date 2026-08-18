# Stage 18.186 — format! Macro MVP (TD-FORMAT-MACRO)

> **Date**: 2026-08-17
> **Version**: v0.453.0 → v0.454.0
> **Task ID**: stage18.186
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.185 (String intrinsics)
> **Blocks**: Stage 18.187 (format! with args)

## 1. Scope

Implement `format!` macro MVP. Per Stage 18.181 task review plan, this
was the last item in the heap/String/format! chain.

**MVP scope**: `format!("literal")` only — no `{}` placeholders.
**Deferred**: `format!("x={}", x)` with args (TD-FORMAT-VARIADIC, Stage 18.187+).

## 2. Dependency Audit

Per user directive (依赖与基础设施完整能力审查):
- ✅ String::from_str intrinsic (Stage 18.185)
- ✅ __landin_alloc + __landin_memcpy runtime stubs
- ✅ format! macro expansion (already exists, expands to __landin_format call)
- ✅ String type (Stage 18.180)
- 🟡 __landin_format runtime stub — NOT needed (intercepted in MIR)

**Conclusion**: Dependencies complete. Chose **方案 B (MIR intrinsic)** over
方案 A (C runtime stub) to avoid C struct-by-value ABI complexity and reuse
String::from_str pattern (通解).

See `docs/develop/v0/stage-18/stage-18.186-dep-audit.md` for full audit.

## 3. Implementation

### 3.1 MIR Intrinsic (src/mir/lower/expr_variants.rs)

Added interception in `lower_call_expr` for `__landin_format` calls:

```rust
if name == "__landin_format" && args.len() == 1 {
    // format!("literal") → String::from_str(literal)
    return lower_string_from_str_intrinsic(cx, expr, arg_locals[0]);
}
if name == "__landin_format" && args.len() > 1 {
    // format!("x={}", x) — not yet supported.
    cx.type_errors.push(TypeError::new(
        "format! with format arguments ({}) is not yet supported (TD-FORMAT-VARIADIC, Stage 18.187+)",
        expr.span,
    ));
    // Emit a placeholder to avoid crash.
    ...
}
```

Per §1.0 原則 6 (通解>特例): reuse String::from_str intrinsic.
Per §2 原則 9 (正确>妥协): MVP is a temporary compromise for literals.

### 3.2 Why not C runtime stub?

**方案 A** (C stub `__landin_format`):
- C function returning `struct String { ptr, len, cap }` by value → sret ABI
- C doesn't know Landin's String layout → fragile coupling
- Variadic args lose type info in C

**方案 B** (MIR intrinsic, chosen):
- Reuses Stage 18.185's `lower_string_from_str_intrinsic` (alloc + memcpy)
- Type info preserved at MIR level
- No new C stub needed (uses existing __landin_alloc + __landin_memcpy)
- Per §1.0 原則 6 (通解>特例): one pattern for all String construction

## 4. Verification

```
format!("hello").len() = 5       ✅
format!("").len() = 0            ✅
format!("Hello, World!").len = 13 ✅
format!("x={}", x) → clean error ✅ (TD-FORMAT-VARIADIC)
format!("{}", 42) → clean error  ✅
format!("a", "b") → clean error  ✅
```

## 5. Tests

### 5.1 New Tests (tests/v0/stage18/plan/stage18_186_format_macro_tests.rs)

8 tests (5 positive + 3 negative):
- Positive: literal length, empty, field access, with methods, owned independent
- Negative: format with args fails, placeholder only fails, multiple args fails

All 8 pass.

## 6. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 1 warning (unused)
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3035 passed (was 3027, +8 new)
- **Total**: 3693 tests, 0 failures

## 7. Tech Debt Status

| ID | Status |
|----|--------|
| TD-FORMAT-MACRO | 🟡 Partial — literal format! done; variadic args deferred |
| TD-FORMAT-VARIADIC | 🟡 New — format! with {} args (Stage 18.187+) |
| TD-STRING-INTRINSICS | 🟡 Partial — from_str + len done; as_str + push_str deferred |
| TD-STRING-AS-STR-ALIAS | ✅ Resolved (Stage 18.180) |
| TD-HEAP-ALLOC | ✅ Resolved (Stage 18.178) |
| TD-ARRAY-INDEX-CODEGEN | ✅ Resolved (Stage 18.182) |
| TD-FAT-PTR-INDEX-PROJ | ✅ Resolved (Stage 18.183) |
| TD-STR-METHODS-RUNTIME | ✅ Resolved (Stage 18.184) |

## 8. Heap/String Chain Summary

The complete heap allocation → Box → String → str methods → format! chain
(Stage 18.177-18.186, 10 stages) is now functionally complete:

| Stage | What | Status |
|-------|------|--------|
| 18.177 | Task review (String=&str divergence) | ✅ |
| 18.178 | heap alloc infrastructure + 6 bug fixes | ✅ |
| 18.179 | Box<T> MVP | ✅ |
| 18.180 | Real String type | ✅ |
| 18.181 | Base types audit + re-plan | ✅ |
| 18.182 | Array index codegen fix (P0) | ✅ |
| 18.183 | Fat pointer Index projection (P1) | ✅ |
| 18.184 | str methods runtime fix (P1) | ✅ |
| 18.185 | String intrinsics (from_str + len) | ✅ |
| 18.186 | format! macro MVP (literal) | ✅ |

## 9. Next Steps

Stage 18.187: Deep review §14.5 D1-D8
- This is the end-of-chain deep review per `docs/stage-committee-process.md` §14.5
- Audit the heap/String chain for architecture health, tech debt, test coverage
- Decide if v0.2 P1 feature work can continue or if refactor is needed
