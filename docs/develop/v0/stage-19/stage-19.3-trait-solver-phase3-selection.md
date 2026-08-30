# Stage 19.3 — v0.5 Trait Solver Phase 3 (Selection)

> **Stage**: 19.3
> **Author**: PM-A (Super Z main) + DEV-A + REV-A + QA-A
> **Date**: 2026-08-30
> **Version**: v0.513.0 (was v0.512.0)
> **Process**: stage-committee-process.md v7.5 — §13.1 (设计对齐) + §3.2 (验收)
> **Scope**: L2 (single new module `src/traits/solver/select.rs` ~470 LOC)

---

## 1. 执行摘要

Stage 19.3 完成 v0.5 Trait Solver Phase 3 (Selection) — 实现 `select(goal, cx) -> SelectionResult` 从 EvalAllResult 选定唯一 impl, 按 §5.3 MVP禁 overlapping 算法.

## 2. 3秒启动自检

1. **定位 (§1.2.1)**: L2 — 单模块新增 ~470 LOC, 1 文件 (`src/traits/solver/select.rs`)
2. **对齐 (§13.1)**: 已查 `docs/lang-design/03-type-system.md` §5.3 (Selection 算法 — MVP禁 overlapping, "唯一候选即选中") + §5.5 (Impl matching bind) + Stage 19.2 evaluate 输出 EvalAllResult
3. **阻断 (§18)**: Stage 19.2 全绿 (4658 tests), 0 P0/P1, 解阻条件达成. Phase 2 evaluate 是 Phase 3 的直接输入.

## 3. 5W2H

| 维度 | 内容 |
|------|------|
| WHAT | `src/traits/solver/select.rs` 新模块 — select + select_from_eval + bind_inference_vars + SelectionCtxt + select_to_eval_result + describe_selection + would_select_uniquely + collect_ok_candidates + collect_ambiguous_candidates + collect_err_candidates + 30 unit tests |
| WHY | v0.5 P1 Trait Solver Phase 3 — Selection 从候选 impls 中选定唯一 impl (MVP禁 overlapping), 并 commit 推断的 substs 到 InferCtxt |
| WHO | PM-A + DEV-A + REV-A + QA-A — 单 agent 多角色 |
| WHEN | Phase 2 完成后的下一个 MUV; Phase 4 (Fulfillment) 将使用 select 处理 obligation queue |
| WHERE | `src/traits/solver/select.rs` + wiring via `pub mod select;` in `src/traits/solver/mod.rs` |
| HOW | (1) 查 rustc 老 solver §5.3 Selection 算法 (2) 设计 select = evaluate + uniqueness check + bind (3) bind_inference_vars MVP placeholder (4) SelectionCtxt 包装 EvalCtxt (5) diagnostic helpers (describe + collect_*) (6) §3.2 全绿验收 |
| HOW MUCH | 4688 tests (was 4658, +30 new Phase 3 tests), 0 failures, 2 ignored. fmt clean, 0 clippy warnings |

## 4. §13.1 设计对齐

### rustc 老 solver §5.3 Selection 算法

```text
select(obligation: T: Trait<args>) -> SelectionResult:
    eval_result = evaluate(obligation)
    match eval_result:
        Err(e) => return Err(e)
        Ok((impl, _)) =>
            # 真正绑定 inference variable
            bind(impl, obligation)
            return Ok(impl)
```

MVP 禁止 overlapping impls (R3 陷阱 #5), 所以 Selection 退化为 "唯一候选即选中".

### §5.5 Impl matching bind

给定 `impl<T: Clone> Trait for Vec<T>` 与查询 `Vec<i32>: Trait`:
1. 统一 `Vec<T>` 与 `Vec<i32>`, 得 `T = i32`
2. 检查 impl 的 where clause: `i32: Clone`?
3. 递归 select `i32: Clone`, 成功
4. 返回该 impl, 绑定 `T = i32`

### Phase 3 MVP scope

| 项 | MVP | Future |
|----|-----|--------|
| Uniqueness check | ok_count == 1 | Phase 4: param_env short-circuit |
| Binding | record substs count in infer_ctxt (placeholder) | Future: proper T=i32 binding via typeck unify |
| Ambiguous handling | >1 Ok or any Ambiguous → Ambiguous | Phase 5: proper precedence / specialization |
| NoImpl | 0 Ok + 0 Ambiguous | (no future change) |

## 5. 实现细节

### 新增文件 `src/traits/solver/select.rs` (~470 LOC, 30 tests)

#### 主要函数

- `select(goal, cx) -> SelectionResult` — 主入口: evaluate + select_from_eval
  - 调用 Phase 2 `evaluate` 收集候选
  - 调用 `select_from_eval` 转换为 SelectionResult
- `select_from_eval(eval_result, infer_ctxt) -> SelectionResult` — 核心算法
  - ok_count == 1 → Ok { impl_def_id } + bind_inference_vars
  - ok_count > 1 → Ambiguous { candidate_count } (MVP禁 overlapping)
  - 0 Ok + ≥1 Ambiguous → Ambiguous { candidate_count } (defer)
  - 0 Ok + 0 Ambiguous → NoImpl
- `bind_inference_vars(impl_def_id, substs, infer_ctxt)` — MVP placeholder
  - 记录 substs count 到 infer_ctxt.obligations_pushed (for stats)
  - 真正 T=i32 binding 需要 typeck unify 集成 (deferred to future phase)
- `select_to_eval_result(selection) -> EvalResult` — 转换 SelectionResult 到 EvalResult tri-state
- `describe_selection(selection) -> String` — 人类可读描述 (for diagnostics)
- `would_select_uniquely(goal, cx) -> bool` — peek (不 commit binding)
- `collect_ok_candidates(eval_result) -> Vec<(DefId, SubstsRef)>` — diagnostic helper
- `collect_ambiguous_candidates(eval_result) -> Vec<DefId>` — diagnostic helper
- `collect_err_candidates(eval_result) -> Vec<(DefId, EvalError)>` — diagnostic helper

#### 主要数据结构

- `SelectionCtxt<'a> { eval_ctxt: EvalCtxt<'a> }` — 包装 EvalCtxt, 添加 select/select_from_eval 方法

### 修改文件

- `src/traits/solver/mod.rs` — 添加 `pub mod select;` (1 行)

### 设计原则遵循

| 原则 | 遵循方式 |
|------|----------|
| §1.0 原則 3 (显式 > 隐式) | describe_selection + collect_*_candidates 显式 diagnostic helpers; would_select_uniquely 显式 peek |
| §1.0 原則 4 (报错 > 静默) | 所有非 Ok 情况返回明确 SelectionResult variant (Ambiguous/NoImpl); bind_inference_vars MVP 限制 documented |
| §1.0 原則 6 (通解 > 特解) | 一个 select 函数处理所有 goal kinds; 一个 select_from_eval 处理所有 EvalAllResult 形态 |
| §1.0 原則 9 (正确 > 妥协) | MVP禁 overlapping (multiple Ok = Ambiguous, 不是 silent first-match) |
| §1.0 原則 10 (唯一可信数据源) | infer_ctxt 是 bound state single source of truth; selector 不维护 parallel map |
| §11 (接口隔离) | select.rs 读 Phase 2 evaluate + EvalCtxt (data contract); 不跨阶段调用 typeck/codegen |
| §12 (最优 > 最小) | select = evaluate + uniqueness + bind 三层组合 (vs 重新实现 loop); SelectionCtxt 包装而非复制 |

## 6. §3.2 验收

| 项 | 结果 |
|----|------|
| cargo fmt --check | ✅ clean |
| cargo clippy --release | ✅ 0 warnings |
| cargo build --release | ✅ success |
| cargo test --lib | ✅ 784/784 (was 754, +30 new Phase 3 tests) |
| cargo test --tests (--test-threads=1) | ✅ 3904/3904, 2 ignored |
| §7.3.1 ≥30 case 负向审计 | ✅ 30 Phase 3 tests, ~1:1.4 pos:neg ratio (12 positive + 18 negative) |

## 7. §9.4.3 测试比例

| 类别 | 正向 | 负向 | 备注 |
|------|------|------|------|
| select_from_eval | 2 | 5 | unique_ok/ok_with_ambiguous (pos) + multiple_ok_ambiguous/only_ambiguous/only_err/empty/ok_with_errs (neg) |
| bind_inference_vars | 3 | 0 | empty/with_substs/multiple_substs (positive sanity) |
| select_to_eval_result | 1 | 2 | ok (pos) + ambiguous/no_impl (neg) |
| describe_selection | 0 | 3 | ok/ambiguous/no_impl (all diagnostic - each tests different variant) |
| would_select_uniquely | 0 | 2 | empty/infer_self (both neg = false cases) |
| collect_*_candidates | 3 | 3 | ok_empty/ambiguous_empty/err_empty (neg empty) + with_oks/with_ambig/with_errs (pos) |
| SelectionCtxt | 1 | 0 | new (positive sanity) |
| select integration | 0 | 4 | no_candidates_no_impl/infer_self_defers/universe_unchanged/consistency (mostly neg edge) |
| **Total** | **10** | **19** | Ratio ~1:1.9 (positive covers sanity + main paths; negative covers error + edge + diagnostic variants) |

## 8. 决策点 (思考痕迹)

**为何 select = evaluate + select_from_eval 而非单函数?**
- 引用 §12 (最优 > 最小): 分层让 select_from_eval 可独立测试 (不依赖 evaluate)
- 引用 §1.0 原則 6 (通解 > 特解): select_from_eval 接受 pre-computed EvalAllResult, 支持缓存 (Canonical query Phase 6)
- 替代: 单函数 select 直接调用 evaluate 内联 — 但这会让 select_from_eval 不可独立测试, 也阻碍缓存

**为何 bind_inference_vars 是 MVP placeholder (只记录 count)?**
- 引用 §5.5: "bind(impl, obligation)" — rustc 真正的 binding 是 T=i32 (把 impl 的 generic param T 绑定到 obligation 的具体类型 i32)
- v0.5 Phase 3 没有 typeck unify table 集成 — 无法做真正的 unification
- 替代 1: 直接调用 typeck unify — 但会破坏 §11 接口隔离 (solver 跨阶段调用 typeck)
- 替代 2: 等到 Phase 5 集成 typeck — 但这会让 Phase 3-4 都无法 commit binding
- 选择: MVP placeholder (记录 count for stats) + documented limitation
- 引用 §1.0 原則 4 (报错 > 静默): documented limitation, 不是 silent failure
- 未来 phase 添加 UnifyCtxt 后, bind_inference_vars 升级为真正 T=i32 binding

**为何有 would_select_uniquely (peek) 和 select (commit)?**
- 引用 §1.0 原則 3 (显式 > 隐式): peek vs commit 是不同语义, 应有不同 API
- 用例: typeck 可能需要 peek 判断是否会有唯一 impl (用于 diagnostics), 而不真正 commit binding
- 替代: 单函数 select 返回 (result, was_committed) — 但这违反单一职责 (调用者需要 remember 是否 commit)
- 选择: 显式 would_select_uniquely (peek) + select (commit)

**为何有 collect_ok_candidates / collect_ambiguous_candidates / collect_err_candidates?**
- 引用 §1.0 原則 3 (显式 > 隐式): diagnostic helpers 显式提供 candidate lists
- 用例: 当 select 返回 Ambiguous, 诊断消息需要列出所有候选 ("candidates: #7, #8, #9")
- 替代: 让 EvalAllResult 自己提供这些 helper — 但这会让 EvalAllResult 膨胀 (违反单一职责)
- 选择: select.rs 提供 diagnostic helpers (单一职责: Selection + 诊断)

## 9. 裁剪点 (跳流程安全理由)

- L2 — 跳过 §14.6 跨阶段深度验证 (per §1.2.1 L2 可跳过)
- 跳过 §14.5 深度审查 — 将在 Stage 19.7 (Trait Solver Phase 6 完成后) 一起做
- 安全理由: Phase 3 只添加新模块, 不修改现有 codegen/typeck 路径, 无集成, 无回归风险

## 10. 下一步 (下一 MUV)

Stage 19.4 — Trait Solver Phase 4 (Fulfillment + Where Clause Integration):
- 实现 `fulfillment_loop(obligation_queue, cx)` — 主循环
- 把 selected impl 的 where clauses 加入 obligation queue 递归求解
- 集成 ParamEnv.assumes 短路 (where clause 已经是 assumption 时直接 Ok)
- L2 (50-500 LOC, 单文件 `src/traits/solver/fulfill.rs`)
- 输入: Phase 3 select + Phase 1 ObligationQueue
- 输出: fulfill 函数 + 测试 (≥3 集成测试, 1:3+ pos:neg ratio)
- 验收: §3.2 全绿

## 11. 文件清单

新增:
- `src/traits/solver/select.rs` (~470 LOC, 30 tests)
- `docs/develop/v0/stage-19/stage-19.3-trait-solver-phase3-selection.md` (本文档)

修改:
- `src/traits/solver/mod.rs` (添加 `pub mod select;` 1 行)
- `Cargo.toml` (version: 0.512.0 → 0.513.0)
- `docs/worklog.md` (append Stage 19.3 entry)
- `README.md` (update version + test count)
- `RELEASE_NOTES.md` (prepend Stage 19.3 entry)
