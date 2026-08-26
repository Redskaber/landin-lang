# Stage 18.292 — 类 Rust Intrinsic 调度架构修正

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26
> **Version**: v0.493.0 → v0.494.0 (planned)
> **Process**: stage-committee-process.md v7.3 §13.5 (设计-审查循环) + §1.0 原則 6 (通解>特解)
> **Status**: Design — awaiting REV-A review

---

## 1. 5W2H 架构哲学审视

### 1.1 WHY: 用户质问 "原始类型允许覆盖，凭什么？"

**Stage 18.290 的错误设计**: `is_marker_loop_body` 检查 — 用户 real body 优先于 prelude marker body。这允许用户覆盖 prelude intrinsic。

**问题**: 这与 Rust 模型不一致。Rust 不允许用户覆盖 core 定义的 `str::len` — 会报 `E0119: conflicting implementations`。

**正确设计 (类 Rust)**:
- prelude `impl str { fn len { loop {} } }` 是权威 intrinsic 声明
- 用户不能定义 `impl str { fn len {} }` (与 prelude 冲突 → 报错)
- 用户可以定义 `impl str { fn my_method {} }` (新方法, 不冲突)
- marker body `loop {}` 是 intrinsic 的实现, 不是可覆盖的占位符

### 1.2 WHAT: 需要做什么?

1. **移除 `is_marker_loop_body` 检查** (Stage 18.290 的错误设计) — 不需要检查 body 是否是 marker, 因为用户根本不能定义同名方法
2. **添加 inherent impl 冲突检测** (Stage 18.291 的正确部分) — 检测所有 inherent impl 方法冲突, **不跳过 marker impl**
3. **冲突检测不跳过 prelude marker impl** — prelude 的 `impl str { fn len { loop {} } }` 与用户的 `impl str { fn len { 42 } }` 冲突 → 报错

### 1.3 WHO: 谁有权限?

**类 Rust 模型**:
- prelude: 定义 intrinsic (marker body) + 原始类型方法 (real body)
- 用户: 定义新方法, **不能覆盖** prelude 方法
- 冲突检测: 对所有 impl 一视同仁 (包括 prelude marker)

### 1.4 WHERE: 在哪里实施?

1. `src/mir/lower/primitive_intrinsics.rs` — 移除 `is_marker_loop_body` 检查 (如果存在)
2. `src/traits/resolver.rs` — 添加 `check_inherent_impl_conflicts()` (不跳过 marker)
3. `src/traits/error.rs` — 添加 `InherentImplConflict` 错误类型
4. `src/driver/driver_validations.rs` — 调用冲突检测

### 1.5 WHEN: 何时报错?

用户定义 `impl str { fn len {} }` (与 prelude 冲突) → **立即报错** "duplicate definitions with name `len`"

### 1.6 HOW: 如何实现?

1. `lookup_primitive_intrinsic` 保持当前逻辑 (通过 `(self_ty, method)` pair 识别 intrinsic)
2. 添加 `check_inherent_impl_conflicts()` — 检测所有 inherent impl 方法冲突, **不跳过 marker**
3. 用户定义 `impl str { fn len {} }` → 与 prelude `impl str { fn len { loop {} } }` 冲突 → 报错

### 1.7 HOW MUCH: 做到什么程度?

- 移除 `is_marker_loop_body` (如果存在) — 当前 Stage 18.288 状态没有这个函数, 所以不需要移除
- 添加 `InherentImplConflict` + `check_inherent_impl_conflicts()` + `all_methods_marker` 字段
- **不跳过 marker impl** — 冲突检测对所有 impl 一视同仁

---

## 2. 与 Stage 18.290/18.291 的区别

| 方面 | Stage 18.290 (错误) | Stage 18.291 (部分正确) | Stage 18.292 (类 Rust, 正确) |
|------|---------------------|------------------------|------------------------------|
| 用户覆盖 prelude intrinsic | ✅ 允许 (marker body 识别) | ✅ 允许 (跳过 marker) | ❌ 不允许 (冲突报错) |
| 冲突检测 | 无 | 有 (跳过 marker) | 有 (**不跳过** marker) |
| `is_marker_loop_body` | 有 | 有 | **移除** |
| `all_methods_marker` | 无 | 有 (跳过 marker) | **不需要** (不跳过) |

---

## 3. 实施计划

1. 添加 `InherentImplConflict` 错误类型 (`src/traits/resolver.rs`)
2. 添加 `check_inherent_impl_conflicts()` 方法 — **不跳过 marker impl**
3. 添加 `TraitError::InherentConflict` variant (`src/traits/error.rs`)
4. 调用冲突检测 (`src/driver/driver_validations.rs`)
5. 添加测试 (覆盖 + 冲突 + 新方法)
6. §3.2 全校验流
