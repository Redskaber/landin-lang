# Stage 16.77 Design — Backend File Organization + Graph Sync + Design Writeback

> **Author**: ARCH-A (Design Agent) + REV-A (Review Agent, inline self-review)
> **Date**: 2026-08-05
> **Version**: design-v1 (定稿 — single round, scope is clear)
> **Status**: ✅ Final
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环)

## 1. 阶段目标

Stage 16.76 完成了 Emitter trait 6 子 trait 拆分 + mod.rs/mir_translation 拆分。但用户再次强调"真正抽象出 codegen，真正组织 llvm 和 text"——当前 `llvm/mod.rs` (2157 LOC) 和 `text/mod.rs` (866 LOC) 仍把 6 个 impl 块集中在单文件，未真正"组织"。

本阶段聚焦 3 个目标：

1. **真正组织 llvm 和 text**：把 `llvm/mod.rs` 和 `text/mod.rs` 按 6 子 trait 拆分为子文件
2. **§15 项目图管理同步**：更新 `docs/graph/codegen/emitter-trait.md` 反映 6 子 trait 新结构
3. **§14.8 阶段末尾设计回写**：在 `docs/lang-design/07-codegen.md` 补写 §16 "Emitter trait 架构"

## 2. 架构现状分析

### 2.1 当前 codegen 模块结构

```
src/codegen/
├── mod.rs               (156 LOC)  — 入口 + re-exports ✅
├── pipeline.rs          (92 LOC)   — run_codegen_pipeline ✅
├── function.rs          (364 LOC)  — codegen_function + helpers ✅
├── drop_glue.rs         (282 LOC)  — emit_drop_glue_functions ✅
├── emitter/             (7 files)  — 6 sub-traits + mod.rs ✅
├── mir_translation/     (5 files)  — types/layouts/places/stdlib + mod.rs ✅
├── llvm/
│   ├── mod.rs           (2157 LOC) — ❌ ALL 6 impl blocks + struct + helpers in 1 file
│   └── function_sigs.rs (56 LOC)   ✅
├── text/
│   └── mod.rs           (866 LOC)  — ❌ ALL 6 impl blocks + struct in 1 file
├── operand.rs           (243 LOC)  ✅
├── rvalue.rs            (529 LOC)  ✅
├── statement.rs         (449 LOC)  ✅
├── terminator.rs        (593 LOC)  ✅
├── dyn_trait_emit.rs    (294 LOC)  ✅
└── trait_dispatch/      (4 files)  ✅
```

### 2.2 已识别的架构问题

| # | 问题 | 严重度 |
|---|------|--------|
| AP-1 | `llvm/mod.rs` 2157 LOC — 6 impl 块 + struct + Drop + 公共方法 + 私有 helpers 全部混合 | P2 (architecture debt) |
| AP-2 | `text/mod.rs` 866 LOC — 6 impl 块 + struct + output_with_globals 混合 | P2 |
| AP-3 | `docs/graph/codegen/emitter-trait.md` 过期（仍显示旧单 trait 结构） | P2 (§15 violation) |
| AP-4 | `docs/lang-design/07-codegen.md` 缺 §16 Emitter trait 架构章节 | P2 (§14.8 violation) |

### 2.3 已识别的优势（保留）

| # | 优势 |
|---|------|
| ST-1 | Stage 16.76 已完成 6 子 trait 拆分 — impl 块边界清晰 |
| ST-2 | 所有测试通过（2844 unit + conformance embedded） |
| ST-3 | §11 接口隔离合规 |

## 3. 重构方案

### 3.1 MUV-1: llvm/mod.rs 拆分

**前**: `llvm/mod.rs` 2157 LOC（struct + Drop + 公共方法 + 私有 helpers + 6 impl 块）
**后**: 8 文件

```
src/codegen/llvm/
├── mod.rs               (~200 LOC) — LLVMSysEmitter struct + new() + Drop + public methods (to_module, to_object_file, set_fn_sigs) + module declarations + re-exports
├── module.rs            (~250 LOC) — impl ModuleEmitter for LLVMSysEmitter (5 methods: emit_header, emit_declare, emit_string_global, emit_vtable_global, emit_dyn_trait_const)
├── function.rs          (~350 LOC) — impl FunctionEmitter for LLVMSysEmitter (8 methods: emit_function_begin, emit_function_end, emit_block, emit_ret, emit_unreachable, emit_br, emit_br_cond, emit_switch)
├── arithmetic.rs        (~500 LOC) — impl ArithmeticEmitter for LLVMSysEmitter (11 methods)
├── memory.rs            (~300 LOC) — impl MemoryEmitter for LLVMSysEmitter (6 methods)
├── aggregate.rs         (~400 LOC) — impl AggregateEmitter for LLVMSysEmitter (5 methods)
├── local_state.rs       (~50 LOC)  — impl LocalStateEmitter for LLVMSysEmitter (4 methods)
├── helpers.rs           (~300 LOC) — 私有 helper 函数 (fresh_named, named, set_value_name, resolve_value, llvm_type, interpret_adhoc, get_or_declare_function, etc.)
└── function_sigs.rs     (56 LOC)   ✅ 已存在
```

**J1-J6 检查**：
- J1 ✅ 与 07-codegen.md §4 (MIR→LLVM IR 映射) 一致 — 每个子文件对应一类 IR 指令
- J2 ✅ 每个文件单一职责（一个 impl 块 或 struct 定义 或 helpers）
- J3 ✅ mod.rs 依赖所有子模块，子模块间无环
- J4 ✅ LLVM backend 概念完整保留
- J5 ✅ 仍在 codegen 阶段
- J6 ✅ 各文件 LOC 合理（最大 arithmetic.rs ~500 LOC）

**迁移步骤**：
1. 创建 7 个新文件（module/function/arithmetic/memory/aggregate/local_state/helpers.rs）
2. 从 llvm/mod.rs 中提取每个 impl 块到对应文件
3. 从 llvm/mod.rs 中提取私有 helpers 到 helpers.rs
4. llvm/mod.rs 只保留 struct + new() + Drop + 公共方法 + module declarations
5. 在 llvm/mod.rs 中声明 `pub(crate) mod module;` 等
6. 验证编译 + 测试

**关键依赖**：
- 子文件需要访问 LLVMSysEmitter 的私有字段（ctx, module, builder, etc.）— 通过 `impl LLVMSysEmitter { pub(crate) fn ctx(&self) -> LLVMContextRef { self.ctx } }` 等 accessor 方法，或通过 `use super::*` 访问 crate-private 字段
- 私有 helpers（fresh_named, named, llvm_type, etc.）需要被所有 impl 块使用 — 放在 helpers.rs 并在 mod.rs 中 `pub(crate) use helpers::*;` 或各子文件 `use super::helpers::*;`

### 3.2 MUV-2: text/mod.rs 拆分

**前**: `text/mod.rs` 866 LOC（struct + output_with_globals + 6 impl 块）
**后**: 8 文件

```
src/codegen/text/
├── mod.rs               (~100 LOC) — TextEmitter struct + new() + output_with_globals + module declarations + re-exports
├── module.rs            (~120 LOC) — impl ModuleEmitter for TextEmitter (5 methods)
├── function.rs          (~150 LOC) — impl FunctionEmitter for TextEmitter (8 methods)
├── arithmetic.rs        (~200 LOC) — impl ArithmeticEmitter for TextEmitter (11 methods)
├── memory.rs            (~100 LOC) — impl MemoryEmitter for TextEmitter (6 methods)
├── aggregate.rs         (~150 LOC) — impl AggregateEmitter for TextEmitter (5 methods)
└── local_state.rs       (~50 LOC)  — impl LocalStateEmitter for TextEmitter (4 methods)
```

**J1-J6 检查**：同 MUV-1，全部通过。

**迁移步骤**：同 MUV-1，但更简单（text backend 无私有 helpers 需要分离 — TextEmitter 的 helper 如 `indent()` 可留在 mod.rs 或提取到 helpers.rs）。

### 3.3 MUV-3: §15 项目图管理同步

更新以下图文件：

| 文件 | 更新内容 |
|------|---------|
| `docs/graph/codegen/emitter-trait.md` | 重写为 6 子 trait 新结构，含 mermaid 类图 |
| `docs/graph/codegen/architecture.md` | 更新 codegen 模块结构图（含 llvm/ 和 text/ 子文件） |
| `docs/graph/codegen/data-flow.md` | 更新数据流图（反映 MUV-3 mir_translation 拆分） |

### 3.4 MUV-4: §14.8 阶段末尾设计回写

在 `docs/lang-design/07-codegen.md` 补写 §16 "Emitter trait 架构"：

```markdown
## 16. Emitter trait 架构（v0.262.0 §25.8 回写）

### 16.1 6 子 trait 拆分

Stage 16.76 MUV-1 把 39-method `Emitter` trait 拆分为 6 个单一职责子 trait：

| Sub-trait | 方法数 | 职责 |
|-----------|--------|------|
| ModuleEmitter | 5 | module-level globals & declarations |
| FunctionEmitter | 8 | function scope & control flow |
| ArithmeticEmitter | 11 | value computation from operands |
| MemoryEmitter | 6 | stack allocation & pointer arithmetic |
| AggregateEmitter | 5 | aggregate construction & calls |
| LocalStateEmitter | 4 | local value/pointer mapping |

`Emitter` 是 super-trait，通过 blanket impl 自动为实现了全部 6 子 trait 的类型提供实现。

### 16.2 Backend 文件组织

Stage 16.77 把每个 backend 的 6 个 impl 块拆分到独立文件：

```
src/codegen/llvm/
├── mod.rs          — struct + public API
├── module.rs       — impl ModuleEmitter
├── function.rs     — impl FunctionEmitter
├── arithmetic.rs   — impl ArithmeticEmitter
├── memory.rs       — impl MemoryEmitter
├── aggregate.rs    — impl AggregateEmitter
├── local_state.rs  — impl LocalStateEmitter
└── helpers.rs      — 私有 helper 函数
```

text backend 同结构。

### 16.3 dyn Emitter 兼容性

20+ 调用点使用 `&mut dyn Emitter`，super-trait + blanket impl 模式保证 dyn 兼容性。
```

## 4. REV-A 自审清单（§13.5.2 内联）

| # | 检查项 | 通过 |
|---|--------|------|
| 1 | 6 子 trait 划分是否合理？ | ✅ Stage 16.76 review-v2 已确认 |
| 2 | 文件命名是否符合 §10？ | ✅ module/function/arithmetic/memory/aggregate/local_state.rs 全小写 + snake_case |
| 3 | 是否破坏 §11 接口隔离？ | ✅ 仍在 codegen 阶段，不跨阶段调用 |
| 4 | LOC 估计是否合理？ | ✅ 基于 Stage 16.76 实测 impl 块 LOC |
| 5 | 迁移步骤是否可执行？ | ✅ 纯文件移动，无逻辑改动 |
| 6 | 是否有新引入的设计缺陷？ | ✅ 无 — 纯文件重组 |
| 7 | 风险盲点？ | ⚠️ R-1: 私有 helpers 访问权限（见下） |

**R-1: 私有 helpers 访问权限**：
- LLVMSysEmitter 的私有字段（ctx, module, builder, etc.）和私有 helpers（fresh_named, named, llvm_type, etc.）需要被子文件访问
- 解决方案：在 mod.rs 中用 `pub(crate) fn ctx(&self) -> LLVMContextRef { self.ctx }` 提供 accessor，或把字段改为 `pub(crate)`
- 推荐：字段保持 `pub(crate)`（LLVMSysEmitter 是 crate-private 类型，字段 pub(crate) 不破坏封装）
- helpers 放在 helpers.rs，在 mod.rs 中 `mod helpers; pub(crate) use helpers::*;`

## 5. 执行计划

| MUV | 估计 LOC 变动 | 风险 | 顺序 |
|-----|--------------|------|------|
| MUV-1 llvm/mod.rs 拆分 | ~2157 LOC 移动 + ~50 LOC 新增（mod declarations） | 中（纯文件移动，但 LOC 大） | 1 |
| MUV-2 text/mod.rs 拆分 | ~866 LOC 移动 + ~30 LOC 新增 | 低 | 2 |
| MUV-3 graph diagrams 更新 | ~300 LOC 新增/重写 | 极低（纯文档） | 3 |
| MUV-4 design writeback | ~80 LOC 新增 | 极低（纯文档） | 4 |

## 6. 验收标准

- `cargo clean && cargo build --features llvm-backend` ✅
- `cargo fmt --check` ✅
- `cargo clippy --all-targets` 0 warnings ✅
- `cargo test` 0 failures ✅（2844+ tests）
- worklog 记录 ✅
- graph diagrams 更新 ✅
- 07-codegen.md §16 补写 ✅

## 7. 结论

定稿 — 本阶段为 Stage 16.76 的自然延续，scope 清晰，风险可控。设计-审查循环 1 轮收敛（自审无 P0/P1 缺陷）。
