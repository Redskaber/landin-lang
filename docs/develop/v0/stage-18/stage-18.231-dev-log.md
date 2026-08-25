# Stage 18.231 — v0.2.5g: Migrate `__landin_format_variadic` → MIR Intrinsic (FINAL)

> **Date**: 2026-08-23
> **Version**: v0.479.0 → v0.480.0
> **Task ID**: stage18.231
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §12 + §17.6 + §17.8
> **设计文档**: docs/lang-design/06-mir.md §16.4 (MIR Intrinsic Ops migration plan)
> **任务审查**: docs/develop/v0/stage-18/stage-18.231-task-review.md

## 1. Scope

Per Stage 18.231 task-review: rewrite `lower_format_variadic_intrinsic` to emit
MIR intrinsic ops (Load + GEP + Store + SwitchInt + Call(alloc/i64_to_str)) instead
of the `__landin_format_variadic` C helper Call.

Per §17.6 (缺陷纳入): TD-C-WRAPPER-OVERUSE migration — 4th (FINAL) of 4 C helpers.

## 2. Dependency Gap Resolution (per §17.8 task review)

**Identified gap**: `__landin_format_variadic` uses `snprintf` (runtime.rs:360) which
is NOT a primitive in §16.5. Without an integer-to-string conversion primitive, the
MIR migration cannot convert integers to strings.

**Resolution**: Added `__landin_i64_to_str` as a new primitive to §16.5:
```c
long long __landin_i64_to_str(char* buf, long long buf_cap, long long val) {
    return (long long)snprintf(buf, (size_t)buf_cap, "%ld", (long)val);
}
```

Files modified for the primitive:
- `src/codegen/runtime.rs` — Added C helper definition
- `src/codegen/llvm/function_sigs.rs` — Added signature `(I64, &[OpaquePtr, I64, I64])`
- `src/driver/driver_validations.rs` — Registered DefId `u32::MAX - 107`
- `docs/lang-design/06-mir.md` §16.5 — Added to primitive list

## 3. Implementation

### 3.1 Migration Sequence (replaces C call)

Format string walker loop (MIR back-edge):
- bb0: Allocate 4096-byte buffer + extract fmt fields + init loop vars
- fmt_loop_bb (BACK-EDGE TARGET): while (fmt_idx < fmt_len)
  - loop_body_bb: GEP + Load byte; if '{' → placeholder_bb; else → literal_bb
  - placeholder_bb: SwitchInt on arg_idx → per-arg blocks
  - per-arg block: Cast to i64, GEP dest, Call __landin_i64_to_str, advance
  - literal_bb: Store byte, out_len++, fmt_idx++
  - Back-edge to fmt_loop_bb
- loop_exit_bb: cap = out_len + 1; Construct String via Aggregate

### 3.2 Key Decisions

| Decision | Rationale |
|----------|-----------|
| Per-arg SwitchInt dispatch | MIR can't dynamically index `arg_locals`; emit one block per known arg |
| Fixed 4096-byte buffer | Matches C helper MVP (runtime.rs:351); dynamic growth deferred |
| i64 args only | Most common case; &str arg support deferred to v0.3 |
| cap = out_len + 1 | Matches C helper convention (null terminator) |

### 3.3 Files Modified

| File | Change | LOC |
|------|--------|-----|
| `src/codegen/runtime.rs` | Add `__landin_i64_to_str` primitive | +10 |
| `src/codegen/llvm/function_sigs.rs` | Add i64_to_str signature | +6 |
| `src/driver/driver_validations.rs` | Register DefId u32::MAX - 107 | +3 |
| `src/mir/lower/expr_variants.rs` | Rewrite `lower_format_variadic_intrinsic` | +400 (replace ~200) |
| `docs/lang-design/06-mir.md` | Update §16.5 + §16.6 + §16.6.5 | +50 |

## 4. Test Verification (per §9.4)

### 4.1 Regression Tests (all pass)

| Test | Verifies |
|------|----------|
| `stage18_186_format_literal_length` | `format!("hello").len()` = 5 |
| `stage18_186_format_empty` | `format!("").len()` = 0 |
| `stage18_186_format_with_args_now_works` | `format!("x={}", x)` compiles |
| `stage18_186_format_placeholder_only_now_works` | `format!("{}", 42)` compiles |
| `stage18_186_format_multiple_literal_args_now_works` | Multiple format! calls |
| `stage18_205_format_len_method_call` | `format!("x={}", 42).len()` = 4 |
| `stage18_205_format_multi_args_len` | `format!("{}+{}={}", 1, 2, 3).len()` = 5 |
| `stage18_205_format_cap_field` | `format!("x={}", 42).cap` = 5 |

## 5. 验收标准 (per §5.3)

| Criterion | Verification |
|-----------|-------------|
| `cargo build --release --features llvm-backend` | ✅ (46.94s) |
| `cargo check --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ (0 warnings) |
| `cargo test --release --features llvm-backend` | ✅ (3783 tests, 0 failures) |

## 6. Design Principles Applied

- §1.0 原則 6 (通解>特解): one MIR sequence for all format! calls
- §1.0 原則 4 (报错>静默): OOM panics via `__landin_alloc` (visible)
- §10 DRY: reuses `__landin_alloc` + `__landin_i64_to_str` (primitives)
- §11 接口隔离: MIR lowering emits MIR intrinsics; codegen only translates MIR
- §12 (最优 > 最小): MIR-level format walker replaces C's snprintf + buffer walk
- §17.6 (缺陷纳入): MVP scope (fixed buffer, i64-only, per-arg dispatch) recorded
- §17.8 (任务审查): dependency gap identified & resolved before implementation

## 7. TD-C-WRAPPER-OVERUSE Migration COMPLETE

All 4 compound C helpers migrated to MIR intrinsics:
- ✅ `__landin_vec_get` → MIR (Stage 18.228)
- ✅ `__landin_vec_push` → MIR (Stage 18.229)
- ✅ `__landin_string_push_str` → MIR (Stage 18.230)
- ✅ `__landin_format_variadic` → MIR (Stage 18.231)

**v0.2 Phase 2 COMPLETE**. Next: v0.3 self-hosting preparation.
