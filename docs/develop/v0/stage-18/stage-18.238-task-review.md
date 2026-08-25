# Stage 18.238 — Task Review: TD-INTRINSIC-OVERUSE Phase 1 — Migrate Field-Access Intrinsics

> **Date**: 2026-08-23
> **Version**: v0.484.0 → v0.485.0 (planned)
> **Task ID**: stage18.238
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.8 (Intrinsic → Stdlib Impl Migration)

## 1. 触发场景

Per Stage 18.237: pointer arithmetic fully working, TD-INTRINSIC-OVERUSE
unblocked. Per §17.8 (任务审查): all prerequisites satisfied.

Per user directive "同类型错误或者存在依赖关系（情况）的应该考虑整体性完整修复":
Phase 1 migrates the simplest intrinsics (field-access methods) first,
then Phase 2+ handles complex intrinsics (alloc, memcpy, growth logic).

## 2. 依赖与基础设施完整能力审查

### 2.1 Prerequisites (from Stage 18.235 audit)

| Prerequisite | Status |
|-------------|--------|
| Pointer arithmetic | ✅ Stage 18.236 |
| Store-through-Deref on GEP | ✅ Stage 18.237 |
| `extern "C"` in prelude | ✅ Already exists |
| While loop | ✅ Already exists |
| `&mut self` | ✅ Already exists |
| Field assignment | ✅ Already exists |
| `impl` block method resolution | ✅ Already exists |

**结论**: All prerequisites satisfied. Proceed with Phase 1.

### 2.2 Current State of Intrinsics

| Method | MIR Lower Hardcode | Prelude Impl | Notes |
|--------|-------------------|-------------|-------|
| `str::len()` | ✅ Hardcoded (Field proj) | ❌ Missing | str is built-in, no impl |
| `str::is_empty()` | ✅ Hardcoded (len==0) | ❌ Missing | str is built-in |
| `str::as_bytes()` | ✅ Hardcoded (no-op) | ❌ Missing | str is built-in |
| `String::len()` | ✅ Hardcoded (Field proj) | ✅ Exists in prelude | **REDUNDANT** — prelude handles it |
| `String::new()` | ✅ Hardcoded (Aggregate) | ✅ Exists in prelude | **REDUNDANT** — prelude handles it |
| `String::as_str()` | ✅ Hardcoded (fat ptr) | ❌ Missing | Needs fat ptr construction |
| `String::from_str()` | ✅ Hardcoded (alloc+memcpy) | ❌ Missing | Needs extern C |
| `String::push_str()` | ✅ Hardcoded (MIR intrinsic) | ❌ Missing | Needs extern C + growth |
| `Vec::len()` | ✅ Hardcoded (Field proj) | ❌ Missing | Need impl Vec |
| `Vec::new()` | ✅ Hardcoded (Aggregate) | ❌ Missing | Need impl Vec |
| `Vec::push()` | ✅ Hardcoded (MIR intrinsic) | ❌ Missing | Needs extern C + growth |
| `Vec::get()` | ✅ Hardcoded (MIR intrinsic) | ❌ Missing | Needs bounds check |
| `Box::new()` | ✅ Hardcoded (alloc+store) | ❌ Missing | Needs extern C |
| `format!()` | ✅ Hardcoded (format walker) | ❌ Missing | Needs i64_to_str |

### 2.3 Phase 1 Scope: Remove Redundant Hardcodes

Two intrinsics are **already handled by prelude impl blocks**:
1. `String::len()` — prelude has `impl String { fn len(&self) -> i64 { self.len } }`
2. `String::new()` — prelude has `impl String { fn new() -> String { ... } }`

The MIR lower hardcodes intercept these BEFORE standard method resolution,
so the prelude impl never runs. Removing the hardcodes lets the prelude
impl handle them — the 通解.

**Verification**: Tested `String::new().len()` — works correctly (returns 0).
The prelude impl already resolves correctly when the receiver has a concrete type.

**Risk**: If the receiver is Infer (e.g., `let s = String::new()`), the
method resolution might fail (same issue as TD-METHOD-RESOLVE-STRICT).
But the KNOWN_INTRINSIC_METHODS whitelist (Stage 18.234) will prevent
false error reporting.

## 3. Implementation Plan

### 3.1 Phase 1: Remove Redundant String Intrinsics

| Task | LOC Impact |
|------|-----------|
| Remove `String::len()` hardcoded check (line 1283) | -30 |
| Remove `String::new()` hardcoded check (already intercepted at line 308) | -10 |
| Remove `String::as_str()` hardcoded check (line 1177) | -40 |
| Add `as_str()` to prelude impl (fat ptr construction) | +20 |
| Verify all String tests still pass | — |

### 3.2 Test Plan

| Test | Category | Verification |
|------|----------|-------------|
| `stage18_185_string_len_method_call` | Regression | `String::new().len()` = 0 |
| `stage18_205_format_len_method_call` | Regression | `format!("x={}", 42).len()` = 4 |
| `stage18_198_push_str_append` | Regression | push_str still works |
| All existing String/Vec/Box tests | Regression | No regressions |

## 4. Recommendation

**Proceed with Phase 1** — remove redundant String::len/new/as_str hardcodes.

This is the first step of TD-INTRINSIC-OVERUSE migration. It's low-risk
because the prelude impl already handles these methods. The hardcodes
are pure redundancy (特解 duplicates of the 通解).
