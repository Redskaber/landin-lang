# Stage 5.5 开发计划：vtable 生成基础

> **阶段**: Stage 5.5
> **版本**: v0.11.3 → v0.11.4
> **状态**: ✅ Complete (with Stage 5.6 amendment)
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1
> **注意**: 当前环境 Rust 工具链不可用，代码变更基于现有模式编写，
>   验证待环境恢复后执行。

## 1. 目标

在 TraitResolver 中添加 vtable 数据结构，为 L5 trait dispatch 奠定基础。
vtable 是实现 `dyn Trait` 动态分派的关键基础设施。

## 2. 背景

当前 TraitResolver 已收集 trait 定义 + impl 块 + DefId→name 映射（Stage 5.1-5.4）。
Stage 5.5 添加：
- `VtableEntry` — vtable 中的一个方法条目
- `Vtable` — 完整的 vtable（trait name + self ty name + impl DefId + 方法条目列表）
- TraitResolver 在 `collect()` 时为每个 `impl Trait for Type` 构建 vtable

## 3. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.5-a | 定义 `VtableEntry` + `Vtable` 数据结构 | L1 |
| 5.5-b | 在 `collect()` 时为每个 trait impl 构建 vtable | L2 |
| 5.5-c | 添加 `find_vtable(trait_name, type_name)` 查询方法 | L1 |
| 5.5-d | 添加测试 | L1 |

## 4. 验收标准

1. 代码变更语法正确（基于现有模式）
2. `cargo fmt --check` 零 diff（待环境恢复验证）
3. `cargo clippy --all-targets` 0 warnings（待环境恢复验证）
4. `cargo test` 全部通过（待环境恢复验证）
5. §17.3 三阶段文档协议执行

## 5. Stage 5.6 修订说明（retroactive）

原始设计中 `VtableEntry` 字段为 `fn_def_id: DefId`。在 Stage 5.6 实施
vtable codegen 发射时发现：HIR 不为 impl 方法分配独立 DefId（impl 块
是 owner），原 `fn_def_id` 实际指向 impl 块而非方法。Stage 5.6 将此
字段替换为 `fn_name: String`（解析后的 LLVM 符号名 `landin_<Type>_<method>`）。

**影响**:
- Stage 5.5 引入的代码（`VtableEntry` 定义 + `collect()` 中的构建逻辑）
  被 Stage 5.6 修订
- Stage 5.5 测试 `test_vtable_*` 仍通过（仅断言 `vtable_count()`，未
  断言 `fn_def_id`）
- 测试 `test_vtable_query` 在 Stage 5.5 audit 阶段补入，验证 `find_vtable`
  返回的 entries 内容

修订详情见 `plan-5.6.md` §3.1。

## 6. 测试增强（audit 补入）

原始 5.5 测试仅覆盖 `vtable_count()`。Audit 阶段补入 `test_vtable_query`
验证 `find_vtable` 返回的 entries 内容（method_name + fn_name）。

## 7. 测试基础设施重构（audit 二轮）

audit 二轮发现 `tests/` 目录存在 14 个 legacy flat `.rs` 文件（11489 行），
与 `tests/v0/stage{N}/plan/` 下的组织化文件 100% 重复。同时 `Cargo.toml`
有 19 个 `[[test]]` 条目，使配置文件被测试部分填满。

重构内容：
- 删除 14 个 legacy flat 文件（`lexer.rs`, `parser.rs`, `codegen_tests.rs`
  等 11489 行重复代码）
- 新建 `tests/all_tests.rs` 统一入口（23 个 `#[path] mod` 声明）
- `Cargo.toml` 添加 `autotests = false` + 单一 `[[test]]` 条目
- Cargo.toml 行数：130 → 38（71% 缩减）
- 测试逻辑零改动：1017 测试预期不变

新增测试文件流程：在 `tests/v0/stage{N}/plan/` 添加文件 → 在
`tests/all_tests.rs` 添加一行 `#[path]` 声明 → 完成（无需改 Cargo.toml）。

---

**创建日期**: 2026-07-22
**修订日期**: 2026-07-22 (audit: Stage 5.6 amendment + test enrichment + tests/ refactor)

