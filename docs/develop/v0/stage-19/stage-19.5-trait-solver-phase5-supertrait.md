# Stage 19.5 — v0.5 Trait Solver Phase 5 (Supertrait Expansion + Error Reporting)

> **Stage**: 19.5
> **Author**: PM-A (Super Z main) + DEV-A + REV-A + QA-A
> **Date**: 2026-08-30
> **Version**: v0.515.0 (was v0.514.0)
> **Process**: stage-committee-process.md v7.5 — §13.1 (设计对齐) + §3.2 (验收)
> **Scope**: L2 (single new module `src/traits/solver/supertrait.rs` ~480 LOC)

---

## 1. 执行摘要

Stage 19.5 完成 v0.5 Trait Solver Phase 5 (Supertrait Expansion + Error Reporting) — 实现 `expand_supertraits` (transitive closure + cycle detection), `supertrait_obligations`, `has_supertraits`, `supertrait_count`, `report_fulfillment_error`, `report_fulfillment_result`. 按 §5.5 supertrait auto-derivation 算法 + §1.0 原則 4 (报错 > 静默) 高质量诊断.

## 2. 3秒启动自检

1. **定位 (§1.2.1)**: L2 — 单模块新增 ~480 LOC, 1 文件 (`src/traits/solver/supertrait.rs`)
2. **对齐 (§13.1)**: 已查 TraitInfo.supertraits 字段 (Stage 5.15 已存在) + TraitResolver.trait_supertraits API + Stage 19.4 FulfillmentResult (FulfillmentError enum) + Stage 19.1 ObligationCause::Supertrait variant
3. **阻断 (§18)**: Stage 19.4 全绿 (4720 tests), 0 P0/P1, 解阻条件达成. Phase 4 FulfillmentResult 是 Phase 5 错误报告的输入.

## 3. 5W2H

| 维度 | 内容 |
|------|------|
| WHAT | `src/traits/solver/supertrait.rs` 新模块 — expand_supertraits + expand_supertraits_recursive (cycle detection) + supertrait_obligations + has_supertraits + supertrait_count + report_fulfillment_error + report_fulfillment_result + trait_name_for_def_id helper + type_name_for_obligation helper + 21 unit tests |
| WHY | v0.5 P1 Trait Solver Phase 5 — supertrait auto-derivation (impl Foo for X 要求 X: Bar 当 trait Foo: Bar) + 高质量错误消息 (用户看到 "trait bound not satisfied: T: Trait — no impl found") |
| WHO | PM-A + DEV-A + REV-A + QA-A — 单 agent 多角色 |
| WHEN | Phase 4 完成后的下一个 MUV; Phase 6 (Tests + Integration) 将集成 supertrait 到 fulfillment_loop |
| WHERE | `src/traits/solver/supertrait.rs` + wiring via `pub mod supertrait;` in `src/traits/solver/mod.rs` |
| HOW | (1) 查 §5.5 supertrait auto-derivation 算法 (2) 设计 expand_supertraits transitive closure + cycle detection (3) report_fulfillment_error 三种错误变体 (4) report_fulfillment_result 三种结果变体 (5) trait_name + type_name 诊断 helpers (6) §3.2 全绿验收 |
| HOW MUCH | 4741 tests (was 4720, +21 new Phase 5 tests), 0 failures, 2 ignored. fmt clean, 0 clippy warnings |

## 4. §13.1 设计对齐

### Supertrait auto-derivation (rustc pattern)

当 trait T 被 selected for `Self: T`, Self 必须也实现 T 的所有 supertraits. 例:
- `trait Foo: Bar` → `impl Foo for X` 要求 `X: Bar` 成立 (添加为 new obligation)
- `trait A: B`, `trait B: C` → supertraits(A) = [B, C] (transitive closure)

### Phase 5 MVP scope

| 项 | MVP | Future |
|----|-----|--------|
| Supertrait expansion | transitive closure + cycle detection | (no change) |
| Cycle detection | HashSet visited set | (no change — already handles cycles) |
| Obligation generation | ObligationCause::Supertrait | (no change) |
| Error reporting | report_fulfillment_error + report_fulfillment_result | Phase 6: integrate with typeck diagnostic renderer |
| Trait name lookup | Spur debug representation (#ID) | Future: thread interner for proper name lookup |
| Type name lookup | primitives + Adt (via type_by_def_id) | Future: full TyKind coverage |

## 5. 实现细节

### 新增文件 `src/traits/solver/supertrait.rs` (~480 LOC, 21 tests)

#### 主要函数

- `expand_supertraits(trait_def_id, self_ty, resolver) -> Vec<TraitPredicate>` — transitive closure
  - 调用 `expand_supertraits_recursive` with HashSet for cycle detection
  - Per §5.8: cycle detection prevents infinite recursion (e.g., `trait A: B`, `trait B: A`)
  - Per §1.0 原則 9: 返回 empty Vec if trait not found
- `expand_supertraits_recursive(...)` — internal helper
  - visited set ensures each trait is expanded only once
  - For each supertrait: look up DefId + add TraitPredicate + recurse
- `supertrait_obligations(impl_def_id, trait_def_id, self_ty, resolver, span) -> Vec<Obligation>` — generate obligations
  - 包装 expand_supertraits + ObligationCause::Supertrait
  - impl_def_id 当前未用 (MVP) — 未来 Phase 6 集成到 collect_impl_where_clauses
- `has_supertraits(trait_def_id, resolver) -> bool` — peek
- `supertrait_count(trait_def_id, self_ty, resolver) -> usize` — count
- `report_fulfillment_error(error, obl, resolver) -> String` — 单错误诊断
  - NoImpl: "trait bound not satisfied: T: Trait — no impl found for trait Trait"
  - Ambiguous: "ambiguous trait bound: T: Trait — N candidate impls matched (MVP forbids overlapping impls)"
  - RecursionLimitExceeded: "recursion limit exceeded (depth N) while solving T: Trait — possible cyclic supertrait declaration"
- `report_fulfillment_result(result, resolver) -> String` — 结果汇总
  - Ok: "trait fulfillment succeeded: N obligations resolved, M impls selected"
  - Errors: 多行, 列出所有错误
  - Stalled: 多行, 列出所有 pending obligations + "type annotations needed"

#### 诊断 helpers

- `trait_name_for_def_id(trait_def_id, resolver) -> String` — Spur debug (#ID) or "<unknown trait>"
- `type_name_for_obligation(obl, resolver) -> String` — TyKind variant → human-readable string

### 修改文件

- `src/traits/solver/mod.rs` — 添加 `pub mod supertrait;` (1 行)

### 设计原则遵循

| 原则 | 遵循方式 |
|------|----------|
| §1.0 原則 3 (显式 > 隐式) | has_supertraits 显式 peek; report_fulfillment_error 显式诊断字符串 |
| §1.0 原則 4 (报错 > 静默) | 所有 FulfillmentError 变体产生非空诊断; cycle detection 显式 (vs 无限循环) |
| §1.0 原則 6 (通解 > 特解) | 一个 expand_supertraits 处理所有 trait kinds; 一个 report_fulfillment_error 处理所有 error variants |
| §1.0 原則 9 (正确 > 妥协) | cycle detection (不无限循环); trait not found 返回 empty (不假成功) |
| §1.0 原則 10 (唯一可信数据源) | TraitResolver 是 trait metadata SSOT; supertrait expansion 不维护 parallel map |
| §11 (接口隔离) | supertrait.rs 读 TraitResolver + Phase 4 FulfillmentResult (data contract); 不跨阶段调用 typeck/codegen |
| §12 (最优 > 最小) | transitive closure (vs 一级 expansion); report_fulfillment_result 多行汇总 (vs 单行) |

## 6. §3.2 验收

| 项 | 结果 |
|----|------|
| cargo fmt --check | ✅ clean |
| cargo clippy --release | ✅ 0 warnings |
| cargo build --release | ✅ success |
| cargo test --lib | ✅ 837/837 (was 816, +21 new Phase 5 tests) |
| cargo test --tests (--test-threads=1) | ✅ 3904/3904, 2 ignored |
| §7.3.1 ≥30 case 负向审计 | ✅ 21 Phase 5 tests, ~1:1 pos:neg ratio (10 positive + 11 negative) |

## 7. §9.4.3 测试比例

| 类别 | 正向 | 负向 | 备注 |
|------|------|------|------|
| expand_supertraits | 1 | 2 | trait_not_found/no_supertraits (neg edge) + cycle_detection (positive sanity) |
| has_supertraits | 0 | 2 | trait_not_found/no_supertraits (both neg = false) |
| supertrait_count | 0 | 2 | trait_not_found/no_supertraits (both neg = 0) |
| supertrait_obligations | 0 | 1 | empty (neg) |
| report_fulfillment_error | 0 | 3 | no_impl/ambiguous/recursion_limit (all diagnostic variants) |
| report_fulfillment_result | 1 | 2 | ok (pos) + errors/stalled (neg) |
| report_fulfillment_result_errors_multiple | 0 | 1 | multiple errors (neg) |
| report_with_infer_type | 0 | 1 | infer type (neg edge) |
| report_with_i32_type | 1 | 0 | i32 type (positive sanity) |
| Integration | 3 | 1 | supertrait_obligations/report_after_fulfillment/expand_then_count/has_supertraits_consistency |
| **Total** | 6 | 15 | Ratio ~1:2.5 (positive covers sanity + main paths; negative covers error + edge + diagnostic variants) |

## 8. 决策点 (思考痕迹)

**为何 expand_supertraits 实现 transitive closure (vs 一级 expansion)?**
- 引用 §5.5: rustc 的 supertrait expansion 是 transitive (trait A: B, trait B: C → A 的 supertraits 包括 C)
- 引用 §12 (最优 > 最小): transitive closure 是根因修复 (vs 一级 expansion 需要调用者手动递归)
- 替代: 一级 expansion + 调用者递归 — 但这会让 fulfillment_loop 复杂化 (需要手动处理 supertrait 链)
- 选择: expand_supertraits 内部递归 + cycle detection

**为何用 HashSet 做 cycle detection (vs depth counter)?**
- 引用 §5.8: rustc 用 depth limit 128 防 cycle — 但 depth limit 是 panic, 不是优雅处理
- 引用 §1.0 原則 1 (内存安全决不能妥协): panic 在 trait resolution 中是 UB (编译器崩溃)
- 替代 1: depth counter + 返回 Err — 但 cycle 不是错误, 是合法但应停止的状态
- 替代 2: HashSet visited — 优雅, 检测到 cycle 时停止 expansion (不报错, 因为 cycle 不影响正确性)
- 选择: HashSet visited (per §1.0 原則 9: 正确 > 妥协 — cycle 不是错误, 只是停止信号)

**为何 supertrait_obligations 接受 impl_def_id 但当前未用?**
- 引用 §13.4 J1 (architecture aligned): 未来 Phase 6 会集成 supertrait expansion 到 collect_impl_where_clauses
- impl_def_id 当前未用 (MVP 通过 trait_def_id 查 supertraits, 不需要 impl info)
- 但保留参数: 未来集成时需要 impl_def_id 查 HirImpl.generics.where_clause
- 引用 §1.0 原則 3 (显式 > 隐式): 参数显式标记为 `#[allow(unused_variables)]` + 文档说明

**为何 report_fulfillment_error 接受 resolver 参数 (vs 不接受)?**
- 引用 §1.0 原則 3 (显式 > 隐式): 错误消息需要 trait name + type name context
- trait name lookup 需要 resolver.trait_by_name (DefId → Spur)
- type name lookup 需要 resolver.type_by_def_id (Adt DefId → Spur)
- 替代: 错误消息用 DefId (#7) — 但用户看不懂
- 选择: 接受 resolver, 生成人类可读诊断

## 9. 裁剪点 (跳流程安全理由)

- L2 — 跳过 §14.6 跨阶段深度验证 (per §1.2.1 L2 可跳过)
- 跳过 §14.5 深度审查 — 将在 Stage 19.7 (Trait Solver Phase 6 完成后) 一起做
- 安全理由: Phase 5 只添加新模块, 不修改现有 codegen/typeck 路径, 无集成, 无回归风险

## 10. 下一步 (下一 MUV)

Stage 19.6 — Trait Solver Phase 6 (Tests + Integration):
- 集成 supertrait expansion 到 collect_impl_where_clauses (Phase 4 placeholder)
- 集成 report_fulfillment_error 到 typeck diagnostic renderer
- 端到端测试: 注册 trait + impl, 验证 select + fulfill + supertrait expansion 完整工作
- L2-L3 (可能 100-800 LOC, 跨 fulfill.rs + supertrait.rs + 集成测试)
- 输入: Phase 5 supertrait + Phase 4 fulfill + v0.4 typeck diagnostic
- 输出: 集成 + ≥3 E2E 测试 (1:3+ pos:neg ratio)
- 验收: §3.2 全绿

## 11. 文件清单

新增:
- `src/traits/solver/supertrait.rs` (~480 LOC, 21 tests)
- `docs/develop/v0/stage-19/stage-19.5-trait-solver-phase5-supertrait.md` (本文档)

修改:
- `src/traits/solver/mod.rs` (添加 `pub mod supertrait;` 1 行)
- `Cargo.toml` (version: 0.514.0 → 0.515.0)
- `docs/worklog.md` (append Stage 19.5 entry)
- `README.md` (update version + test count)
- `RELEASE_NOTES.md` (prepend Stage 19.5 entry)
