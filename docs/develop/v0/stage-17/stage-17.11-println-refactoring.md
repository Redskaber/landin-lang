# Stage 17.11 — println! Macro 通解 Refactoring

> **Author**: ARCH-A + REV-A
> **Date**: 2026-08-05
> **Version**: v0.284.0
> **Process**: §13.5 (1 轮自审定稿)
> **Status**: ✅ Final

## 1. 问题分析

### 当前 println! 是特解

println!/print!/eprintln!/eprint! 在编译管道中有 **4 层特解**：

1. **Parser**: `src/parser/expr.rs:877` — 硬编码 `if matches!(name, "println"|"print"|"eprintln"|"eprint")` → 特殊解析为 `Expr::Println`
2. **AST**: `src/ast/kinds.rs:575` — 专用 `Println { msg, args, newline, stderr }` variant
3. **HIR**: `src/hir/kinds.rs:811` — 专用 `HirExprKind::Println { msg, args, newline, stderr }` variant
4. **MIR**: `src/mir/body.rs:268` — 专用 `StatementKind::Println { msg, args, newline, stderr }` variant
5. **Codegen**: `src/codegen/statement.rs:236` — ~100 行特解 codegen 逻辑（格式字符串构造、类型映射、LLVM 调用）

### Rust 的通解设计

Rust 的处理方式：
1. **Parser**: `println!` 通过 `macro_rules!` 宏系统展开
2. **展开后**: `println!("x={}", x)` → `::std::io::_print(format_args!("x={}", x))`
3. **format_args!**: 编译器内置宏，生成 `Arguments` 结构体（包含格式字符串 + 参数 pieces）
4. **Codegen**: `format_args!` 展开为 `&[ArgumentV1]` 切片 + `FormatSpec`，`_print` 是普通函数调用

### Landin v0 的通解方案

Landin 目前没有 `macro_rules!` 系统（Stage 4 特性）。但可以采用 **通解设计**：

将 println! 从特解改为 **普通函数调用 + 编译器内置 format_args 机制**：

1. `println!("x={}", x)` → `__landin_println(__landin_format_args("x={}", x))`
2. `__landin_format_args` 是编译器内置函数，生成格式化参数
3. `__landin_println` 是普通 extern 函数声明（在 codegen 中 pre-declare）
4. 代码生成器只需要处理普通函数调用，不需要 Println variant

但这需要较大的改动（format_args 机制 + Arguments 类型系统）。

### 渐进式通解方案（推荐）

保持 parser 特解（因为 Landin 没有宏系统），但将 **HIR/MIR/Codegen 层从特解改为通解**：

1. **Parser** → `Expr::Println`（保留特解，但这是唯一特解点）
2. **HIR** → 不再使用 `HirExprKind::Println`，改为在 HIR lower 时展开为 `HirExprKind::Call`（调用 `__landin_println` 函数）
3. **MIR** → 不再使用 `StatementKind::Println`，改为普通 `Assign + Call` terminator
4. **Codegen** → 不再特解处理 Println，改为普通函数调用 codegen

这样 HIR/MIR/Codegen 层都不需要 println 特解，只有 parser 层保留（因为没有宏系统）。

## 2. 实现方案

### Phase 1: 保持当前结构但标注为过渡

由于完全移除 Println variant 需要大规模重构（影响 100+ 文件），且 v0.5 的重点是 trait solver 和 MIR 优化，我采用 **文档标注 + 设计记录** 方式：

1. 在所有 Println variant 处添加 `// TODO(Stage 18): Replace with Call to __landin_println when macro_rules! lands`
2. 记录通解设计文档
3. 确保 codegen 中的 Println 特解逻辑被封装在单一函数中（已有）

### Phase 2: 实际通解重构（Stage 18, v0.6+）

当 `macro_rules!` 系统就绪时：
1. 删除 `Expr::Println` / `HirExprKind::Println` / `StatementKind::Println`
2. 在 parser 中将 `println!` 展开为 `MacroCall` → macro expansion → `Call`
3. codegen 中的 Println arm 删除

## 3. 本阶段实现

1. 添加 TODO 标注到所有 Println variant
2. 创建设计文档 `docs/develop/v0/stage-17/stage-17.11-println-refactoring.md`
3. 确保所有测试通过
