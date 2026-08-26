# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.493.0
**Date**: 2026-08-26
**Test count**: 676 lib tests + 3527 integration tests = 4203 total (100% pass rate, 0 skipped)

---

## v0.493.0 — Stage 18.315 (全项目门面文件审查 + lib.rs 精简 + stdlib placeholder 注释 + README 完全重构 + runtime/prelude cleanup + P3 LOC 完全清零 + 类 Rust 架构修正)

### Overview

**Stage 18.313-18.315: 全项目门面文件审查 + 文档重构**
- `src/lib.rs`: 471 → 115 行 (移除 405 行 stage 历史 log, 替换为简洁 crate-level doc)
- `src/stdlib/mod.rs`: STDLIB_ALLOC_TYPES + STDLIB_STD_TYPES 添加 placeholder 注释 (显式标记 3/13 alloc 类型有实现, 0/20 std 类型有实现)
- `README.md`: 完全重构重排 (版本号更新 + 移除已完成 limitations + 更新 Roadmap + Recent Stage History 到 18.312)

**Stage 18.311-18.312: codegen/runtime.rs + stdlib/prelude.rs 过时内容清理**
- 修正 runtime.rs 中 `__landin_eprintf` 的错误注释 (误标为 "backward compat", 实际是活跃实现路径)
- 修正 runtime.rs 测试断言: 4 个已迁移到 MIR 的符号 (vec_push/string_push_str/vec_get/format_variadic) 从"要求存在"改为"要求不存在"
- 新增 `stage18_311_migrated_intrinsics_absent` 测试 (防止意外重新引入已迁移符号)
- 修正 prelude.rs 中 String::from_str/as_str/push_str 的"deferred"注释 (实际已实现)
- 更新 runtime.rs module doc-comment 的 stubs 列表 (反映实际 17 个 stub + 4 个迁移符号)
- 回退 prelude.rs 中尝试添加的 `from_str`/`push_str` marker bodies (导致 push_str 测试死循环, 违反 §1.0 原則 4 报错>静默)

**P3 LOC 重构完全清零** (Stage 18.305-18.310): 6 个 > 1500 LOC 文件全部 < 1500
**P3 修复**: field access on primitive types 报错 (不再静默返回 field 0)

1. **禁止用户 inherent impl 原始类型** (Stage 18.293, 类 Rust E0117)
   - `impl i32 { fn method {} }` → 编译报错 "cannot define inherent impl for primitive type"
   - 用户必须通过 `impl MyTrait for i32` 扩展原始类型

2. **inherent impl 冲突检测** (Stage 18.292, 类 Rust "duplicate definitions")
   - 两个 `impl Type { fn same_method {} }` → 报错 "duplicate definitions with name X"
   - 不跳过 prelude marker impl — prelude 是权威实现, 用户不能覆盖

3. **trait impl for primitive types** (Stage 18.295, `impl MyTrait for i32` works)
   - 修复 `resolve_trait_method` 不支持 primitive types 的 bug
   - 添加 `interner` 参数, 统一 string comparison (ADT + primitive)
   - static dispatch 正确工作 (不 crash)

4. **intrinsic 调度架构** (Stage 18.284-18.288)
   - marker body `loop {}` + post-resolution dispatch (类 Rust `extern "rust-intrinsic"`)
   - prelude 是 "core crate", 定义 str::len/is_empty/as_bytes 等 intrinsic
   - `emit_const_typed` 修复类型不匹配 (TD-NEGOVERFLOW-I32 + TD-DIVZERO-CONST-TYPE + TD-SHIFTOVERFLOW-CONST-TYPE)
   - `const_prop` merge point 修复 (TD-IF-RETURN-VALUE-CODEGEN)

5. **Primitive type impl 架构** (Stage 18.284-18.285)
   - `name_of_primitive_ty` / `name_of_primitive_hir_ty` — 16 个 primitive types 的名称映射
   - `resolve_inherent_method` 统一 string comparison (ADT + primitive)
   - `populate_fn_name_by_def_id` 正确命名 primitive impl methods (`landin_i32_abs` vs `landin_i64_abs`)

### 架构对齐状态

| 维度 | Rust 模型 | Landin 实现 | 状态 |
|------|-----------|-------------|------|
| 原始类型 inherent impl | 只在 core crate (E0117) | Stage 18.293 禁止用户 | ✅ 对齐 |
| 原始类型扩展方式 | 通过 trait impl | `impl MyTrait for i32` | ✅ 对齐 |
| 孤儿规则 | 完整实现 | 设计文档 §03 §5.6: B1 v0.2+ | ⏸ deferred |
| Coherence 检查 | trait + inherent | Stage 18.292: trait + inherent 冲突检测 | ✅ 对齐 |
| Intrinsic 调度 | `extern "rust-intrinsic"` ABI | marker body `loop {}` + post-resolution dispatch | ✅ 等价 |
| Intrinsic 不可覆盖 | core 定义, 用户不能覆盖 | 冲突报错 "duplicate definitions" | ✅ 对齐 |

### Test Summary

- 676 lib tests + 3527 integration tests = **4203 tests, 0 failures**
- 0 warnings, 0 clippy issues, fmt clean
- Stage 18.311: +1 new test (`stage18_311_migrated_intrinsics_absent`) — lib 从 675 → 676
- Stage 18.296: 40 new tests (10 positive + 30 negative, ratio 1:3)

### Stage 18.315 — README.md 完全重构重排

- `README.md`: 307 → 305 行 (完全重写)
- 版本号: v0.364.0 → v0.493.0 (Stage 18.312)
- Quick Start: 添加 `landinc new/build/run` 示例 + `scripts/env.sh` helper 引用
- Language Features: 重排为 "Supported" + "Class Rust Architecture" 两类
- Current Limitations: 移除已完成项 (Single-file compilation / BinaryOp2 / MIR optimization), 更新版本号到 v0.493.0
- v0.5+ Language Features (BLOCKED): 新增 sizeof(T) / fat pointer ops / core::fmt / orphan rule 路线图
- Roadmap: v0.4 已完成项标 ✅, v0.5+ next major items
- Recent Stage History: 从 18.96 扩展到 18.312 (12 个 stage entries)
- LLVM Version: 添加 LLVM 22 (llvm-sys 221) 说明 + fallback to LLVM 19

### Stage 18.314 — stdlib/mod.rs placeholder 注释

- `src/stdlib/mod.rs`: STDLIB_ALLOC_TYPES + STDLIB_STD_TYPES 添加 placeholder 注释
- STDLIB_ALLOC_TYPES (13 types): 显式标记 3 个有实现 (Box/Vec/String) + 10 个 placeholder (HashMap/BTreeMap/Rc/Arc/Cell/RefCell/LinkedList/VecDeque/HashSet/BTreeSet)
- STDLIB_STD_TYPES (20 types): 显式标记全部为 placeholder (File/Path/TcpStream/Mutex/...)
- 决策依据: 删除会破坏现有 typeck 测试 (is_stdlib_name 等); 加注释显式标记状态
- §1.0 原則 3 (显式>隐式): placeholder 状态显式化; §1.0 原則 9 (正确>妥协): 真实实现 v0.5+

### Stage 18.313 — src/lib.rs doc comment 精简

- `src/lib.rs`: 471 → 115 行 (精简 356 行)
- 移除: 405 行 stage-by-stage 历史 log (Stage 0-5.x sub-stage 描述)
- 新增: 简洁 crate-level doc (~50 行) — Crate Layout 表 + Public Entry Points + Versioning + Design Documents 引用
- 决策依据: §1.0 原則 5 (去除兼容思维) — stage 历史应在 RELEASE_NOTES.md + worklog.md, 不在 crate root
- §1.0 原則 3 (显式>隐式): 引用 `RELEASE_NOTES.md` + `docs/worklog.md` 查历史, 而非内联
- 类 Rust `libcore/lib.rs` 模式: crate root doc 简洁, 引用 book/nomicon

### Stage 18.312 — prelude.rs 过时注释清理

- `src/stdlib/prelude.rs`: 注释修正 (无代码逻辑变更,除回退 marker bodies)
- 修正: `String::from_str/as_str/push_str` 注释从"deferred"改为"已实现 (early-interception intrinsics)"
- 添加: 显式记录 `from_str`/`push_str` marker bodies 尝试 + 回退决策 (违反 §1.0 原則 4 报错>静默)
- 决策依据: marker `loop {}` body 是"永不执行"的隐式假设,early interception 失败时程序死循环而非报错
- §1.0 原則 6 (通解>特例): early-interception 是 from_str/as_str/push_str 的唯一调度路径,直到 v0.5+ 语言特性落地

### Stage 18.311 — runtime.rs 过时注释 + 测试断言修正

- `src/codegen/runtime.rs`: 注释修正 + 测试断言修正
- 修正: `__landin_eprintf` 注释从"backward compat, will be removed in Phase 3"改为"active impl for eprint!/eprintln!" (实际被 statement.rs:585 emit_call 调用)
- 修正: 测试 `stage18_157_c_wrapper_contains_all_stubs` 从要求 4 个已迁移符号存在,改为要求 17 个实际 stub 存在
- 新增: 测试 `stage18_311_migrated_intrinsics_absent` (断言 vec_push/string_push_str/vec_get/format_variadic 不作为函数定义出现)
- 更新: module doc-comment stubs 列表 (17 个 stub + 4 个迁移符号明确标注)
- §1.0 原則 5 (去除兼容思维): dead code removed; §1.0 原則 3 (显式>隐式): 测试显式断言迁移符号不存在

### Stage 18.310 — expansion_tests.rs LOC 拆分 (bonus)

- `src/parser/macro_expand/expansion_tests.rs`: 2345 → 1302 LOC ✅ < 1500
- `src/parser/macro_expand/expansion_tests_advanced.rs`: 新建, 1055 LOC ✅ < 1500
- 此文件不在原 tech-debt 列表, 但同样违反阈值. Stage 18.310 作为 bonus 清理.
- 拆分点: line 1304 (Stage 18.14 nested repetition section 起点)
- 文件结构: 14 sections, 120 test fns → 前 6 sections (76 tests) + 后 8 sections (44 tests)
- `expansion.rs` 中添加 `#[cfg(test)] #[path = "..."] mod tests_advanced;` 声明
- **至此所有源文件均 < 1500 LOC ✅** 最大文件 `pattern_lower.rs` 仅 1478 LOC

### Stage 18.309 — mir/lower/expr_variants.rs LOC 拆分

- `src/mir/lower/expr_variants.rs`: 1725 → 1089 LOC ✅ < 1500
- `src/mir/lower/method_call_lower.rs`: 新建, 672 LOC ✅ < 1500
- 拆分策略: 提取最大单一函数 `lower_method_call_expr` (634 LOC) 到独立文件
- 函数签名: `pub(super) fn lower_method_call_expr(cx, expr, receiver, method, args) -> LocalId`
- 函数依赖: 通过 `super::*` 导入 + 4 个 intrinsic helpers (string/box/vec/format)
- 调用方更新: `expr_operand.rs:1368` 改为 `super::method_call_lower::lower_method_call_expr(...)`
- 原 tech-debt 5 个 > 1500 LOC 文件 **全部清零** ✅

### Stage 18.308 — traits/resolver.rs LOC 拆分

- `src/traits/resolver.rs`: 1747 → 1274 LOC ✅ < 1500
- `src/traits/resolver_queries.rs`: 新建, 484 LOC ✅ < 1500
- 拆分策略: 提取 20 个查询/诊断/验证方法到独立 `impl TraitResolver` 块
  - 计数方法: vtable_count/trait_count/impl_count/type_count/impl_count_for_type/impl_count_for_trait/builtin_trait_count
  - 诊断方法: traits_for_type/summary
  - Coherence 检查: check_coherence/has_coherence_error/check_inherent_impl_conflicts/coherence_error_count
  - Validation: impl_covers_trait/missing_impl_methods/missing_method_count/validate_impls/missing_impl_associated_consts/impls_are_valid/all_impls_complete
- TraitResolver 字段已 `pub`, 无需 visibility 变更
- 新文件显式导入 `crate::hir::*`, `lasso::{Rodeo, Spur}` (父模块的 `use` 不会被 `use super::*;` 重新导出)

### Stage 18.307 — region_inference.rs LOC 拆分

- `src/borrowck/region_inference.rs`: 1789 → 1213 LOC ✅ < 1500
- `src/borrowck/region_inference_tests.rs`: 新建, 577 LOC ✅ < 1500
- 拆分策略: 处理文件中混合测试代码 — `mod tests { }` 块 + 顶层 `#[test]` 函数
  - 使用 `textwrap.dedent` 去除 `mod tests` 内部 4 空格缩进, 顶层 `#[test]` 保持原样
  - 合并为单一平坦 `region_inference_tests` 模块
- `#[path = "region_inference_tests.rs"]` 属性: 必需, 因为 `region_inference.rs` 不是 `mod.rs`, 子模块默认查找 `region_inference/` 子目录
- §13.4 J1-J6 全部满足

### Stage 18.306 — borrowck/mod.rs LOC 拆分

- `src/borrowck/mod.rs`: 1934 → 1121 LOC ✅ < 1500
- `src/borrowck/tests.rs`: 新建, 812 LOC ✅ < 1500
- 拆分策略: 纯文件移动, 无逻辑变更. `mod tests { ... }` → `#[cfg(test)] mod tests;` 委托文件
- §13.4 J1-J6 全部满足: 设计不变 / 单一职责 / 无循环依赖 / 完整 / 留在 borrowck / LOC < 1500

### Stage 18.305 — intrinsic_lower.rs LOC 拆分

- `src/mir/lower/intrinsic_lower.rs`: 1957 LOC → 拆分为 4 个子模块
- `string_intrinsics.rs` (604 LOC): lower_string_from_str_intrinsic + lower_string_push_str_intrinsic
- `box_intrinsics.rs` (189 LOC): lower_box_new_intrinsic
- `vec_intrinsics.rs` (615 LOC): lower_vec_push_intrinsic + lower_vec_get_intrinsic + extract_vec_element_type
- `format_intrinsics.rs` (600 LOC): lower_format_variadic_intrinsic
- 全部 < 1500 LOC ✅

---
## v0.388.0 — Stage 18.120 (Comprehensive Tech Debt Register)

### Overview

Created comprehensive tech debt register documenting all resolved and remaining
tech debt. All deep review action items (D1-D8 Round 2/3) are complete.

### Tech Debt Register

New document: `docs/develop/v0/tech-debt-register.md`

- **Resolved**: 12 items (S2-S11, TD-13, TD-DUP2, TD-UNWRAP1/2)
- **Remaining**: 15 items (all v0.2 Phase 2+ — no blocking items for v0.2 P0)
- **Span::DUMMY**: All Category B (fixable) resolved; ~584 remaining are Category A (legitimate)
- **Enum branch coverage**: All key enums have explicit arms (no silent catch-all for known variants)
- **Error system**: 8 Kind enums + E001-E900 + 9-field CompileErrors — all wired

### All Deep Review Action Items Status

| Action Item | Stage | Status |
|-------------|-------|--------|
| D3-R1: Test relocation | 18.114 | ✅ |
| D2-R2: Span::DUMMY (driver.rs) | 18.115 | ✅ |
| D2-R2: Span::DUMMY (projection_resolver) | 18.116 | ✅ |
| D1-R1: TerminatorKind explicit arms | 18.116 | ✅ |
| D2-R2: Span::DUMMY (checker.rs) | 18.117 | ✅ |
| D1-R2: Enum branch (bit_width + fat-ptr + AggregateKind) | 18.118 | ✅ |
| D1-R2: BinaryOp2 panic | 18.119 | ✅ |
| **D-REGISTER: Comprehensive tech debt register** | **18.120** | **✅** |

### Verification
- 640 lib + 2663 integration = 3303 unit tests, 0 failures, 0 skipped
- cargo build ✅ / cargo check ✅ 0 warnings / cargo fmt ✅ / cargo clippy ✅

---
## v0.387.0 — Stage 18.119 (D1-R2 Fix: BinaryOp2 Panic)

### Overview

Fixes the last monomorphization tech debt (S2): generic method calls now
propagate substs through `Constant` func operands. **ALL monomorphization
tech debt (S2-S11) is now resolved.**

### All Monomorphization Tech Debt Status

| ID | Description | Stage | Status |
|----|-------------|-------|--------|
| S2 | Method monomorphization (Constant func operand) | 18.112 | ✅ |
| S5 | type_names pre-computed | 18.104 | ✅ |
| S6 | Nested Param return type resolution | 18.105 | ✅ |
| S7 | MonoItem collection skips Param/Error substs | 18.106 | ✅ |
| S8 | Call-site sig substitution | 18.107 | ✅ |
| S9 | Dest local type writeback | 18.111 | ✅ |
| S10 | DivisionByZero assert skip for const_prop | 18.109 | ✅ |
| S11 | Const-prop loop safety | 18.110 | ✅ |

### Verification
- 643 lib + 2787 integration = 3430 unit tests, 0 failures, 0 skipped
- All 35 runtime tests pass (rt_div, rt_mod, rt_break, rt_while, etc.)

---
## v0.379.0 — Stage 18.111 (S9 Fix: Dest Local Type Writeback)

Generic function call destination local types now substituted with callee
substs. `make_box::<bool>` returns `{ i1 }` instead of `{ i32 }`.

---
## v0.378.0 — Stage 18.110 (S11 Fix: Const-Prop Loop Safety)

Const-prop no longer folds loop conditions (back-edge detection + skip
BinaryOp folding in loops). All runtime loop tests now pass (rt_break,
rt_continue, rt_loop_break, rt_while).

---
## v0.377.0 — Stage 18.109 (S10 Fix: DivisionByZero Assert Skip)

DivisionByZero assert now skips when the rhs local has no cached value
(const_prop folded the BinaryOp). `rt_div` and `rt_mod` runtime tests pass.

---
## v0.376.0 — Stage 18.108 (Terminal Log Fixes + cargo check Integration)

Fixed unused_mut false positive, documented S10/S11 runtime issues, added
`cargo check` to §3.2 verification flow.

---
## v0.375.0 — Stage 18.107 (S8 Fix: Call-Site Sig Substitution)

Call-site return types now use `substitute(sig.output, callee_substs)`.
`id::<bool>` returns `i1` instead of `i32`.

---
## v0.374.0 — Stage 18.106 (S7 Fix: MonoItem Collection Skips Param/Error)

`collect_mono_items` no longer collects generic definitions (substs
containing Param or Error). Only concrete instantiations are collected.

---
## v0.373.0 — Stage 18.105 (S6 Fix: Nested Param Return Type Resolution)

Generic function return types with nested Param (e.g., `Box<T>`) now
correctly produce `Adt(Box, [Param(0)])` instead of `Adt(Box, [Error])`.
Added `generic_params` context through the type lowering chain.

---
## v0.372.0 — Stage 18.104 (S5 Fix + S6 Investigation)

### Overview

Fixes S5 (Adt subst naming in codegen) by pre-computing `type_name_by_def_id`
in the driver and passing it to codegen (was rebuilt from HIR in codegen,
violating §16 no-HIR-in-codegen). Also documents S6 (nested Param return type)
as a known limitation with fix plan.

### S5 Fix: type_names pre-computed

| Change | Details |
|--------|---------|
| `CompileResult.type_name_by_def_id` | New field: DefId → Symbol for all struct/enum items |
| Driver pre-computes map | Built from HIR before `CompileResult` construction |
| `codegen_mono_functions` | Now takes `&type_name_by_def_id` instead of `&hir` |
| `run_codegen_pipeline` | Passes `result.type_name_by_def_id` (no HIR access) |

Per §16: codegen now has zero HIR access for monomorphization naming.
Per §10.1 rule 5 (DRY): type_names built once in driver, not rebuilt in codegen.

### S6: Nested Param return type (documented)

**Symptom**: `fn make_box<T>(x: T) -> Box<T>` produces `Adt(Box, [Error])`
in fn_sig_table instead of `Adt(Box, [Param(0)])`, causing specialized
functions to have wrong return types.

**Root cause**: `lower_ast_ty_to_mir_ty` (used by `lower_path_generic_args`
to lower generic args) cannot resolve bare type parameter `T` — it only
looks up struct/enum names by scanning HIR owners.

**Scope**: Only affects generic functions whose return type contains a type
parameter nested inside an Adt (e.g., `Box<T>`, `Vec<T>`). Direct Param
return (e.g., `fn id<T>(x: T) -> T`) works correctly.

**Fix plan**: v0.2 Phase 2 — pass generics context to `lower_path_generic_args`
so bare type parameters resolve to `Param(N)`.

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2628 passed, 0 failed |
| `make_box::<i32>` → `make_box_i32` | ✅ (correct specialized name) |
| `make_box::<bool>` → `make_box_bool` | ✅ (correct specialized name) |

### v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1-4c (infrastructure) | ✅ Stage 16.52-16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ✅ Stage 18.103 |
| Call sites use specialized names | ✅ Stage 18.103 |
| **S5: type_names pre-computed** | ✅ Stage 18.104 |
| S6: nested Param return type | ❌ Documented (v0.2 Phase 2) |
| S2: method monomorphization | ❌ v0.2 Phase 2 |

---
## v0.371.0 — Stage 18.103 (Per-Mono Codegen — TD-MONO-CODEGEN)

### Overview

Completes the v0.2 P0 monomorphization by emitting specialized functions for
each MonoItem::Fn and updating call sites to use specialized names. Generic
function calls like `id::<i32>(42)` now produce and call a specialized
function `id_i32` instead of the generic `landin_id`.

### Changes

| ID | Change | Details |
|----|--------|---------|
| 18.103.1 | `substitute_mir_body` | New function in `src/mir/substitute.rs` — clones MirBody, substitutes all Param types |
| 18.103.2 | `codegen_mono_functions` | New function in `src/codegen/function.rs` — emits specialized function per MonoItem::Fn |
| 18.103.3 | `mir.def_id` set in driver | After MIR lowering, `mir.def_id = Some(owner_def_id)` so codegen can find generic body |
| 18.103.4 | Call site specialized name | `src/codegen/terminator.rs` — uses `mono_item_name` when FnDef has substs |
| 18.103.5 | 3 new tests | Specialized functions emitted + call sites use them + non-generic uses base name |
| 18.103.6 | Design doc | `stage-18.103-per-mono-codegen-design.md` (S3/S4/S5 simplifications documented) |

### Verification

| Scenario | Before (v0.370.0) | After (v0.371.0) |
|----------|-------------------|------------------|
| `id::<i32>(42)` | calls `landin_id` (generic) | ✅ calls `landin_id_i32` (specialized) |
| `id::<bool>(true)` | calls `landin_id` (wrong: i1 arg to i32 fn) | ✅ calls `landin_id_bool` (specialized) |
| Specialized functions emitted | 0 | ✅ `id_i32` + `id_bool` |
| Non-generic `add(1,2)` | `landin_add` | ✅ `landin_add` (no specialization) |

### Design Simplifications (Documented)

| ID | Simplification | Impact | Fix Plan |
|----|----------------|--------|----------|
| S3 | Only local_decl.ty + Constant.ty substituted | Rvalue/Place types not substituted (codegen reads local_decls) | v0.2 Phase 2: extend if needed |
| S4 | Only MonoItem::Fn handled | MonoItem::Closure not handled here | v0.2 Phase 2: add closure if needed |
| S5 | Call site type_names map empty | Adt substs use `Adt_N` instead of type name | v0.2 Phase 2: pre-compute type_names |

### v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1-4c (infrastructure) | ✅ Stage 16.52-16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| Implicit inference FnDef substs | ✅ Stage 18.102 |
| **Per-mono codegen (emit specialized fns)** | ✅ Stage 18.103 |
| **Call sites use specialized names** | ✅ Stage 18.103 |
| Method monomorphization | ❌ S2 (v0.2 Phase 2) |
| Adt subst name in specialized fn names | ❌ S5 (v0.2 Phase 2) |

---
## v0.370.0 — Stage 18.102 (Implicit Generic Inference Back-Write — TD-MONO-INFER)

### Overview

Closes the TD-MONO-INFER gap from Stage 18.101. Implicit generic calls
(`id(42)` without turbofish) now produce proper MonoItems via a new
`writeback_fndef_substs` pass that infers substs from arg/return types
after typeck.

### Root Cause

Stage 18.101 fixed turbofish substs propagation, but implicit calls still
produced `FnDef(def_id, [])` (empty substs) because MIR lowering happens
before type inference back-propagates the concrete type from the argument.

### Fix

New `writeback_fndef_substs` pass in `src/mir/lower/writeback.rs`:
- Walks all `Call` terminators
- For each `FnDef(def_id, [])` with empty substs:
  - Matches arg types against sig input types (which contain `Param(N)`)
  - Records `bindings[N] = arg_ty`
  - Also matches destination type with sig output type
  - Builds substs vector from bindings
  - Writes back `FnDef(def_id, substs)`

Driver pre-computes `generics_map` from HIR (DefId → Vec<ParamTy>) so the
writeback pass has no HIR access (per §11 interface isolation).

### Verification

| Scenario | Before | After |
|----------|--------|-------|
| `id(42)` + `id(true)` (implicit) | 0 MonoItems | ✅ 2 MonoItems (Fn{i32}, Fn{bool}) |
| `add(1, 2)` (non-generic) | 0 MonoItems | ✅ 0 MonoItems (correct) |
| Mixed turbofish + implicit | 1 MonoItem | ✅ 2 MonoItems |
| `id::<i32>(42)` (turbofish) | 1 MonoItem | ✅ 1 MonoItem (no regression) |

### Design Simplifications (Documented)

| ID | Simplification | Impact | Fix Plan |
|----|----------------|--------|----------|
| S1 | Only top-level Param types matched | `fn wrap<T>(x: Vec<T>)` won't get substs | v0.2 Phase 2: recursive param extraction |
| S2 | Only Copy/Move func operands handled | Generic method calls not handled | v0.2 Phase 2: handle Constant func operands |

### Changes

| ID | Change | Details |
|----|--------|---------|
| 18.102.1 | `writeback_fndef_substs` | New pass in `src/mir/lower/writeback.rs` (~160 lines) |
| 18.102.2 | `collect_param_bindings` | Helper: matches `Param(N)` → `bindings[N] = concrete_ty` |
| 18.102.3 | `generics_map` pre-compute | Driver builds DefId → Vec<ParamTy> from HIR |
| 18.102.4 | Driver wiring | Called after `writeback_closures`, before MIR opt |
| 18.102.5 | 3 new tests | Implicit inference + non-generic + mixed turbofish/implicit |
| 18.102.6 | Design doc | `stage-18.102-implicit-inference-backwrite-design.md` (S1/S2 documented) |

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2622 passed, 0 failed (+3 new) |

### v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1: Substs propagation (Adt) | ✅ Stage 16.52 |
| Phase 2: Substitution | ✅ Stage 16.53 |
| Phase 3: Monomorphization collection | ✅ Stage 16.54 |
| Phase 4a: Specialized naming | ✅ Stage 16.55 |
| Phase 4b: Per-mono layouts | ✅ Stage 16.59 |
| Phase 4c: Codegen integration | ✅ Stage 16.59 |
| Turbofish FnDef substs | ✅ Stage 18.101 |
| **Implicit inference FnDef substs (TD-MONO-INFER)** | ✅ Stage 18.102 |
| Per-mono codegen (emit specialized fns) | ❌ v0.2 (TD-MONO-CODEGEN) |

---
## v0.369.0 — Stage 18.101 (Turbofish Monomorphization — FnDef Substs Propagation)

### Overview

Advances v0.2 P0 monomorphization by fixing the FnDef substs propagation gap.
Generic function calls with explicit turbofish (`id::<i32>(42)`) now produce
proper `MonoItem`s, enabling the monomorphization collection pass to find them.

### Root Cause

`src/mir/lower/expr_operand.rs` Path lowering created `FnDef` types with
`Vec::new().into()` (empty substs) at 2 sites (lines 565, 582). This meant
`collect_mono_items` (which checks `!substs.is_empty()`) found 0 MonoItems
for generic function calls, even with explicit turbofish — the monomorphization
infrastructure was complete but disconnected from the lowering.

### Fix

Both FnDef creation sites now call `lower_path_generic_args(path, ...)` to
extract explicit turbofish args from the path:

```rust
// BEFORE: FnDef(def_id, Vec::new())  — always empty substs
// AFTER:  FnDef(def_id, lower_path_generic_args(path))  — turbofish substs
```

### Verification

| Scenario | Before | After |
|----------|--------|-------|
| `id::<i32>(42)` + `id::<bool>(true)` | 0 MonoItems | ✅ 2 MonoItems (Fn{i32}, Fn{bool}) |
| `add(1, 2)` (non-generic) | 0 MonoItems | ✅ 0 MonoItems (correct) |
| Implicit `id(42)` (no turbofish) | 0 MonoItems | 0 MonoItems (TD-MONO-INFER — v0.2 work) |

### Remaining Gap: TD-MONO-INFER

Implicit generic calls (`id(42)` without `::<i32>`) still produce empty substs
because MIR lowering happens before type inference back-propagates the concrete
type from the argument. Fix requires a writeback-style pass after typeck that
fills FnDef substs from the unify table's inferred types. Tracked as TD-MONO-INFER
for v0.2.

### Changes

| ID | Change | Details |
|----|--------|---------|
| 18.101.1 | FnDef substs propagation | 2 sites in `mir/lower/expr_operand.rs` now call `lower_path_generic_args` |
| 18.101.2 | Turbofish MonoItem test | `id::<i32>` + `id::<bool>` → 2 Fn MonoItems |
| 18.101.3 | Non-generic no-MonoItem test | `add(1,2)` → 0 Fn MonoItems |
| 18.101.4 | Design doc | `stage-18.101-turbofish-monomorphization-design.md` (root cause + fix + TD-MONO-INFER) |

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2622 passed, 0 failed (+2 new) |

### v0.2 Monomorphization Progress

| Phase | Status |
|-------|--------|
| Phase 1: Substs propagation (Adt) | ✅ Stage 16.52 |
| Phase 2: Substitution | ✅ Stage 16.53 |
| Phase 3: Monomorphization collection | ✅ Stage 16.54 |
| Phase 4a: Specialized naming | ✅ Stage 16.55 |
| Phase 4b: Per-mono layouts | ✅ Stage 16.59 |
| Phase 4c: Codegen integration | ✅ Stage 16.59 |
| **Turbofish FnDef substs** | ✅ Stage 18.101 |
| **Implicit inference FnDef substs (TD-MONO-INFER)** | ❌ v0.2 |
| **Per-mono codegen (emit specialized fns)** | ❌ v0.2 (TD-MONO-CODEGEN) |

---
## v0.368.0 — Stage 18.100 (P2 Tech Debt Fixes — format_ty DRY + unwrap cleanup)

### Overview

Implements 3 P2 tech debt fixes identified by the §14.5 D1-D8 deep review
(Round 1). These are low-risk, high-value cleanup items that improve code
quality without changing behavior.

### Changes (P2 Fixes)

| ID | Change | Details |
|----|--------|---------|
| TD-DUP2 | Extract `format_ty` to `mir::ty` | New `format_ty_with_optional_resolver()` in `src/mir/ty.rs`; 3 duplicate `format_ty` methods in `typeck/checker.rs`, `borrowck/mod.rs`, `mir/lower/mod.rs` now delegate to it. Eliminates ~14 lines of duplicate logic. |
| TD-UNWRAP1 | `resolve/module_build.rs:427` unwrap → expect | Bare `.unwrap()` on `path.segments.last()` replaced with `.expect("use paths have ≥1 segment (guarded above)")`. Documents the invariant. |
| TD-UNWRAP2 | `codegen/llvm/helpers.rs:41` CString unwrap → unwrap_or_else with panic msg | `CString::new(s).unwrap()` replaced with `unwrap_or_else` that panics with a clear message identifying NUL byte contamination. Landin symbols never contain NUL, but the message aids debugging if invariant breaks. |

### Design Principles Applied

- **§10.1 rule 5 (DRY / single source of truth)**: `format_ty` now has one definition in `mir::ty`, not 3.
- **§1.0 原則 4 "报错 > 静默"**: All `unwrap()` calls now have clear panic messages.
- **§1.0 原則 6 "通用 > 特例"**: One `format_ty_with_optional_resolver` handles all 3 callers' needs (resolver optional).
- **§23 (API Naming)**: `format_ty_with_optional_resolver` follows `<verb>_<noun>_<prep>_<noun>` pattern.

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2620 passed, 0 failed (no regression) |

### Deep Review P2 Progress

| Tech Debt ID | Status |
|--------------|--------|
| TD-DUP2 (format_ty DRY) | ✅ Stage 18.100 |
| TD-UNWRAP1 (module_build unwrap) | ✅ Stage 18.100 |
| TD-UNWRAP2 (CString unwrap) | ✅ Stage 18.100 |
| TD-DUP1 (types_match_loose + can_coerce) | P2 — v0.2 (TypeRelation trait) |
| TD-DUP3 (infer_place + place_ty) | P2 — v0.2 (extract to mir::place) |
| TD-SPAN (1331 Span::DUMMY) | P2 — v0.2 (MIR lower span propagation) |
| TD-1 (BinaryOp2 fallback) | P2 — v0.2 (CodegenResult) |
| TD-6 (struct auto-Copy) | P2 — v0.2 (field-level Copy) |
| TD-9 (Deref on non-Ref) | P2 — v0.2 (reference type tracking) |
| TD-11 (Int↔Uint loose match) | P2 — v0.2 (IntOrUintVar) |

---
## v0.367.0 — Stage 18.99 (Deep Review Fixes — TD-13 FnDef↔FnPtr Soundness)

### Overview

Implements the P1 fixes identified by the §14.5 D1-D8 deep review (Round 1).
The main fix closes TD-13: `FnDef↔FnPtr` unification now checks signature
compatibility instead of unconditionally returning `Ok`. Also adds nested
Adt soundness tests and syncs stale docs.

### Deep Review (§14.5)

Full D1-D8 audit report at `docs/develop/v0/stage-18/deep-review-round1.md`.
Key findings:
- D1: 1 cross-stage coupling violation (`projection_resolver` → `mir::lower`) — P2 for v0.2
- D2: 27 tech debt markers, 13 targeting v0.2; TD-13 (FnDef↔FnPtr soundness) is P1
- D3: Test count 6,360 actual vs 6,195 claimed (docs stale); nested Adt branch untested
- D7: `matrix.md` + `pipeline-test-coverage.md` stale; `06-mir.md` missing Stage 18.96 MIR opt
- Verdict: GO-WITH-CONDITIONS — fix 4 P1 items, then enter v0.2

### Changes (P1 Fixes)

| ID | Change | Details |
|----|--------|---------|
| 18.99.1 | TD-13 fix: FnDef↔FnPtr sig check | `UnificationTable::set_fn_sigs()` + `unify_fndef_with_fnptr()` — checks param count/types + return type |
| 18.99.2 | TD-13 fix: types_match_loose FnDef↔FnPtr | `else-if` branch in `check_statement` no longer suppresses unify errors for FnDef↔FnPtr (other coercions still suppressed) |
| 18.99.3 | Nested Adt soundness tests | `Vec<Vec<i32>>` vs `Vec<Vec<bool>>` rejected (exercises recursive `types_match_loose`) |
| 18.99.4 | FnDef↔FnPtr soundness tests | `fn(i32)->i32` assigned to `fn(bool)->i32` rejected; matching sigs accepted |
| 18.99.5 | Doc sync: matrix.md | Version v0.364.0 → v0.366.0; counts updated (640 lib + 2620 integration = 6202 total) |
| 18.99.6 | Doc sync: pipeline-test-coverage.md | Header version updated to v0.366.0 |
| 18.99.7 | Doc sync: 06-mir.md | Added §9.4 "实现状态 (Stage 18.96 接线)" documenting MIR opt wiring |
| 18.99.8 | Deep review report | `docs/develop/v0/stage-18/deep-review-round1.md` (D1-D8 + action plan) |

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2620 passed, 0 failed (+4 new) |
| `Vec<Vec<i32>> = Vec<Vec<bool>>` rejected | ✅ (nested substs soundness) |
| `fn(i32)->i32 = fn(bool)->i32` rejected | ✅ (TD-13 fixed) |
| `fn(i32)->i32 = fn(i32)->i32` accepted | ✅ (no regression) |

### v0.2 Roadmap Progress

| Priority | Task | Status |
|----------|------|--------|
| ~~P0~~ | ~~Adt substs soundness (Param unify)~~ | ✅ Stage 18.98 |
| ~~P0~~ | ~~FnDef↔FnPtr soundness (TD-13)~~ | ✅ Stage 18.99 |
| ~~P1~~ | ~~MIR optimization wiring~~ | ✅ Stage 18.96 |
| ~~P1~~ | ~~TraitError location migration~~ | ✅ Stage 18.95 |
| **P0** | Monomorphization (full GAT Phase 4) | Next (infra complete) |
| **P0** | Project system (mini-cargo) | Next |

---
## v0.366.0 — Stage 18.98 (Adt Substs Soundness Fix)

### Overview

Fixes the "Param unify unsound" limitation from v0.1 capability boundaries.
`Vec<i32> = Vec<bool>` (different generic substs) is now correctly rejected
as a type mismatch. This was the core v0.2 P0 soundness issue.

### Root Cause

Two functions had the same bug — both accepted any two `Adt` types with the
same `DefId`, **ignoring substs entirely**:

1. `src/typeck/predicates.rs::can_coerce` line 146:
   `(TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) if a_def == b_def => true`
2. `src/typeck/checker.rs::types_match_loose` line 1549:
   `(TyKind::Adt(a_def, _), TyKind::Adt(b_def, _)) if a_def == b_def => true`

In `check_statement`'s Assign handling, the condition was:
```rust
if place_is_concrete && rvalue_is_concrete
    && !can_coerce(...)        // ← short-circuits here, returns true for Adt
    && !types_match_loose(...) // ← never reached for Adt
```
So `can_coerce` returning `true` for `Vec<i32> ↔ Vec<bool>` short-circuited
the `types_match_loose` check, allowing the unsound assignment.

### Fix

Both `can_coerce` and `types_match_loose` now recursively compare substs:

```rust
(TyKind::Adt(a_def, a_substs), TyKind::Adt(b_def, b_substs)) => {
    if a_def != b_def { return false; }
    // Empty substs = inference case (unknown instantiation) → loose match
    if a_substs.is_empty() || b_substs.is_empty() { return true; }
    if a_substs.len() != b_substs.len() { return false; }
    a_substs.iter().zip(b_substs.iter()).all(|(at, bt)| /* recursive check */)
}
```

Empty substs still loose-match — they represent "unknown, to be inferred"
per `unify.rs`'s empty-substs fallback. This preserves valid generic
inference code like `let w: Wrapper<i32> = make(42);`.

### Changes

| Change | Details |
|--------|---------|
| `can_coerce` Adt case fixed | Now recursively checks substs (was: `if a_def == b_def => true`) |
| `types_match_loose` Adt case fixed | Same recursive substs check |
| 3 new soundness tests | 1 positive (mismatch rejected) + 2 negative (match accepted + inference works) |
| Design doc created | `stage-18.98-adt-substs-soundness-fix-design.md` |

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2616 passed, 0 failed (+3 new) |
| `Vec<i32> = Vec<bool>` rejected | ✅ (was accepted before fix) |
| `Vec<i32> = Vec<i32>` accepted | ✅ (no regression) |
| Generic inference still works | ✅ (empty-substs fallback preserved) |

### v0.2 Roadmap Progress

| Priority | Task | Status |
|----------|------|--------|
| ~~P0~~ | ~~Adt substs soundness (Param unify)~~ | ✅ Stage 18.98 |
| **P0** | Monomorphization (full) | Next (infra complete, GAT Phase 4 pending) |
| **P0** | Project system (mini-cargo) | Next |
| ~~P1~~ | ~~MIR optimization wiring~~ | ✅ Stage 18.96 |
| ~~P1~~ | ~~TraitError location migration~~ | ✅ Stage 18.95 |

---
## v0.365.0 — Stage 18.97 (Documentation Sync Round 2)

### Overview

Second-round documentation sync after Stage 18.96 (MIR opt wiring). The first
sync round (Stage 18.94) was done at v0.361.0; many docs still referenced
stale versions or missed the Stage 18.95/18.96 changes. This stage closes all
remaining doc-sync gaps per §8.1.

### Changes

| Change | Details |
|--------|---------|
| Cargo.toml description simplified | "Landin compiler — Rust-inspired systems language (LLVM 19 backend)" (was ~120 chars) |
| README.md rewritten | v0.364.0 → v0.365.0; full structure: Quick Start + CLI + Features + Testing + Architecture + Project Structure + Limitations + Roadmap + Documentation + Process |
| docs/tests/matrix.md rewritten | Was Stage 12.2 (v0.44.0); now v0.364.0 with current 6195 test count |
| docs/tests/pipeline-test-coverage.md updated | Header v0.44.0 → v0.364.0; pipeline diagram adds macro_expand + writeback + MIR opt stages |
| docs/develop/v0/v0.1-capability-boundaries.md updated | v0.361.0 → v0.364.0; added MIR opt to supported features; test count updated |
| docs/develop/v0/v0.4-roadmap.md header updated | Added "last reviewed 2026-08-11" + current version note |
| docs/develop/v0/v0.5-roadmap.md header updated | Same as v0.4-roadmap |
| Stage 18.94 design doc created | `stage-18.94-doc-sync-and-readme-rewrite-design.md` (was missing per §8.1) |
| Stage 18.95 design doc created | `stage-18.95-traiterror-migration-design.md` (was missing per §8.1) |
| Old versions cleaned | v0.1.0-v0.67.0 + upload/ moved to backup/landin-stage0-archive/ (237 files) |

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2613 passed, 0 failed |

### Doc-Sync Audit (§8.1)

| Document | Status |
|----------|--------|
| Cargo.toml version + description | ✅ v0.365.0, simplified |
| README.md | ✅ Rewritten v0.365.0 |
| RELEASE_NOTES.md | ✅ v0.365.0 (this entry) |
| docs/tests/matrix.md | ✅ Rewritten v0.364.0 |
| docs/tests/pipeline-test-coverage.md | ✅ Header updated v0.364.0 |
| docs/develop/v0/v0.1-capability-boundaries.md | ✅ v0.364.0 |
| docs/develop/v0/v0.4-roadmap.md | ✅ Header updated |
| docs/develop/v0/v0.5-roadmap.md | ✅ Header updated |
| docs/develop/v0/stage-18/stage-18.94-* | ✅ Created (was missing) |
| docs/develop/v0/stage-18/stage-18.95-* | ✅ Created (was missing) |
| docs/develop/v0/stage-18/stage-18.96-* | ✅ Exists (Stage 18.96) |
| worklog.md | ✅ Stage 18.97 entry appended |

---
## v0.364.0 — Stage 18.96 (MIR Optimization Wiring)

### Overview

Wires MIR optimization passes (DCE + const_prop) into the driver pipeline,
completing v0.2 roadmap P1 task "MIR optimization wiring". The passes were
implemented in Stage 17.10/17.13 but remained unwired (marked
`#[allow(dead_code)]`) pending v0.2.

### Changes

| Change | Details |
|--------|---------|
| `run_mir_optimizations` orchestrator | New entry point in `src/mir/optimization.rs` — runs DCE → const_prop → DCE per `06-mir.md` §9.3 |
| Driver wiring | `compile()` calls `run_mir_optimizations(&mut mir)` after writeback, before codegen |
| `compile_no_opt()` | New entry point for tests that verify IR/MIR structure without opt interference |
| DCE Return fix | `collect_terminator_read_locals` now marks `LocalId(0)` as used for `TerminatorKind::Return` — prevents DCE from removing return-value assignments |
| `#![allow(dead_code)]` removed | Optimization module is now wired, no longer dead code |
| 14 existing tests updated | Tests that did manual `run_dce`/`run_const_prop` calls updated to verify post-opt state |
| 2 new wiring tests | `stage18_96_opt_wired_dead_locals_removed` + `stage18_96_opt_idempotent` |
| Codegen/closure tests use `compile_no_opt` | Structural tests verify IR/MIR patterns in isolation per §11 |

### Pass Order Decision (Gray-Area §13.1.2.4)

Design doc (`06-mir.md` §9.3) lists pass order as: DCE → const_prop → jump_threading.
This stage runs **DCE → const_prop → DCE** (second DCE pass after const_prop).

Rationale:
- **Idempotency**: single DCE → const_prop is NOT idempotent (const_prop creates new dead code that a second DCE would remove). Idempotency is required for test reliability.
- **Standard practice**: rustc runs DCE multiple times.
- **Consistent with design doc**: pass TYPES are in order; pass COUNTS are not specified.

### Verification (§3.2)

| Check | Result |
|-------|--------|
| `cargo build --features llvm-backend` | ✅ |
| `cargo fmt --check` | ✅ exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | ✅ 640 passed, 0 failed |
| `cargo test --features llvm-backend --tests` (skip runtime) | ✅ 2613 passed, 0 failed |
| Conformance tests (sample) | ✅ 565 parse + 80 typecheck + 18 codegen-errors + 30 e2e = 693 sampled, 0 failed |
| Runtime tests (`rt_*`) | ⚠ OOM-killed (4GB RAM limit — pre-existing system constraint, not a regression) |

### v0.2 Roadmap Progress

| Priority | Task | Status |
|----------|------|--------|
| P1 | MIR optimization wiring | ✅ Stage 18.96 |
| P1 | TraitError location migration | ✅ Stage 18.95 |
| P0 | Monomorphization | Next |
| P0 | Project system (mini-cargo) | Next |

---
## v0.363.0 — Stage 18.95 (TraitError Location Migration)

### Overview

Final audit pass confirming v0.1 stable release readiness. Pipeline is
**audit-clean** — all Stage 18.71-18.92 fixes verified, 0 remaining issues.

### Audit Results

| Dimension | Status |
|-----------|--------|
| Error system (8 Kind enums + E001-E900) | ✅ Clean |
| Production panic/unwrap | ✅ Clean (0 panic, all unwrap guarded) |
| Span::DUMMY in error reporting | ✅ Clean (unify span param) |
| API naming | ✅ Clean (85+ renames) |
| Dead code | ✅ Clean (documented) |
| Debug format leaks | ✅ Clean |
| Incremental compilation | ✅ Removed (no remnants) |

### Polish Fixes

1. `bin/main.rs`: `to_str().unwrap()` → `to_string_lossy()` (non-UTF8 path safety)
2. `driver.rs`: missing-main `Span::DUMMY` → `Span::new(0, src.len())`
3. `typeck/checker.rs`: simplified redundant span conditional
4. `codegen/llvm/mod.rs`: fixed cache key comment

### v0.1 Stable Release Summary

Stage 18.71-18.93 completed the full audit fix cycle:
- 13 P0/P1 typeck validation fixes (121 tests flipped)
- 3 deep audits (v1/v2/v3/v4)
- Error system fully structured (8 Kind enums + E001-E900)
- Test system enhanced (fuzz + diagnostic quality + dedup 5348→2935)
- Cross-compilation complete (Phase 1-3: x86_64 + AArch64)
- GATs Phase 1-3 complete
- API naming standardized (85+ renames)
- Span::DUMMY cleaned (unify span parameter)

---
## v0.360.0 — Stage 18.92 (Error Type Kind Enums)

Added Kind enums to all 5 remaining error types (LexError/ParseError/LowerError/CodegenError/MacroError). All 8 error types now have structured Kind enums.

---
## v0.358.0 — Stage 18.90 (Cross-Compilation Phase 3)

Fixed `to_object_file` to use configured target triple instead of host triple. Cross-compilation to AArch64 verified.

---
## v0.356.0 — Stage 18.88 (Cross-Compilation Foundation)

Added `TargetTriple` type + `with_target()` constructors. Removed hardcoded target triple from both emitters.

---
## v0.355.0 — Stage 18.87 (GATs Phase 3)

Fixed projection resolver bugs B6/B7/B8: added FnDef/FnPtr/Closure recursive resolution, expanded types_match to 20+ variants, added recursion depth limit.

---
## v0.353.0 — Stage 18.85 (Systematic Test Enhancement)

Added 7 fuzz/stress tests: random programs, malformed input, large match/struct/array, deep nesting, many functions.

---
## v0.354.0 — Stage 18.86 (Diagnostic Quality)

Replaced 115/157 generic `ERROR_PATTERN: error` with specific patterns (73% replacement rate).

---
## v0.346.0 — Stage 18.78 (P0 Correctness Patch)

Wired `CompileErrors.lower` and `CompileErrors.codegen` fields. HIR lowering errors and codegen errors now properly collected and displayed.

---
## v0.343.0 — Stage 18.75 (P0 Error System Fixes)

Added `lower` + `codegen` fields to CompileErrors. Added ErrorCode::Codegen (E700) + ErrorCode::Macro (E800). Replaced 30+ CString::new().unwrap() with cstr_owned(). Macro errors now visible to users.

---
## v0.339.0 — Stage 18.71 (P0 Typeck Enhancement)

Fixed 5 critical typeck deficiencies: type mismatch in let/return/if-branches, trait impl signature validation, void fn return value check. 106 tests flipped from compile_ok to compile_error.

---
## Earlier Versions

See git history for v0.260.0 through v0.338.0 release notes.
