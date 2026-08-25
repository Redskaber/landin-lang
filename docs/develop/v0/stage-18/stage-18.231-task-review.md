# Stage 18.231 — Task Review: v0.2.5g `__landin_format_variadic` → MIR Intrinsic Migration

> **Date**: 2026-08-23
> **Version**: v0.479.0 → v0.480.0 (planned)
> **Task ID**: stage18.231
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)

## 1. 触发场景

Per Stage 18.230 (v0.2.5f): `__landin_string_push_str` migrated to MIR intrinsic.
Per 06-mir.md §16.6:
> v0.2.5g: 迁移 __landin_format_variadic → MIR intrinsic (最复杂) ← Stage 18.231 (next)

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 4th (final) of 4 C helpers.

## 2. 依赖与基础设施完整能力审查 (per user directive)

### 2.1 Dependency Audit

| Dependency | Status | Notes |
|-----------|--------|-------|
| MIR Load/GEP/Store codegen | ✅ Stage 18.227 | |
| MIR SwitchInt for conditional logic | ✅ Stage 3.x | |
| MIR back-edge loops | ✅ Stage 18.230 | First used in string_push_str growth loop |
| `__landin_realloc` primitive (§16.5) | ✅ Stage 18.194 | For dynamic buffer growth |
| `__landin_memcpy` primitive (§16.5) | ✅ Stage 18.185 | For byte copy |
| `__landin_alloc` primitive (§16.5) | ✅ Stage 18.185 | For initial buffer allocation |
| **`__landin_i64_to_str` primitive** | ❌ **MISSING** | **Needed for integer→string conversion** |
| StatementKind::Store + Deref codegen | ✅ Stage 18.229 | |
| push_statement API | ✅ Stage 18.229 | |
| Mutable PHI-like locals | ✅ Stage 18.229 | |

### 2.2 Critical Dependency Gap Identified

**The `__landin_format_variadic` C helper uses `snprintf` (runtime.rs:360)**:
```c
result_len += snprintf(buffer + result_len, 4096 - result_len, "%ld", (long)val);
```

`snprintf` is NOT a primitive listed in §16.5. Without an integer-to-string
conversion primitive, the MIR migration cannot convert `format!("x={}", 42)`
to a string — there's no MIR-level way to turn the integer `42` into the
bytes `"42"`.

**Per user directive "依赖与基础设施完整能力审查"**: This is a blocking
dependency gap. Per §17.8 (任务审查): "如果不能则应该重构重排任务排版图".

### 2.3 Re-Plan Decision

**Option A**: Add `__landin_i64_to_str` as a new primitive to §16.5, then migrate.
**Option B**: Defer format_variadic migration to v0.3, keep C helper.
**Option C**: Partial migration (MIR walks format string, but keeps snprintf call).

**Decision: Option A** — per user directive "同类型错误或者存在依赖关系（情况）
的应该考虑整体性完整修复（避免存在缺漏和遗失）". Adding the primitive and doing
the full migration is the holistic fix.

### 2.4 New Primitive Design: `__landin_i64_to_str`

```c
// Writes the decimal representation of `val` to `buf`.
// Returns the number of bytes written (not including null terminator).
// Writes at most `buf_cap - 1` bytes + null terminator.
// If buf is NULL, returns the number of bytes that WOULD be written (like snprintf).
long long __landin_i64_to_str(char* buf, long long buf_cap, long long val) {
    return snprintf(buf, buf_cap, "%ld", (long)val);
}
```

This is a thin wrapper around `snprintf` — same pattern as `__landin_alloc`
(wraps `malloc`), `__landin_memcpy` (wraps `memcpy`), etc.

**Per §16.5**: This primitive will be added to the "保留的原语 C Helpers" list
alongside `__landin_alloc`, `__landin_realloc`, `__landin_memcpy`, etc.

### 2.5 Migration Target (MIR Intrinsic Sequence)

The MIR migration replaces the C helper's format string walker + snprintf
logic with a pure MIR loop that:
1. Walks the format string byte by byte (using GEP + Load)
2. When hitting `{` followed by `}`: dispatches based on arg type
   - For i64: Call `__landin_i64_to_str` to write the integer
   - For &str: Call `__landin_memcpy` to copy the string bytes
3. For regular chars: Stores directly to the output buffer
4. Grows the output buffer via `__landin_realloc` when needed
5. Returns String { ptr, len, cap }

**MVP scope (§17.6 record)**:
- **Fixed-size buffer (4096 bytes)**: matches C helper MVP. Dynamic growth
  deferred — the C helper also uses a fixed 4096-byte buffer (runtime.rs:351).
- **i64 args only**: The C helper supports &str args via `%s`, but the MIR
  migration will initially support i64 only (the most common case).
  &str arg support deferred to v0.3 (requires fat pointer handling in the
  format walker).
- **No arg_types array**: The C helper takes an arg_types array for type
  dispatch. The MIR version infers types from the MIR local declarations
  (no runtime type tags needed).

## 3. 任务审查结论 (per §17.8)

### 3.1 Is this the best time?

**Yes, with prerequisite** — must add `__landin_i64_to_str` primitive first.
All other dependencies are ready (Stages 18.226-18.230).

### 3.2 Implementation Plan (2 sub-stages)

| Sub-stage | Task | LOC (est.) |
|-----------|------|-----------|
| 18.231a | Add `__landin_i64_to_str` primitive to §16.5 + runtime.rs + function_sigs.rs + driver_validations.rs | ~40 |
| 18.231b | Migrate `lower_format_variadic_intrinsic` to MIR intrinsics | ~250 |

### 3.3 Risk Assessment

| Risk | Mitigation |
|------|-----------|
| i64_to_str primitive may have wrong ABI | Follow same pattern as __landin_alloc (function_sigs.rs:71) |
| MIR format walker may be too complex | Start with i64-only MVP; &str deferred to v0.3 |
| Fixed buffer may overflow | Use 4096 bytes (same as C helper); add bounds check |
| DCE may remove format string loads | Stage 18.228 fix handles Load/GEP reads |

## 4. Test Plan (per §9.4)

| Test | Category | Verification |
|------|----------|-------------|
| `stage18_186_format_literal_length` | Regression | `format!("hello").len()` = 5 |
| `stage18_186_format_empty` | Regression | `format!("").len()` = 0 |
| `stage18_186_format_with_args_now_works` | Regression | `format!("x={}", x)` compiles |
| `stage18_205_format_len_method_call` | Regression | `format!("x={}", 42).len()` = 4 |
| `stage18_205_format_multi_args_len` | Regression | `format!("{}+{}={}", 1, 2, 3).len()` = 5 |
| `stage18_205_format_cap_field` | Regression | `format!("x={}", 42).cap` = 5 |

## 5. Recommendation

**Proceed with v0.2.5g migration** in two sub-stages:
1. Add `__landin_i64_to_str` primitive (§16.5 + runtime + sigs + driver)
2. Migrate `lower_format_variadic_intrinsic` to MIR intrinsics with i64-only MVP

This completes the TD-C-WRAPPER-OVERUSE migration (all 4 C helpers migrated).
