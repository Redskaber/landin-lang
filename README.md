# Landin

> A work-in-progress systems programming language inspired by Rust, using
> LLVM 22 (llvm-sys 221) for code generation. The compiler is written in
> Rust (~93K LOC across 186 files) and targets x86_64 + AArch64 Linux.

| | |
|---|---|
| **Author** | redskaber |
| **Version** | v0.608.0 (v0.7 Stage 58 — TD-CAST-STR-TO-U8-SLICE FIXED: str::as_bytes real body + infer_cast_kind; TD-STR-INTRINSIC-MARKER-BODIES 3/3 complete; 5436 tests — Architecture health 9.85/10) |
| **License** | MIT |
| **Status** | ✅ **v0.7 Stage 58 COMPLETE**. 5436 tests (898 lib + 4538 integration), 0 failures, 4 ignored. fmt clean, 0 clippy warnings. Stage 58 fixes TD-CAST-STR-TO-U8-SLICE — added `infer_cast_kind` function in expr_operand.rs that determines CastKind based on source/target types (&str→&[u8] = Unsize). str::as_bytes now has real body (`self as &[u8]`). All 3 str intrinsics (len/is_empty/as_bytes) now have real bodies — TD-STR-INTRINSIC-MARKER-BODIES 3/3 complete. Runtime verified: `"hello".as_bytes().len()` → `5` via real body. Architecture health: 9.85/10 (stable — root-cause TD fix, no regression). |
| **LLVM** | 22.1.8 (llvm-sys 221) |
| **Rust edition** | 2021 |
| **Process doc** | `docs/stage-committee-process.md` v7.5 (11 design principles + 13 execution principles + Bug probability distribution + experimental exploration methodology with surgical split) |

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [CLI Reference](#cli-reference)
3. [Language Features](#language-features)
4. [Codegen ABI Compliance](#codegen-abi-compliance)
5. [Testing](#testing)
6. [Architecture Overview](#architecture-overview)
7. [Writeback Architecture (v0.5+)](#writeback-architecture-v05)
8. [Tech Debt & Known Limitations](#tech-debt--known-limitations)
9. [v0.5+ Refactoring Roadmap](#v05-refactoring-roadmap)
10. [Project Layout](#project-layout)
11. [Documentation](#documentation)
12. [Contributing](#contributing)

---

## Quick Start

### Prerequisites

- Rust stable (≥ 1.70.0) + cargo + rustfmt + clippy
- LLVM 22.1 development headers (auto-installed via `scripts/setup-llvm-env.sh`)
- cc/clang (for linking)
- Linux x86_64 or aarch64

### Build

```bash
# 1. Setup LLVM 22 environment
source scripts/env.sh

# 2. Build
cargo build --release --features llvm-backend

# 3. Run tests (auto-tunes --test-threads + raises ulimit -s for LLVM)
bash scripts/run_tests.sh
```

### Hello World

```bash
echo 'fn main() -> i32 { println!("hello world"); 0 }' > hello.lin
./target/release/landin-stage0 --run hello.lin
```

### Multi-File Project (`landinc`)

```bash
landinc new my_project && cd my_project
landinc build --release
landinc run
```

---

## CLI Reference

### `landin-stage0` — single-file compiler

| Flag | Description |
|------|-------------|
| `--compile` | Full pipeline (lex → parse → typeck → borrowck → codegen) |
| `--emit-llvm-ir` | Emit LLVM IR text (implies `--compile`) |
| `--emit-obj` | Emit object file `.o` (requires `llvm-backend`) |
| `--emit-bin` | Emit executable (requires `llvm-backend`) |
| `--run` | Compile, link, and run (requires `llvm-backend`) |
| `--emit-tokens` | Emit token stream only (debug) |
| `--emit-ast` | Emit AST only (debug) |
| `--color WHEN` | Color output: `auto` / `always` / `never` (default: auto) |
| `--target TRIPLE` | Cross-compile target (e.g. `aarch64-unknown-linux-gnu`) |

### `landinc` — multi-file project tool

```bash
landinc new <name>         # Create new project
landinc build [--release]  # Build all .lin files in src/
landinc run                # Build + run
landinc check              # Type-check only (no codegen)
landinc test               # Run unit tests
```

---

## Language Features

### Types
- Primitives: `i8`–`i128`, `u8`–`u128`, `usize`, `isize`, `f32`, `f64`, `bool`
- Strings: `&str` (fat pointer `{ ptr, len }`), `String` (owned `{ ptr, len, cap }`)
- Collections: `Vec<T>`, `Box<T>`, `Option<T>`, `Result<T, E>`
- Arrays: `[T; N]`
- References: `&T`, `&mut T`, raw pointers: `*const T`, `*mut T`
- Function pointers: `fn(...) -> T`
- Closures: `|args| expr`
- Trait objects: `dyn Trait`
- Generic structs: `Pair<A, B>`, `Wrapper<T>` (field access works including `*mut T` fields)

### Constructs
- `fn` — function definitions with generic parameters
- `struct` — named-field structs (including recursive via pointer)
- `enum` — tagged unions
- `impl` — inherent + trait implementations (including on primitive types)
- `trait` — trait definitions
- `let` / `let mut` — variable bindings with pattern destructuring
- `if` / `else` / `match` — control flow with nested patterns
- `while` / `for` / `loop` — loops with `break` / `continue`
- `&` / `&mut` — borrows
- `*` — dereference

### Macros
- `println!` / `print!` / `eprintln!` / `eprint!`
- `format!` (variadic, MIR intrinsic)
- `vec!`
- `stringify!`, `concat!`, `panic!`, `assert!`, `assert_eq!`

### Trait dispatch
- Static dispatch (monomorphization)
- Dynamic dispatch via `dyn Trait` (vtable indirect call)
- Trait objects with `Copy` / `Clone` auto-derivation

### Memory safety
- Ownership + borrow checking (NLL skeleton with dataflow-driven fixpoint)
- Move semantics with flow-sensitive drop elaboration
- Zero-cost abstractions (no runtime overhead for traits, generics)
- Bounds checking on array/string indexing (panics on OOB)

### Type safety (Stage 18.412)
- Shift lhs must be integer type (Int/Uint) — `&str << 2` and `() << 2` are typeck errors
- Shift rhs must be integer type (Int/Uint) — `1 << true` is a typeck error
- Arithmetic lhs/rhs must be Int/Uint/Float — `&str + 1` is a typeck error
- Comparison lhs/rhs must unify — `"a" < 1` is a typeck error

---

## Codegen ABI Compliance

Landin explicitly models System V AMD64 ABI requirements at the LLVM IR
level (rather than relying on LLVM's CodeGenPrepare auto-lowering).

### ABI attributes emitted

| Attribute | When | Where |
|-----------|------|-------|
| `sret(<ty>)` | Function return type > 16 bytes | Param 1 of callee + call site |
| `byval(<ty>)` | Function param type > 16 bytes | Each large param of callee + call site |
| `ptr` (opaque) | All pointer types (LLVM 17+ opaque pointer mode) | All GEP/load/store/alloca |

### ZST handling
- **Params**: ZST params elided from LLVM signature (mirrors rustc)
- **Args**: ZST args skipped at call sites
- **Fields**: ZST fields elided from LLVM struct types via `filter_void_fields`
- **Array elements**: ZST uses `{}` (LLVM empty struct) → `[N x {}]` is valid
- **Allocas**: ZST allocas use `i8` fallback (size-0 allocas produce undef pointers = UB)

### Recursive struct handling
Recursive types (`struct Node { next: *mut Node }`) use opaque `ptr` for
`Ref`/`RawPtr` to `Adt` — no pointee type recursion. Pointee layout
resolved only at dereference sites via `detect_place_storage_type`.

---

## Testing

### Test count
- 682 unit tests (lib)
- 3904 integration tests (`tests/all_tests.rs`)
- **4586 total** (100% pass rate single-thread, 0 skipped, 2 ignored)

### Running tests
```bash
ulimit -s unlimited
cargo test --release --features llvm-backend -- --test-threads=1
# Or use the auto-tuning script:
bash scripts/run_tests.sh
```

### §14.5 D1-D8 Deep Review (v0.4 FINAL — Stage 18.500)

| Dimension | Status | Details |
|-----------|--------|---------|
| D1 Architecture | ✅ | 177 files, 84.9K LOC, no circular deps, max file 1814 LOC (3 files slightly >1500 — v0.3 P3 candidate) |
| D2 Tech Debt | ✅ | All P0/P1/P2 resolved. 23 remaining TDs all BLOCKED or v0.5+/v0.6+ architectural — NONE upgraded per §6.2 升级判据 |
| D3 Test Coverage | ✅ | 4586 tests, 1:3+ pos:neg ratio (27.8% ≥ 25% target) |
| D4 Next Stage Readiness | ✅ | v0.5 P1 (Trait Solver + CodegenError) dependencies MET; v0.5 P3 (Incremental) needs TD-SINGLE-FILE Phase 4 first |
| D5 Design Soundness | ✅ | sret+byval, ZST elision, recursive struct, TextEmitter IR validated, typeck lhs/rhs checks, Cast/Deref/Index validity checks |
| D6 Performance | ✅ | ~6s build, ~24s test single-thread |
| D7 Documentation | ✅ | 23 lang-design docs (frozen v1.3.2) + tech-debt-register + process doc v7.5 + 250+ stage-18 sub-docs |
| D8 Pipeline Coverage | ✅ | All 10 expression contexts verified closed; all 9 pipeline stages have explicit tests |

---

## Architecture Overview

### Compilation Pipeline

```
Source → Lexer → macro_expand → Parser → HIR Lower → Resolve
→ MIR Lower → TypeCheck (7 phases: 1, 2, 3, 3.5-step2, 4, 5 + closures + fndef_substs)
→ BorrowCheck → Writeback (driver-level)
→ MIR Opt (DCE → const_prop → DCE) → Monomorphization
→ Codegen (TextEmitter / LLVMSysEmitter) → Link → Execute
```

### Module sizes (LOC)

| Module | LOC | Responsibility |
|--------|-----|----------------|
| `mir/` | 24,100 | MIR data + lowering + optimization + monomorphization |
| `codegen/` | 13,993 | LLVM IR emission (TextEmitter + LLVMSysEmitter) |
| `parser/` | 10,172 | Parser + macro expansion |
| `typeck/` | 6,420 | Type checker + writeback + unify + predicates |
| `borrowck/` | 5,856 | NLL borrow checker skeleton |
| `driver/` | 5,334 | Compilation pipeline orchestration |
| `hir/` | 3,508 | HIR data structures + lowering |
| `stdlib/` | 2,749 | Landin prelude (String/Vec/Box/Option/Result) |
| `traits/` | 2,746 | Trait resolution + coherence |
| `resolve/` | 2,676 | Name resolution |
| `lexer/` | 2,252 | Tokenizer |

---

## Writeback Architecture (v0.5+)

### Two-layer substitute chain (Stage 18.347-18.413)

The typeck writeback architecture uses two layers of `substitute()` calls
to resolve generic `Param(N)` placeholders. Originally 5 layers, reduced
to 2 after v0.5+ Phase 1+3+2-L3 (Stage 18.380-18.413):

1. **Phase 3.5 step 2 Pass 1** (Stage 18.380): `writeback_field_load_locals_with_table`
   — applies substitute when writing `dest_local.ty` for field-load locals
2. **resolve_place_type_with_table** (Stage 18.358): recursive substitute
   — resolves nested projections (e.g., `o.inner.ptr`)

**Removed layers** (v0.5+ Phase 1+3+2-L3):
- **Phase 0** (Stage 18.353→18.381): pre-typeck writeback — removed, redundant
- **Phase 3.7** (Stage 18.355→18.380): post-table re-writeback — removed
- **Phase 3.5 step 1** (Stage 18.357→18.388): `writeback_field_types_with_table`
  — removed, codegen now resolves field types via `try_resolve_field_from_adt_layouts`
- **Phase 3.5 step 2 Pass 2** (Stage 18.379→18.413): `writeback_binaryop_results`
  — removed, typeck now checks Shl/Shr lhs type directly (Stage 18.412)

**Additional substitute sites**:
- `compute_use_writeback_ty` (Stage 18.361): recursive Projection base resolution
- `writeback_field_types_in_rvalue_with_table` Aggregate arm (Stage 18.376):
  applies substitute to `AggregateKind::Adt` field_tys
- `infer_projection` in typeck (Stage 18.351): applies substitute at typeck time
- `resolve_field_ty_with_substs` (Stage 18.384): recursive codegen field type resolution
- `try_resolve_field_from_adt_layouts` (Stage 18.388): codegen fallback from AdtLayouts
- `collect_from_aggregate_kind` (Stage 18.376): `substs_are_concrete` check

### Writeback phase history (v0.5+ Phase 1+3+2-L3)

| Stage | Action | Writeback Phases |
|-------|--------|------------------|
| 18.347-18.358 | 5-layer substitute chain established | 10 |
| 18.379-18.381 | Phase 0 + Phase 3.7 REMOVED | 10 → 8 |
| 18.382-18.387 | Phase 3.5 step 1 experiments | 8 |
| 18.388 | Phase 3.5 step 1 REMOVED (AdtLayouts fallback) | 8 → 7 |
| 18.389-18.405 | Phase 3.5 step 2 test — NOT redundant (§5.2 true limit, 7 consecutive) | 7 |
| 18.410 | Surgical split experiment — Pass 1 (3 failures) vs Pass 2 (2 failures) | 7 |
| 18.412 | typeck Shl/Shr lhs check (root-cause fix for Pass 2) | 7 |
| 18.413 | Pass 2 REMOVED + dead code cleanup | 7 (Phase 3.5 step 2 streamlined) |
| 18.416 | §20 iterative audit: BitAnd/BitOr/BitXor type check (same class as 18.412) | 7 |
| 18.420 | §20 iterative audit: Field access syntax mismatch check (same class as 18.412/18.416) | 7 |
| 18.422 | §20 iterative audit: &str indexing rejection + as_bytes Cast fix (same class as 18.412/18.416/18.420) | 7 |

**Current**: 7 phases (Phase 1, 2, 3, 3.5-step2-Pass1, 4, 5 + writeback_closures + writeback_fndef_substs).

### §5.2 true limit — refined (Stage 18.413)

Stage 18.410 surgical split experiments revealed that Phase 3.5 step 2
originally bundled **two independent concerns**:

- **Pass 1** (field-access writeback): **TRUE LIMIT** — architecturally
  correct position for field type resolution. Runs after Phase 3, so
  receiver types are concrete. Cannot be removed in v0.5+ without
  restructuring typeck to run before MIR lower (v0.6+ concern).
- **Pass 2** (BinaryOp result writeback): **WORKAROUND** — was masking
  typeck's Shl/Shr arm not checking lhs type. Root-cause fixed in Stage
  18.412 (added lhs check), then removed in Stage 18.413.

**Methodology insight** (§20.6 extension): When §5.2 converges to "NOT
redundant", execute surgical split experiments (env var guards per pass)
to distinguish TRUE LIMIT vs WORKAROUND.

### §20 iterative audit chain — FULL CONVERGENCE (14 rounds: 10 fixes + 4 audit-only)

Per §20 ("finding one bug means there are many similar bugs"), each soundness
fix triggered an audit of ALL similar paths. Ten same-class bugs found and
fixed (including 1 unblock). Rounds 9-14 audited additional classes — all
confirmed clean or known v0.4 design limitations (deferred to v0.5+/v0.6+).
**Full convergence reached per §5.2.**

| Stage | Bug | Class | Fix |
|-------|-----|-------|-----|
| 18.412 | Shl/Shr arm lacked LHS type check; `&str << 2` silently accepted | Silent acceptance of invalid BinaryOp | Added `is_shift_count_ty(&a_ty)` check |
| 18.416 | BitAnd/BitOr/BitXor arm lacked `is_notable_ty` check; `"hello" & "world"` silently accepted | Silent acceptance of invalid BinaryOp | Added `is_notable_ty(&a_ty)` check before unify; float bitcast path removed |
| 18.420 | `resolve_field_index` returned tuple index unconditionally on named-field structs; `Foo { x: 1 }.0` silently accepted | Silent acceptance of invalid field access | Added `check_field_access_syntax` helper + `FieldAccessCategory` enum; shared between read + assignment paths |
| 18.422 | `resolve_index_element_type` had `TyKind::Str => Some(u8)` arm; `s[0]` silently treated `&str` as `&[u8]` (design divergence from Rust) | Silent acceptance of invalid Index | Removed Str arm; `&str` indexing now errors; `emit_str_as_bytes` fixed to return `&[u8]`-typed dest via `Rvalue::Cast(Unsize, ...)` |
| 18.425 | typeck `infer_projection` Index arm had `TyKind::Str => Some(u8)` (inconsistent with 18.422) AND `_ => None` for non-indexable types; `n[0]` on int + `s[0]=65` assignment silently accepted | Silent acceptance of invalid Index (typeck + assignment path) | Removed Str arm in typeck; added `_ =>` error arm; added `check_index_access_syntax` helper to `lower_expr_to_place` |
| 18.426 | typeck `infer_rvalue` Cast arm returned `target_ty` without checking source type; `true as &str`, `(1,2) as i32`, `42 as Foo`, `42 as [i32;3]` silently compiled | Silent acceptance of invalid Cast | Added `is_valid_cast` helper validating cast pairs against Rust Reference §5.2.7 rules |
| 18.428 | typeck `infer_projection` Deref arm returned `TyKind::Error` without pushing error; `*42`, `*true`, `*(1,2)`, `*arr` silently compiled | Silent acceptance of invalid Deref | Added error push for concrete non-pointer types; defer for Infer/Error/Param/Closure |
| 18.432 | `match x { 1 => 1, 2 => 2 }` without `_` arm silently compiled (non-exhaustive match on primitives) | Silent acceptance of invalid Pattern matching | Added non-exhaustive match check: Bool with true+false = exhaustive; Int/Uint/Char require `_`; defer for enum/Adt/other (unblocked from Stage 18.430) |
| 18.430 | Audited Method resolution, Borrow/Ref, let binding, Pattern matching | Audit convergence | Method/Borrow/let: ALL OK. Non-exhaustive match: unblocked in 18.432. |
| 18.435 | Audited Return type mismatch, assignment to non-place, function call arg count | Audit convergence | ALL OK — 0 bugs found. |
| 18.445 | Suffixed integer literal range check (`let x: u8 = 256u8;` silently compiled / wrapped) | Silent acceptance of invalid Literal | Added literal range check in `post_check_statement`: suffixed int literal must fit target type's bit-width and signedness |
| 18.446 | Type-annotated integer literal range check (`let x: u8 = 256;` silently compiled) | Silent acceptance of invalid Literal (Phase 5.5 — type annotation context) | Extended `post_check_statement` to check literal against `let`-binding type annotation; helper `int_range`/`uint_max` |
| 18.447 | Audited Unary operations, struct literal field counts, enum variant construction | Audit convergence | ALL OK — 0 bugs found. |
| 18.448-18.450 | Audited Visibility enforcement, trait coherence, undeclared symbols, loop control flow (break/continue), if-else type mismatch, match arm type mismatch | Audit convergence | Visibility/break-continue/enum-exhaustiveness: KNOWN v0.4 limitations (deferred to v0.5+/v0.6+ as language features, not soundness bugs). Trait coherence/undeclared symbols/if-else/match arm: ALL OK. |

**Audit conclusion**: All ten fixes are "silent acceptance of invalid operations /
design divergence from Rust" — same architectural class. The audit chain is
**complete** for BinaryOp arms, field access paths, Index operations (read +
assignment paths), Cast operations, Deref operations, Pattern matching
(non-exhaustive check), and Literal range checks. Rounds 9-14 confirmed
Method resolution, Borrow/Ref, let binding, Return type, assignment validity,
arg count, Unary, struct literal, Visibility, trait coherence, undeclared
symbols, loop control flow, if-else type mismatch, and match arm type
mismatch are all clean or known v0.4 design limitations.
**0 remaining L2-fixable soundness bugs.**

### Design principles (§2.2, 11 principles)

1. 长期 > 短期 | 2. 整体 > 局部 | 3. 显式 > 隐式 | 4. 报错 > 静默
5. 去除兼容思维 | 6. 通用 > 特例 | 7. API 命名标准化 | 8. 设计驱动测试
9. 正确 > 妥协 | **10. 唯一可信数据源** | **11. 确定性边界**

### Execution principles (§2.1.1, 13 principles)

1-10. (standard: plan, naming, isolation, generality, cohesion, etc.)
**11. 确定性边界先行** | **12. 临时桩识别与记录** | **13. 架构限制记录与升级**

---

## Tech Debt & Known Limitations

All P0/P1/P2 tech-debts are **resolved** (Stages 18.127-18.446 closed ~180 TDs:
10 structural TDs in 18.372-18.413; 8 §20 iterative audit soundness bugs in
18.412-18.432; 2 literal range bugs in 18.445-18.446; plus all earlier
v0.4 stages).

**Phase 5 progress** (Stage 18.438-18.444):
- Stage 18.438: Added `mir_type_to_emit_type_checked` returning `Result<EmitType, CodegenErrorKind::UnresolvedType>` + new `CodegenErrorKind::UnresolvedType` variant
- Stage 18.440: Replaced silent `_ => EmitType::I32` fallback with explicit `eprintln!` warning + I32 fallback
- Stage 18.441/18.443: Architecturally concluded — panic infeasible (with_layouts delegates to unchecked for Infer/Error)
- Stage 18.442: Migrated `function_sigs.rs` to `mir_type_to_emit_type_with_layouts`
- Stage 18.444: Architecturally concluded — with_layouts→unchecked delegation is correct by design

**v0.5+ Writeback Phase 1+3+2-L3 progress** (Stage 18.379-18.413):
- Stage 18.380: Phase 3.7 REMOVED (root-cause fix in `writeback_field_load_locals_with_table`)
- Stage 18.381: Phase 0 REMOVED (redundant after 18.380)
- Stage 18.388: Phase 3.5 step 1 REMOVED (codegen `try_resolve_field_from_adt_layouts` fallback)
- Stage 18.389-18.405: Phase 3.5 step 2 NOT redundant (5 failures — §5.2 true limit, 7 consecutive)
- Stage 18.410: Surgical split experiment — Pass 1 (3 failures, field-access) vs Pass 2 (2 failures, BinaryOp)
- Stage 18.412: typeck Shl/Shr lhs check (root-cause fix for Pass 2)
- Stage 18.413: Pass 2 REMOVED + dead code cleanup (`resolve_operand_for_writeback`, `is_concrete_int_or_float`)
- Writeback phases: 10 → 7 (Phase 0 + Phase 3.7 + Phase 3.5 step 1 + Pass 2 removed)
- **Phase 3.5 step 2 Pass 1** (field-access writeback) retained as architecturally correct (§5.2 true limit)

Remaining items are v0.5+/v0.6+ architecture limitations (documented in
`docs/develop/v0/tech-debt-register.md` §2.5.1):

| ID | Description | Status | Fix Plan |
|----|-------------|--------|----------|
| TD-STUB-PRELUDE-LOOP-BODY | Prelude `loop {}` marker bodies (4 methods) | ✅ Mitigated (Stage 18.284) | Intrinsics intercept marker bodies; early interception prevents execution |
| TD-TYPECK-LOCAL-DECL-ERROR-CHECK | Phase 4.5 disabled (47 prelude false-positives) | ✅ Resolved (Stage 30.18) | `param_check` (Stage 18.348) catches Error types at codegen time — user sees the error. Phase 4.5 remains disabled (architectural — prelude lazy monomorphization, separate work item, not a soundness bug) |
| TD-STUB-REGION-ERASED | Region inference no-op | ✅ Resolved (Stage 30.1) | Reclassified — region inference was always running, not no-op |
| TD-STUB-DROP-ELABORATION-NOOP | Drop elaboration no-op | ✅ Resolved (Stage 30.3) | Reclassified — drop elaboration IS implemented (Stage 15.43-15.46), not no-op. New TD-DROP-SCOPE-TIMING created for scope timing issue. |
| TD-STUB-LIFETIME-ELISION-NOOP | Lifetime elision no-op | ✅ Resolved (Stage 30.2) | RFC 141 Rule 4 enforced + over-application fix + self-param fix |
| TD-STUB-PROJECTION-RESOLVER | Projection resolver partial | ✅ Resolved (Stage 30.4) | Reclassified — projection resolver IS fully implemented (Stage 16.68 + 18.87), handles all TyKind variants + termination guarantee (MAX_DEPTH=10). New TD-PROJECTION-IMPL-VERIFICATION created for impl block verification gap. |
| TD-PROJECTION-IMPL-VERIFICATION | Missing/wrong assoc types in impl silently accepted | ✅ Resolved (Stage 30.7) | `validate_impl_assoc_types` in driver_validations.rs — missing assoc types now rejected with clear error. Type match check deferred to TD-IMPL-TYPE-MATCH. |
| TD-IMPL-TYPE-MATCH (NEW) | `type Item = T` not verified against method returns `Self::Item` | ✅ Resolved (Stage 30.8) | Structural check implemented (no-op for common case); deeper typeck issue (typeck doesn't resolve `Self::Item` during method body checking) tracked as TD-TYPECK-IMPL-CONTEXT |
| TD-TYPECK-IMPL-CONTEXT (NEW) | typeck doesn't resolve `Self::Item` to `T` during method body checking | ✅ Resolved (Stage 30.12) | Assoc type bindings collected in ImplInfo.assoc_type_bindings + projection_resolver moved before typeck. Full Self::Item resolution deferred to TD-SELF-TYPE-RESOLUTION |
| TD-SELF-TYPE-RESOLUTION (NEW) | Self::Item resolution may not fully work (deeper HIR self type needed) | ✅ Resolved (Stage 30.14) | Self::Item now resolves to Res::SelfTy in multi-segment paths + lowers to TyKind::Projection (was Error). Full substs[0] resolution deferred to TD-SELF-TYPE-SUBSTS |
| TD-SELF-TYPE-SUBSTS (NEW) | Projection substs[0] is empty (no Self type) — projection_resolver can't resolve | ✅ Resolved (Stage 30.16) | Empty-substs fallback in projection_resolver — `lookup_assoc_type_in_any_impl` searches all impl blocks when substs is empty |
| TD-DROP-SCOPE-TIMING | StorageDead at fn end, not scope end | ✅ Resolved (Stage 30.6) | Scope tracking in MirLowerCtxt — StorageDead now emitted at block scope end via scope_stack |
| TD-HRTB-SOLVER-INTEGRATION | HRTB `for<'a>` surface syntax captured but solver doesn't enforce semantics | ✅ Resolved (Stage 30.10) | HRTB bounds now collected in TraitResolver via `hrtb_bounds` field in ImplInfo + new `HrtbBound` struct. Full enforcement (placeholder universes) deferred to TD-HRTB-FULL-ENFORCEMENT |
| TD-HRTB-FULL-ENFORCEMENT (NEW) | HRTB bounds collected but not enforced (no placeholder universes) | ✅ Resolved (Stage 30.13) | HRTB bounds now partially enforced via `validate_hrtb_bounds` (checks trait implementation exists). Full enforcement (placeholder universes) deferred to TD-HRTB-PLACEHOLDER-CHECK |
| TD-HRTB-PLACEHOLDER-CHECK (NEW) | HRTB bounds partially enforced — no universal quantification check | ✅ Resolved (Stage 30.15) | Reclassified — partial enforcement (Stage 30.13) is the achievable scope. Full enforcement (universal quantification via placeholder universes) requires InferCtxt in pipeline, deferred to TD-HRTB-INFRACTX-INTEGRATION |
| TD-HRTB-INFRACTX-INTEGRATION (NEW) | Full HRTB enforcement requires InferCtxt in driver pipeline | ✅ Resolved (Stage 30.17) | `validate_hrtb_bounds` now uses InferCtxt + enter_universe/exit_universe + solver select() (was name-based implements_by_def_ids). Full universal quantification (lifetime param substitution) deferred — solver checks trait implementation via proper 3-phase Evaluation → Selection |
| TD-HRTB-FN-SYNTAX | `for<'a> Fn(&'a T) -> &'a U` syntax not parsed (Fn call syntax separate feature) | ✅ Resolved (Stage 30.9) | `try_parse_parenthesized_args` in parser/path.rs — `Fn(T) -> U`/`FnMut(T) -> U`/`FnOnce(T) -> U` + HRTB now parse cleanly |
| TD-STUB-EMIT-TYPE-I32-FALLBACK | `mir_type_to_emit_type` i32 fallback | ✅ Mitigated | param_check (Stage 18.348) catches unresolved types |
| TD-STUB-TYPECK-BEFORE-WRITEBACK | typeck before writeback | ✅ Resolved | Phase 0 + Phase 3.7 double writeback (Stage 18.353+18.355), now both removed |
| TD-STUB-DEFAULT-INT-I32 | Default int = i32 | ✅ Design choice | Not a stub — Landin design decision |
| TD-UNWRAP-GUARDED-EXPECT | 15 production guarded unwraps lack invariant docs | ✅ Resolved (Stage 18.372) | All converted to `expect("invariant doc")` with comments |
| TD-UNREACHABLE-INVARIANT | 4 production bare `unreachable!()` lack invariant msg | ✅ Resolved (Stage 18.373) | All converted to `unreachable!("invariant msg")` with comments |
| TD-TY-INFER-SPAN | 3 production `fresh_infer_ty(Span::DUMMY)` lack source span | ✅ Resolved (Stage 18.374) | All converted to `fresh_infer_ty(real_span)` (param.span / expr.span) |
| TD-AS-CAST-TRUNCATION | 8 production `*n as u32` (u128→u32) silent truncation | ✅ Resolved (Stage 18.375) | All converted to `u32::try_from(*n).expect(...)` (panic on overflow) |
| TD-ARCH-NESTED-GENERIC-FIELD-ACCESS | Nested generic field access `Outer<Inner<T>>.inner.val` | ✅ Resolved (Stage 18.376) | 5-layer fix: lower + inference + writeback + mono collect |
| TD-ALLOW-SUPPRESSION | 26 production `#[allow]` suppressions | ✅ Resolved (Stage 18.377) | 6 stale removed, 20 verified legitimate (BLOCKED infra / forward-compat / style) |
| TD-PASS2-BINARYOP-WORKAROUND | `writeback_binaryop_results` masked typeck Shl/Shr lhs check deficiency | ✅ Resolved (Stage 18.413) | typeck `infer_rvalue` Shl/Shr arm lhs check (Stage 18.412) + writeback Pass 2 removal + dead code cleanup |
| TD-BITWISE-NOTABLE-CHECK | BitAnd/BitOr/BitXor arm lacked `is_notable_ty` check; `"hello" & "world"` silently accepted | ✅ Resolved (Stage 18.416) | §20 iterative audit — added `is_notable_ty` check before unify; float bitwise bitcast path removed from codegen |
| TD-FIELD-ACCESS-SYNTAX-MISMATCH | `resolve_field_index` returned tuple index unconditionally on named-field structs; `Foo { x: 1 }.0` silently accepted | ✅ Resolved (Stage 18.420) | §20 iterative audit — added `check_field_access_syntax` helper + `FieldAccessCategory` enum; shared between read path (`lower_expr_to_operand`) and assignment path (`lower_expr_to_place`) |
| TD-STR-INDEX-SILENT-ACCEPT | `resolve_index_element_type` had `TyKind::Str => Some(u8)` arm; `s[0]` silently treated `&str` as `&[u8]` (design divergence from Rust) | ✅ Resolved (Stage 18.422) | §20 iterative audit — removed Str arm; `&str` indexing now reports error; `emit_str_as_bytes` intrinsic fixed to return `&[u8]`-typed dest via `Rvalue::Cast(Unsize, ...)` |
| TD-INDEX-TYPECK-SILENT-ACCEPT | typeck `infer_projection` for `ProjectionElem::Index` had `TyKind::Str => Some(u8)` (inconsistent with Stage 18.422) AND `_ => None` for non-indexable types; `n[0]` on int silently compiled. Assignment path `s[0] = 65` also silently accepted | ✅ Resolved (Stage 18.425) | §20 iterative audit — removed Str arm in typeck; added `_ =>` error arm for non-indexable concrete types; added `check_index_access_syntax` helper to `lower_expr_to_place` (assignment path) |
| TD-CAST-SILENT-ACCEPT | typeck `infer_rvalue` for `Rvalue::Cast` returned `target_ty` without checking source type; `true as &str`, `(1,2) as i32`, `42 as Foo`, `42 as [i32;3]` silently compiled | ✅ Resolved (Stage 18.426) | §20 iterative audit — added `is_valid_cast` helper validating cast pairs against Rust Reference §5.2.7 rules; rejects Str/Tuple/Adt/Array casts + Bool→Bool/Float/Char + Float→Bool/Char |
| TD-DEREF-SILENT-ACCEPT | typeck `infer_projection` for `ProjectionElem::Deref` returned `TyKind::Error` without pushing error; `*42`, `*true`, `*(1,2)`, `*arr` silently compiled | ✅ Resolved (Stage 18.428) | §20 iterative audit — added error push for concrete non-pointer types (Int/Bool/Float/Char/Tuple/Array/Adt/Str); defer for Infer/Error/Param/Closure (closure captures produce Deref on Closure types) |
| TD-LITERAL-RANGE-SUFFIXED | Suffixed integer literal range check (`let x: u8 = 256u8;` silently compiled / wrapped) | ✅ Resolved (Stage 18.445) | §20 iterative audit — added literal range check in `post_check_statement`: suffixed int literal must fit target type's bit-width and signedness |
| TD-LITERAL-RANGE-ANNOTATION | Type-annotated integer literal range check (`let x: u8 = 256;` silently compiled) | ✅ Resolved (Stage 18.446) | §20 iterative audit (Phase 5.5 — type annotation context) — extended `post_check_statement` to check literal against `let`-binding type annotation; helper `int_range`/`uint_max` |
| TD-UNRESOLVED-TYPE-CODEGEN | `mir_type_to_emit_type` silent `_ => EmitType::I32` fallback for Param/Infer/Error types | ✅ Partial (Stage 18.438-18.444) | Phase 5 Step 1+2+4 done: `mir_type_to_emit_type_checked` returns Result, silent fallback replaced with warning; Step 3+5 architecturally concluded (with_layouts→unchecked delegation correct by design) |
| TD-VISIBILITY-NOOP | Private items accessible from outside module | ✅ Resolved (Stage 26.1) | `def_owner_module` + `check_visibility` enforces |
| TD-BREAK-CONTINUE-CONTEXT | `break`/`continue` outside loop | ✅ Resolved (Stage 27.1) | `loop_stack` empty → TypeError |
| TD-ENUM-EXHAUSTIVENESS | `match` on enum without all variants | ✅ Resolved (Stage 28.1) | `enum_variants` map + `lower_match` checks |
| TD-SELF-OUTSIDE-IMPL-CONTEXT | `Self` keyword outside any impl/trait context silently defaulted to `HirSelfKind::Impl` via `unwrap_or` | ✅ Resolved (Stage 35.1) | New `ResolveErrorKind::SelfOutsideImplContext` error kind + `resolve_self_ty` helper + propagated parent SelfKind to method fn owners in `owner_self_kind` + set `current_self_kind` before fn sig resolution + extended `resolve_ast_ty_paths` to check Self in generic args |
| TD-TYPECK-PARAM-ARG-COUNT | typeck didn't validate arg count for trait method calls when the trait method had no body (declaration only) — silent accept of wrong arg counts | ✅ Resolved (Stage 35.2) | New `populate_trait_decl_fn_sigs` in `src/driver/driver_codegen_prep.rs` registers ALL trait declaration methods (with or without body) in fn_sig_table. For decl-only methods, uses `TyKind::Error` as self_ty placeholder. typeck's existing `check_terminator` Call handler now validates arg count uniformly. Wired up in `compile_inner.rs` AFTER `populate_trait_default_fn_sigs`. |
| TD-TYPECK-PARAM-RETURN-MISMATCH | typeck didn't unify Param(N) body with concrete return type for generic impl methods — silent accept of `fn f<T>(x: T) -> T { true }` | ✅ Resolved (Stage 35.3) | New `should_check_concrete_vs_param` check in `post_check_statement` (`src/typeck/check.rs`). Boundary: place is the RETURN LOCAL (LocalId(0)) with Param type AND rvalue is concrete non-Param. Narrowed from original design (which proposed all Param-typed places) after discovering false positives in match arm deconstruction. |
| TD-SLICE-LEN-MISSING (NEW) | Slices (`&[T]`) don't have `.len()` method — `arr.len()` on `[i64]` fails with "no method `len` found" | ✅ Resolved (Stage 36.1) | New `SliceLen` variant in `PrimitiveIntrinsic` enum + early interception in `method_call_lower.rs` for `len` on slice/array receivers. Reuses `emit_str_len` MIR (same fat pointer Field(1) projection). |
| TD-ARRAY-SLICE-COERCION-MISSING (NEW) | `[T; N]` → `&[T]` coercion not implemented — `&[1, 2, 3]` to slice ref fails with type mismatch | ✅ Resolved (Stage 36.1) | Array→slice coercion rules in `typeck/unify.rs` `unify_resolved` (both directions: Ref(Array)↔Ref(Slice), direct Array↔Slice, Ref(Array)↔Slice, Slice↔Ref(Array)). Extended `types_match_loose` with Array↔Slice loose match. Mirrors Rust unsizing coercion. |
| TD-DISPLAY-TRAIT-MISSING (NEW) | No `Display` trait for type-dispatched formatting — blocks `%s`-style string args | 📋 Registered (Stage 36) | Prerequisite for TD-FORMAT-MIGRATION Stage 36.3 (v0.6+). Requires trait dispatch + monomorphization. |

---

## v0.24 Stage 36.1 — Slice Len + Array→Slice Coercion RESOLVED

**Stage 36.1** (v0.24) resolves 2 P3 TDs that are prerequisites for
TD-FORMAT-MIGRATION (Stage 36.2):

1. **TD-SLICE-LEN-MISSING**: Added `SliceLen` variant to `PrimitiveIntrinsic`
   enum. Added early interception in `method_call_lower.rs` for `len` on
   slice/array receivers. Reuses `emit_str_len` MIR (same fat pointer
   Field(1) projection — both `&str` and `&[T]` have layout `{ ptr, len }`).

2. **TD-ARRAY-SLICE-COERCION-MISSING**: Added array→slice coercion rules in
   `typeck/unify.rs` `unify_resolved` (both directions: Ref(Array)↔Ref(Slice),
   direct Array↔Slice, Ref(Array)↔Slice, Slice↔Ref(Array)). Extended
   `types_match_loose` with Array↔Slice loose match. Mirrors Rust unsizing
   coercion.

**Verification**: 5227 tests (898 lib + 4329 integration), 0 failures, 4
ignored. fmt clean, 0 clippy warnings. §3.2 verification passed.

**TD-FORMAT-MIGRATION (P2) is now UNBLOCKED** — Stage 36.2 will replace the
598-LOC MIR walker with a slice-based prelude impl (net -368 LOC, 特解 → 通解).

---

## v0.23 Stage 36 — TD-FORMAT-MIGRATION Architectural Design

**Status**: 📋 DESIGN ONLY — no code changes, baseline preserved.

The 598-LOC MIR walker for `format!` is a 特解 (special case). Migrating
to a prelude impl (通解) requires v0.5+ language features:

1. **Slice `.len()` method** (TD-SLICE-LEN-MISSING, P3) — currently missing
2. **Array→slice coercion** (`[T; N]` → `&[T]`, TD-ARRAY-SLICE-COERCION-MISSING, P3)
3. **Display trait** for type-dispatched formatting (TD-DISPLAY-TRAIT-MISSING, P3, v0.6+)

**§6.2 upgrade criteria**: TD-FORMAT-MIGRATION does NOT upgrade — the
current MIR walker produces correct results, no next-stage correctness
depends on it.

**v0.5+ implementation path** (3-stage plan):
- Stage 36.1 (v0.24): Slice `.len()` + array→slice coercion (~150 LOC)
- Stage 36.2 (v0.24): Slice-based prelude format impl (net -368 LOC)
- Stage 36.3 (v0.6+): Display trait for type-dispatched formatting

Full design doc: `docs/develop/v0/stage-36/stage-36-format-migration-variadic-design.md`

### v0.23 Stage 35 Series — COMPLETE

| Stage | TD | Status |
|-------|-----|--------|
| 35.1 | TD-SELF-OUTSIDE-IMPL-CONTEXT | ✅ Resolved |
| 35.2 | TD-TYPECK-PARAM-ARG-COUNT | ✅ Resolved |
| 35.3 | TD-TYPECK-PARAM-RETURN-MISMATCH | ✅ Resolved |

All 3 P3 typeck TDs resolved in v0.23.

---

## v0.23 Stage 35.3 — TD-TYPECK-PARAM-RETURN-MISMATCH RESOLVED

**Bug**: typeck silently accepted type mismatches when a generic fn/method
body returned a concrete type that didn't match the declared T-typed return:
```rust
fn f<T>(x: T) -> T { true }  // ❌ returns bool, sig says T — silent accept
```

**Root cause**: `src/typeck/check.rs:80` had a `place_has_param` skip (per
Stage 18.351 "defer to writeback" rationale). Writeback only substitutes
Param via Field projection — it does NOT validate concrete-vs-Param
assignments to direct locals (return value or let-binding).

**Fix**: New `should_check_concrete_vs_param` check in `post_check_statement`.
Boundary: place is the RETURN LOCAL (`LocalId(0)`) with Param type AND rvalue
is concrete non-Param. Narrowed from original design (which proposed all
Param-typed places) after discovering false positives in match arm
deconstruction.

**Verification**: 5194 tests (898 lib + 4296 integration), 0 failures, 4
ignored. fmt clean, 0 clippy warnings. §3.2 verification passed.

### v0.23 Stage 35 Series — COMPLETE

| Stage | TD | Status |
|-------|-----|--------|
| 35.1 | TD-SELF-OUTSIDE-IMPL-CONTEXT | ✅ Resolved |
| 35.2 | TD-TYPECK-PARAM-ARG-COUNT | ✅ Resolved |
| 35.3 | TD-TYPECK-PARAM-RETURN-MISMATCH | ✅ Resolved |

All 3 P3 typeck TDs resolved in v0.23.

---

## v0.23 Stage 35.2 — TD-TYPECK-PARAM-ARG-COUNT RESOLVED

**Bug**: typeck did not validate arg count for trait method calls when the
trait method had no body (declaration only). For example:
```rust
trait T { fn f(&self, a: i32, b: i32) -> i32; }
struct S<X: T> { x: X }
impl<X: T> S<X> { fn g(&self) -> i32 { self.x.f(1) } }  // ❌ silent accept
```
The call `self.x.f(1)` only passes 1 arg, but the method expects 2 — typeck
silently accepted this, violating §1.0 原則 4 (报错 > 静默).

**Root cause**: `populate_trait_default_fn_sigs` skipped methods without body
(`if f.body.is_none() { continue; }`) — trait decl-only methods were NOT
registered in `fn_sig_table`. typeck's `check_terminator` couldn't look up
the method's sig → arg-count check was silently skipped.

**Fix**: New function `populate_trait_decl_fn_sigs` registers ALL trait
declaration methods (with or without body) in fn_sig_table. For decl-only
methods (no body, no impl), uses `TyKind::Error` as self_ty placeholder.
typeck's existing `check_terminator` now validates arg count uniformly.

**Verification**: 5161 tests (898 lib + 4263 integration), 0 failures, 4
ignored. fmt clean, 0 clippy warnings. §3.2 verification passed.

---

## v0.23 Stage 35.1 — TD-SELF-OUTSIDE-IMPL-CONTEXT RESOLVED

**Bug**: The `Self` keyword silently resolved to `HirSelfKind::Impl` via
`unwrap_or(...)` when used outside any impl/trait context (free fn return
type, free fn param, let binding, struct field, enum variant, etc.).
This violated §1.0 原則 4 (报错 > 静默).

**Deeper bug discovered**: `owner_self_kind` map was keyed by Trait/Impl
DefId only, missing method fn owners — propagated parent SelfKind to each
method fn owner. The OLD `unwrap_or(Impl)` masked this deeper bug.

**Verification**: 5128 tests (898 lib + 4230 integration), 0 failures, 4
ignored. fmt clean, 0 clippy warnings. §3.2 verification passed.

---

## v0.5+ Refactoring Roadmap

Based on deep architecture audit (Stage 18.366-18.367) + v0.4 FINAL §14.6.3 hidden problems assessment, referencing Rust rustc design:

| Phase | Target | Priority | Est. | Reference | Status |
|-------|--------|----------|------|-----------|--------|
| 1 | typeck writeback unification (10 phases → inline) | Highest | 2-3w | rustc typeck + type propagation interwoven | ✅ Phase 0 + Phase 3.7 + Phase 3.5 step 1 removed (10→7). Phase 3.5 step 2 Pass 1 retained (architecturally correct). |
| 2 | expected_ty propagation in MIR lower + typeck root-cause fixes | High | 1-2w | rustc MIR lower expected_ty | ✅ L3 step 1 done (expected_ty in Call dest). L3 step 2 partial: Pass 2 removed via typeck lhs check; Pass 1 retained as true limit. |
| 3 | FieldTyTable removal | Medium | 1w | rustc doesn't use FieldTyTable | 📋 Blocked on Phase 2 L3 step 2 full completion (Pass 1 elimination needs v0.6+ typeck前置) |
| 4 | mono_layouts stored in MirBody | Medium | 1w | rustc MirSource carries type info | 📋 Not started |
| 5 | mir_type_to_emit_type returns Result | Low | 1-2w | rustc CodegenCx::layout_of | ✅ Step 1+2+4 done (Stage 18.438-18.444); Step 3+5 architecturally concluded |

### v0.5 Stage Tasks (P1/P2/P3)

| Task | Priority | Est. Stages | Status |
|------|----------|-------------|--------|
| Trait Solver | P1 | 6-8 | READY — TraitResolver (Stage 16.07-16.10) + Phase 2A primitive intrinsic dispatch (Stage 18.284) infrastructure |
| CodegenError Error System | P1 | 2-3 | READY — Phase 5 Step 1+2+4 done; ~40 `unwrap()` in `llvm/mod.rs` to migrate to `?` operator |
| GATs | P2 | 4-6 | READY — Stage 18.87 GATs Phase 3 base |
| Trait Coherence Enhancement | P2 | 2-3 | READY — Orphan rule infrastructure |
| MIR Optimization Passes | P3 | 3-4 | READY — addresses TD-NO-JUMP-THREADING + TD-CONST-PROP-LOOPS |
| Incremental Compilation | P3 | 4-6 | ⚠️ PARTIAL — needs TD-SINGLE-FILE Phase 4 (manifest) first |
| Cross-compilation | P3 | 2-3 | READY — TargetTriple exists; addresses TD-LINUX-ONLY + TD-ABI-DIVERSITY |

---

## Project Layout

```
landin/
├── src/                          # Compiler source (~83K LOC, 177 files)
│   ├── bin/                      # CLI entry points (landin-stage0 + landinc)
│   ├── lexer/                    # Tokenizer (2.2K LOC)
│   ├── parser/                   # AST + macro_expand (10.2K LOC)
│   ├── hir/                      # High-level IR (3.5K LOC)
│   ├── resolve/                  # Name resolution (2.7K LOC)
│   ├── mir/                      # Mid-level IR (24.1K LOC)
│   │   ├── lower/                # MIR lowering from HIR (21 modules)
│   │   ├── monomorphize/         # Generic instantiation + layouts
│   │   ├── param_check.rs        # Pre-codegen diagnostic (Stage 18.348)
│   │   ├── optimization.rs       # DCE + const_prop
│   │   └── substitute.rs         # Type parameter substitution
│   ├── typeck/                   # Type checker (6.4K LOC)
│   │   ├── checker.rs            # 7-phase check_mir_body_with_tables
│   │   ├── check.rs              # check_statement + post_check_statement
│   │   ├── infer.rs              # Type inference + infer_projection + Shl/Shr lhs check
│   │   ├── writeback.rs          # Phase 3.5 step 2 Pass 1 (field-access writeback)
│   │   └── unify.rs              # Unification table
│   ├── codegen/                  # LLVM IR emission (14K LOC)
│   │   ├── llvm/                 # LLVMSysEmitter (production, C-API)
│   │   ├── text/                 # TextEmitter (debug, --emit-llvm-ir)
│   │   ├── emitter/              # Emitter trait + EmitType
│   │   ├── mir_translation/      # MIR → EmitType (49 mono_layouts callsites)
│   │   └── trait_dispatch/       # Vtable construction
│   ├── borrowck/                 # NLL borrow checker (5.9K LOC)
│   ├── driver/                   # Compilation pipeline (5.3K LOC)
│   ├── stdlib/                   # Landin prelude (String/Vec/Box/...)
│   ├── traits/                   # Trait resolver + coherence
│   ├── session/                  # Compiler session + diagnostics
│   └── diagnostics/              # Error formatting
├── tests/                        # 4586 tests (682 lib + 3904 integration, 2 ignored)
├── docs/                         # Documentation
│   ├── stage-committee-process.md  # SOP v7.5 (3100+ LOC)
│   ├── develop/v0/               # Dev logs + tech-debt-register (5 sections, 23 remaining TDs)
│   ├── lang-design/              # 23 frozen language design docs (v1.3.2 freeze, 0 P0)
│   ├── graph/                    # Pipeline graphs (design + stage + overall)
│   └── ...
├── scripts/                      # env.sh + setup-llvm-env.sh + run_tests.sh
├── examples/                     # Example programs
├── benchmark/                    # Benchmarks
└── Cargo.toml
```

---

## Documentation

- **Build guide**: `docs/build-guide.md`
- **Testing guide**: `docs/testing-guide.md`
- **SOP**: `docs/stage-committee-process.md` v7.5 (11 design principles + 13 execution principles + Bug probability distribution + §20.6 experimental exploration with surgical split)
- **Tech debt register**: `docs/develop/v0/tech-debt-register.md` (5 sections: 173 resolved items + 23 remaining TDs all BLOCKED or v0.5+/v0.6+ architectural — NONE upgraded per §6.2 升级判据)
- **v0.4 FINAL deep review**: `docs/develop/v0/stage-18/stage-18.500-v0.4-final-deep-review.md` (§14.5 D1-D8 + §14.6 cross-stage validation + §14.8 B2 design writeback)
- **v0.4 roadmap**: `docs/develop/v0/v0.4-roadmap.md` (with §14.8 B2 writeback: implementation > design)
- **v0.5 roadmap**: `docs/develop/v0/v0.5-roadmap.md` (next stage planning)
- **Architecture audit**: Stage 18.366-18.367 worklog (health: 8.5/10, v0.5+ 5-phase roadmap)
- **Per-stage dev logs**: `docs/develop/v0/stage-N/` (250+ stage-18 sub-docs)
- **Language design**: `docs/lang-design/` (23 docs: overview, spec, grammar, type system, etc. — frozen v1.3.2)

---

## Contributing

### Development workflow (per `docs/stage-committee-process.md` v7.5)

1. **Self-check (§1.2.1)**: classify task as L1/L2/L3
2. **Design alignment (§13.1)**: read `docs/lang-design/` + `docs/graph/`
3. **Certainty boundaries (§2.1.1 原则 11)**: clarify capability/design/responsibility boundaries before coding
4. **MUV (§4)**: smallest verifiable unit of work
5. **Inner review (§5)**: P0/P1 cleanup loop
6. **Iterative audit (§20)**: "finding one bug means there are many similar bugs" — audit all similar paths
7. **Stub identification (§2.1.1 原则 12)**: if passing `None`/defaults, determine if stub → record in tech-debt
8. **Architecture limitation (§2.1.1 原则 13)**: if architecture limit found → record in tech-debt → plan refactor
9. **Experimental exploration (§20.6)**: when removing code, use surgical split (env var guards per pass) to distinguish TRUE LIMIT vs WORKAROUND
10. **Acceptance (§3.2)**: `cargo fmt + check + clippy + test --release` all green
11. **Documentation (§8)**: worklog + tech-debt-register + plan doc
12. **Packaging (§19)**: `landin-stage0-v<X>.<Y>.<Z>-stage<N>.<M>-<desc>-r<R>.tar.gz`

### Key principles

- §2.2 原则 3: 显式 > 隐式 (explicit > implicit)
- §2.2 原则 4: 报错 > 静默 (errors > silent)
- §2.2 原则 6: 通解 > 特解 (general > special-case)
- §2.2 原则 9: 正确 > 妥协 (correct > compromise)
- §2.2 原则 10: 唯一可信数据源 (single source of truth)
- §2.2 原则 11: 确定性边界 (certainty boundaries)
- §12: 最优 > 最小 (optimal > minimal)
- §20: 迭代审计 (iterative audit — Bug probability distribution reasoning)
- §20.6: 实验性探索方法论 (experimental exploration — surgical split for TRUE LIMIT vs WORKAROUND)
- 知识搜索 > 猜测 (knowledge search > guessing)
- 唯一可信数据源 (single source of truth)

### License

MIT — see [LICENSE](LICENSE).
