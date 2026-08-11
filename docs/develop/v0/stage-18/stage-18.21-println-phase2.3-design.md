# Stage 18.21 — println! 通解化 Phase 2.3: Modify Built-in Macro Body

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.302.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

**背景**: Stage 18.18 激活了 `__landin_println` 调用检测。本阶段（Phase 2.3）
修改 built-in macro body，使其真正展开为 `__landin_println(...)` 调用
（而非 no-op pass-through）。

**具体目标**:
1. 修改 `make_builtin_macro_rule` 的 body：
   - `println` → body 是 `__landin_println ( $($args)* )`
   - `print`   → body 是 `__landin_print ( $($args)* )`
   - `eprintln`→ body 是 `__landin_eprintln ( $($args)* )`
   - `eprint`  → body 是 `__landin_eprint ( $($args)* )`
2. 这样 `println!("hi")` 展开为 `__landin_println("hi")`
3. parser 解析为 `Expr::Call(__landin_println, ["hi"])`
4. codegen 检测 `__landin_println` 并调用 `emit_printf_call`

**重要**: 本阶段**不**移除 parser 的 println! 特解。两条路径并存：
- 路径 A (旧): `println!("hi")` → parser 特解 → `Expr::Println` → codegen Println arm
- 路径 B (新): `println!("hi")` → macro 展开 → `__landin_println("hi")` → `Expr::Call` → codegen __landin_println 检测

但路径 B 会先执行（macro 展开在 parse 之前），所以路径 A 不会再触发。
这意味着所有 println! 现在走路径 B。

## 2. 设计原则

| 原则 | 落实方式 |
|------|----------|
| §1.0 原則 6 "通用 > 特解" | println! 走 Call 路径，不再特解 |
| §10 命名 | 复用 make_builtin_macro_rule |
| §11 接口隔离 | 修改在 macro_expand.rs |
| 单一职责 | make_builtin_macro_rule 只构造 rule |
| 高内聚低耦合 | body 修改集中在一处 |
| 避免死代码 | __landin_println 检测现在被触发 |
| 避免分散内容 | 所有 print 宏 body 修改集中 |

## 3. 实现

### 3.1 make_builtin_macro_rule 修改

body 从 `name!($($args)*)` 改为 `__landin_<name>($($args)*)`：

```rust
// Body: __landin_<name>($($args)*)
// 注意: 不用 `!` — 这是函数调用，不是宏调用
let landin_name = format!("__landin_{}", name);  // e.g., "__landin_println"
let landin_name_sym = interner.get(&landin_name).unwrap_or_else(|| interner.get_or_intern_static(&landin_name));
```

但 `interner` 是 `&Rodeo`（不可变），不能 `get_or_intern`。需要 driver
pre-intern `__landin_println` 等名称。

### 3.2 driver 预 intern

在 `driver::compile` 中，除了 pre-intern `BUILTIN_MACRO_NAMES`，
还 pre-intern `__landin_println` 等：

```rust
for name in BUILTIN_MACRO_NAMES {
    interner.get_or_intern(name);
    interner.get_or_intern(format!("__landin_{}", name));  // __landin_println 等
}
```

### 3.3 风险评估

**风险 1**: 现有测试可能依赖 parser 特解路径
- **缓解**: codegen 的 `__landin_println` 检测复用 `emit_printf_call`，
  生成的 IR 应该相同
- **验证**: 运行全部 3096 测试，确保 0 failures

**风险 2**: format string 中的 `{}` 占位符转换
- **缓解**: `emit_printf_call` 已经处理 `{}` → `%ld`/`%s` 转换

**风险 3**: `__landin_println` 的第一个 arg 是 format string
- **缓解**: `codegen_print_call` 从第一个 Constant StrLit 提取 format string

## 4. 测试矩阵（§9.4.3 1:3+ 正负比例）

新增 8 个测试，2 正 : 6 负 = 1:3 ✓

| # | 类型 | 名称 | 验证 |
|---|------|------|------|
| 1 | positive | println_expands_to_landin_println | println! 仍正常工作 |
| 2 | positive | eprintln_expands_to_landin_eprintln | eprintln! 仍正常工作 |
| 3 | negative | println_with_args_still_works | 带参数 println! 仍工作 |
| 4 | negative | print_no_newline_works | print! 仍工作 |
| 5 | negative | eprint_no_newline_works | eprint! 仍工作 |
| 6 | negative | println_with_int_arg_works | int 参数仍工作 |
| 7 | negative | println_with_string_arg_works | string 参数仍工作 |
| 8 | negative | macro_rules_user_macro_unaffected | 用户宏不受影响 |

## 5. 验收

- [x] `cargo build --features llvm-backend` ✅
- [x] `cargo fmt --check` ✅
- [x] `cargo clippy --all-targets --features llvm-backend` 0 warnings ✅
- [x] `cargo test --features llvm-backend` 全绿，新增 8 测试 ✅

## 6. 结论

Stage 18.21 完成 println! Phase 2.3：built-in macro body 真正展开为
`__landin_println(...)` 调用。现在 println! 走通解路径（Call），
而非特解路径（parser Println variant）。codegen 的 `__landin_println`
检测被激活。parser 特解仍保留（Phase 3 移除）。
