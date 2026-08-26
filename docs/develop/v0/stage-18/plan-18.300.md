# Stage 18.300 — Phase B 重新评估: marker body 是正确架构 (非特解)

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26

## 重新评估结论

经过 5W2H + Rust 官方设计依据深度分析，**Phase B 的原计划需要修正**:

### 原计划 (plan-18.298 Phase B)
> 添加 fat pointer 字段访问语法 → str::len/is_empty/as_bytes 用 real body → 移除 marker body + intrinsic dispatch

### 重新评估

**Rust 官方设计**:
- `str::len()` 在 Rust core 中是 **compiler intrinsic** — 编译器直接生成 fat pointer 字段访问代码
- `str::len` 的 "body" 是 `unreachable!()` 或 `#[rustc_intrinsic]` 标记 — **不是 real body**
- `i64::abs()` 在 Rust core 中有 **real body** — `if self < 0 { -self } else { self }`
- Rust 明确区分: **intrinsic methods** (需要编译器支持) vs **normal methods** (源码可表达)

**Landin 当前架构**:
- `str::len` = marker body `loop {}` + `lookup_primitive_intrinsic` + `emit_str_len` — **等价于 Rust intrinsic**
- `i64::is_zero` = real body `match self { 0i64 => true, _ => false }` — **等价于 Rust normal method**
- `bool::to_int` = real body `match self { true => 1i32, false => 0i32 }` — **等价于 Rust normal method**

**结论**: marker body `loop {}` + intrinsic dispatch **不是特解** — 它是正确的 intrinsic 实现方式, 等价于 Rust 的 `#[rustc_intrinsic]` 或 `extern "rust-intrinsic"`。

### 什么是真正的特解?

**6 个 early interception** (String::as_str, String::from_str, String::push_str, Vec::push, Vec::get, Box::new) — 这些在 Rust 中有 **real body** (调用 extern "C" 函数), 但在 Landin 中用 `if method_name == "xxx"` hardcoded dispatch。

**这才是 Phase B/C 应该修复的** — 不是 str::len 的 intrinsic dispatch, 而是这 6 个 early interception。

### 修正后的 Phase 路径

| Phase | 原计划 | 修正后 | 理由 |
|-------|--------|--------|------|
| A | i64 → usize | ✅ 已完成 | — |
| B | Fat pointer 字段访问 → 移除 marker body | **跳过** — marker body 是正确架构 | Rust str::len 也是 intrinsic |
| C | extern "C" in prelude → 移除 6 early interception | **Phase B (修正)** | 这是真正的特解 |
| D | format! macro | Phase C | 依赖 Phase B |

### 下一步

**Phase B (修正)**: 添加 extern "C" in prelude impl body 支持 → 将 6 个 early interception 改为 real body → 移除 `intrinsic_lower.rs` (1957 LOC)
