# Stage 6.11 开发计划：系统性架构审查 + Stage 6.10 后续 + 流程 v3.21 落地

> **阶段**: Stage 6.11
> **版本**: v0.12.9 → v0.13.0
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21（§13.4 阶段开始设计对齐 + §14.4 重构治理 + §25.8 设计回写）

## 1. 阶段开始设计对齐（§13.4 强制）

### 1.1 对应设计文档

依据 §13.4.1 流程，本阶段（Stage 6.11）对应以下 `docs/lang-design/` 文档：

| 阶段 | 设计文档 | 章节范围 |
|------|---------|---------|
| Stage 6 整体（架构性拆分） | 06-mir.md（§2 顶层结构、§3 Statement、§7 Terminator） | MIR 数据结构边界 |
| Stage 6 整体（架构性拆分） | 07-codegen.md（§1 总体流程、§2 类型映射） | codegen pipeline 边界 |
| Stage 6 整体（架构性拆分） | 12-roadmap.md §1-§3 | v0.1 → v0.3 阶段划分 |
| Stage 6 整体（流程治理） | stage-committee-process.md v3.21 §13.4 / §14.4 / §25.8 | 流程治理 |

### 1.2 设计意图摘要

- **06-mir.md §2**: MIR Body 是 MIR-first 设计的灵魂，所有静态分析在 MIR 上做。
  数据结构必须简单、CFG-based、三地址码、类型保留、SSA-like。模块边界应
  尊重"Body 数据结构 / 构建算法 / 优化 pass / 文本表示 / 数据流分析"五层分离。
- **07-codegen.md §1**: codegen 是 MIR → LLVM IR 的单向管道。MIR → LLVM IR
  per-function + LLVM 优化 + LLVM codegen → 目标文件。不允许 codegen 反查 HIR。
- **12-roadmap.md**: Stage 0 = 可用编译器（Rust 实现），v0.1；Stage 1 = Landin
  重写；v0.3 自举完成。当前在 Stage 0 中后段。
- **流程 v3.21**: 把"重构 = 架构设计"固化为可操作规则，每个新阶段必须先对齐
  设计文档，每个大阶段末尾必须回写设计文档。

### 1.3 当前实现 vs 设计文档

#### 已对齐项（无偏差）

- ✅ MIR 数据结构（MirBody / BasicBlock / Statement / Terminator）与设计 §2 一致
- ✅ AdtLayout 已下沉到 MirBody（§16 闭合，L-PIPE-1 已关）
- ✅ dyn Trait 调用信息已下沉到 MirBody.dyn_trait_calls（§16 闭合）
- ✅ codegen 5-module 架构（mod.rs + trait_dispatch + mir_translation + emitter + text_emitter）符合 §1 单向管道
- ✅ stdlib 3-module 架构（types / trait_methods / vtable_layout）符合数据域分离
- ✅ mir/lower 7-module 架构（mod.rs + 6 子模块 + expr_operand）符合算法/上下文分离

#### 已知偏差（需 §25.8 处理）

| 设计文档章节 | 偏差类型 | 描述 |
|-------------|---------|------|
| 06-mir.md §2 | B1 | 设计要求 `BasicBlock.is_cleanup: bool` 字段，实现未做（unwind 是 v0.2） |
| 06-mir.md §2 | B1 | 设计要求 `source_scopes: IndexVec` 字段，实现用 `LocalDecl.source_info: Span` 简化 |
| 06-mir.md §2 | B1 | 设计要求 `LocalDecl.is_temp / is_arg` 字段，实现未做 |
| 06-mir.md §8 | B4 | 设计有"MIR 构建算法"章节但未描述 dyn Trait lowering，实现已做 |
| 07-codegen.md | B4 | 设计文档未描述 vtable / dynptr / dyn Trait codegen，实现已做（Stage 5.78-5.80） |

### 1.4 本阶段灰区决策

依据 §13.4.2 强制要求 3，本阶段对灰区做出明确决策：

| 灰区 | 决策 | 理由 |
|------|------|------|
| Stage 6 是否结束？ | 否，继续推进 | 用户要求"继续推进 stage 6" + 流程 v3.21 已落地但尚未应用过 |
| 是否做 §25.8 设计回写？ | 是（部分） | 本阶段执行轻量级回写：对 06-mir.md + 07-codegen.md 补 §25.8 偏差清单小节，完整回写留待 Stage 6 末尾 |
| 是否立即重构 expr_operand.rs？ | 否 | Stage 6.10 刚拆分，需观察一轮；按 §14.4 J6（科学合理粒度），1275 LOC 仍可接受 |
| 下一个架构性拆分目标？ | parser.rs（3112 LOC） | 全项目最大文件，符合 §14.4 触发条件 J6 |

## 2. 系统性架构审查（§14.4 J1-J6）

### 2.1 当前项目组织结构

```
src/
├── lib.rs               (466 LOC)   — crate 入口 + re-exports
├── bin/main.rs          (76 LOC)    — CLI 入口
├── driver.rs            (915 LOC)   — 全 pipeline 编排
├── cargo.rs             (171 LOC)   — mini-cargo
├── lexer/
│   ├── mod.rs           (50 LOC)
│   ├── reader.rs        (1537 LOC)  ← 第 2 大文件
│   └── token.rs         (390 LOC)
├── parser/
│   ├── mod.rs           (44 LOC)
│   ├── error.rs         (34 LOC)
│   └── parser.rs        (3112 LOC)  ← 第 1 大文件，超阈值 2x
├── ast/
│   ├── mod.rs           (19 LOC)
│   └── kinds.rs         (752 LOC)
├── hir/
│   ├── mod.rs           (31 LOC)
│   ├── id.rs            (236 LOC)
│   ├── kinds.rs         (963 LOC)
│   ├── map.rs           (63 LOC)
│   └── lower/{cx,item,body,...}
├── resolve/
│   ├── mod.rs           (21 LOC)
│   ├── module_tree.rs   (145 LOC)
│   ├── resolver.rs      (1131 LOC)  ← 第 5 大文件
│   ├── scope.rs         (174 LOC)
│   └── error.rs         (36 LOC)
├── mir/
│   ├── mod.rs           (55 LOC)
│   ├── body.rs          (434 LOC)
│   ├── dyn_trait.rs     (885 LOC)
│   ├── lvalue.rs        (250 LOC)
│   ├── place.rs         (258 LOC)
│   ├── ty.rs            (203 LOC)
│   └── lower/           (7 子模块，已优化)
├── typeck/
│   ├── mod.rs           (31 LOC)
│   ├── checker.rs       (1320 LOC)  ← 第 4 大文件
│   ├── unify.rs         (715 LOC)
│   └── error.rs         (62 LOC)
├── borrowck/
│   ├── mod.rs           (1452 LOC)  ← 第 3 大文件
│   ├── borrow_set.rs    (341 LOC)
│   ├── error.rs         (92 LOC)
│   └── move_tracker.rs  (90 LOC)
├── codegen/             (5 子模块，已优化)
├── traits/
│   ├── mod.rs           (24 LOC)
│   ├── builtin.rs       (23 LOC)
│   ├── resolver.rs      (903 LOC)
│   └── vtable.rs        (30 LOC)
├── stdlib/              (3 子模块，已优化)
├── session/mod.rs       (154 LOC)
└── diagnostics/mod.rs   (149 LOC)
```

### 2.2 §14.4 J1-J6 判据检查

对当前组织结构逐判据检查：

| # | 判据 | 当前状态 | 是否合规 |
|---|------|---------|---------|
| J1 | 架构设计对齐 | mir / codegen / stdlib 子模块拆分均对齐设计文档章节边界；parser.rs 是单一巨型递归下降 parser，对齐 02-grammar.md（设计文档也按一个文件描述） | ✅ |
| J2 | 单一职责 | 大部分模块单一职责清晰；`parser.rs` 3112 LOC 含 expr/stmt/ty/pat/path/item 6 类解析职责混合 | ⚠️ parser.rs 待拆 |
| J3 | 单向流动 | 数据流 lexer → parser → HIR → resolve → MIR → typeck → borrowck → codegen 单向，无环 | ✅ |
| J4 | 编译相关表达完整 | hir/kinds.rs（963 LOC）+ ast/kinds.rs（752 LOC）单一职责完整 | ✅ |
| J5 | 阶段划分清晰 | 各阶段目录隔离，无跨阶段调用（§16 已验证） | ✅ |
| J6 | 科学合理粒度 | mod.rs 阈值 < 1500 LOC 全部达标；最大文件 parser.rs 3112 LOC 超阈值 2x | ⚠️ parser.rs 待拆 |

### 2.3 审查结论

- **当前架构整体健康**，7 个模块已完成架构性拆分（mir/lower + codegen + stdlib）
- **唯一显著问题**：`parser/parser.rs` 3112 LOC 是全项目最大文件，违反 §14.4 J2 + J6
- **不紧急但需要处理**：`lexer/reader.rs` 1537 LOC、`borrowck/mod.rs` 1452 LOC、`typeck/checker.rs` 1320 LOC，均接近或超过 mod.rs 1500 LOC 阈值

### 2.4 下一步选择

依据 §14.4 重构执行流程，本阶段（Stage 6.11）选择**继续推进 Stage 6.10 后的流程治理 + 系统性架构审查报告**，**不立即执行新拆分**。理由：

1. Stage 6.10 刚完成 mir/lower expr_operand 拆分，需观察一轮稳定性
2. 流程 v3.21 刚落地，需在本阶段验证 §13.4 / §14.4 / §25.8 三协议可用性
3. parser.rs 拆分需要独立的 Stage 6.12 来执行（按 §14.4 完整流程：现状分析 → 设计对齐 → 候选方案 → J1-J6 检查 → 执行）

**本阶段（Stage 6.11）的实际产出**：

1. ✅ 流程 v3.21 重构（§13.4 + §14.4 + §25.8 + §28.4）
2. ✅ 系统性架构审查报告（本文档 §2）
3. ✅ §25.8 轻量级设计回写：在 06-mir.md + 07-codegen.md 补"实现状态"小节
4. ✅ 版本号 v0.12.9 → v0.13.0（流程大版本变更，主版本号 +1）

## 3. §25.8 轻量级设计回写计划

依据 §25.8.3 强制要求，本阶段对 06-mir.md + 07-codegen.md 做"实现状态"小节
回写（B1/B4 偏差标注），完整 §25.8 回写留待 Stage 6 末尾。

### 3.1 06-mir.md 回写内容

在 §2 顶层结构末尾加"实现状态（v0.13.0）"小节：
- B1: `is_cleanup` / `source_scopes` / `is_temp` / `is_arg` 字段未实现 → v0.2 unwind 阶段补
- B4: dyn Trait lowering 已实现但设计文档未描述 → 补 §8.X 子节描述设计意图

### 3.2 07-codegen.md 回写内容

在 §1 总体流程末尾加"实现扩展（v0.13.0）"小节：
- B4: vtable / dynptr / dyn Trait codegen 已实现但设计文档无章节 → 补 §X 子节描述设计意图

## 4. 验收标准（§1.2）

- [ ] `cargo clean && cargo test` — 1881 tests 全过
- [ ] `cargo fmt` — clean
- [ ] `cargo clippy --all-targets` — 0 warnings, 0 errors
- [ ] docs/stage-committee-process.md 升级到 v3.21，含 §13.4 / §14.4 / §25.8
- [ ] docs/lang-design/06-mir.md 补"实现状态"小节
- [ ] docs/lang-design/07-codegen.md 补"实现扩展"小节
- [ ] plan-6.11.md（本文件）+ gate-review-6.11.md
- [ ] dev-log.md + RELEASE_NOTES.md + README.md + worklog.md 更新
- [ ] 版本 v0.12.9 → v0.13.0

## 5. 后续 Stage 6.12+ 候选

完成本轮后：

- **Stage 6.12**: parser.rs 架构性拆分（3112 LOC → 按解析类别：expr / stmt / ty / pat / path / item）
- **Stage 6.13**: lexer/reader.rs 拆分（1537 LOC → 按词法类别）
- **Stage 6.14**: borrowck/mod.rs 拆分（1452 LOC → 按分析类别）
- **Stage 6.15**: typeck/checker.rs 拆分（1320 LOC → 按检查类别）
- **Stage 6 末尾**: 完整 §25.8 设计回写（全 docs/lang-design/ 文档）
- **TD-015**: Region inference
- **TD-018**: 用户自定义 trait dyn 支持

---

**创建日期**: 2026-07-25
