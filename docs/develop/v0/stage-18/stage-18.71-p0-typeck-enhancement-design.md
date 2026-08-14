# Stage 18.71 — P0 Typeck Enhancement (Type Mismatch Checks)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.338.0 → v0.339.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.70 建立了 P0/P1/P2 缺漏修复计划表，其中 P0 包含 5 项 typeck 基础缺失：

1. type mismatch in let binding (`let x: i32 = true;`)
2. type mismatch in fn return (`fn f() -> i32 { true }`)
3. if branch type mismatch (`let x = if true { 1 } else { true };`)
4. trait impl method signature mismatch
5. return with value in void fn (`fn f() { return 42; }`)

这些场景在 Stage 0 中被静默接受（编译通过），是 typeck 正确性的重大缺口。
共 36 个 conformance 测试以 `EXPECTED: compile_ok` + `// SOURCE: Stage 0 limitation`
的形式记录了这些缺口。

## 2. 根因分析

### 2.1 P0-1/2/3: Bool → Int/Uint 过度 coercion

`src/typeck/predicates.rs::can_coerce` 包含以下规则：

```rust
// Bool → Int/Uint: comparison results widen to integers
(TyKind::Int(_), TyKind::Bool) | (TyKind::Uint(_), TyKind::Bool) => true,
```

这条规则原本是为早期 codegen 兼容性添加的，但实际效果是让
`let x: i32 = true;` 也通过 typeck。Rust 不允许这种隐式转换。

`src/typeck/checker.rs::check_statement` (lines 500-558) 已经实现了
type mismatch 检查逻辑，但因为 `can_coerce(i32, bool) == true`，
检查被跳过。Stage 18.71 已添加的 `type_has_unresolved_substs` 和
`types_match_loose` 辅助函数也已完成。

### 2.2 P0-4: trait impl signature 未校验

`src/traits/resolver.rs::validate_impls` 只检查：
- coherence: 同一 (trait, type) 对是否有多个 impl
- completeness: impl 是否实现了所有 trait method

**未检查**：impl method 的 signature 是否与 trait declaration 一致。
fn_sig_table 已经包含两者的 signature，但缺少比对逻辑。

### 2.3 P0-5: void fn 的 return local 类型为 Infer

`src/driver.rs::owner_return_ty` 对 `HirFnRetTy::Default(_)` 返回 `None`。
`src/mir/lower/mod.rs` 对 `None` 使用 `cx.fresh_infer_ty()`。

效果：`fn f() { return 42; }` 中 `_0` 类型为 Infer，与 Int unify 成功，
typeck 检测不到 mismatch。Fix 应该将 `None` 映射为 `Tuple(vec![])` (unit)。

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 3 显式 > 隐式 | return local 类型显式为 unit，而非 Infer |
| 4 报错 > 静默 | 类型不匹配必须报错，不静默 coerce |
| 6 通用 > 特例 | 一个 `check_statement` 覆盖 let/return/if-branches/match-arms |
| 9 正确 > 妥协 | 严格按 Rust 语义拒绝 Bool→Int 隐式转换 |

### 3.2 P0-1/2/3 Fix: 移除 Bool→Int/Uint 规则

**File**: `src/typeck/predicates.rs`

```rust
// 移除以下两条规则：
// (TyKind::Int(_), TyKind::Bool) | (TyKind::Uint(_), TyKind::Bool) => true,
```

**效果**：
- `let x: i32 = true;` — place=i32, rvalue=Bool, can_coerce=false → error ✅
- `fn f() -> i32 { true }` — place=i32 (return local), rvalue=Bool → error ✅
- `let x = if true { 1 } else { true };` — temp unified to Int in then-branch,
  Bool rvalue in else-branch fails to coerce → error ✅
- `let x = match 1 { 0 => 1, _ => true };` — same mechanism → error ✅

### 3.3 P0-4 Fix: trait impl signature 校验

**File**: `src/driver.rs` (新函数 `validate_impl_method_signatures`)

逻辑：
1. 遍历 `hir.owners`，找到所有 `HirItem::Impl` 且 `of_trait.is_some()` 的 impl 块
2. 找到对应 trait 声明 (`HirItem::Trait`)
3. 对 impl 中每个 `HirImplItem::Fn`，按 name 匹配 trait 中的 `HirTraitItem::Fn`
4. 比对两者 signature：
   - inputs 长度
   - 每个对应 input 类型 (after self substitution)
   - output 类型
5. 任何不一致推送 `TypeErrorKind::SignatureMismatch` 错误

**Per §10 naming**: `validate_impl_method_signatures` follows `validate_<noun>_<noun>`.

### 3.4 P0-5 Fix: void fn return local 用 unit 类型

**File**: `src/mir/lower/mod.rs`

修改点：在 `lower_hir_body_to_mir_full` 中，将 `None => cx.fresh_infer_ty(Span::DUMMY)`
改为 `None => Ty::new(TyKind::Tuple(vec![]), Span::DUMMY)`。

**效果**：
- `fn f() { return 42; }` — `_0` 类型 = unit, `return 42` 赋值 Int → mismatch ✅
- `fn f() { return; }` — `_0 = Aggregate(Tuple, [])` → 匹配 ✅
- `fn f() { }` — 无 return，最终 `_0` 默认 unit → 匹配 ✅

## 4. 测试矩阵

### 4.1 测试转换 (compile_ok → compile_error)

| 类别 | 数量 | 来源 |
|------|------|------|
| e2e-err type-mismatch-let | 1 | Stage 18.69 |
| e2e-err extra-return-value | 1 | Stage 18.69 |
| cg-err type-mismatch-return | 1 | Stage 18.69 |
| cg-err invalid-trait-impl | 1 | Stage 18.69 |
| int-err trait-method-wrong-sig | 1 | Stage 18.69 |
| snd type-mismatch (let i32=true) | 27 | Stage 11.9 |
| r5 return-type-mismatch | 1 | Stage 11.5 |
| r5 if-branches-type-mismatch | 1 | Stage 11.5 |
| r5 match-arms-type-mismatch | 1 | Stage 11.5 |
| **合计** | **35** | |

### 4.2 新增负向测试 (per §9.4.3 1:3+ ratio)

| # | 文件 | 测试场景 |
|---|------|---------|
| 1 | e2e-err-021-void-fn-return-value.lin | `fn f() { return 42; }` |
| 2 | e2e-err-022-trait-impl-ret-mismatch.lin | trait fn 返回 i32, impl 返回 bool |
| 3 | e2e-err-023-trait-impl-arg-mismatch.lin | trait fn 1 arg, impl 2 args |
| 4 | e2e-err-024-if-branch-mismatch.lin | if-branches 不同类型 |
| 5 | e2e-err-025-match-arm-mismatch.lin | match arms 不同类型 |

### 4.3 正向回归测试 (确保 Fix 不破坏合法代码)

- `let x: bool = true;` ✅
- `let x: i32 = 42;` ✅
- `let x: i32 = if cond { 1 } else { 2 };` ✅
- `fn f() -> i32 { if cond { 1 } else { 2 } }` ✅
- `fn f() { return; }` ✅
- `let b: bool = (a == b);` ✅ (comparison 仍返回 bool)

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P0 修复是 typeck 正确性基础 |
| REV-A | GO | 严格按 Rust 语义，移除过度 coerce |
| DEV-A | GO | 实现简洁，复用现有 check_statement |
| QA-A | GO | 35 个 limitation 测试转 compile_error |
| PM-A | GO | P0 路线图项目 |

**5/5 GO** ✅

## 6. 验证步骤

```bash
cargo clean
cargo build --features llvm-backend
cargo fmt
cargo clippy --all-targets --features llvm-backend -- -D warnings
cargo test --features llvm-backend
python3 tests/conformance/run_all.py
```

## 7. 风险与缓解

### 7.1 风险：移除 Bool→Int 规则可能破坏现有合法代码

**缓解**：
- 现有 36 个 limitation 测试明确记录了这些场景为 Stage 0 缺口
- 移除规则后这些测试转为 compile_error，符合预期
- 全量 conformance 测试（5333 个）会暴露任何意外破坏

### 7.2 风险：void fn return local 改为 unit 可能影响 codegen

**缓解**：
- codegen 已对 unit 返回类型有处理（`ret void` for unit return functions）
- MIR lower 已有 `Aggregate(Tuple, [])` 处理 `return;`
- 全量测试会暴露任何 codegen 回归

### 7.3 风险：trait impl signature 校验可能误报

**缓解**：
- 仅当 trait method 有完整 signature (`p.ty.is_some()`) 时才校验
- self 参数类型通过 `resolve_self_param_type_for_sig` 解析（已有逻辑）
- 任何误报会立即在 conformance 测试中暴露
