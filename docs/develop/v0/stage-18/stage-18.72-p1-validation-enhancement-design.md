# Stage 18.72 — P1 Typeck Enhancement (Struct Field Count + Tuple Index Bounds + Pattern Arity)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.339.0 → v0.340.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.71 完成了 5 个 P0 typeck 缺漏修复。Stage 18.72 继续按 Stage 18.70
计划表推进 P1 修复，本次聚焦 3 项验证缺失：

1. **P1-A**: struct field count validation (missing/extra fields in constructor)
2. **P1-B**: tuple index bounds check (`t.5` where t has only 2 elements)
3. **P1-C**: pattern arity check (`let (a, b, c) = (1, 2)`)

这 3 项都是"用户代码格式校验"缺失——编译器静默接受了不合法代码。

## 2. 根因分析

### 2.1 P1-A: struct literal field count 未校验

`src/mir/lower/expr_operand.rs:1856` 处理 `HirExprKind::Struct` 时，
直接将 `fields` 转为 operands，未校验：
- 提供的字段名是否都在 struct 定义中
- 是否有重复字段名
- 是否缺少必需字段（Stage 0 无字段默认值，所有字段都是必需的）

### 2.2 P1-B: tuple index bounds 未校验

`src/typeck/checker.rs:1026` 的 `infer_projection` 函数中，
`ProjectionElem::Field(field_id, field_ty)` 分支直接返回 `field_ty`，
未校验 `field_id.0 < tys.len()` 当 base 类型是 `TyKind::Tuple(tys)` 时。

### 2.3 P1-C: pattern arity 未校验

`src/mir/lower/control_flow.rs:285` 处理 `HirPatKind::Tuple(sub_pats)`
时，直接为每个 sub-pattern 创建 local，未校验 `sub_pats.len()` 是否等于
init 表达式的 tuple 长度。

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | 字段/index/arity 不匹配显式报错 |
| 4 报错 > 静默 | 不静默接受不合法代码 |
| 6 通用 > 特例 | 一个校验函数覆盖所有 struct/tuple/pattern 场景 |
| 9 正确 > 妥协 | 严格按 Rust 语义校验 |

### 3.2 P1-A Fix: struct field count validation

**File**: `src/driver.rs` (新函数 `validate_struct_literal_fields`)

逻辑：
1. 遍历 `hir.owners`，找到所有 `HirItem::Struct` 定义，构建
   `DefId → Vec<Spur>` (字段名列表) 查找表
2. 遍历所有 body 的表达式，找到 `HirExprKind::Struct { path, fields }`
3. 对每个 struct literal：
   - 解析 `path.res` 获取 struct DefId
   - 查找表获取声明的字段名列表
   - 校验：
     a. 每个 provided field 的名字在声明列表中（否则 "no field" 错误）
     b. 无重复字段名（否则 "duplicate field" 错误）
     c. 所有声明字段都已提供（否则 "missing field" 错误）

**Per §10 naming**: `validate_struct_literal_fields` follows `validate_<noun>_<noun>_<noun>`.

### 3.3 P1-B Fix: tuple index bounds check

**File**: `src/typeck/checker.rs`

修改 `infer_projection` 函数的 `ProjectionElem::Field` 分支：
- 当 base 类型解析为 `TyKind::Tuple(tys)` 时，校验 `field_id.0 < tys.len()`
- 超出范围时推送 TypeError ("tuple index out of bounds")

**挑战**: `infer_projection` 目前是 `&self`（不可变借用），无法 push errors。
解决方案：改为 `&mut self`，或返回 `Result<Ty, TypeError>`。

选择方案：改为 `&mut self`，与 `infer_rvalue` 一致。
调用链：`infer_place` (已 &self) → `infer_projection` (改 &mut self)。
`infer_place` 也需改为 `&mut self`。

### 3.4 P1-C Fix: pattern arity check

**File**: `src/driver.rs` (复用 `validate_struct_literal_fields` 的遍历框架)

逻辑：
1. 遍历所有 body 的语句，找到 `HirStmt::Local(local)` where
   `local.pat.kind` is `HirPatKind::Tuple(sub_pats)`
2. 获取 init 表达式的类型（通过 typeck 结果或 MIR local type）
3. 如果 init 类型是 `TyKind::Tuple(tys)`，校验 `sub_pats.len() == tys.len()`

**挑战**: init 表达式的类型在 HIR 层面不一定已知（可能需要类型推断）。
解决方案：在 typeck 之后执行，使用 `TypeckResults.local_types`。

**Per §10 naming**: `validate_pattern_arity` follows `validate_<noun>_<noun>`.

## 4. 测试矩阵

### 4.1 测试转换 (compile_ok → compile_error)

| 文件 | 描述 |
|------|------|
| cg-err-008-missing-struct-field.lin | `S { x: 1 }` missing `y` |
| cg-err-009-extra-struct-field.lin | `S { x: 1, y: 2 }` extra `y` |
| cg-err-020-tuple-index-out-of-bounds.lin | `t.5` where t = (1, 2) |
| e2e-err-017-invalid-let-pattern.lin | `let (a, b, c) = (1, 2)` |

### 4.2 新增负向测试 (per §9.4.3 1:3+ ratio)

| # | 文件 | 测试场景 |
|---|------|---------|
| 1 | e2e-err-026-struct-duplicate-field.lin | `S { x: 1, x: 2 }` duplicate field |
| 2 | e2e-err-027-struct-unknown-field.lin | `S { z: 1 }` unknown field |
| 3 | e2e-err-028-tuple-index-zero-on-unit.lin | `().0` index on unit |
| 4 | e2e-err-029-pattern-arity-extra.lin | `let (a) = (1, 2)` too few patterns |
| 5 | e2e-err-030-pattern-arity-mismatch.lin | `let (a, b) = (1, 2, 3)` mismatch |

### 4.3 正向回归测试 (确保 Fix 不破坏合法代码)

- `S { x: 1, y: 2 }` ✅ (all fields provided)
- `t.0`, `t.1` on `let t = (1, 2)` ✅ (valid indices)
- `let (a, b) = (1, 2)` ✅ (matching arity)

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P1 验证缺失修复 |
| REV-A | GO | 严格按 Rust 语义 |
| DEV-A | GO | 实现简洁，复用现有遍历框架 |
| QA-A | GO | 1:3+ ratio，4+5 测试 |
| PM-A | GO | P1 路线图项目 |

**5/5 GO** ✅

## 6. 风险与缓解

### 6.1 风险：infer_place 改 &mut self 可能破坏现有调用

**缓解**：
- `infer_place` 的所有调用点都在 `check_statement` 和 `post_check_statement`
  中，这些已经是 `&mut self`
- `infer_place` 也被 `infer_rvalue` 调用，后者也是 `&mut self`
- 全量测试会暴露任何破坏

### 6.2 风险：pattern arity 校验可能误报

**缓解**：
- 仅当 init 类型明确为 `TyKind::Tuple` 且长度已知时才校验
- Infer 类型的 tuple 跳过校验（避免 false positive）
- 全量 conformance 测试验证
