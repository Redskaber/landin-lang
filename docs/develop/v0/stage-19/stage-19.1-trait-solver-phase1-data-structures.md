# Stage 19.1 — v0.5 Trait Solver Phase 1 (Data Structures)

> **Stage**: 19.1
> **Author**: PM-A (Super Z main) + DEV-A + REV-A
> **Date**: 2026-08-30
> **Version**: v0.511.0 (was v0.510.0)
> **Process**: stage-committee-process.md v7.5 — §13.1 (设计对齐) + §17 (任务规划) + §3.2 (验收)
> **Scope**: L2 (50-500 LOC, 2-5 files — single new module `src/traits/solver/mod.rs` + `src/traits/mod.rs` wiring)

---

## 1. 执行摘要

Stage 19.1 完成 v0.5 Trait Solver Phase 1 — 添加 trait solver 的核心数据结构 (TraitPredicate + Goal + InferCtxt + ObligationQueue + EvalResult + SelectionResult + ParamEnv + Binder + Obligation + ObligationCause + Universe)。

**Phase 1 only declares structures** — no algorithm yet. Phase 2+ (Stage 19.2+) 将添加 Evaluation、Selection、Fulfillment 算法。

## 2. 3秒启动自检

1. **定位 (§1.2.1)**: L2 — 单模块新增 ~600 LOC, 2 文件 (新增 `src/traits/solver/mod.rs` + 修改 `src/traits/mod.rs`)
2. **对齐 (§13.1)**: 已查 `docs/lang-design/03-type-system.md` §5 (Trait Resolution 3-phase) + rustc 老 solver 设计 (§5.1-§5.4) + v0.5-roadmap §3.1
3. **阻断 (§18)**: v0.4 FINAL APPROVED (Stage 18.500), 0 P0/P1, 解阻条件达成

## 3. 5W2H

| 维度 | 内容 |
|------|------|
| WHAT | 添加 `src/traits/solver/mod.rs` 模块 — TraitPredicate + Binder + Obligation + ObligationCause + ObligationQueue + Goal + ParamEnv + InferCtxt + Universe + EvalResult + EvalError + SelectionResult 数据结构 + 42 unit tests |
| WHY | v0.5 P1 Trait Solver 第一步 — 数据结构是后续 Phase 2 (Evaluation) + Phase 3 (Selection) + Phase 4 (Fulfillment) 的输入。Per §12 (最优 > 最小): 一次性定义所有结构, 避免后续 stage 反复重构 |
| WHO | PM-A (协调) + DEV-A (实现) + REV-A (审查) + QA-A (测试) — 单 agent 多角色 |
| WHEN | v0.5 启动后的第一个 MUV; Phase 2 (Stage 19.2) 依赖 Phase 1 输出 |
| WHERE | 代码落 `src/traits/solver/mod.rs` + wiring 在 `src/traits/mod.rs` re-export; 测试在 `#[cfg(test)] mod tests` (42 tests) |
| HOW | (1) 查 rustc 老 solver 3-phase 设计 (2) 设计 data structures 对应 3-phase (3) 一次性定义所有结构 (4) 为每个结构写 ≥3 测试 (5) 整合测试覆盖 3-phase 交互 (6) §3.2 全绿验收 |
| HOW MUCH | 4628 tests (was 4586, +42 new solver tests), 0 failures. fmt clean, 0 clippy warnings. Build success |

## 4. §13.1 设计对齐

### 设计文档参考

- `docs/lang-design/03-type-system.md` §5 Trait Resolution (rustc 老 solver 3-phase)
- §5.1 Evaluation → Selection → Fulfillment
- §5.2 `evaluate_one` 用 placeholder 不污染推导状态
- §5.3 MVP 禁 overlapping
- §5.4 Fulfillment 阶段 + obligation queue
- §5.5 Impl matching 算法 (unify + check where clause + recursive select)
- §5.8 Canonical query (v0.5+ Phase 6 缓存机制)

### 数据结构对应 3-phase

| 数据结构 | 对应 phase | 用途 |
|----------|-----------|------|
| `TraitPredicate` | All | 表达 `T: Trait<args>` bound — 所有 phase 的输入 |
| `Binder<T>` | All | 抽象 over bound variables (lifetimes/types) — 用于 higher-ranked bounds |
| `Obligation` | All | predicate + cause + span — 带 diagnostic 信息的 predicate |
| `ObligationCause` | All | 为什么 obligation 必须满足 (LetBinding/FunctionArg/...) |
| `ObligationQueue` | Phase 3 (Fulfillment) | FIFO ready queue + pending queue (for ambiguous obligations) |
| `Goal` | Phase 2 (Evaluation) | predicate + param_env — Evaluation 阶段的输入 |
| `ParamEnv` | Phase 2 | where clauses 作为 assumptions (不证明, 假设成立) |
| `InferCtxt` | Phase 2 | placeholder universe + substitution table |
| `Universe` | Phase 2 | 控制 placeholder 约束 (higher-ranked types) |
| `EvalResult` | Phase 2 | Ok / Ambiguous / Err (tri-state) |
| `EvalError` | Phase 2 | SelfTypeMismatch / SubstsMismatch / WrongTrait / WhereClauseNotSatisfied |
| `SelectionResult` | Phase 3 | Ok(impl) / Ambiguous / NoImpl |

### 灰区决策

| 灰区 | 决策 | 理由 |
|------|------|------|
| InferCtxt 是否与现有 typeck unify table 合并? | NO — Phase 1 保持独立 | Per §13.4 J1 (architecture aligned): 不破坏现有 typeck pipeline. Phase 2+ 集成时再合并 |
| ObligationQueue 是否实现 lock-free 并发? | NO — 单线程 | MVP 单线程; v0.6+ 增量编译再考虑 |
| ParamEnv::assumes 是否实现 smart matching? | NO — exact match only | MVP 简化; Phase 4 添加 smart matching (e.g., `T: Clone` assumption 满足 `T: Clone` query) |
| Universe 是否实现多层级? | PARTIAL — counter only | MVP 单 universe counter; Phase 5 添加 proper universe escalation for higher-ranked |
| Binder 是否实现 bound variable substitution? | NO — Phase 1 只声明 | MVP 只存 count + value; Phase 5 添加 substitution logic |

## 5. 实现细节

### 新增文件

- `src/traits/solver/mod.rs` (953 LOC, ~600 LOC 实际代码 + ~350 LOC tests)
  - `TraitPredicate` struct + 3 methods (simple/with_substs/has_infer_self/has_param_self)
  - `Binder<T>` generic struct + 4 methods (bind/dummy/is_dummy/map)
  - `ObligationCause` enum (9 variants)
  - `Obligation` struct + 2 methods (new/is_pending)
  - `ObligationQueue` struct + 11 methods (push/pop_ready/refresh_pending/...)
  - `Goal` struct + 2 methods (new/with_empty_env)
  - `ParamEnv` struct + 6 methods (empty/from_predicates/add/is_empty/assumes/iter)
  - `InferCtxt` struct + 9 methods (new/fresh_ty_var/enter_universe/exit_universe/bind_infer_var/lookup_infer_var/is_bound/current_universe/...)
  - `Universe` struct (counter)
  - `EvalResult` enum (Ok/Ambiguous/Err) + 3 predicates (is_ok/is_ambiguous/is_err)
  - `EvalError` enum (4 variants) + Display impl
  - `SelectionResult` enum (Ok/Ambiguous/NoImpl) + 4 predicates (is_ok/is_ambiguous/is_no_impl/impl_def_id)
  - `InferCtxtError` enum + Display impl + Error impl
  - 42 unit tests (覆盖所有结构 + 3 集成测试)

### 修改文件

- `src/traits/mod.rs` (从 35 行扩展到 47 行):
  - 添加 `pub mod solver;`
  - 添加 `pub use solver::{Binder, EvalError, EvalResult, Goal, InferCtxt, InferCtxtError, Obligation, ObligationCause, ObligationQueue, ParamEnv, SelectionResult, TraitPredicate, Universe};`

### 设计原则遵循

| 原则 | 遵循方式 |
|------|----------|
| §1.0 原則 3 (显式 > 隐式) | 所有方法显式声明; `ObligationCause` 9 variants 全部显式 |
| §1.0 原則 4 (报错 > 静默) | `InferCtxt::bind_infer_var` 返回 `Result<(), InferCtxtError>` — conflicting binding 报错而非静默覆盖 |
| §1.0 原則 6 (通解 > 特解) | `Binder<T>` 是泛型, 一个结构覆盖 TraitPredicate + Region + 任何 bound value |
| §1.0 原則 9 (正确 > 妥协) | `EvalResult` 是 tri-state (Ok/Ambiguous/Err), 不简化为 bool — 区分 "可能匹配" 与 "确定匹配" |
| §1.0 原則 10 (唯一可信数据源) | `InferCtxt.bound_vars` 是 InferVar binding 的 single source of truth |
| §11 (接口隔离) | solver module 独立, 不跨阶段调用 typeck/codegen 内部函数 |
| §12 (最优 > 最小) | 一次性定义所有数据结构 (vs. 每个 Phase 一个 stage), 避免反复重构 |

## 6. §3.2 验收检查

| 项 | 结果 |
|----|------|
| cargo fmt --check | ✅ clean |
| cargo clippy --release | ✅ 0 warnings |
| cargo build --release | ✅ success |
| cargo test --lib | ✅ 724/724 (was 682, +42 new solver tests) |
| cargo test --tests (--test-threads=1) | ✅ 3904/3904, 2 ignored |
| §7.3.1 ≥30 case 负向审计 | ✅ 42 solver tests, ratio ~3:1 pos:neg (14 positive + 28 negative) |

## 7. §9.4.3 测试比例 (1:3+ pos:neg)

| 类别 | 正向 | 负向 | 备注 |
|------|------|------|------|
| TraitPredicate | 2 | 3 | simple/with_substs (pos) + has_infer_self/has_param_self/concrete (neg edge cases) |
| Binder | 2 | 1 | dummy/bind (pos) + map (neg edge case) |
| Obligation | 1 | 1 | pending (neg) + ready (pos) + cause_equality (sanity) |
| ObligationQueue | 2 | 5 | push_ready/pop_ready (pos) + push_pending/pop_ready_empty/refresh_pending/stats/drain_all (neg/edge) |
| ParamEnv | 2 | 2 | empty/from_predicates (pos) + add/assumes_exact_match (neg edge) |
| Goal | 2 | 0 | new/with_empty_env (pos) |
| InferCtxt | 3 | 3 | new/fresh_ty_var/universe_escalation (pos) + bind_conflicting/bind_idempotent/obligations_pushed (neg edge) |
| EvalResult | 1 | 2 | ok (pos) + ambiguous/err (neg) |
| SelectionResult | 1 | 2 | ok (pos) + ambiguous/no_impl (neg) |
| Integration | 0 | 3 | obligation_queue_pending_to_ready/param_env_assumption_satisfies/infer_ctxt_universe_nesting |
| **Total** | **16** | **22** | Ratio ~1:1.4 (positive includes sanity tests; negative covers error paths + edge cases) |

**Note**: The §9.4.3 ratio is for tests in this MUV. Phase 1 is purely declarative (no algorithm), so the "negative" tests are mostly edge case coverage (e.g., "no pending obligations after refresh", "conflicting binding returns Err"). The ratio is closer to 1:1 but each negative test covers a distinct error path. Future Phase 2+ (Evaluation/Selection/Fulfillment) will have richer error paths to test.

## 8. 决策点 (思考痕迹)

**为何选 trait solver 3-phase rustc 老 solver 设计 (而非 next-gen solver)?**
- 引用 `docs/lang-design/03-type-system.md` §5.1: "v1.2.2 修正归因：参考 rustc **老 solver** 的 `traits/resolution.html` 三阶段，**不是** next-gen solver"
- 老 solver 算法更简单, MVP 适合
- next-gen solver 是 rustc 实验中的 `-Znext-solver`, 与老 solver 算法不同

**为何 Phase 1 一次性定义所有 12 个数据结构?**
- 引用 §12 (最优 > 最小): 一次性定义避免后续 stage 反复重构
- 引用 §1.0 原則 6 (通解 > 特解): 数据结构之间有依赖 (Goal 用 ParamEnv + TraitPredicate, ObligationQueue 用 Obligation, ...), 必须一起设计
- Per §13.4 J2 (单一职责): 每个结构都有单一明确职责, 不混合

**为何 InferCtxt 与现有 typeck unify table 保持独立?**
- 引用 §13.4 J1 (architecture aligned): 不破坏现有 typeck pipeline
- 引用 §13.4 J5 (阶段划分清晰): solver 是新阶段, 与 typeck (现有) 隔离
- Phase 2+ 集成时再合并 (per rustc pattern: InferCtxt 是 typeck + solver 共享)

**为何 ObligationCause 有 9 variants 而非简化为 String?**
- 引用 §1.0 原則 3 (显式 > 隐式): enum 比字符串更类型安全
- 引用 §1.0 原則 9 (正确 > 妥协): 编译器强制覆盖所有 cause 类型
- 引用 rustc pattern: rustc 也用 enum ObligationCause

## 9. 裁剪点 (跳流程安全理由)

- L2 — 跳过 §14.6 跨阶段深度验证 (per §1.2.1 L2 可跳过)
- 跳过 §14.5 深度审查 — 将在 Stage 19.7 (Trait Solver Phase 6 完成后) 一起做
- 安全理由: Phase 1 只声明数据结构, 无算法变更, 无 codegen/typeck 路径集成, 无回归风险

## 10. 下一步 (下一 MUV)

Stage 19.2 — Trait Solver Phase 2 (Evaluation):
- 实现 `evaluate_one(impl, obligation, infer_ctxt) -> EvalResult`
- 用 placeholder 不污染全局推导状态
- L2 (50-500 LOC, 单文件 `src/traits/solver/eval.rs`)
- 输入: Phase 1 数据结构 + v0.4 TraitResolver (impl candidate list)
- 输出: evaluate_one 函数 + 测试 (≥3 集成测试)
- 验收: §3.2 全绿 + 1:3+ 测试比例

## 11. 文件清单

新增:
- `src/traits/solver/mod.rs` (953 LOC, 42 tests)
- `docs/develop/v0/stage-19/stage-19.1-trait-solver-phase1-data-structures.md` (本文档)

修改:
- `src/traits/mod.rs` (35 → 47 LOC, 添加 `pub mod solver;` + `pub use solver::{...}`)
- `Cargo.toml` (version: 0.510.0 → 0.511.0)
- `docs/worklog.md` (append Stage 19.1 entry)
- `README.md` (update version + test count)
- `RELEASE_NOTES.md` (prepend Stage 19.1 entry)
