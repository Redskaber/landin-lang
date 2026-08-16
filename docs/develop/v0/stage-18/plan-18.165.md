# Plan 18.165 — Option/Result 内置类型实现 (stdlib 第一步)

> **Author**: redskaber (PM-A + ARCH-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.433.0 (Stage 18.165 plan)
> **Process**: docs/stage-committee-process.md v6.4 §13.1 (设计对齐) + §13.4 (重构即架构设计)
> **Complexity**: L3 (新增 prelude 注入系统 + 内置类型)
> **Task ID**: stage18.165

## 1. 任务审查 (§5.1 复杂度预评估)

### 1.1 能力具备性

| 维度 | 评估 | 详情 |
|------|------|------|
| Enum 支持 | ✅ 具备 | AST/HIR/MIR/codegen 全链路支持 enum |
| 泛型支持 | ✅ 具备 | 泛型函数/结构体/枚举已实现 |
| 模式匹配 | ✅ 具备 | match + enum variant pattern 已实现 |
| Prelude 注入 | ❌ 不具备 | 当前无 prelude 注入机制 |
| 内置类型定义 | ❌ 不具备 | Option/Result 仅作为名称注册, 无实际类型定义 |

### 1.2 阻塞项

**无阻塞项** — Option/Result 不依赖 heap allocation (enum 是栈分配), 可直接实现。

### 1.3 复杂度评估

- **代码变动量**: ~200 LOC (prelude 注入 + Option/Result 定义)
- **依赖风险**: 中 (修改 HIR lowering 入口, 影响 resolve/typeck)
- **历史缺陷密度**: 低 (prelude 是新功能, 无历史)

**复杂度等级**: L3 (核心架构 — prelude 注入是新的基础设施)

## 2. 设计文档对齐 (§13.1)

### 2.1 对应设计文档

- `docs/lang-design/09-stdlib.md` §2.4 (Option 与 Result)

### 2.2 设计意图

Option/Result 是 Landin 标准库的核心类型, 应作为 prelude 自动导入。设计文档定义了完整的 Option/Result API (is_some, unwrap, map, and_then 等)。

### 2.3 本 stage 范围

**In scope** (本 stage):
- Option<T> enum 定义 (Some/None)
- Result<T, E> enum 定义 (Ok/Err)
- Prelude 注入机制 (自动导入 Option/Result)
- 基本构造: Some(x), None, Ok(x), Err(e)
- 基本方法: is_some, is_none, is_ok, is_err, unwrap, unwrap_or

**Out of scope** (后续 stage):
- map/and_then/or 等高级方法 (依赖闭包 + trait bound)
- take (依赖 core::mem::replace)
- format! (依赖 String)

### 2.4 灰区决策

**灰区1**: Option/Result 用 Landin 源码实现还是编译器内置?
- **决策**: 编译器内置 (注入 HIR), 因为:
  1. Landin 自举尚未完成 (不能用 Landin 写 stdlib)
  2. 内置方式可立即工作, 无需等待自举
  3. 后续自举完成后可替换为 Landin 源码
- **简写记录**: 内置方式是临时方案, 自举完成后替换为 Landin 源码。

**灰区2**: Option/Result 方法如何实现?
- **决策**: 本 stage 仅实现构造 + 基本查询方法 (is_some/is_none/is_ok/is_err/unwrap/unwrap_or)。高级方法 (map/and_then) 依赖闭包和 trait bound, 推迟到后续 stage。
- **简写记录**: 高级方法推迟, 记录为 TD-OPTION-ADVANCED-METHODS。

## 3. 架构设计 (§13.4 J1-J6)

### 3.1 新结构

```
src/stdlib/
├── mod.rs                  # 现有: 类型注册 + prelude
├── trait_methods.rs        # 现有: trait 方法
├── vtable_layout.rs        # 现有: vtable 布局
└── prelude.rs              # 新增: 内置 prelude 类型注入 (Option/Result)
```

### 3.2 J1-J6 评估

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | 对齐 09-stdlib.md §2.4 | ✅ |
| J2 单一职责 | prelude.rs 仅负责内置类型注入 | ✅ |
| J3 单向流动 | driver → prelude inject → HIR lower (无环) | ✅ |
| J4 编译相关表达完整 | Option/Result 定义完整在一个模块 | ✅ |
| J5 阶段划分清晰 | prelude 注入在 HIR lower 之前 | ✅ |
| J6 科学合理粒度 | prelude.rs ~150 LOC, 合理 | ✅ |

## 4. 实现方案

### 4.1 Prelude 注入机制

在 `compile_inner` 的 HIR lowering 之前, 注入 Option/Result 的 AST 定义:

```rust
// src/stdlib/prelude.rs
pub fn inject_prelude(krate: &mut ast::Crate) {
    // 注入 Option<T> enum
    krate.items.push(make_option_enum());
    // 注入 Result<T, E> enum
    krate.items.push(make_result_enum());
}
```

### 4.2 Option<T> 定义

```landin
enum Option<T> {
    None,
    Some(T),
}
```

### 4.3 Result<T, E> 定义

```landin
enum Result<T, E> {
    Ok(T),
    Err(E),
}
```

### 4.4 调用点

`compile_inner` 在 parse 之后、HIR lower 之前调用 `inject_prelude`:
```
parse_crate → inject_prelude (AST 注入) → lower_crate (HIR) → resolve → ...
```

## 5. 测试计划 (§9)

### 5.1 测试矩阵

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `test_option_some_construction` | 正向 | `let x = Some(42)` 编译通过 |
| `test_option_none_construction` | 正向 | `let x: Option<i32> = None` 编译通过 |
| `test_result_ok_construction` | 正向 | `let x = Ok(42)` 编译通过 |
| `test_result_err_construction` | 正向 | `let x = Err("error")` 编译通过 |
| `test_option_pattern_match` | 正向 | `match x { Some(v) => v, None => 0 }` |
| `test_option_type_annotation` | 正向 | `let x: Option<bool> = Some(true)` |
| `test_option_wrong_type` | 负向 | `let x: Option<i32> = Some(true)` 类型错误 |
| `test_result_pattern_match` | 正向 | `match x { Ok(v) => v, Err(e) => 0 }` |

## 6. 简写和缺陷记录

### 6.1 简写

**简写1**: Option/Result 方法 (is_some/unwrap/map 等) 本 stage 不实现。
- **原因**: 方法实现需要 impl 块 + trait bound, 复杂度高。
- **修订计划**: Stage 18.166 实现 is_some/is_none/is_ok/is_err/unwrap/unwrap_or。

**简写2**: Option/Result 用编译器内置 (AST 注入) 而非 Landin 源码。
- **原因**: Landin 自举未完成。
- **修订计划**: 自举完成后替换为 Landin 源码 (v0.3+)。

### 6.2 新技术债

- **TD-OPTION-ADVANCED-METHODS**: Option/Result 高级方法 (map/and_then/or/take) 未实现, 依赖闭包 + trait bound。
