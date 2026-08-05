# Stage 16.77 — Backend File Organization + Graph Sync + Design Writeback

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.263.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

Stage 16.76 完成了 Emitter trait 6 子 trait 拆分 + mod.rs/mir_translation 拆分。本阶段是 Stage 16.76 的自然延续，聚焦"真正组织 llvm 和 text"——把 `llvm/mod.rs` (2157 LOC) 和 `text/mod.rs` (866 LOC) 按 6 子 trait 拆分为子文件。

4 个目标：
1. **真正组织 llvm**：`llvm/mod.rs` 拆分为 8 文件
2. **真正组织 text**：`text/mod.rs` 拆分为 7 文件
3. **§15 项目图管理同步**：更新 codegen 图文件
4. **§14.8 设计回写**：07-codegen.md 补写 §16

## 2. 设计-审查 Agent 循环 (§13.5)

本阶段为 1 轮自审定稿（scope 清晰，无 P0/P1 缺陷）：
- Design v1: `stage-16.77-backend-file-organization-design.md`
- 自审清单 7 项全部通过
- 风险盲点 R-1（私有 helpers 访问权限）已有缓解措施

## 3. 重构六大判据检查 (§13.4.1)

| # | 判据 | 满足情况 |
|---|------|----------|
| J1 | 架构设计对齐 | ✅ 与 07-codegen.md §4 一致 |
| J2 | 单一职责 | ✅ 每个文件一个 impl 块或 struct 定义 |
| J3 | 单向流动 | ✅ mod.rs 依赖所有子模块 |
| J4 | 编译相关表达完整 | ✅ 39 methods 100% 归属 |
| J5 | 阶段划分清晰 | ✅ 仍在 codegen 阶段 |
| J6 | 科学合理粒度 | ✅ 最大 arithmetic.rs 420 LOC |

## 4. MUV 执行

### MUV-1: llvm/mod.rs 拆分

**前**: 2157 LOC 单文件（struct + Drop + 公共方法 + 私有 helpers + 6 impl 块 + tests）
**后**: 9 文件

| 文件 | LOC | 职责 |
|------|-----|------|
| mod.rs | 562 | struct + new() + Drop + public API + module declarations |
| module.rs | 225 | impl ModuleEmitter (5 methods) |
| function.rs | 235 | impl FunctionEmitter (8 methods) |
| arithmetic.rs | 420 | impl ArithmeticEmitter (11 methods) |
| memory.rs | 159 | impl MemoryEmitter (6 methods) |
| aggregate.rs | 316 | impl AggregateEmitter (5 methods) |
| local_state.rs | 33 | impl LocalStateEmitter (4 methods) |
| helpers.rs | 143 | 私有 helpers (cstr, is_float, parse_*, collect_cstring) |
| function_sigs.rs | 56 | build_fn_sigs_map (LLVM-only, 已存在) |
| tests.rs | 173 | 单元测试 |

### MUV-2: text/mod.rs 拆分

**前**: 866 LOC 单文件（struct + 6 impl 块）
**后**: 7 文件

| 文件 | LOC | 职责 |
|------|-----|------|
| mod.rs | 189 | struct + new() + output_with_globals + helpers (emit_type_to_llvm_str, binop_to_llvm_str) |
| module.rs | 100 | impl ModuleEmitter (5 methods) |
| function.rs | 87 | impl FunctionEmitter (8 methods) |
| arithmetic.rs | 280 | impl ArithmeticEmitter (11 methods) |
| memory.rs | 81 | impl MemoryEmitter (6 methods) |
| aggregate.rs | 144 | impl AggregateEmitter (5 methods) |
| local_state.rs | 28 | impl LocalStateEmitter (4 methods) |

### MUV-3: §15 项目图管理同步

更新 2 个图文件：

| 文件 | 更新内容 |
|------|---------|
| `docs/graph/codegen/emitter-trait.md` | 重写为 6 子 trait 新结构，含 mermaid class diagram + backend file organization flowchart + caller compatibility diagram + history |
| `docs/graph/codegen/architecture.md` | 重写为完整模块结构（含 llvm/ 9 文件 + text/ 7 文件）+ architecture layers mermaid + data flow + key design decisions + history |

### MUV-4: §14.8 设计回写

在 `docs/lang-design/07-codegen.md` 追加 §16 "Emitter trait 架构"（6 小节）：
- §16.1 6 子 trait 拆分（Stage 16.76 MUV-1）
- §16.2 Backend 文件组织（Stage 16.77 MUV-1/2）
- §16.3 dyn Emitter 兼容性
- §16.4 共享翻译层（mir_translation/）
- §16.5 历史背景
- §16.6 偏差处理（无偏差）

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo clean` | 成功 | ✅ |
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 349 lib + 2494 integration = 2843 unit + conformance embedded |

## 6. 设计偏差清单 (§14.8)

| 设计文档章节 | 偏差类型 | 偏差描述 | 最优判断 | 重构判断 | 回写动作 |
|-------------|---------|---------|---------|---------|---------|
| 07-codegen.md §4 | B4 | 实现已做 6 子 trait 拆分 + backend 文件组织，设计文档未涉及 | 实现即事实 | N/A | ✅ 已补写 §16 |

## 7. 结论

GO — Stage 16.77 全部 4 MUVs 完成：
- MUV-1 llvm/mod.rs 拆分 ✅
- MUV-2 text/mod.rs 拆分 ✅
- MUV-3 项目图管理同步 ✅
- MUV-4 设计回写 ✅

"真正组织 llvm 和 text" 目标达成：
- llvm/ 从 1 文件 2157 LOC → 9 文件最大 562 LOC
- text/ 从 1 文件 866 LOC → 7 文件最大 280 LOC

## 8. 后续工作

Stage 16.78+ 可继续：
- v0.4 roadmap items (where clause semantic checking, Task 14 Phase 3 supertrait safety)
- CodegenError 错误系统改造（Stage 16.76 design-v2 中 MUV-4 推迟到 v0.4+）
- 性能优化（MIR lowering, typeck unification table, MonoLayoutMap caching）
