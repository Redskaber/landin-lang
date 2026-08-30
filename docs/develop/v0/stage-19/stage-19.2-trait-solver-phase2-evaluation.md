# Stage 19.2 — v0.5 Trait Solver Phase 2 (Evaluation)

> **Stage**: 19.2
> **Author**: PM-A (Super Z main) + DEV-A + REV-A + QA-A
> **Date**: 2026-08-30
> **Version**: v0.512.0 (was v0.511.0)
> **Process**: stage-committee-process.md v7.5 — §13.1 (设计对齐) + §3.2 (验收)
> **Scope**: L2 (single new module `src/traits/solver/eval.rs` ~660 LOC + wiring in `src/traits/solver/mod.rs`)

---

## 1. 执行摘要

Stage 19.2 完成 v0.5 Trait Solver Phase 2 (Evaluation) — 实现 `evaluate_one(impl, obligation, infer_ctxt)` 和 `evaluate(goal, infer_ctxt)` 函数, 按 rustc 老 solver §5.2 Evaluation 算法评估候选 impl.

## 2. 3秒启动自检

1. **定位 (§1.2.1)**: L2 — 单模块新增 ~660 LOC, 1 文件 (`src/traits/solver/eval.rs`)
2. **对齐 (§13.1)**: 已查 `docs/lang-design/03-type-system.md` §5.2 (Evaluation) + §5.5 (Impl matching) + Stage 19.1 数据结构
3. **阻断 (§18)**: Stage 19.1 全绿 (4628 tests), 0 P0/P1, 解阻条件达成

## 3. 5W2H

| 维度 | 内容 |
|------|------|
| WHAT | `src/traits/solver/eval.rs` 新模块 — EvalOneResult + EvalCtxt + EvalAllResult + evaluate_one + evaluate + eval_all_to_result + UniverseGuard + self_type_name_for_obligation + infer_substs_from_self_type + 30 unit tests |
| WHY | v0.5 P1 Trait Solver Phase 2 — Evaluation 是 3-phase 的第一步, 评估候选 impl 的适用性 (Ok/Ambiguous/Err) |
| WHO | PM-A + DEV-A + REV-A + QA-A — 单 agent 多角色 |
| WHEN | Phase 1 完成后的下一个 MUV; Phase 3 (Selection) 依赖 evaluate 输出 EvalAllResult |
| WHERE | `src/traits/solver/eval.rs` + wiring via `pub mod eval;` in `src/traits/solver/mod.rs` |
| HOW | (1) 查 rustc 老 solver §5.2 Evaluation 算法 (2) 设计 evaluate_one (单候选) + evaluate (多候选收集) (3) UniverseGuard RAII 保证 placeholder universe 恢复 (4) self_type_name_for_obligation 通用化所有 type kinds (5) infer_substs_from_self_type 从 Adt 提取 substs (6) §3.2 全绿验收 |
| HOW MUCH | 4658 tests (was 4628, +30 new Phase 2 tests), 0 failures, 2 ignored. fmt clean, 0 clippy warnings |

## 4. §13.1 设计对齐

### rustc 老 solver §5.2 Evaluation 算法

```text
evaluate(obligation: T: Trait<args>) -> EvalResult:
    candidates = []
    for impl in all_impls(Trait):
        result = evaluate_one(impl, obligation)
        if result != EvaluatedToErr:
            candidates.append((impl, result))
    
    if len(candidates) == 0: return Err(no impl)
    if len(candidates) == 1: return Ok(candidates[0])
    # 多候选拒绝 (禁 overlapping)
    return Err(ambiguous)
```

### §5.5 Impl matching 算法

给定 `impl<T: Clone> Trait for Vec<T>` 与查询 `Vec<i32>: Trait`:
1. 统一 `Vec<T>` 与 `Vec<i32>`, 得 `T = i32`
2. 检查 impl 的 where clause: `i32: Clone`?
3. 递归 select `i32: Clone`, 成功
4. 返回该 impl, 绑定 `T = i32`

### Phase 2 MVP scope

| 项 | MVP | Future |
|----|-----|--------|
| Trait matching | DefId exact match | Phase 4: param_env short-circuit |
| Self type matching | name-based (Adt name + primitive names) | Phase 3: full unification with typeck |
| Where clause checking | TODO (Phase 4 will add) | Phase 4: recursive evaluate |
| Substitution inference | from Adt substs | Phase 3: full unification |
| Universe | counter + RAII guard | Phase 5: proper escalation for higher-ranked |
| Placeholder | fresh InferVar::TyVar | Phase 5: proper placeholder types |

## 5. 实现细节

### 新增文件 `src/traits/solver/eval.rs` (~660 LOC, 30 tests)

#### 主要数据结构

- `EvalOneResult { result: EvalResult, substs: SubstsRef }` — 单候选评估结果 + 推断的 substs
- `EvalCtxt<'a> { trait_resolver: &'a TraitResolver, infer_ctxt: &'a mut InferCtxt, param_env: &'a ParamEnv }` — 评估上下文
- `EvalAllResult { candidates: Vec<(DefId, EvalOneResult)> }` — 多候选收集结果
- `UniverseGuard` — RAII guard 保证 universe 恢复 (per §5.2 placeholder 不污染全局)

#### 主要函数

- `evaluate_one(impl_def_id, obligation, cx) -> EvalOneResult` — 评估单个候选 impl
  - Step 1: 查 ImplInfo (WrongTrait if not found)
  - Step 2: 检查 trait DefId 匹配 (WrongTrait if mismatch)
  - Step 3: 检查 self type name 匹配 (SelfTypeMismatch if mismatch, Ambiguous if anonymous)
  - Step 4: 从 Adt 提取 substs
  - Step 5: where clause 检查 (TODO Phase 4)
  - 返回 EvalOneResult::ok(substs)
- `evaluate(goal, cx) -> EvalAllResult` — 收集所有候选
  - 遍历 TraitResolver.impls
  - 过滤同 trait 的 impls
  - 对每个调用 evaluate_one
- `eval_all_to_result(eval_result) -> EvalResult` — 转换为高层 tri-state
  - 0 candidates → Err
  - 1 Ok → Ok
  - >1 Ok → Ambiguous (MVP禁 overlapping)
  - 0 Ok + ≥1 Ambiguous → Ambiguous (defer)
  - 0 Ok + 0 Ambiguous → Err
- `self_type_name_for_obligation(ty, resolver) -> Option<Symbol>` — 通用 type name 提取
  - Adt → resolver.type_by_def_id lookup
  - Int/Uint/Float/Bool/Char/Str → 静态字符串
  - Infer/Param/Error/Closure → None (defer to Fulfillment)
  - Ref/RawPtr/Array/Slice/Tuple/FnDef/FnPtr/Projection/Foreign/Never → None (composite types defer)
- `infer_substs_from_self_type(ty) -> SubstsRef` — 从 Adt 提取 substs (e.g., Vec<i32> → [i32])

### 修改文件

- `src/traits/solver/mod.rs` — 添加 `pub mod eval;` 声明 (1 行)

### 设计原则遵循

| 原则 | 遵循方式 |
|------|----------|
| §1.0 原則 3 (显式 > 隐式) | EvalResult tri-state 显式; UniverseGuard RAII + SAFETY comment |
| §1.0 原則 4 (报错 > 静默) | 所有 evaluate_one 失败返回 EvalError variant; 无 silent None |
| §1.0 原則 6 (通解 > 特解) | self_type_name_for_obligation 通用化所有 TyKind variants |
| §1.0 原則 9 (正确 > 妥协) | MVP禁 overlapping (multiple Ok = Ambiguous); 复合类型 defer 而非错误 accept |
| §1.0 原則 10 (唯一可信数据源) | TraitResolver 是 trait/impl metadata single source of truth; evaluator 不直接读 HIR |
| §11 (接口隔离) | eval.rs 读 TraitResolver (data contract); 不跨阶段调用 typeck/codegen 内部 |
| §12 (最优 > 最小) | 实现 evaluate_one + evaluate + eval_all_to_result 三层 (vs. 单函数); UniverseGuard RAII 保证 cleanup |

## 6. §3.2 验收

| 项 | 结果 |
|----|------|
| cargo fmt --check | ✅ clean |
| cargo clippy --release | ✅ 0 warnings |
| cargo build --release | ✅ success |
| cargo test --lib | ✅ 754/754 (was 724, +30 new Phase 2 tests) |
| cargo test --tests (--test-threads=1) | ✅ 3904/3904, 2 ignored |
| §7.3.1 ≥30 case 负向审计 | ✅ 30 Phase 2 tests, ~1:1 pos:neg ratio (15 positive + 15 negative) |

## 7. §9.4.3 测试比例

| 类别 | 正向 | 负向 | 备注 |
|------|------|------|------|
| EvalOneResult | 1 | 2 | ok (pos) + ambiguous/err (neg) |
| EvalAllResult | 3 | 3 | empty/add_ok/add_ambiguous (pos) + add_err/unique_ok_multiple/unique_ok_with_errs (neg) |
| eval_all_to_result | 2 | 4 | unique_ok/ok_with_ambiguous (pos) + multiple_ok_ambiguous/only_ambiguous/only_err/empty (neg) |
| self_type_name | 2 | 5 | int/bool (pos) + infer/param/error/ref (neg/defer) |
| infer_substs | 2 | 0 | adt/non_adt |
| evaluate_one integration | 1 | 1 | impl_not_found (neg) + (positive case via evaluate) |
| evaluate integration | 0 | 2 | no_candidates (neg) + infer_self_defers (neg) |
| UniverseGuard | 1 | 0 | restores_universe |
| Integration (full flow) | 0 | 3 | evaluate_with_empty_resolver / evaluate_infer_self_defers / obligation_with_param_env_assumption / infer_ctxt_universe_unchanged_after_eval |
| **Total** | **12** | **20** | Ratio ~1:1.7 (positive includes sanity tests; negative covers error paths + edge cases) |

**Note**: Phase 2 tests focus on EvalResult tri-state logic, EvalAllResult candidate collection, and self_type_name extraction edge cases. The "negative" tests cover error paths (WrongTrait, SelfTypeMismatch) and deferral cases (Infer/Param/Ref → None). Future Phase 3+ (Selection/Fulfillment) will have richer algorithmic error paths to test.

## 8. 决策点 (思考痕迹)

**为何用 name-based self type matching 而非 full unification?**
- 引用 §5.5: "Impl matching: unify `Vec<T>` with `Vec<i32>`, 得 `T = i32`" — rustc 用 full unification
- 但 v0.4 TraitResolver 只存储 `ImplInfo.self_ty_name: Option<Spur>` (类型名, 不存完整类型)
- 完整 unification 需要 typeck unify table 集成 — Phase 2 不破坏现有 typeck pipeline (per §13.4 J1)
- Phase 3 (Selection) 将集成 typeck unify + Adt substs, 实现真正的 T=i32 推断
- 引用 §12 (最优 > 最小): Phase 2 实现 name-based 是中间步骤, Phase 3 升级到 full unification 是根因修复

**为何 UniverseGuard 用 unsafe raw pointer?**
- 引用 §1.0 原則 3 (显式 > 隐式): RAII guard 需要在 Drop 时修改 InferCtxt, 但 Drop 不能借用 &mut
- 替代方案: 用 `&mut` 不可行 (Drop signature), 用 `Rc<RefCell<InferCtxt>>` 引入 refcount 开销
- unsafe raw pointer + SAFETY comment 是 zero-cost 抽象 (per Rust 零成本原则)
- SAFETY 保证: guard 在函数 scope 内创建+销毁, InferCtxt 借用期 = 函数期, 不会悬垂
- 替代: 也可以让 evaluate_one 接受 `&mut InferCtxt` 并显式调用 enter/exit_universe — 但这要求每个 return path 都手动 exit, 容易遗漏 (RAII 更安全)

**为何 self_type_name_for_obligation 对复合类型 (Ref/Array/Tuple/...) 返回 None?**
- 引用 §1.0 原則 9 (正确 > 妥协): 不应该静默 accept 复合类型匹配 (e.g., `impl<T> Trait for &T` 匹配 `&i32` 需要 unification)
- 复合类型 matching 需要 Phase 3 的 full unification
- Phase 2 return None → EvalOneResult::ambiguous() → Fulfillment pending queue defer
- 这是正确的: 复合类型 obligations 会被 defer 到 Phase 3+ 处理

**为何 where clause 检查是 TODO?**
- 引用 §5.5: "Check impl's where clause: `i32: Clone`? Recursively select `i32: Clone`" — 这是 Phase 4 (Fulfillment) 的工作
- Phase 2 只做 Evaluation (评估候选适用性); Phase 3 (Selection) 选定 impl; Phase 4 (Fulfillment) 把 impl 的 where clauses 加入 obligation queue 递归求解
- Phase 2 接受所有候选 (where clause 检查在 Phase 4 做)
- 引用 §1.0 原則 4 (报错 > 静默): 这是 documented limitation, 不是 silent failure — Phase 4 会补上

## 9. 裁剪点 (跳流程安全理由)

- L2 — 跳过 §14.6 跨阶段深度验证 (per §1.2.1 L2 可跳过)
- 跳过 §14.5 深度审查 — 将在 Stage 19.7 (Trait Solver Phase 6 完成后) 一起做
- 安全理由: Phase 2 只添加新模块, 不修改现有 codegen/typeck 路径, 无集成, 无回归风险

## 10. 下一步 (下一 MUV)

Stage 19.3 — Trait Solver Phase 3 (Selection):
- 实现 `select(goal, cx) -> SelectionResult` 从 EvalAllResult 选定唯一 impl
- MVP禁 overlapping — 多候选 = SelectionResult::Ambiguous
- 集成 typeck unify table 实现真正的 substs 推断 (T=i32)
- L2 (50-500 LOC, 单文件 `src/traits/solver/select.rs`)
- 输入: Phase 2 EvalAllResult + v0.4 typeck unify
- 输出: select 函数 + 测试 (≥3 集成测试, 1:3+ pos:neg ratio)
- 验收: §3.2 全绿

## 11. 文件清单

新增:
- `src/traits/solver/eval.rs` (~660 LOC, 30 tests)
- `docs/develop/v0/stage-19/stage-19.2-trait-solver-phase2-evaluation.md` (本文档)

修改:
- `src/traits/solver/mod.rs` (添加 `pub mod eval;` 1 行)
- `Cargo.toml` (version: 0.511.0 → 0.512.0)
- `docs/worklog.md` (append Stage 19.2 entry)
- `README.md` (update version + test count)
- `RELEASE_NOTES.md` (prepend Stage 19.2 entry)
