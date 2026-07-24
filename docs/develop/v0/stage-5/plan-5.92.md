# Stage 5.92 开发计划：param_kinds 数据准确性精化

> **阶段**: Stage 5.92
> **版本**: v0.11.87 → 0.11.88
> **状态**: ✅ Complete

## 1. 目标

精化 Stage 5.84 添加的 `param_kinds` 数据准确性。Stage 5.84 的 Python 脚本
将所有参数类型默认为 `StdlibTypeKind::AllocType`，但这对于某些方法不准确：

- `Display::fmt(&self, f: &mut Formatter)` — `f` 应该是 `StdType`（Formatter 是 std 类型）
- `Debug::fmt(&self, f: &mut Formatter)` — `f` 应该是 `StdType`
- `Hash::hash(&self, state: &mut Hasher)` — `state` 应该是 `StdType`

本 stage 修正这些不准确的数据，让 codegen 生成更精确的参数类型 IR。

## 2. 设计

### 2.1 修正的方法

| 方法 | 当前 param_kinds | 正确 param_kinds | 原因 |
|------|-----------------|-----------------|------|
| Display::fmt | [AllocType] | [StdType] | Formatter 是 std 类型 |
| Debug::fmt | [AllocType] | [StdType] | Formatter 是 std 类型 |
| Hash::hash | [AllocType] | [StdType] | Hasher 是 std 类型 |

其他方法（Clone::clone_from, PartialEq::eq/ne, PartialOrd::partial_cmp, 
Ord::cmp 等）的 `&Self` 参数用 AllocType 是正确的，无需修改。

### 2.2 命名标准化

无新 API——本 stage 仅修正现有数据。

### 2.3 §16 接口隔离

无新依赖，仅修正静态表数据。

### 2.4 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | Display::fmt param_kinds == [StdType] | ✓ |
| 2 | Debug::fmt param_kinds == [StdType] | ✓ |
| 3 | Hash::hash param_kinds == [StdType] | ✓ |
| 4 | Clone::clone_from param_kinds == [AllocType] (unchanged) | ✓ |
| 5 | PartialEq::eq param_kinds == [AllocType] (unchanged) | ✓ |
| 6 | PartialOrd::partial_cmp param_kinds == [AllocType] (unchanged) | ✓ |
| 7 | Ord::cmp param_kinds == [AllocType] (unchanged) | ✓ |
| 8 | param_count matches param_kinds.len() for all methods | ✓ |

## 3. 不在本 stage 范围

- ❌ 用户自定义 trait 支持（TD-018, Stage 6+）
- ❌ mir/lower 拆分（TD-011, Stage 6）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
