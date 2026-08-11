# Stage 18.23 — codegen_print_call MIR Operand Handling

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.303.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.21 发现 `codegen_print_call` 只能从 `Operand::Constant`
提取 format string，不能从 `Operand::Move`/`Copy` 提取。这是因为 MIR
lowering 将字符串字面量先赋值给一个临时 local，然后用 `Operand::Move`
传递给 Call terminator。

**具体目标**:
1. 新增 `extract_format_string` 辅助函数
2. 当第一个 arg 是 `Operand::Constant(ConstVal::Str)` → 直接提取
3. 当第一个 arg 是 `Operand::Move/Copy(place)` → 扫描 MIR basic blocks
   查找 `Assign(place, Rvalue::Use(Constant(Str)))` → 追溯提取
4. 更新 `codegen_print_call` 使用新函数

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | 一个 `extract_format_string` 处理所有 operand 类型 |
| §10 命名 | `extract_format_string` (`<verb>_<noun>_<noun>`) |
| §11 接口隔离 | 辅助函数在 codegen::terminator 内部 |
| 单一职责 | `extract_format_string` 只提取 format string |
| 高内聚低耦合 | MIR 追溯逻辑集中在 codegen_print_call |
| 避免死代码 | `extract_format_string` 被 `codegen_print_call` 调用 |
| 避免分散内容 | format string 提取逻辑集中 |

## 3. 实现

### 3.1 extract_format_string 函数

```rust
/// Stage 18.23: Extract the format string from a call argument operand.
///
/// Handles two cases:
/// 1. `Operand::Constant(Const { val: Str(sym) })` → direct extraction
/// 2. `Operand::Move/Copy(place)` → trace back through MIR basic blocks
///    to find `Assign(place, Rvalue::Use(Constant(Str)))` → extract
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
fn extract_format_string(
    arg: &Operand,
    mir: &MirBody,
    interner: &Rodeo,
) -> String;
```

### 3.2 codegen_print_call 更新

```rust
// Before (Stage 18.21):
let msg = if let Some(first) = args.first() {
    match first {
        Operand::Constant(c) => match c.val {
            ConstVal::Str(sym) => interner.try_resolve(&sym)...,
            _ => String::new(),
        },
        _ => String::new(),  // ← Move/Copy not handled!
    }
} else { String::new() };

// After (Stage 18.23):
let msg = if let Some(first) = args.first() {
    extract_format_string(first, mir, interner)
} else {
    String::new()
};
```

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_constant_format_works | Constant arg format string |
| 2 | positive | println_move_format_traced | Move arg traced to constant |
| 3 | negative | extract_constant_str | 直接 Constant 提取 |
| 4 | negative | extract_move_traces_to_constant | Move 追溯到 Constant |
| 5 | negative | extract_copy_traces_to_constant | Copy 追溯到 Constant |
| 6 | negative | extract_non_string_constant_returns_empty | 非 Str constant → empty |
| 7 | negative | extract_no_assignment_returns_empty | 无赋值 → empty |
| 8 | negative | println_runtime_still_works | 运行时 println! 仍正常 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
  - 575 lib (567 → 575, +8) + 2537 integration = **3,112** total, 0 failures
