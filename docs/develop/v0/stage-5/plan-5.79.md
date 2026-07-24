# Stage 5.79 开发计划：codegen dyn Trait vtable indirect call

> **阶段**: Stage 5.79
> **版本**: v0.11.74 → v0.11.75
> **状态**: ✅ Complete

## 1. 目标

在 codegen 中识别 Stage 5.78 引入的 dyn Trait Const marker
(`Const{ty: Error, val: Int(index)}` where `index < mir.dyn_trait_calls.len()`)，
读取 `mir.dyn_trait_calls[index]` 获取 `(trait, type, method, slot_index, param_count)`，
emit vtable indirect call：
1. `getelementptr` 从 dynptr 全局获取 vtable 指针（第二字段）
2. `load` 从 vtable 加载方法函数指针（slot_index 偏移）
3. `call` 间接调用加载的函数指针（self + args）

添加新的 emitter trait method `emit_dyn_trait_method_call()` 和 free function
`codegen_dyn_trait_call()`，并修改 `codegen_terminator` 的 `Terminator::Call`
分支在检测到 marker 时 dispatch 到新路径。

## 2. 设计

### 2.1 新增 Emitter trait method

```rust
/// Stage 5.79: Emit a dyn Trait vtable indirect call.
///
/// Produces LLVM IR that:
/// 1. Loads the vtable pointer from the dynptr global (second field, index 1)
/// 2. Loads the method function pointer from the vtable at slot_index
/// 3. Calls the loaded function pointer with the given args
///
/// Args list already includes self as the first element.
fn emit_dyn_trait_method_call(
    &mut self,
    dynptr_symbol: &str,
    slot_index: u32,
    args: &[(EmitType, &EmitValue)],
    ret_ty: &EmitType,
) -> EmitValue;
```

### 2.2 新增 free function

```rust
/// Stage 5.79: Codegen a dyn Trait method call.
///
/// Reads `mir.dyn_trait_calls[index]` to get the (trait, type, method,
/// slot_index, param_count) info, computes the dynptr symbol
/// (`.dynptr.<trait>.<type>`), and calls
/// `emitter.emit_dyn_trait_method_call()` with the slot_index + args.
pub fn codegen_dyn_trait_call(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    index: u128,
    args: &[Operand],
    interner: &Rodeo,
    layouts: &AdtLayouts,
) -> EmitValue
```

### 2.3 Terminator::Call dispatch 修改

在 `codegen_terminator` 的 `Terminator::Call` 分支顶部，检查 `func`：

```rust
Terminator::Call { func, args, destination, target } => {
    // Stage 5.79: dyn Trait path
    if let Operand::Constant(c) = func {
        if matches!(c.ty.kind, TyKind::Error) {
            if let ConstVal::Int(idx) = c.val {
                if (idx as usize) < mir.dyn_trait_calls.len() {
                    let ret_val = codegen_dyn_trait_call(
                        emitter, mir, idx, args, interner, layouts);
                    // store ret_val to destination, emit branch to target
                    return;
                }
            }
        }
    }
    // ... existing legacy path ...
}
```

### 2.4 命名标准化

| API | 命名规则 | 合规 |
|-----|---------|------|
| `emit_dyn_trait_method_call` | `<verb>_<noun>_<noun>_<noun>_<noun>` | ✅ |
| `codegen_dyn_trait_call` | `<verb>_<noun>_<noun>_<noun>` | ✅ |

`emit_` 前缀（§8.1 codegen emit 约定，与 `emit_call`/`emit_load` 同家族）。
`codegen_` 前缀（§8.1 codegen top-level entry，与 `codegen_terminator`/`codegen_operand` 同家族）。

### 2.5 §16 接口隔离

- `emit_dyn_trait_method_call` 在 `codegen::emitter` trait + `codegen::text_emitter` impl
- `codegen_dyn_trait_call` 在 `codegen::mod`
- 输入：`&MirBody` (mir::body) + index + args + interner + layouts
- 输出：`EmitValue` (codegen 内部)
- 数据流：mir → codegen 单向，无循环依赖
- 不引入新依赖

### 2.6 标记检测的安全性

- 必须满足三个条件才走 dyn Trait 路径：
  1. `func` 是 `Operand::Constant`
  2. `c.ty.kind` 是 `TyKind::Error`（marker 约定）
  3. `c.val` 是 `ConstVal::Int(idx)` 且 `idx < mir.dyn_trait_calls.len()`
- 否则回退到 legacy 路径——保证现有测试不受影响

### 2.7 测试矩阵

| # | 用例 | 期望 |
|---|------|------|
| 1 | emit_dyn_trait_method_call 基本：返回 EmitValue | ✓ |
| 2 | IR 包含 getelementptr 指令 | ✓ |
| 3 | IR 包含 load 指令（vtable 指针 + method fn 指针） | ✓ |
| 4 | IR 包含 call 指令（间接调用） | ✓ |
| 5 | IR 引用正确的 dynptr symbol `.dynptr.<trait>.<type>` | ✓ |
| 6 | IR 使用正确的 slot_index 偏移 | ✓ |
| 7 | codegen_dyn_trait_call: index 0 返回有效 EmitValue | ✓ |
| 8 | codegen_dyn_trait_call: 多个 args 正确传递 | ✓ |
| 9 | codegen_terminator 检测 marker 后走 dyn Trait 路径 | IR 含 vtable 间接调用 |
| 10 | 无 marker 时走 legacy 路径（向后兼容） | 现有测试通过 |
| 11 | index 越界时回退到 legacy 路径 | ✓ |
| 12 | emit_dyn_trait_method_call 与 emit_call 输出可区分 | ✓ |

## 3. 不在本 stage 范围

- ❌ driver 自动调用 `set_dyn_trait_plan`（Stage 5.80+）
- ❌ 实际的 LLVM IR 链接验证（需要 LLVM 工具链）
- ❌ 非 dyn Trait 的 MethodCall 路径（struct/enum inherent methods）

---

**创建日期**: 2026-07-24
**作者**: Super Z (main)
