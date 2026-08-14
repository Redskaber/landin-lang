# Stage 18.73 — P1 Validation Enhancement (Array Index + Cast + Assignment Target + Missing Main + Assoc Const)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.340.0 → v0.341.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.72 完成了 3 项 P1 验证修复。Stage 18.73 继续按 Stage 18.70
计划表推进剩余 5 项 P1 修复：

1. **P1-D**: array index type check (`a[true]` where index is not integer)
2. **P1-E**: assignment target check (`42 = 99` — can't assign to non-place)
3. **P1-F**: cast type check (`42 as bool` — invalid cast)
4. **P1-G**: missing main detection (no `fn main()` in crate)
5. **P1-H**: associated const completeness (`trait T { const X: i32; } impl T for S {}` — missing X)

## 2. 根因分析

### 2.1 P1-D: array index type 未校验

`src/typeck/checker.rs::infer_projection` 的 `ProjectionElem::Index(_)`
分支只检查 base 类型是否为 Array/Slice，未校验 index 操作数是否为整数类型。

### 2.2 P1-E: assignment target 未校验

`src/mir/lower/expr_operand.rs` 处理 `HirExprKind::Assign { lhs, rhs }`
时，直接 lower lhs 为 place，未校验 lhs 是否为合法的 place expression
（local、field access、deref、index）。

### 2.3 P1-F: cast type 未校验

`src/mir/lower/expr_operand.rs` 处理 `HirExprKind::Cast { expr, ty }`
时，直接生成 `Rvalue::Cast`，未校验 source 类型是否可转换为目标类型。

### 2.4 P1-G: missing main 未校验

`src/driver.rs` 假设存在 `fn main()`，未在编译前校验。如果用户代码没有
`fn main()`，编译器会静默通过（或产生 confusing codegen 错误）。

### 2.5 P1-H: associated const completeness 未校验

`src/traits/resolver.rs::validate_impls` 只校验 method completeness，
未校验 associated const completeness。`trait T { const X: i32; } impl T for S {}`
（缺少 X 的实现）被静默接受。

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | 所有校验显式报错 |
| 4 报错 > 静默 | 不静默接受不合法代码 |
| 6 通用 > 特例 | 通用校验函数覆盖所有场景 |
| 9 正确 > 妥协 | 严格按 Rust 语义 |

### 3.2 P1-D Fix: array index type check

**File**: `src/typeck/checker.rs`

修改 `infer_projection` 的 `ProjectionElem::Index(idx)` 分支：
- 获取 index 操作数的类型
- 校验为 Int/Uint/Bool（Rust 允许 bool 作为 index？不允许——必须是 integer）
- 实际上 Rust 要求 index 类型实现 `Index` trait，但 Stage 0 简化为：只允许 Int/Uint

**挑战**: `infer_projection` 当前签名不接收 index operand（只接收 `ProjectionElem`）。
`ProjectionElem::Index(Place)` 包含 index place。需要 infer index place 的类型。

### 3.3 P1-E Fix: assignment target check

**File**: `src/driver.rs` (新函数 `validate_assignment_targets`)

逻辑：
1. 遍历所有 body 的表达式
2. 找到 `HirExprKind::Assign { lhs, .. }`
3. 校验 lhs 是否为合法 place expression：
   - `HirExprKind::Path` (resolves to Local)
   - `HirExprKind::Field` (struct/tuple field access)
   - `HirExprKind::Unary` with `Deref` (`*ptr`)
   - `HirExprKind::Index` (`arr[i]`)
4. 不合法时报错 "invalid assignment target"

### 3.4 P1-F Fix: cast type check

**File**: `src/driver.rs` (复用 `validate_assignment_targets` 的遍历框架)

逻辑：
1. 遍历所有 body 的表达式
2. 找到 `HirExprKind::Cast { expr, ty }`
3. 获取 expr 的类型（HIR 层面保守检查：只检查 literal）
4. 校验 cast 是否合法：
   - Int/Uint → Int/Uint/Bool/Char: OK
   - Float → Float: OK
   - Bool → Int/Uint: OK
   - 其他: 报错 "invalid cast"

### 3.5 P1-G Fix: missing main detection

**File**: `src/driver.rs` (新函数 `validate_main_exists`)

逻辑：
1. 遍历 `hir.owners`
2. 查找 `HirItem::Fn` with `ident.name == "main"` and no params
3. 如果找不到，报错 "missing `main` function"

### 3.6 P1-H Fix: associated const completeness

**File**: `src/traits/resolver.rs`

修改 `validate_impls`：
1. 在 `TraitInfo` 添加 `associated_consts: Vec<Spur>` 字段
2. 在 `ImplInfo` 添加 `associated_consts: Vec<Spur>` 字段
3. 在 `collect()` 中收集 associated const names
4. 在 `validate_impls` 中校验：每个 trait associated const 必须在 impl 中提供

## 4. 测试矩阵

### 4.1 测试转换 (compile_ok → compile_error)

| 文件 | 描述 |
|------|------|
| cg-err-006-array-index-out-of-bounds-type.lin | `a[true]` non-integer index |
| cg-err-005-invalid-cast.lin | `42 as bool` invalid cast |
| cg-err-016-use-before-decl.lin | forward reference (P1 but different) |
| e2e-err-018-invalid-assign-target.lin | `42 = 99` invalid target |
| int-err-005-no-main.lin | missing main function |
| int-err-019-undefined-associated-const.lin | missing associated const in impl |

### 4.2 新增负向测试

| # | 文件 | 测试场景 |
|---|------|---------|
| 1 | e2e-err-031-array-index-non-int.lin | `a["x"]` string index |
| 2 | e2e-err-032-cast-str-to-int.lin | `"x" as i32` invalid cast |
| 3 | e2e-err-033-assign-to-literal.lin | `1 = 2` assign to literal |
| 4 | e2e-err-034-assign-to-call.lin | `f() = 1` assign to call result |
| 5 | e2e-err-035-missing-associated-const.lin | trait const not in impl |

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P1 验证缺失修复 |
| REV-A | GO | 严格按 Rust 语义 |
| DEV-A | GO | 复用 Stage 18.72 的遍历框架 |
| QA-A | GO | 1:3+ ratio |
| PM-A | GO | P1 路线图项目 |

**5/5 GO** ✅
