# Stage 18.293 — 类 Rust 架构修正: 禁止用户 inherent impl 原始类型 + 孤儿规则

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26
> **Version**: v0.493.0 → v0.494.0 (planned)
> **Process**: §13.5 (设计-审查循环) + §1.0 原則 6 (通解>特解) + §12 (最优>最小)
> **Status**: Design

---

## 1. 问题分析

### 1.1 用户的核心论点

> "原始类型用户就根本不能直接修改他的能力边界和函数成员，孤儿规则 (Orphan Rule)，应该通过 trait 进行（不然，你可以想象 这个人改原始类型上添加A,B,C， 另一个给原始类型上添加A,D,E，... 还玩不玩了？）"

### 1.2 Rust 的模型

1. **禁止用户 inherent impl 原始类型**: `impl i32 { fn method {} }` 报 `E0117`
   - 原始类型的 inherent impl 只能在 `core` crate 中定义
   - 用户必须通过 `trait` + `impl MyTrait for i32` 扩展

2. **孤儿规则 (Orphan Rule)**: `impl Trait for Type` 必须满足:
   - `Trait` 在当前 crate 定义, 或
   - `Type` 在当前 crate 定义
   - 否则报 `E0117: only traits defined in the current crate can be implemented for types defined outside of the crate`
   - 这防止了多 crate 场景下的冲突 ("这个人给 i32 加 A,B,C，另一个人给 i32 加 A,D,E")

3. **Prelude 是 "core crate"**: prelude 注入的 `impl str { fn len {} }` 等是"core"级别的, 用户不能重复定义

### 1.3 当前 Landin 的缺陷

| 问题 | 当前 Landin | Rust 模型 |
|------|-------------|-----------|
| `impl i32 { fn method {} }` | ✅ 允许 | ❌ 禁止 (E0117) |
| 扩展原始类型方式 | inherent impl | trait impl |
| 孤儿规则 | ❌ 无 | ✅ 有 |
| 多 crate 冲突防护 | ❌ 无 | ✅ 孤儿规则 |

---

## 2. 设计: 类 Rust 修正

### 2.1 禁止用户 inherent impl 原始类型

**规则**: 当用户代码中出现 `impl PrimitiveType { ... }` (inherent impl on primitive type), 报错:
```
error: cannot define inherent `impl` for primitive type `i32`
note: inherent impls for primitive types are only allowed in the prelude
```

**实现**: 在 `traits/resolver.rs:collect()` 中, 当检测到 inherent impl (trait_name == None) 且 self_ty 是原始类型 (Int/Uint/Bool/Char/Float/Str) 时, 记录错误。

**例外**: prelude 注入的 `impl str { fn len { loop {} } }` 等 — prelude 是"core crate", 允许。

### 2.2 如何区分 prelude vs 用户?

**方案 A (推荐)**: 检查 impl block 的 span — prelude 注入的代码有特殊的 span (在用户代码之前)。但这是脆弱的。

**方案 B**: 在 `inject_prelude` 时, 标记 prelude impl 为 `is_prelude: true`。然后在 collect 中检查。

**方案 C (最干净)**: 在 collect 中检查 self_ty 是否是原始类型 + 是否是 inherent impl。如果是, 检查这个 impl 是否来自 prelude (通过 DefId 范围或 span)。但 prelude DefId 是动态分配的, 难以区分。

**方案 D (最简单且正确)**: prelude 注入在用户代码之前, 所以 prelude impl 的 DefId 比用户 impl 小。检查 DefId 是否在 prelude 范围内 (小于用户代码的第一个 DefId)。

**选择**: 方案 D — 通过 DefId 范围判断。prelude 注入在 `inject_prelude` 中, 在 HIR lowering 之前。所以 prelude 的 DefId 在 `hir.owners` 中排在最前面。我可以在 collect 中记录 prelude DefId 的最大值, 大于它的就是用户代码。

**更简单**: 在 `inject_prelude` 中标记 prelude items, 或者在 collect 中检查 span 是否在 prelude 范围。

**最终选择**: 在 `inject_prelude` 中, 为 prelude impl 添加一个标记 (通过 attributes 或 span)。但 Landin 的 AST 不支持 attributes on impl。

**实际最简单**: prelude impl 的 span 在 prelude source 范围内 (0 到 prelude source length)。用户 impl 的 span 在用户代码范围内。检查 span 是否在 prelude 范围内。

### 2.3 孤儿规则 (简化版)

Landin 目前是单 crate (没有多 crate 支持), 所以孤儿规则的全量实现是未来工作。当前可以实现的简化版:

- inherent impl on primitive types → 禁止 (只有 prelude 允许)
- trait impl: 允许 `impl MyTrait for i32` (MyTrait 在当前 crate 定义 → 符合孤儿规则)
- 这解决了"这个人加 A,B,C，另一个人加 A,D,E"的问题 — 因为必须通过 trait, 而 trait 有 coherence 检查

---

## 3. 实施计划

### Step 1: 禁止用户 inherent impl 原始类型

在 `traits/resolver.rs:collect()` 中:
- 检测 inherent impl (trait_name == None) 且 self_ty 是原始类型
- 检查 impl 是否来自 prelude (通过 span 范围)
- 如果来自用户 → 报错

### Step 2: 更新已有测试

Stage 18.292 的测试中, 有允许 `impl i32 { fn double {} }` 的 positive tests — 这些需要改为 negative tests (应该报错)。

### Step 3: 添加新测试

- 禁止 `impl i32 { fn method {} }` (用户)
- 允许 `impl str { fn len {} }` (prelude)
- 允许 `impl MyTrait for i32` (trait impl, 正确扩展方式)

---

## 4. §13.4 J1-J6 Compliance

- J1: traits/resolver design unchanged (collect 扩展)
- J2: single responsibility (primitive type impl restriction)
- J3: no circular deps
- J4: complete restriction
- J5: stays within traits stage
- J6: LOC driven by responsibility (~50 LOC)
