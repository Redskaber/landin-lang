# Stage 16.76 — Codegen Pipeline Refactoring (3 MUVs)

> **Author**: redskaber + ARCH-A (Design Agent) + REV-A (Review Agent)
> **Date**: 2026-08-05
> **Version**: v0.262.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环)
> **Status**: ✅ Complete

## 1. 阶段目标

按用户要求"胆大心细优化重构 codegen 编译管道"：
1. 真正抽象出 codegen（Emitter trait 拆分）
2. 真正组织 llvm 和 text（backend 分离）
3. 数据结构选型与架构设计审视
4. 编译流水线组织优化
5. API 接口设计规范化

## 2. 设计-审查 Agent 循环 (§13.5)

### Round 1

- **Design v1**: `stage-16.76-codegen-refactor-design-v1.md`
- **Review v1**: `stage-16.76-codegen-refactor-design-review-v1.md`
- **缺陷**: 4 P1 + 4 P2 + 4 P3 = 12 defects
- **关键发现**:
  - P1-1: 方法数事实错误（36 → 实际 39）
  - P1-2: MUV-1 未引用 Stage 16.38 失败尝试
  - P1-3: MUV-3 mir_translation 拆分遗漏 733 LOC place codegen
  - P1-4: CodegenError 字段违反 §10.1.8

### Round 2

- **Design v2**: `stage-16.76-codegen-refactor-design-v2.md` (定稿)
- **Review v2**: `stage-16.76-codegen-refactor-design-review-v2.md`
- **缺陷**: 0 P0 + 0 P1 + 1 P2 (implementation-phase) + 4 P3
- **结论**: 定稿 with limitations — 所有 P1 已解决，可进入实现阶段

## 3. 重构六大判据检查 (§13.4.1)

| # | 判据 | 通过条件 | 满足情况 |
|---|------|---------|----------|
| J1 | 架构设计对齐 | 与 07-codegen.md 章节划分一致 | ✅ MUV-3 按 §2.1-§2.4 + §4.4 拆分 |
| J2 | 单一职责 | 每个模块/trait 单一职责 | ✅ 6 sub-trait 各司其职（5/8/11/6/5/4 方法） |
| J3 | 单向流动 | 模块间依赖无环 | ✅ mod.rs → sub-modules 单向 |
| J4 | 编译相关表达完整 | 概念完整保留 | ✅ 39 methods 100% 归属 |
| J5 | 阶段划分清晰 | 不破坏 §11 | ✅ 仍在 codegen 阶段 |
| J6 | 科学合理粒度 | LOC 合理 | ✅ 最大 places.rs 791 LOC（单一职责完整集合） |

## 4. MUV 执行（按风险递增）

### MUV-3: mir_translation.rs 拆分（lowest risk）

**前**: 1144 LOC 单文件
**后**: 5 文件按 07-codegen.md 章节对齐

| 文件 | LOC | 对应 07-codegen.md 章节 | 内容 |
|------|-----|------------------------|------|
| mod.rs | 33 | — | re-exports |
| types.rs | 241 | §2.1-§2.3 | mir_type_to_emit_type_with_layouts[_and_mono] |
| layouts.rs | 72 | §2.3-§2.4 | adt_layout_to_emit_type |
| places.rs | 791 | §4.4 | 7 个 place codegen 函数 |
| stdlib.rs | 31 | 跨章节 | stdlib_type_kind_to_emit_type |

### MUV-2: codegen/mod.rs 拆分（low risk）

**前**: 931 LOC mod.rs（混合入口+pipeline+drop_glue+per-function+helper）
**后**: 5 文件各司其职

| 文件 | LOC | 职责 |
|------|-----|------|
| mod.rs | 156 | 入口 + re-exports |
| pipeline.rs | 92 | run_codegen_pipeline |
| function.rs | 371 | codegen_function + 3 helper |
| drop_glue.rs | 281 | emit_drop_glue_functions |
| llvm/function_sigs.rs | 56 | build_fn_sigs_map (LLVM-only) |

### MUV-1: Emitter trait 拆分（medium risk, atomic）

**前**: 39-method Emitter trait + 2 impl blocks (text 648 LOC + llvm 1279 LOC = 1927 LOC)
**后**: 6 sub-traits + 12 impl blocks (6 per backend)

| Sub-trait | 方法数 | 职责 |
|-----------|--------|------|
| ModuleEmitter | 5 | module-level globals & declarations |
| FunctionEmitter | 8 | function scope & control flow |
| ArithmeticEmitter | 11 | value computation from operands |
| MemoryEmitter | 6 | stack allocation & pointer arithmetic |
| AggregateEmitter | 5 | aggregate construction & calls |
| LocalStateEmitter | 4 | local value/pointer mapping |
| **Total** | **39** | (matches original) |

**Breaking change**: external `Emitter` implementers must now implement 6 sub-traits. Blanket impl preserves `dyn Emitter` compatibility for 20+ caller sites.

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo clean` | 成功 | ✅ |
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 350 lib + 2494 integration = 2844 unit + conformance embedded |

## 6. 设计偏差清单 (§14.8)

| 设计文档章节 | 偏差类型 | 偏差描述 | 最优判断 | 重构判断 | 回写动作 |
|-------------|---------|---------|---------|---------|---------|
| 07-codegen.md §4 | B4 | 实现已做 codegen sub-trait 拆分，设计文档未涉及 | 实现即事实 | N/A | 补写 07-codegen.md §4.X codegen trait 架构（推迟 Stage 16.77） |

## 7. 结论

GO — Stage 16.76 全部 3 MUVs 完成：
- MUV-3 mir_translation 拆分 ✅
- MUV-2 mod.rs 拆分 ✅
- MUV-1 Emitter trait 6 sub-trait 拆分 ✅

架构债闭合：
- AP-1 Emitter trait 臃肿 ✅ CLOSED
- AP-2 mod.rs 职责混合 ✅ CLOSED
- AP-3 mir_translation 过大 ✅ CLOSED

## 8. 后续工作

- Stage 16.77: 项目图管理更新（§15）+ 07-codegen.md §4.X 设计回写 + final packaging
- Stage 16.78+: 继续按 v0.4 roadmap 推进（where clause semantic checking, Task 14 Phase 3 supertrait safety, etc.）
