# Stage 18.18 — println! 通解化 Phase 2.2: Activate __landin_println Detection

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.300.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.15 准备了 `is_landin_print_macro` + `codegen_print_call`
接口（标记 `#[allow(dead_code)]`）。本阶段（Phase 2.2）**激活**这条
路径：

1. 在 `codegen/terminator.rs` 的 `TerminatorKind::Call` 处理中，
   检测 callee name 是否为 `__landin_println` 等
2. 如果是，调用 `codegen_print_call`（Stage 18.15 准备的函数）
3. 移除 `#[allow(dead_code)]` 标注（函数现在被使用了）

**注意**: 本阶段**不**修改 built-in macro body（仍是 no-op）。
所以 `__landin_println` 调用路径仍不会被触发——但接口已激活，
为 Phase 2.3（修改 macro body）铺路。

实际上，由于 built-in macro body 仍是 no-op（`name!($($args)*)`），
parser 仍走特解路径生成 `Println` variant，codegen 仍走
`StatementKind::Println` arm。`__landin_println` 调用路径只有在
用户显式调用 `__landin_println(...)` 时才会触发（罕见）。

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | __landin_println 走 Call 路径 |
| §10 命名 | 复用 is_landin_print_macro / codegen_print_call |
| §11 接口隔离 | 检测在 codegen 内部 |
| 单一职责 | 检测只检测；codegen_print_call 只 codegen |
| 高内聚低耦合 | 复用 Stage 18.12 的 emit_printf_call |
| 避免死代码 | 移除 #[allow(dead_code)]，函数现在被使用 |
| 避免分散内容 | 检测逻辑集中在 terminator.rs |

## 3. 实现

### 3.1 Call terminator 集成

在 `TerminatorKind::Call` 处理中，在获取 `fn_name` 后，添加
`__landin_println` 检测：

```rust
// src/codegen/terminator.rs
if let Some(ref name) = fn_name {
    // Stage 18.18: Detect __landin_println etc. and route to
    // codegen_print_call (which calls emit_printf_call).
    if is_landin_print_macro(name) {
        codegen_print_call(
            name,
            args,
            emitter,
            mir,
            interner,
            layouts,
            mono_layouts,
            fn_name_by_def_id,
        );
        // Handle destination (return value is void/unit).
        if let Some(target) = target {
            emitter.emit_br(&format!("bb{}", target.0));
        } else {
            emitter.emit_unreachable();
        }
        return; // Don't fall through to regular call codegen.
    }
    // ... existing call codegen ...
}
```

### 3.2 移除 #[allow(dead_code)]

`is_landin_print_macro` 和 `codegen_print_call` 现在被 Call terminator
调用，所以移除 `#[allow(dead_code)]` 标注。

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

由于 `__landin_println` 路径需要用户显式调用（built-in macro 仍 no-op），
测试重点验证：
1. 现有 println! 仍工作（走特解路径）
2. `is_landin_print_macro` 检测正确
3. codegen_print_call 能被调用（通过检测函数）

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_still_works_after_activation | println! 仍正常 |
| 2 | positive | eprintln_still_works_after_activation | eprintln! 仍正常 |
| 3 | negative | is_landin_print_macro_still_detects | 检测仍正确 |
| 4 | negative | is_landin_print_macro_println_after_activation | 检测 __landin_println |
| 5 | negative | is_landin_print_macro_eprintln_after_activation | 检测 __landin_eprintln |
| 6 | negative | regular_call_not_affected | 普通函数调用不受影响 |
| 7 | negative | print_macro_not_broken_by_activation | print! 宏不受影响 |
| 8 | negative | macro_rules_user_macro_not_affected | 用户宏不受影响 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 551 lib (543 → 551, +8) + 2537 integration = **3,088** total, 0 failures

## 6. 结论

Stage 18.18 完成 println! Phase 2.2：激活 `__landin_println` 调用检测。
Call terminator 现在能识别 `__landin_println` 等调用并路由到
`emit_printf_call`。`#[allow(dead_code)]` 标注已移除。

下一阶段 (Stage 18.19):
- v0.6 P2.5 review: 评估平衡, 规划 Phase 2.3 (修改 macro body)
- 或继续 macro 系统改进
