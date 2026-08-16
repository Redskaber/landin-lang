# Landin Compiler — Comprehensive Tech Debt Register

> **Author**: redskaber
> **Date**: 2026-08-16 (last updated Stage 18.135)
> **Version**: v0.403.0
> **Status**: Active — all P0/P1 items resolved, remaining items are v0.2 Phase 2+ + structural TDs (5 resolved + 2 partial: 18.127 × 2, 18.128 × 1, 18.129-18.130 × 1, 18.131-18.133 × 1, 18.134 × 1 partial, 18.135 × 1 partial)

## 1. Resolved Tech Debt (S2-S11 + D1-D8)

All monomorphization tech debt (S2-S11) and deep review action items (D1-D8) are resolved.

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
| TD-13 | FnDef↔FnPtr soundness | 18.99 | ✅ |
| TD-DUP2 | format_ty DRY | 18.100 | ✅ |
| TD-UNWRAP1 | module_build unwrap → expect | 18.100 | ✅ |
| TD-UNWRAP2 | CString unwrap → unwrap_or_else | 18.100 |
| TD-UNWRAP-DRIVER | driver.rs 4 unwrap (`f.body.unwrap()` after `is_some()`) → `if let Some(b)` pattern | 18.127 | ✅ |
| TD-UNWRAP-BORROWCK-REGION | borrowck/region_inference.rs 3 SCC algorithm unwrap → `expect("...")` with invariant docs | 18.127 | ✅ |
| TD-LOC-TYPECK-CHECKER | typeck/checker.rs 2635 LOC → split into 4 files (checker 1371 + infer 544 + check 476 + writeback 339), all < 1500 LOC per §13.4 J1-J6 | 18.128 | ✅ |
| TD-LOC-MIR-LOWER-MOD (partial) | mir/lower/mod.rs 2857 LOC → mod.rs 2016 + ty_lower.rs 863 (type lowering extracted); mod.rs still > 1500, needs Stage 18.130 body lowering split | 18.129 | 🟡 Partial |
| TD-LOC-MIR-LOWER-MOD (complete) | mir/lower/mod.rs 2016 LOC → mod.rs 960 + body_lower.rs 1110 (body lowering + elision + resolve_self + tests extracted); all 3 files < 1500 LOC | 18.130 | ✅ |
| TD-LOC-MIR-LOWER-EXPR (partial) | mir/lower/expr_operand.rs 3599 LOC → expr_operand.rs 2503 + method_resolution.rs 1132 (method resolution extracted); expr_operand still > 1500 (lower_expr_to_operand 2106 LOC), needs Stage 18.132 | 18.131 | 🟡 Partial |
| TD-LOC-MIR-LOWER-EXPR (partial continued) | mir/lower/expr_operand.rs 2503 LOC → expr_operand.rs 2171 + call_lower.rs 362 (call helpers extracted); MethodCall arm extraction attempted+reverted (type signature issues); expr_operand still > 1500, needs Stage 18.133 | 18.132 | 🟡 Partial |
| TD-LOC-MIR-LOWER-EXPR (complete) | mir/lower/expr_operand.rs 2171 LOC → expr_operand.rs 1156 + expr_variants.rs 1016 (4 largest match arms extracted as functions: Path + Call + For + MethodCall); all 4 mir/lower/ files < 1500 LOC | 18.133 | ✅ |
| TD-LOC-DRIVER (partial) | driver.rs 4038 LOC → driver/mod.rs 2351 + driver_validations.rs 936 + driver_scan.rs 618 + driver_object_safety.rs 164 (validation + scan + object safety extracted); mod.rs still > 1500 (compile_inner 1442 LOC), needs Stage 18.136 | 18.134 | 🟡 Partial |
| TD-LOC-MACRO-EXPAND (partial) | macro_expand.rs 5962 LOC → macro_expand.rs 3904 + builtin_macros.rs 2069 (27 builtin macro functions extracted); both files still > 1500, needs Stage 18.136 (core matching + substitution + repetition + hygiene) | 18.135 | 🟡 Partial |

## 2. Remaining Tech Debt (v0.2 Phase 2+)

### 2.1 Codegen Architecture

| ID | Description | Root Cause | Fix Plan |
|----|-------------|------------|----------|
| TD-CODEGEN-RESULT | codegen returns `String` not `Result`, forcing `panic!()` for BinaryOp2 | All codegen functions return `EmitValue` (String), not `Result<EmitValue, CodegenError>` | ✅ Resolved Stage 18.151: `codegen_rvalue` → `CodegenResult<EmitValue>`, propagated through `codegen_statement` → `codegen_function` → `run_codegen_pipeline` → `codegen_crate` → driver |
| TD-PROJECTION-RESOLVER | `projection_resolver.rs` lives under `typeck/` but is a driver-stage operation | Module was created during Stage 18.87 GATs Phase 3; location mirrors the original typeck integration point | v0.2 Phase 2: move to `driver::post_typeck` or `mir::lower::post_typeck` |

### 2.2 Span::DUMMY

| Category | Count | Description | Action |
|----------|-------|-------------|--------|
| (A) Legitimate | ~490 | `parser/macro_expand.rs` synthesized tokens (no source location exists) | Leave — correct by design |
| (A) Legitimate | ~5 | `driver.rs` synthetic Infer/Error types (created before typeck) | Leave — correct by design |
| (A) Legitimate | ~13 | `mir/substitute.rs` (documented: Ty interning doesn't preserve span) | Leave — documented decision |
| (A) Legitimate | ~76 | Test code (`#[cfg(test)]` modules) | Leave — test infrastructure |
| (B) Fixed | ~31 | `driver.rs` (7), `projection_resolver.rs` (10), `where_clause.rs` (1), `checker.rs` (~14) — all converted to `Ty::from_kind()` or `p.span` | ✅ Stages 18.115-18.117 |
| **Remaining (B)** | **~0** | All fixable Span::DUMMY have been addressed | ✅ Complete |

**Conclusion**: All Category (B) Span::DUMMY (where a real span was available but unused) have been fixed. Remaining ~584 occurrences are Category (A) — legitimate synthetic values with no source span.

### 2.3 Type System

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-INT-UINT-VAR | `types_match_loose` has hardcoded Int↔Uint same-width pairs (workaround for unify table's lossy Uint→Int conversion) | `let x: u32 = 1;` accepted via loose match (isize instead of usize) | v0.2 Phase 2: separate `IntOrUintVar` in unification table |
| TD-DEREF-NON-REF | Deref on non-Ref types in pattern bindings silently returns Error | Pattern bindings on `&self` don't propagate reference types | v0.2 Phase 2: reference type tracking through pattern bindings |
| TD-LOCALID0-FALLBACK | Non-Local borrowed places use LocalId(0) fallback in region constraints | Overly conservative borrow regions for field projections | v0.2 Phase 2: field projection region tracking |

### 2.4 Code Generation

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-SINGLE-FILE | No project/crate system — only single-file compilation | Cannot compile multi-file programs | 🟡 Phase 1-3 Resolved Stage 18.152-18.154: `ModuleLoader` + `compile_project` + cross-file resolution + `landinc` CLI. Phase 4 (manifest integration) remains |
| TD-NO-INCREMENTAL | Full recompile every time | Slow iteration cycle | v0.2 P2: incremental compilation (requires project system) |
| TD-BINARYOP2-PANIC | BinaryOp2 panics if it reaches codegen (should be desugared) | Range expressions that aren't desugared will crash the compiler | ✅ Resolved Stage 18.151: BinaryOp2 arm now returns `Err(CodegenError)` instead of `panic!()`, propagated via `CodegenResult` (depends on TD-CODEGEN-RESULT) |
| TD-RVALUE-NO-SPAN | `Rvalue` enum doesn't carry `Span` info; BinaryOp2 error uses `Span::DUMMY` | Codegen errors for BinaryOp2 lack source location | v0.2 P2: add `span: Span` field to `Rvalue` (or wrap in spanned container); populate during MIR lowering |
| TD-EMITTER-PANIC | `src/codegen/emitter/mod.rs` has 2 `panic!()` in `fat_ptr_type` (line 321) and `array_of` (line 357) for unreachable match arms | Type-conversion utility panics on misuse (not on codegen pipeline path) | v0.2 P2: convert to `Result<EmitType, CodegenError>` or use `unreachable!()` with clear message |
| TD-SPAN-DUMMY-CLEANUP | 错误路径中 ~6 处 `Span::DUMMY` 可用真实 span 替换 (typeck/check, mir/lower/expr_variants 等) | 错误诊断丢失源码位置 | 🟡 Partial Stage 18.159: 修复 `expr_variants.rs` 2 处 discriminant span (改用 `expr.span`); 其余经评估为合法合成用法 (合成 token/类型无源码位置), 保留 |
| TD-MODULELOAD-ERROR-FIELD | `ModuleLoadError` 强转为 `LowerError`, 丢失 `path` 字段 | 用户看到的模块加载错误丢失文件路径上下文 | ✅ Resolved Stage 18.159: 添加 `CompileErrors.module_load` 字段 + `ErrorCode::ModuleLoad` (E850) + 诊断渲染含 path note |
| TD-NEGATIVE-TEST-COVERAGE | 负面测试比例 6.5% (低于 §9.4.3 建议的 25%) | 错误路径覆盖不足 | v0.2 P2: 补充负面测试至 25% 比例 (重点: codegen/ModuleLoader/typeck) |
| TD-UNWRAP-NONGUARDED | 9 处非测试 `unwrap()`, 其中 `codegen/llvm/arithmetic.rs:381` 无明显 guard | 潜在 panic 风险 | ✅ Resolved Stage 18.159: `codegen/llvm/arithmetic.rs:381` 改为 `if let Some(&v)` 模式 (显式>隐式); 其余 8 处有 invariant guard, 保留 |

### 2.5 Platform Support

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-LINUX-ONLY | No Windows/macOS target triples | Cannot cross-compile to non-Linux platforms | v0.2 P2: cross-compile expansion |
| TD-ABI-DIVERSITY | Only `extern "C"` tested | No `extern "system"`, `extern "Rust"` | v0.2 P2: ABI diversity |

### 2.6 Standard Library

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-STDLIB-FACADE | String/Vec/Option/Result are type stubs, not real implementations | No heap allocation, no collections | v0.2 P1: full standard library |
| TD-NO-FORMAT-MACRO | No `format!`/`write!` macros | Only `println!`/`print!`/`eprintln!`/`eprint!` | v0.2 P1: format macros |

### 2.7 Test Infrastructure

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-IGNORE-DISCIPLINE | Only 2 `#[ignore]` markers despite many "known limitations" in comments | Hard to track which limitations are temporary vs permanent | v0.2 Phase 2: convert documented limitations to `#[ignore = "..."]` |
| TD-CODEGEN-NEGATIVE | Codegen negative test ratio is 3% (vs typeck 22%) | Error-path coverage in codegen is thin | v0.2 Phase 2: add explicit negative codegen tests |

### 2.8 MIR Optimization

| ID | Description | Impact | Fix Plan |
|----|-------------|--------|----------|
| TD-NO-JUMP-THREADING | Jump threading not implemented | Unnecessary goto chains in optimized MIR | v0.3: jump threading pass |
| TD-CONST-PROP-LOOPS | const_prop skips all BinaryOp folding when back-edges exist (Stage 18.110) | Misses some optimization opportunities in loops | v0.2 Phase 2: fixpoint iteration for const_prop in loops |

### 2.9 Structural — LOC Threshold Violations (§13.4 J6) — Stage 18.126 新增, 18.128-18.135 部分修复

> **背景**：Stage 18.126 §17 任务规划排版图扫描发现 9 个文件超过 §13.4 J6 阈值（mod.rs < 1500 LOC；子模块 100-1500 LOC）。这些是"上帝模块"，违反单一职责原则 (J2)。
>
> **Stage 18.128 进展**: TD-LOC-TYPECK-CHECKER 已修复 ✅ — 拆分为 4 文件 (checker 1371 + infer 544 + check 476 + writeback 339), 全部 < 1500 LOC。
>
> **Stage 18.129-18.130 进展**: TD-LOC-MIR-LOWER-MOD 已修复 ✅ — Stage 18.129 提取 ty_lower.rs (863 LOC), Stage 18.130 提取 body_lower.rs (1110 LOC), mod.rs 从 2857 降至 960, 全部 < 1500 LOC。
>
> **Stage 18.131-18.133 进展**: TD-LOC-MIR-LOWER-EXPR 已修复 ✅ — Stage 18.131 提取 method_resolution.rs (1132 LOC), Stage 18.132 提取 call_lower.rs (362 LOC), Stage 18.133 提取 expr_variants.rs (1016 LOC, 4 个最大 match arm), expr_operand.rs 从 3599 降至 1156, 全部 < 1500 LOC。
>
> **Stage 18.134 进展**: TD-LOC-DRIVER 部分修复 🟡 — 提取 driver_validations.rs (936 LOC) + driver_scan.rs (618 LOC) + driver_object_safety.rs (164 LOC), driver.rs 从 4038 降至 2351 (driver/ 目录模块转换)。
>
> **Stage 18.135 进展**: TD-LOC-MACRO-EXPAND 部分修复 🟡 — 提取 builtin_macros.rs (2069 LOC, 27 个 builtin macro 函数), macro_expand.rs 从 5962 降至 3904。

| ID | File | LOC | 阈值倍数 | Root Cause | Fix Plan | Status |
|----|------|-----|---------|------------|----------|--------|
| TD-LOC-MACRO-EXPAND | `src/parser/macro_expand.rs` | ~~5962~~ → 3904 | 4.0× → 2.6× | macro_rules! 全功能集中（fragment specifiers + repetition + hygiene + builtin macros） | 🟡 Stage 18.135: 提取 builtin_macros.rs (2069); Stage 18.136: 提取 core matching + substitution + repetition + hygiene | 🟡 Partial 18.135 |
| TD-LOC-DRIVER | `src/driver/mod.rs` | ~~4038~~ → 2351 | 2.7× → 1.6× | 编排层全功能集中 + validation + scan + object safety 混合 | 🟡 Stage 18.134: 提取 driver_validations.rs (936) + driver_scan.rs (618) + driver_object_safety.rs (164); Stage 18.137: 拆分 compile_inner | 🟡 Partial 18.134 |
| TD-LOC-MIR-LOWER-EXPR | `src/mir/lower/expr_operand.rs` | ~~3599~~ → 1156 | 2.4× → ✅ | MIR 表达式 lowering 全集中 | ✅ Stage 18.131-18.133: 提取 method_resolution.rs (1132) + call_lower.rs (362) + expr_variants.rs (1016) | ✅ Resolved 18.131-18.133 |
| TD-LOC-MIR-LOWER-MOD | `src/mir/lower/mod.rs` | ~~2857~~ → 960 | 1.9× → ✅ | MIR lower 顶层 + body lowering + local decls | ✅ Stage 18.129-18.130: 提取 ty_lower.rs (863) + body_lower.rs (1110), mod.rs 960 | ✅ Resolved 18.129-18.130 |
| TD-LOC-TYPECK-CHECKER | `src/typeck/checker.rs` | ~~2635~~ → 1371 | 1.8× → ✅ | typeck 主入口全集中（unify + infer + coerce + check） | ✅ Stage 18.128: 拆分为 checker/infer/check/writeback 4 文件 | ✅ Resolved 18.128 |

> 其余 4 个文件（`mir/lower/control_flow.rs` 2228 LOC、`borrowck/mod.rs` 1857 LOC、`borrowck/region_inference.rs` 1776 LOC、`traits/resolver.rs` 1558 LOC）阈值倍数 < 2.0×，归入 v0.3 P3 优化。

### 2.10 Structural — Span::DUMMY 待审计 (§6.2.1 分类索引) — Stage 18.126 新增

> **背景**：tech-debt-register.md §2.2 已声明"所有 Category B Span::DUMMY 已修复"，但 Stage 18.126 扫描发现 8 个文件共 ~491 个 Span::DUMMY **未做 Category A/B 分类审计**。这些可能是漏网的 Category B（可修复）。

| ID | File | Count | Status | Action |
|----|------|-------|--------|--------|
| TD-DUMMY-BORROWCK-MOD | `src/borrowck/mod.rs` | 162 | 待审计 | v0.2 P2: 逐个审计, Category B 改 `Ty::from_kind()` 或 `p.span` |
| TD-DUMMY-TYPECK-CHECKER | `src/typeck/checker.rs` | 91 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-MIR-LOWER-MOD | `src/mir/lower/mod.rs` | 54 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-TYPECK-UNIFY | `src/typeck/unify.rs` | 48 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-BORROWCK-LIVENESS | `src/borrowck/liveness.rs` | 40 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-BORROWCK-REGION | `src/borrowck/region_inference.rs` | 33 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-MIR-LOWER-EXPR | `src/mir/lower/expr_operand.rs` | 30 | 待审计 | v0.2 P2: 逐个审计 |
| TD-DUMMY-BORROWCK-BORROWSET | `src/borrowck/borrow_set.rs` | 23 | 待审计 | v0.2 P2: 逐个审计 |

**预估**: ~491 待审计, 预计 ~50 是 Category B (可修复), 其余 ~441 是 Category A (legitimate)。

### 2.11 Structural — unwrap/expect 静默吞错 (§2 原则 4) — Stage 18.126 新增, 18.127 修正

> **背景**：Stage 18.126 扫描发现 borrowck/typeck/parser 共 162 个 unwrap/expect 调用, 部分缺少 message 或使用 unwrap() 静默吞错, 违反 §2 原则 4 "报错 > 静默"。
>
> **Stage 18.127 修正**：经详细审计, 大部分 unwrap 在 `#[cfg(test)] mod tests` 内 (合法), 仅 7 个在 real code 中:
> - driver.rs: 4 unwrap (已修复 → TD-UNWRAP-DRIVER ✅)
> - borrowck/region_inference.rs: 3 unwrap (SCC 算法不变量, 已修复 → TD-UNWRAP-BORROWCK-REGION ✅)
> - borrowck/borrow_set.rs: 9 unwrap 全部在 test code (合法, 不修复)
> - codegen/llvm/helpers.rs: 3 unwrap 全部在 test code 或防御性 fallback (合法, 不修复)
> - codegen/llvm/mod.rs: 0 unwraps (Stage 18.151 fixed `name.strip_prefix('@').unwrap()` → safe `if let Some` pattern)

| ID | File | unwrap (real) | unwrap (test) | expect | Risk | Action | Status |
|----|------|---------------|---------------|--------|------|--------|--------|
| TD-UNWRAP-DRIVER | `src/driver.rs` | 4 | 0 | 0 | 🟡 MEDIUM | `if let Some(b)` pattern | ✅ Resolved 18.127 |
| TD-UNWRAP-BORROWCK-REGION | `src/borrowck/region_inference.rs` | 3 | 10 | 0 | 🔴 HIGH → 🟢 LOW | `expect("...")` + invariant docs | ✅ Resolved 18.127 |
| TD-EXPECT-TYPECK-SOLVER | `src/typeck/solver.rs` | 0 | 0 | 37 | 🟡 MEDIUM | 审计每个 expect 的 message | Open — v0.2 P2 |
| TD-EXPECT-PARSER-ITEMS | `src/parser/items.rs` | 0 | 0 | 36 | 🟡 MEDIUM | 审计每个 expect 的 message | Open — v0.2 P2 |
| TD-UNWRAP-BORROWCK-BORROWSET | `src/borrowck/borrow_set.rs` | 0 | 9 | 0 | 🟢 LOW (test only) | N/A — test code 合法 | Closed 18.127 (reclassified) |
| TD-UNWRAP-CODEGEN-LLVM-HELPERS | `src/codegen/llvm/helpers.rs` | 0 | 3 | 0 | 🟢 LOW (test/fallback) | N/A — test code 合法 | Closed 18.127 (reclassified) |
| TD-UNWRAP-CODEGEN-LLVM-MOD | `src/codegen/llvm/mod.rs` | 0 | 0 | 0 | ✅ CLOSED | ✅ Resolved Stage 18.151: `name.strip_prefix('@').unwrap()` replaced with safe `if let Some(stripped) = name.strip_prefix('@')` pattern |

## 3. Architecture Summary

### 3.1 Pipeline (v0.393.0)

```
Source → Lexer → macro_expand → Parser → HIR Lower → Resolve
→ MIR Lower → TypeCheck → BorrowCheck → Writeback
→ MIR Opt (DCE → const_prop → DCE) → Monomorphization
→ Codegen → Link → Execute
```

### 3.2 Test Counts

| Category | Count |
|----------|-------|
| Rust lib tests | 640 |
| Rust integration tests | 2,663 |
| Conformance tests | 2,935 |
| Fuzz/stress tests | 7 |
| **Total** | **6,245** |
| **Failures** | **0** |
| **Skipped** | **0** |

### 3.3 Span::DUMMY Status

- **Total non-test**: ~584 (all Category A — legitimate)
- **Fixable (Category B)**: 0 (all fixed in Stages 18.115-18.117)
- **Ty::from_kind adoption**: All `Ty::new(K, Span::DUMMY)` calls in typeck/ replaced with `Ty::from_kind(K)`

### 3.4 Enum Branch Coverage

- **TerminatorKind**: All 7 variants explicitly covered in typeck + borrowck (no `_ =>` catch-all)
- **StatementKind**: All 5 variants explicitly covered in typeck (no `_ =>` catch-all)
- **Rvalue**: All 7 variants explicitly covered in typeck + borrowck + codegen
- **EmitType**: bit_width match has explicit arms for all integer types + documented fallback
- **AggregateKind**: All 4 variants explicitly covered in typeck + codegen

### 3.5 Error System

- **8 structured Kind enums**: LexErrorKind(7), ParseErrorKind(7), LowerErrorKind(4), ResolveErrorKind(8), TypeErrorKind(6), BorrowErrorKind(9), CodegenErrorKind(5), MacroErrorKind(5)
- **ErrorCode E001-E900**: All wired
- **9-field CompileErrors**: All wired
- **Diagnostic display**: Source snippets + color output (auto/always/never)

---

## 4. Classification Index (§6.2.1 强制结构) — Stage 18.126 新增

### 4.1 By Severity (§6.1)

| Severity | Count | IDs |
|----------|-------|-----|
| P0 (致命) | 0 | — (all resolved) |
| P1 (严重) | 0 | — (all resolved) |
| P2 (一般) | 23 | TD-INT-UINT-VAR, TD-DEREF-NON-REF, TD-LOCALID0-FALLBACK, TD-SINGLE-FILE, TD-NO-INCREMENTAL, TD-RVALUE-NO-SPAN, TD-EMITTER-PANIC, TD-SPAN-DUMMY-CLEANUP, TD-MODULELOAD-ERROR-FIELD, TD-NEGATIVE-TEST-COVERAGE, TD-UNWRAP-NONGUARDED, TD-LINUX-ONLY, TD-ABI-DIVERSITY, TD-STDLIB-FACADE, TD-NO-FORMAT-MACRO, TD-IGNORE-DISCIPLINE, TD-CODEGEN-NEGATIVE, TD-NO-JUMP-THREADING, TD-CONST-PROP-LOOPS, TD-LOC-MACRO-EXPAND, TD-LOC-DRIVER, TD-LOC-MIR-LOWER-EXPR, TD-LOC-MIR-LOWER-MOD, TD-DUMMY-* (8), TD-EXPECT-TYPECK-SOLVER, TD-EXPECT-PARSER-ITEMS |
| P3 (优化) | 4 | 4 文件 LOC < 2.0× 阈值（control_flow/mod.rs/region_inference/resolver.rs） |
| ✅ Resolved in 18.127 | 2 | TD-UNWRAP-DRIVER, TD-UNWRAP-BORROWCK-REGION |
| ✅ Resolved in 18.128 | 1 | TD-LOC-TYPECK-CHECKER (拆分为 4 文件, 全部 < 1500 LOC) |
| ✅ Resolved in 18.129-18.130 | 1 | TD-LOC-MIR-LOWER-MOD (提取 ty_lower.rs 863 + body_lower.rs 1110, mod.rs 2857→960, 全部 < 1500) |
| ✅ Resolved in 18.131-18.133 | 1 | TD-LOC-MIR-LOWER-EXPR (提取 method_resolution.rs 1132 + call_lower.rs 362 + expr_variants.rs 1016, expr_operand 3599→1156, 全部 < 1500) |
| 🟡 Partial in 18.134 | 1 | TD-LOC-DRIVER (提取 driver_validations.rs 936 + driver_scan.rs 618 + driver_object_safety.rs 164, driver.rs 4038→2351, 仍超 1500) |
| 🟡 Partial in 18.135 | 1 | TD-LOC-MACRO-EXPAND (提取 builtin_macros.rs 2069, macro_expand.rs 5962→3904, 仍超 1500) |
| ✅ Reclassified in 18.127 | 2 | TD-UNWRAP-BORROWCK-BORROWSET (test only), TD-UNWRAP-CODEGEN-LLVM-HELPERS (test/fallback) |
| ✅ Resolved in 18.148 | 1 | TD-PROJECTION-RESOLVER (moved typeck → driver) |
| ✅ Resolved in 18.151 | 3 | TD-CODEGEN-RESULT, TD-BINARYOP2-PANIC, TD-UNWRAP-CODEGEN-LLVM-MOD |
| 🟡 Phase 1 Resolved in 18.152 | 1 | TD-SINGLE-FILE (ModuleLoader + compile_project; phases 2-4 remain) |
| 🟡 Phase 2 Resolved in 18.153 | 1 | TD-SINGLE-FILE (cross-file use/path resolution; phases 3-4 remain) |
| 🟡 Phase 3 Resolved in 18.154 | 1 | TD-SINGLE-FILE (landinc CLI build/run/new/check/clean; phase 4 remains) |
| 🟡 Phase 4 Resolved in 18.155 | 1 | TD-SINGLE-FILE (mini-cargo deficiency fixes: colored diagnostics + compile_project_opt + project name validation) |
| 📋 Deep Review in 18.158 | 4 | TD-SPAN-DUMMY-CLEANUP, TD-MODULELOAD-ERROR-FIELD, TD-NEGATIVE-TEST-COVERAGE, TD-UNWRAP-NONGUARDED (跨阶段审查 §14.7 发现) |
| ✅ Resolved in 18.159 | 2 | TD-MODULELOAD-ERROR-FIELD (CompileErrors.module_load + ErrorCode::ModuleLoad), TD-UNWRAP-NONGUARDED (codegen/llvm/arithmetic.rs if-let pattern) |
| 🟡 Partial in 18.159 | 1 | TD-SPAN-DUMMY-CLEANUP (2 处 discriminant span 修复; 其余为合法合成用法) |

### 4.2 By §11.3 Pipeline Coupling (L-PIPE-N)

| ID | Description | Status |
|----|-------------|--------|
| TD-PROJECTION-RESOLVER | `projection_resolver.rs` 位置错（在 typeck/ 下，应在 driver/mir::lower::post_typeck） | Open — v0.2 Phase 2 |

### 4.3 By §10 Naming Violations (L-NAMING-N)

无 open 项 (Stage 3.63 已全量修复)

### 4.4 By §13.4 Refactoring Judgments (J1-J6)

| ID | J# Violated | Description | Status |
|----|-------------|-------------|--------|
| TD-LOC-MACRO-EXPAND | J2 (单一职责) + J6 (LOC) | macro_expand.rs 5962 → 3904 LOC (builtin_macros.rs 2069 提取) | 🟡 Partial 18.135 — Stage 18.136 core matching |
| TD-LOC-DRIVER | J2 + J6 | driver.rs 4038 → 2351 LOC (driver_validations.rs 936 + driver_scan.rs 618 + driver_object_safety.rs 164 提取) | 🟡 Partial 18.134 — Stage 18.136 compile_inner |
| TD-LOC-MIR-LOWER-EXPR | J2 + J6 | mir/lower/expr_operand.rs 3599 → 1156 LOC (method_resolution.rs 1132 + call_lower.rs 362 + expr_variants.rs 1016 提取) | ✅ Resolved 18.131-18.133 |
| TD-LOC-MIR-LOWER-MOD | J2 + J6 | mir/lower/mod.rs 2857 → 960 LOC (ty_lower.rs 863 + body_lower.rs 1110 提取) | ✅ Resolved 18.129-18.130 |
| TD-LOC-TYPECK-CHECKER | J2 + J6 | typeck/checker.rs 2635 LOC → 1371 LOC (4 文件) | ✅ Resolved 18.128 |

### 4.5 By §2 Principle Violations

| ID | Principle | Description | Status |
|----|-----------|-------------|--------|
| TD-UNWRAP-BORROWCK-REGION | §2 原则 4 (报错 > 静默) | 3 SCC 算法 unwrap → `expect("...")` | ✅ Resolved 18.127 |
| TD-UNWRAP-DRIVER | §2 原则 3 (显式 > 隐式) + §2 原则 4 | 4 `f.body.unwrap()` after `is_some()` → `if let Some(b)` | ✅ Resolved 18.127 |
| TD-EXPECT-TYPECK-SOLVER | §2 原则 4 | 37 个 expect 部分缺 message | Open — v0.2 P2 |
| TD-EXPECT-PARSER-ITEMS | §2 原则 4 | 36 个 expect 部分缺 message | Open — v0.2 P2 |
| TD-UNWRAP-CODEGEN-LLVM-MOD | §2 原则 4 | 1 unwrap (`strip_prefix('@').unwrap()`) | ✅ Resolved Stage 18.151 (replaced with `if let Some` pattern) |
| TD-BINARYOP2-PANIC | §2 原则 4 + §2 原则 9 (正确 > 妥协) | panic 替代 CodegenError 传播 | ✅ Resolved Stage 18.151 (returns `Err(CodegenError)` via `CodegenResult`) |
