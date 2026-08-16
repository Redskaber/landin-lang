# Stage 18.159 — 整体修复错误系统 (TD-MODULELOAD-ERROR-FIELD + TD-UNWRAP-NONGUARDED + TD-SPAN-DUMMY-CLEANUP)

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.427.0 (Stage 18.159 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §12 (最优>最小) + §13.4 (重构即架构设计) + §3.2 (交付前验收)
> **Complexity**: L2 (错误系统整体性修复)
> **Task ID**: stage18.159

## 1. 阶段目标

按用户要求"同类型错误或者存在依赖关系的应该整体性完整修复", 修复 Stage 18.158 跨阶段审查发现的 3 项相互关联的错误系统技术债:

| TD | 描述 | 依赖关系 |
|----|------|---------|
| TD-MODULELOAD-ERROR-FIELD | ModuleLoadError 强转为 LowerError | 核心结构修复, 其他依赖 |
| TD-UNWRAP-NONGUARDED | codegen/llvm/arithmetic.rs 无 guard unwrap | 错误路径健壮性 |
| TD-SPAN-DUMMY-CLEANUP | 错误路径 Span::DUMMY 清理 | 错误诊断精度 |

**整体性**: 三者都涉及错误系统质量 — 错误分类精度 (TD-MODULELOAD-ERROR-FIELD) + 错误路径健壮性 (TD-UNWRAP-NONGUARDED) + 错误诊断精度 (TD-SPAN-DUMMY-CLEANUP)。本 stage 整体修复, 避免遗漏。

## 2. 修复实现

### 2.1 TD-MODULELOAD-ERROR-FIELD: 添加 CompileErrors.module_load 字段

**Before** (Stage 18.152): `ModuleLoadError` 在 `compile_inner` 中被强转为 `LowerError`, 丢失 `path` 字段:
```rust
errors.lower.push(LowerError::new(le.message, le.span));  // path 丢失
```

**After** (Stage 18.159):
1. `CompileErrors` 新增 `module_load: Vec<ModuleLoadError>` 字段
2. `ErrorCode` 新增 `ModuleLoad` variant (E850)
3. `compile_inner` 改为 `errors.module_load.extend(load_errors)` (保留 path)
4. `to_diagnostics_with_resolver` 渲染 module_load 错误, path 作为 note

Per §1.0 原則 6 (通解>特例): 专用字段而非重载 lower 错误。
Per §2 原則 4 (报错>静默): 结构化 path 信息到达用户。
Per §2 原則 9 (正确>妥协): 保留 path 字段, 不妥协。

### 2.2 TD-UNWRAP-NONGUARDED: codegen/llvm/arithmetic.rs if-let 模式

**Before** (Stage 18.156):
```rust
let intrinsic_fn = if self.values.contains_key(&name) {
    *self.values.get(&name).unwrap()  // 有 guard 但不显式
} else { ... };
```

**After** (Stage 18.159):
```rust
let intrinsic_fn = if let Some(&v) = self.values.get(&name) {
    v  // 显式 pattern match
} else { ... };
```

Per §2 原則 3 (显式>隐式): pattern match 比 contains_key + unwrap 更清晰。

**其余 8 处非测试 unwrap**: 经评估均有 invariant guard (前序 match/检查保证 Some), 风险低, 保留。

### 2.3 TD-SPAN-DUMMY-CLEANUP: expr_variants.rs discriminant span

**Before** (Stage 18.152):
```rust
let discr = Operand::Constant(Const {
    ty: Ty::new(TyKind::Int(I32), Span::DUMMY),  // 丢失 expr 位置
    ...
});
```

**After** (Stage 18.159): 2 处改为 `expr.span`:
- `expr_variants.rs:84` (generic enum variant discriminant)
- `expr_variants.rs:414` (enum variant discriminant)

**其余 Span::DUMMY**: 经评估为合法合成用法 (合成 token/类型无源码位置), 保留:
- `builtin_macros.rs` ~350 处: 合成 Token, 合法
- `typeck/infer.rs` ~14 处: 合成 Infer 类型, 合法
- `mir/substitute.rs` ~12 处: 类型替换, 合成
- `typeck/check.rs` 剩余: 已有 fallback 逻辑 (stmt.span != DUMMY 时用 stmt.span)

Per §2 原則 4 (报错>静默): 错误路径用真实 span。
Per 用户要求: "设计主干可以暂时使用，但必须纳入清理计划" — 合成用法保留, 错误路径已清理。

## 3. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 字段 | `CompileErrors.module_load` | `<noun>_<noun>` | ✅ |
| Enum variant | `ErrorCode::ModuleLoad` | `<Noun><Noun>` | ✅ |
| Error code | `E850` | 数字编码 | ✅ |
| Category | `module_load` | `<noun>_<noun>` | ✅ |

## 4. 接口设计 (§11)

- `CompileErrors.module_load` 是 `pub` 字段 — 公共 API
- `ErrorCode::ModuleLoad` 是 `pub enum` variant — 公共 API
- 不跨阶段调用 — 错误系统在 driver 层统一收集
- `to_diagnostics_with_resolver` 渲染所有 10 类错误 (含新 module_load)

## 5. 测试更新

### 5.1 测试修复 (3 个 negative + 5 个 positive)

Stage 18.152 的测试原检查 `errors.lower`, 现更新为检查 `errors.module_load`:
- 3 个 negative: `missing_module`, `circular_dep`, `parse_error_in_submodule` → 检查 `errors.module_load`
- 5 个 positive: 检查 `errors.module_load.is_empty()`

### 5.2 §3.2 验收

- ✅ cargo check --features llvm-backend: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 warnings
- ✅ cargo test --features llvm-backend: 656 lib + 2696 integration, 0 failed

## 6. 简写和缺陷记录

### 6.1 已修复

- **TD-MODULELOAD-ERROR-FIELD**: ✅ Resolved — 专用字段 + ErrorCode + path note
- **TD-UNWRAP-NONGUARDED**: ✅ Resolved — arithmetic.rs if-let 模式; 其余有 guard 保留
- **TD-SPAN-DUMMY-CLEANUP**: 🟡 Partial — 2 处 discriminant 修复; 其余合法合成保留

### 6.2 剩余简写

**TD-NEGATIVE-TEST-COVERAGE**: 负面测试比例 6.5% (低于 25% 建议)。
- **修订计划**: v0.2 P2 补充负面测试 (重点: codegen/ModuleLoader/typeck)

## 7. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | module_load 字段匹配其他错误字段模式 | ✅ |
| J2 单一职责 | 专用字段, 不重载 lower | ✅ |
| J3 单向流动 | compile_inner → errors.module_load → to_diagnostics | ✅ |
| J4 编译相关表达完整 | 错误分类完整 (10 类) | ✅ |
| J5 阶段划分清晰 | 错误系统在 driver 层 | ✅ |
| J6 科学合理粒度 | 3 修复点分散在合适模块 | ✅ |

## 8. Stage Summary

- **Stage 18.159 PASSED** — 整体修复错误系统 (3 项关联 TD)
- **修复**: TD-MODULELOAD-ERROR-FIELD ✅ + TD-UNWRAP-NONGUARDED ✅ + TD-SPAN-DUMMY-CLEANUP 🟡 Partial
- **新增**: `CompileErrors.module_load` 字段 + `ErrorCode::ModuleLoad` (E850)
- **修改**: `compile_inner` 用 module_load 字段 + arithmetic.rs if-let + expr_variants.rs span
- **测试**: 8 个测试更新 (errors.lower → errors.module_load)
- **§3.2 全套验收**: cargo check/fmt/clippy/test 全绿 (656 lib + 2696 integration)
- **v0.427.0**: patch bump
- **下一步**: Stage 18.160 补充负面测试 (TD-NEGATIVE-TEST-COVERAGE)
