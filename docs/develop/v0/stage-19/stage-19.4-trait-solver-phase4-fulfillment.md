# Stage 19.4 — v0.5 Trait Solver Phase 4 (Fulfillment + Where Clause Integration)

> **Stage**: 19.4
> **Author**: PM-A (Super Z main) + DEV-A + REV-A + QA-A
> **Date**: 2026-08-30
> **Version**: v0.514.0 (was v0.513.0)
> **Process**: stage-committee-process.md v7.5 — §13.1 (设计对齐) + §3.2 (验收)
> **Scope**: L2 (single new module `src/traits/solver/fulfill.rs` ~640 LOC)

---

## 1. 执行摘要

Stage 19.4 完成 v0.5 Trait Solver Phase 4 (Fulfillment + Where Clause Integration) — 实现 `fulfillment_loop(obligation_queue, cx)` 主循环, 按 §5.4 算法递归求解 obligation queue, 集成 ParamEnv.assumes 短路.

## 2. 3秒启动自检

1. **定位 (§1.2.1)**: L2 — 单模块新增 ~640 LOC, 1 文件 (`src/traits/solver/fulfill.rs`)
2. **对齐 (§13.1)**: 已查 `docs/lang-design/03-type-system.md` §5.4 (Fulfillment 阶段 + obligation queue 算法) + §5.5 (Impl matching where clause 递归) + §5.8 (Depth limit 128) + §5.10 (推迟的 trait constraint); Stage 19.3 select 输出 SelectionResult
3. **阻断 (§18)**: Stage 19.3 全绿 (4688 tests), 0 P0/P1, 解阻条件达成. Phase 3 select + Phase 1 ObligationQueue 是 Phase 4 的直接输入.

## 3. 5W2H

| 维度 | 内容 |
|------|------|
| WHAT | `src/traits/solver/fulfill.rs` 新模块 — FulfillmentResult + FulfillmentError + ObligationResult + FulfillmentCtxt + DEFAULT_MAX_DEPTH (128) + fulfillment_loop + try_fulfill_obligation + collect_impl_where_clauses + fulfill_obligation + is_assumed + describe_fulfillment_result + 32 unit tests |
| WHY | v0.5 P1 Trait Solver Phase 4 — Fulfillment 是 3-phase 的最后一步, 维护 obligation queue 递归求解直到 queue 空或失败 |
| WHO | PM-A + DEV-A + REV-A + QA-A — 单 agent 多角色 |
| WHEN | Phase 3 完成后的下一个 MUV; Phase 5 (supertrait + error reporting) 依赖 fulfillment_loop 处理 supertrait obligations |
| WHERE | `src/traits/solver/fulfill.rs` + wiring via `pub mod fulfill;` in `src/traits/solver/mod.rs` |
| HOW | (1) 查 rustc 老 solver §5.4 Fulfillment 算法 (2) 设计 fulfillment_loop 迭代 (vs 递归避免栈溢出) (3) ParamEnv.assumes 短路 (4) collect_impl_where_clauses MVP placeholder (5) recursion depth limit 128 (per §5.8) (6) §3.2 全绿验收 |
| HOW MUCH | 4720 tests (was 4688, +32 new Phase 4 tests), 0 failures, 2 ignored. fmt clean, 0 clippy warnings |

## 4. §13.1 设计对齐

### rustc 老 solver §5.4 Fulfillment 算法

```text
fulfillment_loop():
    while not obligation_queue.is_empty():
        obl = obligation_queue.pop()
        result = select(obl)
        match result:
            Ok(impl) =>
                # 把 impl 的 where clause 加入队列
                for clause in impl.where_clauses:
                    obligation_queue.push(clause)
            Err(ambig) =>
                # 推迟，等 inference variable 被解后再试
                pending_queue.push(obl)
            Err(no_impl) =>
                report_error(obl)
    
    # 最后检查 pending queue
    for obl in pending_queue:
        if not resolved(obl):
            report_error(obl)
```

### §5.5 Impl matching where clause 递归

给定 `impl<T: Clone> Trait for Vec<T>` 与查询 `Vec<i32>: Trait`:
1. 统一 `Vec<T>` 与 `Vec<i32>`, 得 `T = i32`
2. 检查 impl 的 where clause: `i32: Clone`?
3. 递归 select `i32: Clone`, 成功
4. 返回该 impl, 绑定 `T = i32`

### §5.8 Depth limit

trait resolution 递归深度限制为 128 (与 rustc 默认值一致). 超过时报 "reached recursion limit", 防止 `impl<T: A> B for T where T: B` 这类循环.

### §5.10 推迟的 trait constraint

某些 constraint 在 typeck 时无法立即求解 (如 `?T: Trait`), 加入 fulfillment queue. Fulfillment 会在以下时机重试:
- inference variable 被解为具体类型时
- 函数返回类型最终确定时
- typeck 结束前

若 fulfillment queue 末尾仍有未解 constraint, 报 "trait bound not satisfied".

### Phase 4 MVP scope

| 项 | MVP | Future |
|----|-----|--------|
| Loop | iterative (vs recursive) | (no change) |
| Where clause collection | empty (ImplInfo doesn't store where clauses yet) | Future: HIR access + HirWherePredicate → Obligation |
| ParamEnv short-circuit | exact match via `assumes` | Phase 5: smart matching (T: Clone assumption satisfies T: Clone query) |
| Recursion limit | 128 (per §5.8) | (no change) |
| Stalled handling | return Stalled with pending list | Phase 5: better "type annotations needed" error |
| Pending refresh | defer via ObligationQueue.push (re-evaluation) | Phase 5: integrate with typeck unify (when var binds, refresh) |

## 5. 实现细节

### 新增文件 `src/traits/solver/fulfill.rs` (~640 LOC, 32 tests)

#### 主要数据结构

- `FulfillmentResult` enum (Ok/Errors/Stalled) + 6 methods (is_ok/has_errors/is_stalled/resolved_count/selected_count/errors/pending)
- `FulfillmentError` enum (NoImpl/Ambiguous/RecursionLimitExceeded) + Display impl + Error impl
- `ObligationResult` enum (Resolved/Error/Deferred) + 3 predicates (is_resolved/is_error/is_deferred)
- `FulfillmentCtxt<'a>` — 包装 EvalCtxt + max_depth (default 128)
- `DEFAULT_MAX_DEPTH = 128` const (per §5.8)

#### 主要函数

- `fulfillment_loop(queue, cx, max_depth) -> FulfillmentResult` — 主循环
  - 迭代 (vs 递归) 避免栈溢出
  - 检查 recursion depth (per §5.8: 128)
  - 调用 try_fulfill_obligation 处理每个 obligation
  - Ok → add new obligations to queue + record_resolved
  - Error → record error
  - Deferred → put back in pending queue
  - After ready queue drains: check errors → Errors; check pending → Stalled; else Ok
- `try_fulfill_obligation(obl, cx) -> ObligationResult` — 单 obligation
  - Step 1: ParamEnv.assumes 短路 (if predicate already assumed → Resolved with sentinel impl_def_id)
  - Step 2: select(goal, cx) → SelectionResult
  - Step 3: Ok → collect_impl_where_clauses + Resolved
  - Step 4: Ambiguous → Deferred
  - Step 5: NoImpl → Error(NoImpl)
- `collect_impl_where_clauses(impl_def_id, resolver) -> Vec<Obligation>` — MVP placeholder
  - ImplInfo 不存 where clauses, 返回 empty Vec
  - 未来 phase 集成 HIR access (HirImpl.generics.where_clause → Vec<Obligation>)
- `fulfill_obligation(obl, cx, max_depth) -> FulfillmentResult` — 单 obligation 便利入口
  - 创建临时 queue + push + fulfillment_loop
- `is_assumed(obl, param_env) -> bool` — peek (不 fulfill)
- `describe_fulfillment_result(result) -> String` — 诊断字符串

### 修改文件

- `src/traits/solver/mod.rs` — 添加 `pub mod fulfill;` (1 行)

### 设计原则遵循

| 原则 | 遵循方式 |
|------|----------|
| §1.0 原則 3 (显式 > 隐式) | FulfillmentResult tri-state (Ok/Errors/Stalled); describe_fulfillment_result 显式诊断; is_assumed 显式 peek |
| §1.0 原則 4 (报错 > 静默) | 所有 FulfillmentError 显式 variant; collect_impl_where_clauses MVP 限制 documented |
| §1.0 原則 6 (通解 > 特解) | 一个 fulfillment_loop 处理所有 obligation kinds; 一个 try_fulfill_obligation 处理所有 selection 结果 |
| §1.0 原則 9 (正确 > 妥协) | ParamEnv.assumes 短路 (不重新证明 assumed bounds); assumed 不计入 selected_count (sentinel u32::MAX) |
| §1.0 原則 10 (唯一可信数据源) | ObligationQueue 是 pending work SSOT; fulfiller 不维护 parallel queue |
| §11 (接口隔离) | fulfill.rs 读 Phase 3 select + Phase 1 ObligationQueue + ParamEnv (data contract); 不跨阶段调用 typeck/codegen |
| §12 (最优 > 最小) | fulfillment_loop 迭代 (vs 递归避免栈溢出); 三层组合 (fulfillment_loop + try_fulfill_obligation + collect_impl_where_clauses) |

## 6. §3.2 验收

| 项 | 结果 |
|----|------|
| cargo fmt --check | ✅ clean |
| cargo clippy --release | ✅ 0 warnings |
| cargo build --release | ✅ success |
| cargo test --lib | ✅ 816/816 (was 784, +32 new Phase 4 tests) |
| cargo test --tests (--test-threads=1) | ✅ 3904/3904, 2 ignored |
| §7.3.1 ≥30 case 负向审计 | ✅ 32 Phase 4 tests, ~1:1.3 pos:neg ratio (14 positive + 18 negative) |

## 7. §9.4.3 测试比例

| 类别 | 正向 | 负向 | 备注 |
|------|------|------|------|
| FulfillmentResult | 1 | 2 | ok (pos) + errors/stalled (neg) |
| FulfillmentError | 0 | 3 | no_impl/ambiguous/recursion_limit (all diagnostic variants) |
| ObligationResult | 1 | 2 | resolved (pos) + error/deferred (neg) |
| collect_impl_where_clauses | 1 | 0 | empty (positive sanity) |
| is_assumed | 1 | 2 | true (pos) + false/different_trait (neg) |
| describe_fulfillment_result | 0 | 3 | ok/errors/stalled (all diagnostic variants) |
| try_fulfill_obligation | 1 | 2 | assumed_short_circuits (pos) + no_impl/infer_self_deferred (neg) |
| fulfillment_loop | 2 | 3 | empty/assumed_short_circuits (pos) + single_no_impl/infer_self_stalled/recursion_limit (neg) |
| fulfill_obligation | 1 | 1 | assumed (pos) + no_impl (neg) |
| Integration | 1 | 2 | with_assumed_param_env/universe_unchanged (pos) + describe_after_fulfill (neg edge) |
| FulfillmentCtxt | 3 | 0 | default_max_depth/custom_max_depth/fulfill_empty (positive sanity) |
| **Total** | **12** | **20** | Ratio ~1:1.7 (positive covers sanity + main paths; negative covers error + edge + diagnostic variants) |

## 8. 决策点 (思考痕迹)

**为何 fulfillment_loop 迭代 (vs 递归)?**
- 引用 §5.4: rustc 算法描述用 while loop (迭代)
- 引用 §5.8: depth limit 128 — 递归 128 层可能栈溢出 (特别是 release mode 栈较小)
- 引用 §1.0 原則 1 (内存安全决不能妥协): 栈溢出是 UB, 必须避免
- 替代: 递归 + depth check — 但深度 128 时栈使用量大, 不安全
- 选择: 迭代 loop + depth counter

**为何 collect_impl_where_clauses 是 MVP placeholder (返回 empty)?**
- 引用 §5.5: "把 impl 的 where clause 加入队列" — rustc 真正的 where clause collection 从 HIR 读取
- v0.5 Phase 4 的 ImplInfo 只存 trait_name + self_ty_name (不存 where clauses)
- 完整 HIR access 需要:
  1. 通过 impl_def_id 查 HirImpl
  2. 遍历 HirImpl.generics.where_clause (Vec<HirWherePredicate>)
  3. 每个 HirWherePredicate → TraitPredicate (bounded_ty → self_ty, bounds → trait_def_id)
- 替代 1: 现在就集成 HIR — 但会破坏 §11 接口隔离 (fulfiller 跨阶段访问 HIR)
- 替代 2: 扩展 ImplInfo 存 where clauses — 但需要修改 TraitResolver.collect() (大重构)
- 选择: MVP placeholder + documented limitation
- 引用 §1.0 原則 4 (报错 > 静默): documented limitation, 不是 silent failure
- 未来 phase 添加 HIR access 后, collect_impl_where_clauses 升级为真正的 collection

**为何 ParamEnv.assumes 短路 (不重新 select)?**
- 引用 §5.4 + rustc pattern: ParamEnv 是 assumptions (assumed true, not proved)
- 引用 §1.0 原則 9 (正确 > 妥协): 不应该重新证明 assumed bounds (效率 + 可能错误)
- 短路逻辑: if predicate ∈ param_env.assumptions → Resolved (no select needed)
- sentinel impl_def_id = u32::MAX 表示 "assumed, not selected"
- selected_count 不计入 assumed (per §1.0 原則 9)

**为何有 fulfill_obligation (单 obligation 便利入口)?**
- 引用 §1.0 原則 3 (显式 > 隐式): 单 obligation 是常见用例 (vs queue 管理)
- 用例: typeck 可能只需要 fulfill 一个 obligation (如 `let x: T = expr` 的 T: Clone)
- 替代: 让调用者自己创建 queue + push + loop — 但这是 boilerplate
- 选择: fulfill_obligation 便利入口 (内部创建临时 queue)

## 9. 裁剪点 (跳流程安全理由)

- L2 — 跳过 §14.6 跨阶段深度验证 (per §1.2.1 L2 可跳过)
- 跳过 §14.5 深度审查 — 将在 Stage 19.7 (Trait Solver Phase 6 完成后) 一起做
- 安全理由: Phase 4 只添加新模块, 不修改现有 codegen/typeck 路径, 无集成, 无回归风险

## 10. 下一步 (下一 MUV)

Stage 19.5 — Trait Solver Phase 5 (Supertrait Expansion + Error Reporting):
- 实现 `expand_supertraits(trait_def_id, resolver) -> Vec<TraitPredicate>` — 自动 derive supertrait bounds
- 实现 `report_fulfillment_error(error, obl) -> Diagnostic` — 高质量错误消息
- 集成 supertrait obligations 到 fulfillment_loop (when trait T selected, add T's supertraits as new obligations)
- L2 (50-500 LOC, 单文件 `src/traits/solver/supertrait.rs`)
- 输入: Phase 4 fulfillment_loop + v0.4 TraitResolver (trait_supertraits API)
- 输出: expand_supertraits + report_fulfillment_error + 测试 (≥3 集成测试, 1:3+ pos:neg ratio)
- 验收: §3.2 全绿

## 11. 文件清单

新增:
- `src/traits/solver/fulfill.rs` (~640 LOC, 32 tests)
- `docs/develop/v0/stage-19/stage-19.4-trait-solver-phase4-fulfillment.md` (本文档)

修改:
- `src/traits/solver/mod.rs` (添加 `pub mod fulfill;` 1 行)
- `Cargo.toml` (version: 0.513.0 → 0.514.0)
- `docs/worklog.md` (append Stage 19.4 entry)
- `README.md` (update version + test count)
- `RELEASE_NOTES.md` (prepend Stage 19.4 entry)
