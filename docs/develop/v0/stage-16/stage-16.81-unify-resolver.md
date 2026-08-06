# Stage 16.81 — Migrate unify.rs to mismatch_with_resolver

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.267.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

Stage 16.80 添加了 `TypeError::mismatch_with_resolver` 但 unify.rs 仍用旧 API。本阶段迁移 unify.rs 使实际编译错误显示类型名。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿：
- Design v1: `stage-16.81-unify-resolver-design.md`
- SAFETY 审查通过（裸指针方案）
- J1-J6 全部满足

## 3. 实现内容

### 3.1 UnificationTable 新增 resolver/interner 字段

```rust
pub struct UnificationTable {
    // ... existing fields ...
    resolver: Option<*const TraitResolver>,
    interner: Option<*const Rodeo>,
}
```

裸指针避免 lifetime 传染所有调用点。

### 3.2 新增 set_resolver 方法

```rust
pub fn set_resolver(&mut self, resolver: &TraitResolver, interner: &Rodeo)
```

### 3.3 新增 make_mismatch helper

有 resolver 时用 `mismatch_with_resolver`，否则 fallback 到 `mismatch`。

### 3.4 替换 10 处 TypeError::mismatch 为 self.make_mismatch

### 3.5 driver.rs 集成

`typeck_main_body` 新增 resolver + interner 参数，调用 `set_resolver`。

### 3.6 修复递归解析

`type_kind_to_string_with_resolver` 现在递归处理 Ref/Ptr/Array/Slice/Tuple 内部的 Adt。

## 4. 测试计划 (§9.4.3 1:3+ ratio)

| # | 测试名 | 极性 | 描述 |
|---|--------|------|------|
| 1 | unify_with_resolver_shows_struct_name | positive | unify 错误显示 struct 名 |
| 2 | unify_without_resolver_falls_back | positive | 无 resolver 时显示 <adt> |
| 3 | compile_mismatch_struct_int_shows_name | negative | 编译错误含 "MyStruct" |
| 4 | compile_mismatch_two_structs_shows_names | negative | 两个 struct 名都显示 |
| 5 | compile_mismatch_enum_int_shows_name | negative | enum 名显示 |
| 6 | compile_mismatch_struct_ref_shows_name | negative | &MyStruct 名显示 |
| 7 | compile_mismatch_fn_arg_shows_name | negative | 函数参数错误显示 struct 名 |
| 8 | compile_mismatch_return_type_shows_name | negative | 返回类型错误显示 struct 名 |

**比例**: 2:6 = 1:3 ✓

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 381 lib + 2494 integration = 2875 unit tests |

## 6. 结论

GO — unify.rs 迁移完成：
- 10 处 mismatch 调用全部迁移到 make_mismatch ✅
- 实际编译错误显示类型名（"expected MyStruct, found i32"）✅
- Ref/Ptr/Array/Slice/Tuple 递归解析 ✅
- 8 新测试 1:3 正负比例 ✅

## 7. 后续工作

- BorrowError 错误消息改进（show borrow lifetime）
- Trait bound not satisfied 错误消息
- Performance Optimization (P3)
