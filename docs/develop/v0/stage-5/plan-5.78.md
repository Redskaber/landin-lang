# Stage 5.78 开发计划：HirExprKind::MethodCall dyn Trait 集成

> **阶段**: Stage 5.78
> **版本**: v0.11.73 → v0.11.74
> **状态**: ✅ Complete

## 1. 目标

**首次**在 `mir/lower/` 中实际使用 dyn Trait 数据：修改
`lower_expr_to_operand` 的 `HirExprKind::MethodCall` 分支，当
`cx.dyn_trait_plan()` 返回 `Some` 且 `find_dyn_trait_method_call_in_plan_by_method()`
找到匹配项时，使用 dyn Trait 专用的 `Terminator::Call` 替换原来的
`Error` placeholder func。

同时添加 free function `build_dyn_trait_call_terminator()` —— 构造
dyn Trait 方法调用的 `Terminator::Call`，便于测试和后续 codegen 复用。

## 2. 设计动机

Stage 5.74-5.77 完成了所有 dyn Trait MIR 基础设施：
- 5.74 完整 IR 文本生成器
- 5.75 精确查询 (`find_dyn_trait_method_call_in_plan`)
- 5.76 `MirLowerCtxt` 上下文接线 (`dyn_trait_plan` 字段 + setter/getter)
- 5.77 模糊查询 (`find_dyn_trait_method_call_in_plan_by_method`)

但 `HirExprKind::MethodCall` 分支仍使用 `Error` placeholder func——
这是 Stage 2.1 的占位逻辑。Stage 5.78 **首次**让 dyn Trait 数据
真正影响 MIR 输出。

## 3. 设计

### 3.1 新增 API

```rust
/// Stage 5.78: Build a `Terminator::Call` for a dyn Trait method call.
///
/// The function operand is a `Const` whose `ConstVal::Str` symbol
/// encodes the trait/type/method triple — codegen (Stage 5.79+) will
/// translate this marker into a vtable indirect call.
pub fn build_dyn_trait_call_terminator(
    call: &DynTraitMethodCall,
    recv_local: LocalId,
    arg_locals: &[LocalId],
    dest: LocalId,
    span: Span,
) -> Terminator
```

### 3.2 修改 `HirExprKind::MethodCall` 分支

伪代码：

```rust
HirExprKind::MethodCall { receiver, method, args, .. } => {
    let recv_local = lower_expr_to_operand(cx, receiver);
    let arg_locals: Vec<LocalId> = args.iter().map(...).collect();

    // Stage 5.78: dyn Trait path
    let dyn_terminator = cx.dyn_trait_plan().and_then(|plan| {
        let method_name = cx.interner.resolve(&method.name).to_string();
        find_dyn_trait_method_call_in_plan_by_method(plan, &method_name)
    }).map(|call| {
        let dest_ty = cx.fresh_infer_ty(expr.span);
        let dest = cx.mir.new_local(dest_ty, None, expr.span);
        let cont = cx.new_block();
        let terminator = build_dyn_trait_call_terminator(
            call, recv_local, &arg_locals, dest, expr.span);
        cx.terminate_and_goto(terminator, cont);
        dest
    });

    if dyn_terminator.is_some() {
        return dyn_terminator.unwrap();
    }

    // Legacy placeholder path (unchanged)
    // ... existing code ...
}
```

### 3.3 Const marker 编码

dyn Trait 的 `Operand::Constant` 用一个特殊 Const 标记：

```rust
Const {
    ty: Box::new(Ty::new(TyKind::Error, Span::DUMMY)),  // placeholder type
    val: ConstVal::Str(symbol_for("dyn.<trait>.<type>.<method>")),
}
```

codegen (Stage 5.79+) 通过检查 `ConstVal::Str` 内容识别 dyn Trait 调用，
翻译成 vtable indirect call。

### 3.4 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `build_dyn_trait_call_terminator` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |

参考 §8.1 helper-verb `build_` 前缀，与 `build_dyn_trait_mir_plan` 等
Stage 5.61-5.74 的构建函数同家族。

### 3.5 §16 接口隔离

- `build_dyn_trait_call_terminator` 在 `mir::lower` 中定义
- 输入：`&DynTraitMethodCall` (来自 `mir::dyn_trait`) + `LocalId`/`Span`
  (来自 `mir::place`/`session`)
- 输出：`Terminator` (来自 `mir::body`)
- 数据流：`mir::dyn_trait` → `mir::lower` → `mir::body`，单向依赖
- 不引入新依赖

### 3.6 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | `build_dyn_trait_call_terminator` 基本构造 | 返回 `Terminator::Call` |
| 2 | 函数操作数是 `Operand::Constant` | ✓ |
| 3 | 函数操作数的 `ConstVal::Str` 包含 trait.type.method | ✓ |
| 4 | 参数列表第一项是 self (receiver) | ✓ |
| 5 | 参数列表后续项是 args | ✓ |
| 6 | destination 是给定 local | ✓ |
| 7 | target 是 Some（cont block 由调用方设置） | 实际由 cx.terminate_and_goto 设置 |
| 8 | cx 无 plan 时，MethodCall 走旧路径（Error placeholder） | ✓ |
| 9 | cx 有 plan 但 method 不匹配时，走旧路径 | ✓ |
| 10 | cx 有 plan 且 method 匹配时，走 dyn Trait 路径 | 函数操作数是 Str |
| 11 | 跨多个测试：相同 method_name 跨 traits，first-match-wins | 用第一个 |
| 12 | 集成测试：lower + plan 后 MIR 中含 dyn Trait 标记 | ✓ |

## 4. 不在本 stage 范围

- ❌ codegen 实际翻译 dyn Trait Const marker 为 vtable indirect call
  （Stage 5.79+）
- ❌ 在 driver 中自动调用 `set_dyn_trait_plan`（Stage 5.80+）
- ❌ MethodCall 的非 dyn Trait 路径（struct/enum inherent methods）——
  这些仍走 Error placeholder，是后续 stage 的工作

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
