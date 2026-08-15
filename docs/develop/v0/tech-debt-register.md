# Landin Compiler — Comprehensive Tech Debt Register

> **Author**: redskaber
> **Date**: 2026-08-15 (last updated Stage 18.119)
> **Version**: v0.387.0
> **Status**: Active — all P0/P1 items resolved, remaining items are v0.2 Phase 2+

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

## 2. Remaining Tech Debt (v0.2 Phase 2+)

### 2.1 Codegen Architecture

| ID | Description | Root Cause | Fix Plan |
|----|-------------|------------|----------|
| TD-CODEGEN-RESULT | codegen returns `String` not `Result`, forcing `panic!()` for BinaryOp2 | All codegen functions return `EmitValue` (String), not `Result<EmitValue, CodegenError>` | v0.2 Phase 2: change `codegen_rvalue` → `Result<EmitValue, CodegenError>`, propagate through `codegen_statement` → `codegen_function` → `run_codegen_pipeline` |
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
| TD-SINGLE-FILE | No project/crate system — only single-file compilation | Cannot compile multi-file programs | v0.2 P0: mini-cargo project system |
| TD-NO-INCREMENTAL | Full recompile every time | Slow iteration cycle | v0.2 P2: incremental compilation (requires project system) |
| TD-BINARYOP2-PANIC | BinaryOp2 panics if it reaches codegen (should be desugared) | Range expressions that aren't desugared will crash the compiler | v0.2 Phase 2: codegen returns Result (TD-CODEGEN-RESULT) |

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

## 3. Architecture Summary

### 3.1 Pipeline (v0.387.0)

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
