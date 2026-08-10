# Stage 18.07 — v0.6 P1 Mid-Review (macro_rules! Phase 1-6 评估)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-06
> **Version**: v0.293.0
> **Process**: stage-committee-process.md v5.0 §14.5 (阶段末尾深度审查 D1-D8)
> **Status**: ✅ Complete

## 1. 阶段目标

§14.5 阶段末尾深度审查 v0.6 P1 (macro_rules! 系统) 进展。v0.6 P1 规划
6-8 stages，已完成 6 stages (18.01-18.06)。本阶段对 Phase 1-6 进行
深度审查，规划剩余 stages。

## 2. v0.6 P1 完成状态

| Stage | 内容 | Status | Tests |
|-------|------|--------|-------|
| 18.01 | Phase 1: 设计 + v0.6 roadmap | ✅ | — |
| 18.02 | Phase 2: macro_rules! 定义解析 | ✅ | — |
| 18.03 | Phase 3: token tree 匹配 + 替换 | ✅ | +8 |
| 18.04 | Phase 4: macro call invocation + driver 集成 | ✅ | +8 |
| 18.05 | Phase 5: 额外 fragment specifiers (ty/literal/block/path) | ✅ | +8 |
| 18.06 | Phase 6: repetition `$(...)*` / `+` / `?` | ✅ | +8 |
| **Total** | | | **+24 tests** |

总测试数：3,024 unit tests (487 lib + 2,537 integration)，0 failures。

## 3. §14.5 8 维度深度审查 (D1-D8)

### D1 — 架构健康度

**评估**：✅ 优秀

- `src/parser/macro_expand.rs` 是 macro_rules! 的单一真理源
- driver 通过 `expand_macros` 自由函数入口与 parser 模块交互 (§11 合规)
- `MacroTable` 类型集中在 `macro_expand.rs` 内部
- 没有跨阶段耦合：macro_expand 不依赖 HIR/MIR/Codegen

### D2 — API 命名标准化 (§10)

**评估**：✅ 合规

| API | 模式 | 评估 |
|-----|------|------|
| `expand_macros` | `<verb>_<noun>` | ✅ 自由函数入口 |
| `expand_macro` | `<verb>_<noun>` | ✅ 单宏展开 |
| `expand_macro_calls` | `<verb>_<noun>_<noun>` | ✅ |
| `collect_macro_defs` | `<verb>_<noun>_<noun>` | ✅ |
| `MacroTable` | `<Noun>Table` | ✅ (mirrors FnSigTable) |
| `CaptureValue` | `<Noun><Noun>` | ✅ |
| `RepetitionKind` | `<Noun>Kind` | ✅ (mirrors BorrowKind) |
| `parse_repetition_op` | `<verb>_<noun>_<noun>` | ✅ |
| `match_pattern` / `match_pattern_at` | `<verb>_<noun>` | ✅ |
| `match_repetition` | `<verb>_<noun>` | ✅ |
| `substitute_body` / `substitute_repetition` | `<verb>_<noun>` | ✅ |
| `capture_expr/ident/tt/ty/literal/block/path` | `<verb>_<fragment>` | ✅ |

无 glob re-export，无违反 DRY 的类型重复定义。

### D3 — 接口隔离 (§11)

**评估**：✅ 合规

- `expand_macros` 是 driver 唯一入口
- `expand_macro` / `collect_macro_defs` / `expand_macro_calls` 是 `pub`
  (供测试和未来 Phase 7 用)
- 所有 `capture_*`、`match_*`、`substitute_*`、`parse_repetition_op`、
  `collect_pattern_inner` 等都是 `fn` (内部)
- `CaptureValue` 是 `pub(crate)` (供测试)

### D4 — 测试覆盖 (§9.4.3)

**评估**：✅ 全部 1:3+ 比例合规

| Stage | 正负比例 | 评估 |
|-------|---------|------|
| 18.03 | 2:6 | ✅ |
| 18.04 | 2:6 | ✅ |
| 18.05 | 2:6 | ✅ |
| 18.06 | 2:6 | ✅ |

### D5 — 死代码检查

**评估**：✅ 无死代码

- 所有 `pub` API 都被 driver 或测试调用
- 所有 `fn` 内部函数都被 `match_pattern`/`substitute_body`/`match_repetition`/`substitute_repetition` 调用
- Stage 18.06 中 `match_pattern` 重构为 `match_pattern_at` 的 wrapper，
  消除了原来对内联 `ii` 的依赖
- Stage 18.05 移除了 `capture_tt` 中的 `let _ = open;` dead binding
- Stage 18.04 移除了 `expand_macro_calls` 中的 `let _ = (name_span, open_kind)` dead binding

### D6 — 性能

**评估**：✅ 可接受

- `expand_macros` 在无 macro_rules! 时走快速路径 (返回原 token 流)
- `MAX_EXPANSION_ROUNDS = 32` 防止无限递归
- `tokens_eq` 终止检查使用结构比较，正确性高于长度比较
- 每次迭代 `expand_macro_calls` 是 O(n) token 扫描

### D7 — 错误处理

**评估**：⚠️ 待改进

- macro_rules! 定义语法错误时，`collect_macro_defs` 静默跳过该定义
- macro 展开失败时 (no rule matches)，`expand_macro_calls` 原样保留调用形式
- 这些设计避免 panic，但用户得不到明确的错误信息

**TODO** (Stage 18.08+)：
- 添加 macro 展开错误收集机制 (Vec<MacroError>)
- driver 将这些错误传播到 errors.macro 字段

### D8 — 文档同步

**评估**：✅ 合规

- `docs/develop/v0/v0.6-roadmap.md` — 总体路线图
- `docs/develop/v0/stage-18/stage-18.01-*.md` — 6 个 stage 设计文档
- `RELEASE_NOTES.md` — 每个 stage 都有详细 release notes
- `docs/worklog.md` — 每个 stage 都有 worklog 条目
- 代码内 doc-comment 覆盖所有公共 API 和大部分内部函数

## 4. 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A (架构) | GO | 架构清晰，§11 合规 |
| REV-A (审查) | GO | §10/§9.4.3 合规 |
| DEV-A (开发) | GO | 24 新测试，3024 总测试 |
| QA-A (测试) | GO | 全 1:3+ 比例 |
| PM-A (项目管理) | GO | 文档同步，roadmap 更新 |

**5/5 GO** ✅

## 5. v0.6 P1 剩余 stages 规划

基于当前进度，v0.6 P1 还需 1-3 stages:

| Stage | 内容 | 优先级 |
|-------|------|--------|
| 18.08 | macro 展开错误收集 + driver 集成 | P2 (D7 改进) |
| 18.09 | println! 通解化迁移 (将 println! 从特解改为 macro_rules!) | P1 (println! 通解化) |
| 18.10 | v0.6 P1 最终审查 + 打包 | P1 |

## 6. 验收

- [x] §14.5 8 维度深度审查完成
- [x] 委员会 5/5 GO
- [x] 当前 build/test/clippy 全绿
- [x] 文档同步

## 7. 结论

v0.6 P1 (macro_rules! 系统) 中期审查通过。Phase 1-6 全部完成，
3,024 unit tests，0 failures。继续推进 Stage 18.08 (错误收集) 和
Stage 18.09 (println! 通解化)。
