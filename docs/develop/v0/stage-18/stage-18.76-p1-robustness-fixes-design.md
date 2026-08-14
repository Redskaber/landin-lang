# Stage 18.76 — P1 Robustness Fixes

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.343.0 → v0.344.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.75 修复了 5 项 P0 错误系统缺陷。本 Stage 继续修复 Stage 18.74
审计识别的 P1 健壮性 + 错误精确性问题。

## 2. P1 修复项

| P1 # | 描述 | 修复方案 |
|------|------|---------|
| P1-A | 3 处静默 Ty::Error (Deref/Index on wrong type) | 推送 TypeError 而非静默返回 Error |
| P1-B | 2 处生产 panic! (And/Or/Deref) | 替换为返回 Error + 诊断 |
| P1-C | LocalId(0) 静默降级 | 使用 Option<LocalId> 或推送错误 |
| P1-D | 5 处 Debug 格式泄露 | 替换为 Display 格式 |

### 2.1 不修复项 (需更大重构)

- **TraitError 位置迁移** (driver.rs → traits/error.rs): 涉及大量 import 变更，延后到独立重构 stage
- **5 个错误类型 Kind enum**: 需要 API 设计，延后到独立 stage
- **Param unify**: 需 v0.2 单态化，已记录为 Stage 0 限制

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 4 报错 > 静默 | 所有静默 fallback 改为推送错误 |
| 6 通用 > 特例 | 统一的错误推送模式 |
| 9 正确 > 妥协 | panic! 替换为优雅错误处理 |

### 3.2 P1-A: 3 处静默 Ty::Error

**File**: `src/typeck/checker.rs`

`infer_projection` 中 3 处静默返回 `Ty::new(TyKind::Error, Span::DUMMY)`：
1. `Deref` on non-Ref/RawPtr → 推送 "cannot dereference non-pointer type"
2. `Index` on non-Array/Slice → 推送 "cannot index non-array type"
3. `ConstantIndex`/`Subslice` on non-Array/Slice → 推送同样错误

### 3.3 P1-B: 2 处生产 panic!

**File**: `src/mir/lower/mod.rs`

```rust
// Before:
HirBinOp::And | HirBinOp::Or => panic!("lower_bin_op called with ..."),
HirUnaryOp::Deref => panic!("lower_un_op called with Deref ..."),

// After:
HirBinOp::And | HirBinOp::Or => {
    // And/Or should be lowered via short-circuit path, not here.
    // Return a placeholder BinOp::BitAnd (best-effort) + push error.
    cx.type_errors.push(TypeError::new(
        "internal error: And/Or should be lowered via short-circuit path",
        Span::DUMMY,
    ));
    BinOp::BitAnd
}
```

### 3.4 P1-C: LocalId(0) 静默降级

**File**: `src/borrowck/region_inference.rs`

2 处 `_ => LocalId(0)` 改为推送错误到 region constraint (best-effort):
- 保持 `LocalId(0)` 作为 fallback (region inference 是 best-effort)
- 但添加注释说明这是已知限制，并记录到 worklog

### 3.5 P1-D: 5 处 Debug 格式泄露

| 文件 | 行 | 修复 |
|------|---|------|
| lexer/reader.rs:340 | `{:?}` char | 改用 Display |
| borrowck/borrow_set.rs:98 | `{:?}` BorrowKind | 改用 BorrowKind Display |
| mir/lower/field_resolution.rs:270,282 | `{:?}` TyKind | 改用 type_to_string |
| resolve/module_build.rs:138,313,443 | `{:?}` DefId | 改用 span Display |

## 4. 测试

- 新增 Rust 单元测试验证 P1-A/B 修复
- 全量 conformance 测试验证无回归

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P1 健壮性修复 |
| REV-A | GO | 静默 fallback 和 panic! 是关键问题 |
| DEV-A | GO | 实现简洁 |
| QA-A | GO | 新测试 + 全量回归 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
