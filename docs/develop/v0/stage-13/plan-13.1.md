# Stage 13 — Plan (Active): v0.3 Self-Hosting Preparation (Compile Pipeline Fixes)

> **状态**: 🔄 Active (Stage 13.1 MUV-1 ✅ DONE; MUV-2 deferred to Stage 13.1b per design alignment)
> **版本目标**: v0.21.4 → v0.21.5 (Stage 13.1 MUV-1) → v0.21.6 (Stage 13.1b MUV-2) → v0.22.0 (Stage 13.2-13.4 P0 closure)
> **流程**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25 + §25.8)
> **基于**: r216 first-pass audit + r217 second-pass audit (3 reports, 2055 lines total) + r219 Stage 12 §25 deep review + Stage 13.1 design alignment
> **创建日期**: 2026-07-26 (Stage 12.2 first-pass), 2026-07-26 (Stage 12.5 reframe), 2026-07-26 (Stage 13.1 active)

> **Stage 13.1 状态更新** (per Stage 13.1 design alignment §5):
> Stage 13.1 = MUV-1 only (TD-028 §16 violation fix — LOW risk, 11 files).
> MUV-2 (TD-029 TyKind::Dynamic) deferred to Stage 13.1b (Option B — variant-only,
> MEDIUM risk, 5 src files) per §15 + §25.7. Stage 13.2 (TD-031 if-let) is P0 priority.

---

## 1. 阶段定位 (per §13.4 design alignment)

### 1.1 设计文档对照

依据 §13.4「阶段开始时的设计对齐」，本阶段必须先查阅 `docs/lang-design/`：

| 设计文档 | 关键章节 | Stage 13 触达点 |
|---------|---------|----------------|
| `12-roadmap.md` §2 月 11-12 | Stage 1 rewrite 5 phases (lexer → parser → HIR → MIR → codegen) | Stage 13 必须先关闭 5 个 P0/P1 阻塞项，否则 Stage 1 无法启动 |
| `13-stage1-feature-whitelist.md` §2.1-2.6 | Stage 1 允许使用的语言特性 | Stage 13 必须实现白皮书要求但 Stage 0 缺失的特性（closures-callable, if-let, macro_rules!） |
| `03-type-system.md` §13 (新) | §25.8 回写：TyKind::Dynamic 缺失 | Stage 13.1 实施 TyKind::Dynamic 重构 |
| `06-mir.md` | MIR 数据结构 | Stage 13.3 闭包调用 lowering 触动 MIR |
| `07-codegen.md` | Codegen 架构 | Stage 13.4 macro_rules! 触动 codegen |
| `17-conformance-suite.md` §5.1 | v0.1 gate (5000 tests) | Stage 13 不动 conformance 数量，重点是 FAIL → PASS |

### 1.2 当前项目实际状况

依据 r216 跨阶段审计（D1-D7 七维度）：

- **v0.1 gate**: 5026/5000 ✅ — 已 ratified
- **TD open**: 7 项（P0=3, P1=1, P2=2, P3=1-on-hold）
- **§16 violations**: 1 active（mir::dyn_trait → codegen）+ 4 deprecated（已标记，不活跃）
- **Design deviations**: B1=18 (大部分已记录), B2=0, B3=7 (已接受), B4=3 (已回写)
- **新发现**: TyKind::Dynamic 缺失（B1，TD-029）

### 1.3 §15「最优 > 最小」决策

| 选项 | 内容 | 长期价值 | 短期成本 | 决策 |
|------|------|---------|---------|------|
| A: v0.1 发布公告 | 仅写 release notes + 推广 | 低（公告可随时发） | 低 | ❌ 延后到 Stage 13.6 |
| **B: 编译管线修复** | 关闭 TD-030, TD-031, TD-032, TD-033 | **高**（v0.3 自举硬前置） | 中（4-8 周） | ✅ **采用** |
| C: v0.2 特性 (Send/Sync, GATs) | 新特性 | 中（v0.2 roadmap） | 高 | ❌ 延后到 v0.3 完成后 |
| D: 重构 + 设计回填 | 仅 TD-028, TD-029 | 低（不阻塞 v0.3） | 低 | ❌ 合并到 Stage 13.1 |

**结论**: Stage 13 = Option B（编译管线修复），TD-028, TD-029 作为 Stage 13.1 前置基础。

---

## 2. 子阶段拆分 (MUV)

### Stage 13.1 — 架构基线 + §16 修复 + §25.8 回填 (1-2 天)

**MUV-1**: 修复 §16 违规 TD-028
- 输入：`src/mir/dyn_trait.rs` 中 7 个 `emit_*` 函数
- 输出：将 7 个函数迁移到 `src/codegen/trait_dispatch.rs`
- 验收：`grep "crate::codegen" src/mir/dyn_trait.rs` = 0
- Task ID: stage13.1-muv1-r217

**MUV-2**: TyKind::Dynamic 重构 (TD-029)
- 输入：`src/mir/ty.rs::TyKind` 缺 Dynamic 变体
- 输出：添加 `Dynamic { trait_def: DefId, lifetime: Lifetime }` 变体；DynTraitFatPtr 改为内部表示
- 验收：cargo test 全绿；r216 audit §3.3 偏差消除
- Task ID: stage13.1-muv2-r217

**MUV-3**: 6 个 `docs/tests/v0/stage{0-5}/plan/README.md` 已经在 Stage 12 跨阶段审计中补齐（本 plan 创建时已存在）
- 验收：`ls docs/tests/v0/stage{0-5}/plan/README.md` = 6 files
- Task ID: stage13.1-muv3-r217 (CLOSED — 完成于 Stage 12)

### Stage 13.2 — if-let / while-let (1-2 周)

**MUV-4**: AST + HIR if-let/while-let
- 输入：`src/ast/kinds.rs` + `src/hir/kinds.rs` 缺 IfLet/WhileLet 变体
- 输出：添加变体；parser 支持 (`src/parser/expr.rs`)
- 验收：12 个 conformance FAIL 测试转 PASS（00-parse/02-control-flow/）
- Task ID: stage13.2-muv4-r218

**MUV-5**: MIR lowering (desugar to match)
- 输入：`src/mir/lower/expr_operand.rs` 不识别 IfLet/WhileLet
- 输出：desugar IfLet(p, b, e) → Match(p, [Arm(pat=b, guard=none), Arm(_, e)])；同理 WhileLet
- 验收：cargo test 全绿；新增 conformance 测试转 PASS
- Task ID: stage13.2-muv5-r218

**MUV-6**: typeck + borrowck 支持
- 输入：`src/typeck/checker.rs` 不处理 if-let 的 refinement scope
- 输出：if-let 后续 block 内 pat 绑定变量有正确类型；borrowck 知道 pat 绑定引用的有效范围
- 验收：borrowck 接受 `if let Some(x) = &opt { /* x: &T */ }`
- Task ID: stage13.2-muv6-r218

### Stage 13.3 — 闭包调用 lowering (2-3 周)

**MUV-7**: 闭包调用 Terminator::Call 生成
- 输入：`src/mir/lower/expr_operand.rs` HirExprKind::Call arm 不识别 closure callee
- 输出：当 callee 是 closure local 时，emit Terminator::Call 到 closure 的合成 call fn
- 验收：41 个 conformance FAIL 测试转 PASS（01-typecheck/03-closures/, 02-borrowck/03-closure-capture/, 04-e2e/03-closures/）
- Task ID: stage13.3-muv7-r219

**MUV-8**: 闭包 Fn/FnMut/FnOnce trait 自动实现
- 输入：`src/traits/builtin.rs` 不自动 impl Fn/FnMut/FnOnce for closures
- 输出：根据 closure capture 模式 (by-ref → Fn, by-mut-ref → FnMut, by-value → FnOnce) 自动 impl
- 验收：`let f = |x| x + 1; f(2)` 编译通过
- Task ID: stage13.3-muv8-r219

### Stage 13.4 — macro_rules! + 26 内置宏 (4-8 周)

**MUV-9**: macro_expand 模块骨架
- 输入：无 macro 系统模块
- 输出：新增 `src/macro_expand/` 模块（1500-2500 LOC）；macro definition parser + macro expander
- 验收：`macro_rules! vec { ($($x:expr),*) => { ... } }` 解析通过
- Task ID: stage13.4-muv9-r220

**MUV-10**: 26 内置宏替换为 macro_rules! 实现
- 输入：26 个内置宏硬编码在 codegen
- 输出：用 macro_rules! 重新实现；硬编码移除
- 验收：`vec![1, 2, 3]` / `println!("{}", x)` / `format!(...)` 等通过 macro_rules! 展开
- Task ID: stage13.4-muv10-r220

**MUV-11**: HIR integration + hygiene
- 输入：HIR 不识别 macro expansion
- 输出：HIR 添加 ExpnId + SyntaxContext；resolver 处理 hygiene
- 验收：宏内变量不污染外层 scope
- Task ID: stage13.4-muv11-r220

### Stage 13.5 — TD-033 P1 子项 (3-6 月, 并行 Stage 1 起草)

**MUV-12**: for 循环（验证 + MIR desugar to while + iter.next()）
**MUV-13**: move 闭包（is_move flag 已存在，激活）
**MUV-14**: HRTB `for<'a>` (parser + typeck + region inference)
**MUV-15**: 关联类型 normalization
**MUV-16**: Two-phase borrows (method-call 子集, ~200-400 LOC in borrowck/)
**MUV-17**: Disjoint closure captures (RFC 2229, ~300-500 LOC in hir/lower/)
**MUV-18**: 性能修复 5.1.1 + 5.1.2 (NLL Vec → HashSet)

### Stage 13.6 — v0.1 发布公告 (1-2 天, P0 关闭后)

**MUV-19**: 公告 + tag + changelog finalize
- 输入：v0.1 已 ratified (Stage 12) + Stage 13 P0 已关闭
- 输出：v0.1.0 release announcement (HN/Reddit/技术博客) + git tag v0.1.0
- 验收：公开发布完成

---

## 3. 验收标准 (per §3.3 + §1.2)

| 维度 | 标准 | Stage 13.1 | Stage 13.2-13.4 | Stage 13.5+ |
|------|------|-----------|-----------------|-------------|
| `cargo test` | 0 failed | ✅ | ✅ | ✅ |
| `cargo fmt --check` | exit 0 | ✅ | ✅ | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ | ✅ | ✅ |
| Conformance gate | 5026+ (no regression) | ✅ | ✅ | ✅ |
| Conformance FAIL → PASS | +N tests | 0 | +53 (12 + 41) | +N |
| §16 violations | 0 active | 0 (TD-028 closed) | 0 | 0 |
| §25.8 回写 | design docs updated | TD-029 closed | — | — |
| `docs/tests/v0/stage*/plan/README.md` | all 13 stages | 13/13 ✅ | 13/13 ✅ | 13/13 ✅ |

---

## 4. §14.4 重构六大判据

Stage 13.1 (TD-028 + TD-029) 是重构活动，必须符合 §14.4 J1-J6 判据：

| 判据 | TD-028 (mir::dyn_trait 重构) | TD-029 (TyKind::Dynamic 重构) |
|------|----------------------------|-----------------------------|
| J1 架构对齐 | ✅ 恢复 §16 数据流单向 | ✅ 与设计 §1.1 对齐 |
| J2 单一职责 | ✅ MIR 不再产 codegen 文本 | ✅ dyn Trait 是 first-class type |
| J3 单向流动 | ✅ 消除 mir → codegen 反向 | ✅ 类型层级一致 |
| J4 编译表达完整 | ✅ | ✅ |
| J5 阶段划分清晰 | ✅ ≤3 文件 | ✅ ≤5 文件 |
| J6 科学粒度 | ✅ 不影响其他模块 | ✅ |

**结论**: 两个重构均符合 §14.4 六大判据，可执行。

---

## 5. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| Stage 13.4 macro_rules! 工作量超预期 | 高 | 高（阻塞 v0.3） | 分 3 子阶段（MUV-9/10/11），可单独交付 |
| Stage 13.3 闭包 Fn/FnMut/FnOnce trait 推断错误 | 中 | 中 | 参考 rustc closure kind inference |
| Stage 13.2 if-let borrowck refinement scope | 中 | 中 | 参考 rustc NLL if-let scope |
| 重构 (13.1) 引入回归 | 低 | 中 | 5026 conformance + 2179 integration tests 把关 |

---

## 6. 文档同步计划 (per §17.3 + §18)

### 6.1 开发轮文档 (Stage 13.1)
- `docs/develop/v0/stage-13/` (新建目录)
- `plan-13.1.md` (本文件)
- `gate-review-13.1.md` (Stage 13.1 完成后)
- `dev-log.md` (持续更新)

### 6.2 测试文档
- `docs/tests/v0/stage13/plan/README.md` (新建)
- `tests/v0/stage13/plan/stage13_1_tests.rs` (新建 — 验证审计文档 + Stage 13 plan + 文档完整性)
- `tests/v0/stage13/plan/stage13_2_tests.rs` (if-let 实现)
- `tests/v0/stage13/plan/stage13_3_tests.rs` (closure call 实现)
- `tests/v0/stage13/plan/stage13_4_tests.rs` (macro_rules! 实现)

### 6.3 设计文档同步
- `docs/lang-design/03-type-system.md` §13 — Stage 13.1 完成后更新 TD-029 状态为 ✅
- `docs/lang-design/02-grammar.md` — Stage 13.2 完成后补充 if-let/while-let 产生式
- `docs/lang-design/05-ast.md` — Stage 13.2 完成后补充 IfLet/WhileLet 变体
- `docs/lang-design/06-mir.md` — Stage 13.3 完成后补充 closure call lowering
- `docs/lang-design/09-stdlib.md` — Stage 13.4 完成后补充 macro_rules! 系统

### 6.4 Worklog + RELEASE_NOTES
- 每个子阶段完成后追加 `docs/worklog.md`
- 每个子阶段完成后追加 `RELEASE_NOTES.md` 版本小节

---

## 7. 下一步行动

| 序号 | 行动 | 责任 | 预计 |
|------|------|------|------|
| 1 | 创建 `docs/develop/v0/stage-13/` 目录 | REC-A | 1 小时 |
| 2 | 创建 `docs/tests/v0/stage13/plan/` 目录 + README | REC-A | 1 小时 |
| 3 | 创建 `tests/v0/stage13/plan/` 目录 + stage13_1_tests.rs | QA-A | 2 小时 |
| 4 | 接入 `tests/all_tests.rs` | DEV-A | 30 分钟 |
| 5 | 执行 Stage 13.1 MUV-1 (TD-028) | DEV-A + ARCH-A | 4 小时 |
| 6 | 执行 Stage 13.1 MUV-2 (TD-029) | DEV-A + ARCH-A | 1-2 天 |
| 7 | Stage 13.1 gate review | REV-A | 1 小时 |
| 8 | 进入 Stage 13.2 (if-let/while-let) | DEV-A | 1-2 周 |

---

**Stage 13 启动条件**: 本 plan 通过委员会审批 → Stage 13.1 开始执行。

**Plan 创建**: 2026-07-26
**Plan 审批**: 待 Stage Committee 投票
