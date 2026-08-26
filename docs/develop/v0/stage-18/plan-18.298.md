# Stage 18.298 — 深度架构审查: Landin vs Rust 模型全面对比与重新规划

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26
> **Version**: v0.493.0
> **Process**: §13.5 (设计-审查循环) + §2.2 (根因思维) + §12 (最优>最小)

---

## 1. 审查方法

按用户指示，使用 5W2H + Rust 官方设计哲学，系统性地审查 Landin 当前架构与 Rust 模型的偏差。

---

## 2. 发现的片面性与特解 (7 个维度)

### 2.1 🔴 特解: marker body `loop {}` + intrinsic dispatch (str::len/is_empty/as_bytes)

**当前**: `impl str { fn len(&self) -> i64 { loop {} } }` — marker body + `lookup_primitive_intrinsic` 后置 dispatch

**Rust**: `impl str { pub const fn len(&self) -> usize { ... } }` — REAL body, 直接访问 fat pointer 的 len 字段

**根因**: Landin 不支持 fat pointer 字段访问语法 (`self.1` 或 `(*self).len`)，无法在源码中表达 "取 fat pointer 的第 1 个字段"

**为什么是特解**: 只有 str 的 3 个方法用 marker body + intrinsic dispatch，其他 primitive 方法 (i64::is_zero, bool::to_int) 用 real body + normal call。两条 dispatch 路径用于同一件事 (primitive type 方法) — 违反 §1.0 原則 6 (通解>特解)

**最优方案**: 添加 fat pointer 字段访问语法 → 所有 primitive 方法用 real body → 移除 marker body + intrinsic dispatch

### 2.2 🔴 特解: 6 个 early interception hardcoded intrinsics

**当前**: `String::as_str`, `String::from_str`, `String::push_str`, `Vec::push`, `Vec::get`, `Box::new` — 每个都有独立的 `if method_name_str == "xxx"` + 专用 lower 函数

**Rust**: 这些都是 core/std 中的 REAL 方法，使用 extern "C" FFI 调用 (alloc/memcpy/realloc)

**根因**: Landin prelude 不支持 extern "C" 函数声明，无法在 prelude impl body 中调用 C 运行时函数

**为什么是特解**: 每个方法有独立的 dispatch 路径 — 6 处 `if method_name_str ==` 检查

**最优方案**: 添加 extern "C" in prelude impl support → 所有方法用 real body → 移除 early interception

### 2.3 🟡 类型不一致: i64 vs usize

**当前**: `str::len` 返回 `i64`, `String.len` 字段是 `i64`, `Vec.len` 字段是 `i64`

**Rust**: 全部使用 `usize` — 指针大小的无符号整数

**设计文档**: `docs/lang-design/09-stdlib.md` 明确写 `pub fn len(&self) -> usize`

**为什么是偏差**: Landin 有 `usize` 类型 (UintTy::Usize)，但 prelude 和 fat pointer 使用 `i64` — 与设计文档和 Rust 不一致

**最优方案**: 统一使用 `usize` 替换 `i64` (len/cap 字段、fat pointer len 字段、str::len 返回类型)

### 2.4 🟡 str::as_bytes 返回 &[u8] 但不工作

**当前**: `impl str { fn as_bytes(&self) -> &[u8] { loop {} } }` — marker body, intrinsic dispatch 是 no-op (返回 receiver)

**Rust**: `impl str { pub fn as_bytes(&self) -> &[u8] { ... } }` — 实际返回 fat pointer 的 reinterpret

**问题**: Landin 的 as_bytes intrinsic 是 no-op (返回 receiver local)，但 `&str` 和 `&[u8]` 有相同的 fat pointer 布局 — 在 typeck 层面可能有类型不匹配

### 2.5 🟡 format! macro 仍是 C runtime helper

**当前**: `format!` 通过 `lower_format_variadic_intrinsic` 生成 C runtime 调用 (`__landin_i64_to_str` 等)

**Rust**: `format!` 是宏展开为 `core::fmt` 的 real 代码

**为什么是特解**: 整个 format 实现是一个大的 intrinsic 函数，而非宏展开 + real codegen

### 2.6 🟡 STDLIB_ALLOC_TYPES 硬编码类型列表

**当前**: `src/stdlib/mod.rs` 有 `STDLIB_ALLOC_TYPES: &[&str]` — 硬编码 13 个类型名

**Rust**: 没有硬编码列表 — 类型通过 trait (如 `Sized`, `Drop`) 和 ABI 区分

**为什么是特解**: 每新增一个 alloc 类型都需要修改这个列表

### 2.7 🟡 孤儿规则未实现

**设计文档**: §03 §5.6 "orphan rule ❌ 未实现 B1 v0.2+"

**Rust**: 完整的 orphan rule — `impl Trait for Type` 必须满足 Trait 或 Type 在当前 crate 定义

**当前**: Landin 是单 crate，多 crate 场景未实现 — 合理的 deferred

---

## 3. 重新规划: 消除特解的优先级排序

| 优先级 | 问题 | 方案 | 复杂度 | 依赖 |
|--------|------|------|--------|------|
| P1 | 2.3 i64 vs usize | 统一 usize | L2 | 无 |
| P1 | 2.1 marker body + intrinsic dispatch | 添加 fat pointer 字段访问 | L3 | 无 |
| P2 | 2.2 6 个 early interception | extern "C" in prelude | L3 | 无 |
| P2 | 2.4 as_bytes type mismatch | 依赖 2.1 (fat pointer 字段访问) | L2 | 2.1 |
| P3 | 2.5 format! macro | 依赖 2.2 (extern "C") | L3 | 2.2 |
| P3 | 2.6 STDLIB_ALLOC_TYPES | 改用 trait 区分 | L3 | 2.2 |
| deferred | 2.7 orphan rule | 多 crate 支持 | L3 | v0.2+ |

---

## 4. 最优架构目标 (类 Rust 完整模型)

```
Prelude (core):
  impl str {
    fn len(&self) -> usize { /* fat pointer field 1 access */ }
    fn is_empty(&self) -> bool { self.len() == 0 }
    fn as_bytes(&self) -> &[u8] { /* reinterpret fat pointer */ }
  }
  
  impl String {
    fn from_str(s: &str) -> String { /* extern C: alloc + memcpy */ }
    fn as_str(&self) -> &str { /* fat pointer construction from fields */ }
    fn push_str(&mut self, s: &str) { /* extern C: realloc + memcpy */ }
  }
  
  impl<T> Vec<T> {
    fn push(&mut self, v: T) { /* extern C: realloc + store */ }
    fn get(&self, i: usize) -> Option<&T> { /* bounds check + GEP */ }
  }
  
  impl<T> Box<T> {
    fn new(v: T) -> Box<T> { /* extern C: alloc + store */ }
  }

NO marker body. NO intrinsic dispatch. NO early interception.
ALL methods have REAL bodies. ALL use standard method resolution.
```

---

## 5. 实施路径

### Phase A: 类型统一 (i64 → usize) — P1, L2
1. 修改 fat pointer len 字段类型: i64 → usize
2. 修改 String/Vec len/cap 字段类型: i64 → usize
3. 修改 str::len 返回类型: i64 → usize
4. 修改所有 intrinsic emit 函数的 len 类型
5. 更新所有测试

### Phase B: Fat pointer 字段访问 — P1, L3
1. 添加 `(*self).0` 或 `self.0` 语法支持 (fat pointer 字段访问)
2. 将 str::len/is_empty/as_bytes 的 marker body 改为 real body
3. 移除 `lookup_primitive_intrinsic` + `emit_primitive_intrinsic`
4. 移除 `primitive_intrinsics.rs`
5. 更新所有测试

### Phase C: extern "C" in prelude — P2, L3
1. 支持 prelude impl body 中声明 extern "C" fn
2. 将 String::from_str/push_str, Vec::push/get, Box::new 改为 real body
3. 移除 6 个 early interception + 6 个专用 lower 函数
4. 移除 `intrinsic_lower.rs` (1957 LOC)
5. 更新所有测试

### Phase D: format! macro — P3, L3
1. 将 format! 改为宏展开 + real codegen
2. 移除 `lower_format_variadic_intrinsic`
3. 更新所有测试
