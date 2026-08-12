# Stage 18.29 — Built-in Non-Print Macros (assert!/panic!/vec!)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-07
> **Version**: v0.307.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**用户反馈**: "macro 不只有 print macro 这一类，还有其他很多的macro"

本阶段扩展 macro 系统的非 print 内置宏，添加:
1. `assert!(cond)` — 断言，失败时 panic
2. `panic!("msg")` — 立即终止程序
3. `vec![$($x),*]` — 创建数组

这些宏通过 `macro_rules!` 定义，走 `expand_macros` 通道（通解 > 特解）。

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | 所有内置宏走同一 expand_macros 通道 |
| §10 命名 | `BUILTIN_MACRO_NAMES` 扩展, `make_builtin_macro_rule` 复用 |
| §11 接口隔离 | 内置宏注册逻辑在 macro_expand.rs |
| 单一职责 | make_builtin_macro_rule 只构造 rule |
| 高内聚低耦合 | 内置宏定义集中 |
| 避免死代码 | 所有新宏被测试覆盖 |
| 避免分散内容 | 内置宏集中注册 |

## 3. 实现

### 3.1 BUILTIN_MACRO_NAMES 扩展

```rust
// Before:
pub const BUILTIN_MACRO_NAMES: &[&str] = &["println", "print", "eprintln", "eprint"];

// After:
pub const BUILTIN_MACRO_NAMES: &[&str] = &[
    "println", "print", "eprintln", "eprint",  // print macros
    "assert", "panic", "vec",                   // non-print macros
];
```

### 3.2 make_builtin_macro_rule 扩展

print 宏的 body 是 `__landin_<name>($($args)*)`。
非 print 宏需要不同的 body:

- `assert!($cond)` → body: `if !($cond) { __landin_panic("assertion failed"); }`
  简化版: `__landin_assert($cond)` (codegen 检测)
- `panic!($msg)` → body: `__landin_panic_msg($msg)` (codegen 检测)
- `vec!($($x),*)` → body: `[$($x),*]` (直接展开为数组字面量)

**简化方案** (本阶段采用):
- `assert!` 和 `panic!` 展开为 `__landin_assert(...)` / `__landin_panic_msg(...)` 调用
- `vec!` 展开为 `[$($args),*]` (数组字面量)
- codegen 检测 `__landin_assert` / `__landin_panic_msg` 并生成相应代码

### 3.3 make_builtin_macro_rule 分发

```rust
fn make_builtin_macro_rule(name: &str, name_sym: Symbol, interner: &mut Rodeo) -> MacroRule {
    match name {
        "println" | "print" | "eprintln" | "eprint" => make_print_macro_rule(name, name_sym, interner),
        "assert" => make_assert_macro_rule(interner),
        "panic" => make_panic_macro_rule(interner),
        "vec" => make_vec_macro_rule(interner),
        _ => make_noop_macro_rule(name_sym, interner),
    }
}
```

### 3.4 各宏的 rule 定义

**assert!**:
- Pattern: `$( $cond:expr )?` (optional expression — 简化为 `$cond:expr`)
- Body: `__landin_assert ( $cond )`

**panic!**:
- Pattern: `$( $msg:expr )?` (optional message)
- Body: `__landin_panic_msg ( $msg )`

**vec!**:
- Pattern: `$( $x:expr ),*`
- Body: `[ $( $x ),* ]`

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅
