# Stage 18.15 — println! 通解化 Phase 2.1: __landin_println 调用检测

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.298.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.10 注册了内置 macro_rules! (no-op 展开)，Stage 18.12
提取了 `emit_printf_call`。本阶段（Phase 2.1）在 codegen 的 Call
terminator 中添加 `__landin_println` 调用检测，为 Phase 2.2（修改
built-in macro body 真正展开为 Call）做准备。

**具体目标**:
1. 在 `codegen/terminator.rs` 的 `TerminatorKind::Call` 处理中，
   检测 callee name 是否为 `__landin_println` / `__landin_print` /
   `__landin_eprintln` / `__landin_eprint`
2. 如果是，调用 `emit_printf_call`（Stage 18.12 提取的函数）
3. 但**不修改** built-in macro body（仍是 no-op），所以这条路径
   暂时不会被触发
4. 这是**接口准备**——为 Phase 2.2 铺路

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | __landin_println 走 Call 路径，复用 emit_printf_call |
| §10 命名 | `is_landin_print_macro` / `codegen_print_call` |
| §11 接口隔离 | 检测逻辑在 codegen 内部 |
| 单一职责 | `is_landin_print_macro` 只检测；`codegen_print_call` 只 codegen |
| 高内聚低耦合 | 复用 Stage 18.12 的 emit_printf_call |
| 避免死代码 | 检测函数被 Call terminator 调用 |
| 避免分散内容 | print 宏检测集中在 terminator.rs |

## 3. 实现

### 3.1 新增辅助函数

```rust
// src/codegen/terminator.rs

/// Stage 18.15: Check if a function name is a Landin built-in print macro
/// runtime function (`__landin_println` / `__landin_print` /
/// `__landin_eprintln` / `__landin_eprint`).
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
fn is_landin_print_macro(name: &str) -> bool {
    matches!(
        name,
        "__landin_println" | "__landin_print"
            | "__landin_eprintln" | "__landin_eprint"
    )
}

/// Stage 18.15: Codegen a call to a Landin print macro runtime function.
///
/// Routes to `emit_printf_call` (Stage 18.12) with the appropriate
/// `newline` and `stderr` flags derived from the function name:
/// - `__landin_println` → newline=true,  stderr=false
/// - `__landin_print`   → newline=false, stderr=false
/// - `__landin_eprintln`→ newline=true,  stderr=true
/// - `__landin_eprint`  → newline=false, stderr=true
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
fn codegen_print_call(
    name: &str,
    args: &[Operand],
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    interner: &Rodeo,
    layouts: &AdtLayouts,
    mono_layouts: Option<&MonoLayoutMap>,
    fn_name_by_def_id: &HashMap<DefId, String>,
);
```

### 3.2 Call terminator 集成

在 `TerminatorKind::Call` 处理中，在获取 `fn_name` 后：

```rust
if let Some(ref name) = fn_name {
    if is_landin_print_macro(name) {
        // Stage 18.15: Route __landin_println etc. to emit_printf_call.
        codegen_print_call(
            name, &args, emitter, mir, interner,
            layouts, mono_layouts, fn_name_by_def_id,
        );
        // Handle destination (return value is void/unit).
        ...
        return;
    }
    // ... existing call codegen ...
}
```

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_still_works_via_special_case | println! 仍走 parser 特解 |
| 2 | positive | eprintln_still_works_via_special_case | eprintln! 仍走 parser 特解 |
| 3 | negative | is_landin_print_macro_println | 检测 __landin_println |
| 4 | negative | is_landin_print_macro_print | 检测 __landin_print |
| 5 | negative | is_landin_print_macro_eprintln | 检测 __landin_eprintln |
| 6 | negative | is_landin_print_macro_eprint | 检测 __landin_eprint |
| 7 | negative | is_landin_print_macro_rejects_other | 非 print 宏返回 false |
| 8 | negative | is_landin_print_macro_rejects_empty | 空字符串返回 false |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 535 lib (527 → 535, +8) + 2537 integration = **3,072** total, 0 failures

## 6. 结论

Stage 18.15 完成 println! Phase 2.1：__landin_println 调用检测接口
准备。codegen 现在能识别 `__landin_println` 等调用并路由到
`emit_printf_call`。但 built-in macro body 仍是 no-op，所以这条
路径暂不触发。

下一阶段 (Stage 18.16):
- Phase 2.2: 修改 built-in macro body 真正展开为 `__landin_println(...)`
- 这将激活 Phase 2.1 的检测路径
- 然后可以逐步移除 parser 特解
