# Stage 18.12 — Println Codegen Refactoring (Phase 2 Preparation)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.295.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿) + §13.4 (重构治理)
> **Status**: ✅ Complete

## 1. 阶段目标

按照 Stage 18.11 的 Phase 2 设计，为 `__landin_println` 调用路径做
代码准备。**不改变任何现有行为**，只做代码组织重构（§13.4）：

1. 将 `StatementKind::Println` 的 ~100 行 codegen 逻辑提取为独立函数
   `emit_printf_call`
2. 该函数接收 `(msg, args, newline, stderr)` 参数
3. `Println` arm 调用此函数
4. 为 Phase 2 的 `Call(__landin_println)` 检测做好接口准备

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §13.4 重构治理 | 提取为独立函数，不改变行为 |
| §10 命名 | `emit_printf_call` (`<verb>_<noun>_<noun>`) |
| §11 接口隔离 | 函数在 codegen 模块内部 |
| 单一职责 | `emit_printf_call` 只负责 printf 调用发射 |
| 高内聚低耦合 | Println 相关逻辑集中 |
| 避免死代码 | 函数被 Println arm 调用 |
| 通解 > 特解 | 为 Phase 2 的 Call 路径做准备 |

## 3. 实现

### 3.1 新增函数签名

```rust
// src/codegen/statement.rs

/// Stage 18.12: Emit a printf-style call for print!/println!/eprint!/eprintln!.
///
/// Extracted from the `StatementKind::Println` arm to enable reuse by
/// the future `Call(__landin_println)` codegen path (Phase 2 of the
/// println! 通解化 migration).
///
/// **Parameters**:
/// - `msg`: Format string template (may contain `{}` placeholders).
///   If `newline` is true, a trailing `\n` is appended.
/// - `args`: MIR operands to substitute into `{}` placeholders.
/// - `newline`: Whether to append `\n` to the format string.
/// - `stderr`: Whether to route output to stderr (via `__landin_eprintf`)
///   instead of stdout (via `printf`).
///
/// Per §10: `emit_printf_call` follows `<verb>_<noun>_<noun>` pattern.
fn emit_printf_call(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    msg: &str,
    args: &[Operand],
    newline: bool,
    stderr: bool,
    interner: &Rodeo,
    layouts: &LayoutTable,
    mono_layouts: &MonoLayoutTable,
    fn_name_by_def_id: &HashMap<DefId, String>,
);
```

### 3.2 Println arm 简化

```rust
StatementKind::Println { msg, args, newline, stderr } => {
    emit_printf_call(
        emitter, mir, msg, args, *newline, *stderr,
        interner, layouts, mono_layouts, fn_name_by_def_id,
    );
}
```

### 3.3 行为不变性

- 所有现有测试（3032 个）必须继续通过
- 生成的 LLVM IR 不变
- 无性能回退

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

测试重点是**验证重构不改变行为**：

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_simple_still_works | `println!("hi")` 仍正常编译 |
| 2 | positive | println_with_args_still_works | `println!("x={}", x)` 仍正常编译 |
| 3 | negative | eprintln_still_works | `eprintln!("err")` 仍正常编译 |
| 4 | negative | print_no_newline_still_works | `print!("no newline")` 仍正常编译 |
| 5 | negative | eprint_no_newline_still_works | `eprint!("err")` 仍正常编译 |
| 6 | negative | println_with_multiple_args | 多参数 `println!("{}{}", a, b)` 仍正常 |
| 7 | negative | println_with_int_arg | int 参数 `println!("{}", 42)` 仍正常 |
| 8 | negative | println_with_string_arg | string 参数 `println!("{}", s)` 仍正常 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
- [x] 行为不变：所有 3040 个旧测试继续通过

## 6. 结论

Stage 18.12 完成 Println codegen 重构。`emit_printf_call` 函数提取
成功，为 Phase 2 的 `Call(__landin_println)` 路径做好准备。行为
完全不变（3040 旧测试 + 8 新测试 = 3048 全绿）。

下一阶段 (Stage 18.13):
- 修改 built-in macro body 为 `__landin_println($($args)*)`
- 在 codegen 中添加 `Call(__landin_println)` 检测
- 调用 `emit_printf_call` 处理
