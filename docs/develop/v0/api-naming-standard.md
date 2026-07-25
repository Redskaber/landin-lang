# Landin Compiler API Naming Standard (Stage 0-3)

> **Effective from**: Stage 3.63 (2026-07-22)
> **Process ref**: stage-committee-process.md v3.15 §23
> **Scope**: All Rust source under `src/`, all stages (0-3 + future 4+)
> **Purpose**: Single source of truth for naming conventions across the
> Landin compiler pipeline. Any new code MUST conform to this standard;
> any deviation MUST be documented in the worklog with a justification.

---

## 1. Why a Standard?

The Landin compiler grew organically across 30+ gate review rounds and
4 stages. As a result, naming conventions drifted:

- Some stage modules used glob re-exports (`pub use X::*;`) while others
  used explicit lists. (Stage 3.57 fixed hir/mir; Stage 3.63 fixed ast/lexer.)
- The HIR lowering context was named `LowerCtxt` while the MIR lowering
  context was named `MirLowerCtxt` — asymmetric prefixes. (Stage 3.63
  renamed `LowerCtxt` → `HirLowerCtxt`.)
- `BorrowKind` was defined twice — in `mir::lvalue` and `borrowck::borrow_set`
  — with a manual conversion function and a `BkKind` alias. (Stage 3.63
  unified to a single definition in `mir::lvalue`.)
- `DefKind` was defined in `resolve::module_tree` but consumed by the HIR
  type `Res::Def(DefId, DefKind)` — backwards dependency direction.
  (Stage 3.63 moved `DefKind` to `hir::kinds`.)
- `check_crate` was claimed deprecated in the Stage 3.62 worklog but the
  code showed full working implementations — process-vs-code drift.
  (Stage 3.63 marked both `typeck::check_crate` and `borrowck::check_crate`
  as `#[deprecated]` with notes pointing to §16-compliant replacements.)
- `fat_ptr_type` lacked the `emit_` prefix used by sibling translation
  functions. (Stage 3.63 renamed to `emit_fat_ptr_type`.)

This document captures the conventions established by the Stage 3.63
cross-stage naming standardization round (per §21 audit findings) so
that future contributors don't reintroduce these inconsistencies.

---

## 2. Entry-Point Convention

### 2.1 Free-function pattern

Each stage exposes a **free-function entry point** with the pattern:

```rust
pub fn <verb>_<noun>(<data>: &<Type>, ...) -> <ReturnType>
```

Where:
- `<verb>` is the stage's primary action: `tokenize`, `parse`, `lower`,
  `resolve`, `check`, `codegen`.
- `<noun>` is the data being processed: `src`, `crate`, `body`, `mir`.
- The first parameter is always the **data being consumed** (not a context).

### 2.2 Canonical entry points (Stage 0-3)

| Stage | Entry | Signature |
|-------|-------|-----------|
| 0 lexer | `lexer::tokenize` | `(src: &str, interner: &mut Rodeo) -> (Vec<Token>, Vec<LexError>)` |
| 0 parser | `parser::parse_crate` | `(tokens: Vec<Token>, interner: &mut Rodeo) -> (Crate, Vec<ParseError>)` |
| 1.2 HIR lower | `hir::lower::lower_crate` | `(ast: &ast::Crate, interner: &Rodeo) -> HirCrate` |
| 1.3 resolve | `resolve::resolve_crate` | `(hir: &mut HirCrate, interner: &mut Rodeo) -> Vec<ResolveError>` |
| 2.1 MIR lower | `mir::lower::lower_hir_body_to_mir_full` | `(body: &Body, interner: &Rodeo, hir: &HirCrate, ret_ty: Option<Ty>) -> (MirBody, UnificationTable)` |
| 2.2 typeck | `TypeChecker::check_mir_body_with_tables` | `(&mut self, mir: &mut MirBody, field_ty_table: Option<&FieldTyTable>)` — **§16-compliant canonical** |
| 2.3 borrowck | `BorrowChecker::check_mir_body` | `(&mut self, mir: &MirBody)` |
| 3 codegen | `codegen::codegen_crate` | `(result: &CompileResult) -> String` — **§16-compliant** |

### 2.3 When to use struct-based entry instead

Use the struct-based variant when the operation requires **heavy mutable
state** that's expensive to reconstruct:

- `Lexer<'a>` — holds `&mut Rodeo`, position, error buffer
- `Parser` — holds `&mut Rodeo`, token cursor, error buffer, no-struct-literal flag
- `HirLowerCtxt<'a>` — holds def_id counter, owner stack, body/owner storage
- `MirLowerCtxt<'a>` — holds local_map, current_block, unification table
- `TypeChecker` — holds unification table, fn_sigs map, results
- `BorrowChecker` — holds borrow_set, move_tracker, initialized set

In all cases, the struct's primary method follows the `<verb>_<noun>`
pattern (e.g. `Parser::parse_crate`, `TypeChecker::check_mir_body_with_tables`).

### 2.4 Driver is the orchestrator

The driver (`src/driver.rs`) is the **sole orchestrator** that calls all
stage entry points in order. It is the only module allowed to read HIR
directly (per §16.6 exception). Downstream consumers should use
`driver::compile` rather than reaching into individual stages.

```rust
// Canonical usage
use landin_compiler::{compile, codegen_crate};

let result = compile(src)?;
if !result.errors.has_errors() {
    let ir = codegen_crate(&result);
    println!("{ir}");
}
```

---

## 3. Context Type Convention

### 3.1 Naming pattern

Context types (stateful objects that drive a stage) follow these patterns:

| Role | Pattern | Examples |
|------|---------|----------|
| Lexer / parser | `<Verb>er` (single-word OK) | `Lexer`, `Parser` |
| Lowering context | `<Stage>LowerCtxt<'a>` | `HirLowerCtxt`, `MirLowerCtxt` |
| Analysis context | `<Stage>Checker` / `<Stage>Resolver` | `TypeChecker`, `BorrowChecker`, `Resolver` |
| Trait (pluggable) | `<Verb>er` (trait) | `Emitter` |

### 3.2 The `Ctxt` vs `-er` split

The split is intentional:
- `Ctxt` suffix → ephemeral context that exists only during the lowering
  pass. Built fresh per call, discarded when lowering completes.
- `-er` suffix → stateful agent that may carry over results (e.g.
  `TypeChecker.results`, `BorrowChecker.borrows`).

### 3.3 Prefix rules

- **HIR lowering context**: `HirLowerCtxt` (NOT `LowerCtxt`) — the `Hir`
  prefix is required for parity with `MirLowerCtxt`.
- **MIR lowering context**: `MirLowerCtxt` (NOT `LowerCtxt`) — the `Mir`
  prefix is required.
- **Type/Borrow checkers**: no prefix — `TypeChecker`, `BorrowChecker`
  are unambiguous because they live under `typeck::` / `borrowck::` module paths.
- **Resolver**: no prefix — `Resolver` lives under `resolve::`.

---

## 4. Type Prefix Convention

### 4.1 Per-stage prefixes

| Stage | Prefix | When to use |
|-------|--------|-------------|
| AST | (none) | All AST node types: `Crate`, `Item`, `ItemKind`, `Ty`, `Pat`, `Expr`, `Stmt` |
| Lexer | (none) | All token types: `Token`, `TokenKind`, `IntTy`, `FloatTy` |
| HIR | `Hir` | All HIR node types: `HirItem`, `HirExpr`/`HirExprKind`, `HirTy`/`HirTyKind`, `HirCrate`, `HirPath`, etc. |
| HIR IDs | (none, infrastructure) | `HirId`, `DefId`, `BodyId`, `OwnerId`, `ItemLocalId` — shared across stages |
| Resolve | (none) | `Resolver`, `ResolveError`, `Scope`, `ScopeKind`, `ModuleNode` |
| MIR | `Mir` (when needed) | `MirBody`, `MirLowerCtxt` — most MIR types (`Ty`, `TyKind`, `Sig`, `BasicBlock`, `Statement`) rely on `mir::` qualification |
| Typeck | (none) | `TypeChecker`, `TypeckResults`, `TypeError`, `FieldTyTable`, `FnSigTable`, `UnificationTable` |
| Borrowck | (none) | `BorrowChecker`, `BorrowSet`, `BorrowError`, `MoveTracker`, `PlacePath` |
| Codegen | `Emit` | `Emitter`, `TextEmitter`, `EmitType`, `EmitValue` |

### 4.2 When to add a prefix

Add a stage prefix when:
1. The type might be confused with a similar type from another stage
   (e.g. `HirExpr` vs AST `Expr`, `MirBody` vs HIR `Body`).
2. The type is re-exported at the crate root or used widely outside its
   defining module.

Don't add a prefix when:
1. The type is unambiguous within its module path (e.g. `typeck::TypeChecker`
   doesn't need to be `TypeckTypeChecker`).
2. The type is infrastructure shared across stages (`Span`, `DefId`, `HirId`).

### 4.3 ID types (infrastructure)

ID types (`HirId`, `DefId`, `BodyId`, `OwnerId`, `ItemLocalId`, `LocalId`,
`BasicBlockId`, `FieldId`) live in their respective stage modules but are
intentionally prefixless or use the stage prefix only when needed for
disambiguation. They are considered infrastructure (per §16.2.3).

---

## 5. Re-Export Convention

### 5.1 Explicit lists, no globs

**Every stage module's `mod.rs` uses an explicit re-export list.** Glob
re-exports (`pub use X::*;`) are FORBIDDEN.

**Rationale**: Glob re-exports leak internal types unintentionally, make
the public API surface undiscoverable, and create maintenance hazards
(adding a private type to a `kinds.rs` module would silently become public).

### 5.2 Required comment

Every explicit re-export list must be preceded by a comment explaining
the convention. Use this template:

```rust
// Stage 3.57 (P0-3 fix) / Stage 3.63 (cross-stage naming standardization):
// explicit re-exports instead of `pub use *::*;` to prevent accidental
// leakage of internal types.
pub use kinds::{
    Type1, Type2, Type3,
};
```

### 5.3 Backwards-compatibility re-exports

When a type is moved to a new architectural home (e.g. `DefKind` moved
from `resolve::module_tree` to `hir::kinds`), the old module MUST
re-export the type for backwards compatibility:

```rust
// In src/resolve/mod.rs
// Stage 3.63: `DefKind` is now defined in `crate::hir::kinds`.
// Re-export here for backwards compatibility with callers that
// historically used `crate::resolve::DefKind`.
pub use crate::hir::DefKind;
```

### 5.4 Current state (post-Stage-3.63)

All stage `mod.rs` files use explicit re-export lists:

| File | Re-export count |
|------|----------------|
| `src/ast/mod.rs` | 62 types |
| `src/lexer/mod.rs` | 6 types |
| `src/parser/mod.rs` | 2 types + 1 free fn |
| `src/hir/mod.rs` | ~40 types |
| `src/resolve/mod.rs` | ~6 types |
| `src/mir/mod.rs` | ~30 types |
| `src/typeck/mod.rs` | ~7 types |
| `src/borrowck/mod.rs` | ~6 types |
| `src/codegen/mod.rs` | ~6 types |

---

## 6. Single Source of Truth (DRY)

### 6.1 Rule

When a type is consumed across multiple stages, it has **exactly one
definition**. Cross-stage re-exports via `pub use` are allowed for
backwards compatibility, but the definition lives in the architecturally
correct module.

### 6.2 Architectural dependency direction

The pipeline's dependency direction is:

```
session (infrastructure)
   ↑
ast, lexer  ← stage 0
   ↑
hir (incl. DefKind, Res)  ← stage 1
   ↑
resolve  ← stage 1.3 (reads hir)
   ↑
mir  ← stage 2 (reads hir)
   ↑
typeck  ← stage 2 (reads mir)
   ↑
borrowck  ← stage 2 (reads mir)
   ↑
codegen  ← stage 3 (reads mir + driver::CompileResult)
   ↑
driver  ← sole orchestrator, sole hir reader
```

A type defined in stage N may be re-exported from stage N+1 for
convenience, but the **definition** must stay in stage N. Violations
of this rule create circular dependencies and are tracked as
`L-PIPE-N` (pipeline coupling debt) per §16.3.

### 6.3 Current DRY-correct types (post-Stage-3.63)

| Type | Defined in | Re-exported from | Notes |
|------|-----------|------------------|-------|
| `Span`, `BytePos` | `session::mod` | all stages via `crate::session::` | Infrastructure |
| `DefId`, `HirId`, `BodyId`, `OwnerId`, `ItemLocalId` | `hir::id` | `hir::mod`, `resolve::mod` | Infrastructure |
| `DefKind` | `hir::kinds` | `resolve::mod` (backwards compat) | Stage 3.63 moved |
| `BorrowKind` | `mir::lvalue` | `borrowck::mod` (backwards compat) | Stage 3.63 unified |
| `Ty`, `TyKind`, `Sig`, `Const`, `Region`, `Mutability` | `mir::ty` | `mir::mod` | Stage 2 types |
| `MirBody`, `BasicBlock`, `Statement`, `Terminator` | `mir::body` | `mir::mod` | Stage 2 types |
| `Lvalue`, `Operand`, `Rvalue`, `AggregateKind`, `BinOp`, `UnOp` | `mir::lvalue` | `mir::mod` | Stage 2 types |

---

## 7. Deprecation Convention

### 7.1 When to deprecate

Mark a function `#[deprecated]` when:
1. It violates §16 (interface isolation) — e.g. it takes `&HirCrate` and
   re-lowers internally.
2. It has been superseded by a §16-compliant replacement.
3. The driver-based orchestration makes it redundant.

### 7.2 Deprecation attribute format

```rust
#[deprecated(note = "Use <Replacement> (§16-compliant) or driver::compile instead")]
pub fn legacy_function(...) -> ... { ... }
```

The note MUST:
1. Name the canonical replacement (function path or `driver::compile`).
2. Mention `§16-compliant` if applicable.
3. Be a single sentence.

### 7.3 Module re-export of deprecated items

When a module re-exports a deprecated item, the re-export must be
wrapped in `#[allow(deprecated)]`:

```rust
// Stage 3.63: `check_crate` and `check_mir_body_with_hir` are kept as
// deprecated legacy entry points for backwards compatibility.
#[allow(deprecated)]
pub use checker::{
    check_crate, check_mir_body, FieldTyTable, FnSigTable, TypeChecker, TypeckResults,
};
```

### 7.4 Current deprecated items (post-Stage-3.63)

| Function | Deprecated in | Replacement |
|----------|--------------|-------------|
| `typeck::TypeChecker::populate_fn_sigs` | Stage 3.62 | Set `tc.fn_sigs` directly from `FnSigTable` |
| `typeck::TypeChecker::check_mir_body_with_hir` | Stage 3.62 | `TypeChecker::check_mir_body_with_tables` |
| `typeck::check_crate` | Stage 3.63 | `TypeChecker::check_mir_body_with_tables` or `driver::compile` |
| `borrowck::check_crate` | Stage 3.63 | `BorrowChecker::check_mir_body` or `driver::compile` |

---

## 8. Function Naming Conventions

### 8.1 Verb prefixes

| Prefix | Meaning | Examples |
|--------|---------|----------|
| `lex_` | Lexer internal: scan a specific token kind | `lex_ident`, `lex_number`, `lex_doc_comment` |
| `parse_` | Parser internal: parse a specific grammar construct | `parse_crate`, `parse_expr`, `parse_ty`, `parse_path_with_ctx` |
| `lower_` | Lowering: convert from one IR to another | `lower_crate`, `lower_body`, `lower_expr`, `lower_hir_ty_to_mir_ty` |
| `resolve_` | Name resolution: look up a path | `resolve_path`, `resolve_crate`, `resolve_uses` |
| `check_` | Type/borrow checking: walk + verify | `check_mir_body_with_tables`, `check_statement`, `check_terminator` |
| `emit_` | Codegen: emit LLVM IR construct | `emit_header`, `emit_declare`, `emit_fat_ptr_type` |
| `codegen_` | Codegen: top-level entry | `codegen_crate`, `codegen_from_mir` |
| `mir_type_to_emit_type` | Translation ladder: MIR → Emit | (long form, explicit) |
| `emit_type_to_llvm_str` | Translation ladder: Emit → LLVM | (long form, explicit) |

### 8.2 Translation function ladder

The codegen stage has a 3-step translation ladder:
1. `mir_type_to_emit_type` / `mir_type_to_emit_type_with_layouts` — MIR `Ty` → `EmitType`
2. `emit_type_to_llvm_str` — `EmitType` → LLVM IR type string
3. `emit_fat_ptr_type` — Helper constructor for fat-pointer `EmitType`s (Stage 3.63 renamed from `fat_ptr_type`)

All three prefixes (`mir_`, `emit_`, `llvm_`) coexist intentionally —
each indicates which IR the function translates **from** or **to**.

### 8.3 Helper verbs (lex/parse internal)

- `peek` — look at the next token without consuming
- `bump` — consume the next token, return it
- `eat` — consume a specific token kind if it matches; return bool
- `expect` — consume a specific token kind or error
- `is_at_end` — check if input is exhausted

These are stage-internal; not part of any public API.

---

## 9. Error Type Convention

### 9.1 Suffix

All error types use the `Error` suffix:

| Stage | Error types |
|-------|-------------|
| 0 lexer | `LexError` |
| 0 parser | `ParseError` |
| 1 HIR | `LowerError` |
| 1 resolve | `ResolveError` |
| 2 typeck | `TypeError` |
| 2 borrowck | `BorrowError`, `BorrowErrorKind` |

### 9.2 Structure

All error types share the same minimal shape:

```rust
pub struct <Stage>Error {
    pub message: String,
    pub span: Span,
}
```

Additional context (e.g. `BorrowErrorKind` for borrowck error categorization)
is added as needed. All error types should implement `Display` (most do;
P2 item to add `std::error::Error` trait impls).

### 9.3 Error collection

Errors are non-fatal — each stage collects a `Vec<<Stage>Error>` and
continues processing. The driver aggregates all errors into
`CompileErrors` and reports them at the end.

---

## 10. Enforcement

### 10.1 CI checks

The following CI checks enforce this standard:

1. **`cargo fmt --check`** — enforces formatting (catches typos in type names).
2. **`cargo clippy --all-targets`** — 0 warnings required (catches unused
   imports, dead code, naming-convention violations).
3. **`cargo test`** — 977+ tests must pass (includes 5 §21 audit tests
   that verify §16 compliance programmatically).
4. **§21 audit** (per stage-committee-process.md §21) — runs at the end
   of each major stage; verifies the conventions in this document.

### 10.2 Manual review checklist

Before merging any change to `src/`, the reviewer verifies:

- [ ] No new `pub use X::*;` globs added (use explicit lists).
- [ ] No new types without the correct stage prefix (per §4).
- [ ] No new context types without the correct suffix (`Ctxt` / `-er` per §3).
- [ ] No new entry points that violate the free-function pattern (per §2).
- [ ] No new types duplicated across modules (per §6 — single source of truth).
- [ ] No new `#[deprecated]` without a `note = "..."` pointing to the replacement.
- [ ] If a type is moved, the old module re-exports it for backwards compat.

### 10.3 Process integration

This standard is referenced by:
- `docs/stage-committee-process.md` v3.15 §23 (naming standardization protocol)
- `docs/develop/v0/stage-0-3-cross-stage-audit.md` (Stage 3.63 audit report)
- §21 cross-stage audit checklist (per §21.3)

Any deviation from this standard MUST be:
1. Documented in the worklog with a justification.
2. Tracked as `L-NAMING-N` (naming debt) in the gate review.
3. Fixed in the next standardization round.

---

## 11. Change Log

### v1.0 (Stage 3.63, 2026-07-22)

Initial version. Captures the conventions established by the Stage 3.63
cross-stage naming standardization round (per §21 audit findings).

**Fixes applied in this round**:
1. `src/lexer/mod.rs`: glob → explicit list (6 types)
2. `src/ast/mod.rs`: glob → explicit list (62 types)
3. `src/hir/lower/*.rs`: `LowerCtxt` → `HirLowerCtxt` (9 files)
4. `src/typeck/checker.rs`: `check_crate` marked `#[deprecated]`
5. `src/borrowck/mod.rs`: `check_crate` marked `#[deprecated]`
6. `src/typeck/mod.rs`: doc-comment updated to point to canonical entry
7. `src/mir/lvalue.rs` + `src/borrowck/borrow_set.rs`: `BorrowKind` unified
   (single source of truth in `mir::lvalue`; duplicate + `BkKind` alias removed)
8. `src/mir/mod.rs`: `lower_hir_body_to_mir_full` + `_with_return_ty` added
   to re-exports
9. `src/parser/mod.rs`: `parser::parse_crate` free function added
10. `src/codegen/emitter.rs` + `src/codegen/mod.rs`: `fat_ptr_type` → `emit_fat_ptr_type`
11. `src/codegen/mod.rs`: module docs expanded with status, §16 compliance,
    open limitations table, architectural debt
12. `src/hir/kinds.rs`: `DefKind` moved here from `resolve::module_tree`
    (aligns dependency direction)
13. `src/resolve/module_tree.rs` + `src/resolve/mod.rs`: import `DefKind`
    from `hir::kinds` (backwards compat re-export preserved)

**Test impact**: 0 (pure refactoring — 977/977 tests still pass).
**Clippy impact**: 0 (0 warnings before, 0 warnings after).
**Fmt impact**: clean.

### v1.1 (Stage 3.64, 2026-07-22)

P2 ergonomics + feature round. Builds on v1.0 (Stage 3.63 naming
standardization) by adding standard error-trait impls, re-exporting
the codegen pluggability surface, and implementing the previously-stub
`use` declaration resolution feature.

**Fixes applied in this round**:
1. `src/lexer/reader.rs`: `LexError` impl `Display` + `std::error::Error`
2. `src/parser/error.rs`: `ParseError` impl `Display` + `std::error::Error`
3. `src/hir/lower/error.rs`: `LowerError` impl `std::error::Error` (Display already existed)
4. `src/resolve/error.rs`: `ResolveError` impl `std::error::Error`
5. `src/typeck/error.rs`: `TypeError` impl `std::error::Error`
6. `src/borrowck/error.rs`: `BorrowError` impl `std::error::Error`
7. `src/lexer/token.rs`: removed 2 orphaned doc comments (lines 26, 156)
8. `src/lib.rs`: re-export `Emitter` + `TextEmitter` + `EmitType` + `EmitValue` (pluggability)
9. `src/codegen/emitter.rs` + `src/codegen/text_emitter.rs`: `Emitter::output()` → `emit_output()` (prefix consistency)
10. `src/resolve/module_tree.rs`: new `UseImport` struct + `use_imports` table on `ModuleNode` + `lookup_use_import` + `insert_use_import` methods
11. `src/resolve/resolver.rs`: implemented real `resolve_uses` (was no-op stub) — handles leaf, glob, path-prefix, and aliased imports; `resolve_path` consults `use_imports` as fallback
12. `src/resolve/mod.rs`: re-export `UseImport` + `UseDecl`

**New feature**: `use` declaration resolution (Stage 1.3 Phase C).
Real Landin programs that use `use a::b::c;` imports now resolve
correctly, where previously they would silently fail. 5 new tests
in `tests/v0/stage1/plan/hir_resolution_tests.rs` cover leaf / glob / path-prefix / alias
/ table-populated cases.

**Test impact**: +5 (982/982 tests pass — was 977, +5 new use-resolution tests).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.2 (Stage 3.65, 2026-07-22)

P2 architectural fixes round. Builds on v1.1 (Stage 3.64) by closing
the `unsafe impl/trait` AST debt, adding `Res::SelfTy` trait/impl
discrimination, and providing `lower_body` short-form aliases.

**Fixes applied in this round**:
1. `src/ast/kinds.rs`: added `is_unsafe: bool` to `ImplDecl` and `TraitDecl`
2. `src/hir/kinds.rs`: added `is_unsafe: bool` to `HirImpl` and `HirTrait`;
   added new `HirSelfKind` enum (`Trait`/`Impl`); `Res::SelfTy` now carries
   `HirSelfKind`
3. `src/parser/parser.rs`: `parse_impl` and `parse_trait` now take
   `is_unsafe: bool`; the `KwUnsafe` + `KwImpl`/`KwTrait` match arms now
   pass `true` (previously dropped the qualifier)
4. `src/hir/lower/item.rs`: `lower_trait` and `lower_impl` now propagate
   `is_unsafe` from AST to HIR
5. `src/hir/mod.rs`: re-export `HirSelfKind`
6. `src/resolve/resolver.rs`: `Res::SelfTy` construction now passes
   `HirSelfKind::Impl` (defaults to Impl; threading owner context is Stage 4)
7. `src/mir/lower/mod.rs`: added `lower_body` + `lower_body_full` short-form
   aliases per `api-naming-standard.md` §2.2 verb_noun convention
8. `src/mir/mod.rs`: re-export `lower_body` + `lower_body_full`
9. `src/codegen/emitter.rs` + `src/codegen/mod.rs`: documented
   `mir_type_to_emit_type` (legacy fallback) vs
   `mir_type_to_emit_type_with_layouts` (canonical §16-compliant) with
   "When to use which" guidance

**New types**: `hir::HirSelfKind` (`Trait` / `Impl`) — discriminant for
`Res::SelfTy`. Named `HirSelfKind` (not `SelfKind`) to avoid collision
with the pre-existing `ast::SelfKind` enum (method receiver kinds).

**New functions**: `mir::lower::lower_body`, `mir::lower::lower_body_full`
— short-form aliases for `lower_hir_body_to_mir` / `_full`.

**Test impact**: +1 (983/983 tests pass — was 982, +1 new
`test_safe_impl_and_trait_have_is_unsafe_false`).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.3 (Stage 3.66, 2026-07-22)

The big rename round. Completes the largest remaining P2 item from the
§21 audit: `Lvalue` → `Place` (167+ references across 7+ files). Also
threads owner context through the resolver for accurate `HirSelfKind`.

**Fixes applied in this round**:
1. `src/mir/lvalue.rs` → `src/mir/place.rs` (file renamed)
2. `src/mir/mod.rs`: `pub mod lvalue` → `pub mod place`; `pub use lvalue::{...}` → `pub use place::{...}`
3. Type rename: `Lvalue` → `Place` (167 refs)
4. Enum rename: `LvalueKind` → `PlaceKind` (75 refs)
5. All `crate::mir::lvalue::` module paths → `crate::mir::place::`
6. All function names renamed (examples):
   - `lower_expr_to_lvalue` → `lower_expr_to_place`
   - `detect_lvalue_type` → `detect_place_type`
   - `detect_lvalue_storage_type` → `detect_place_storage_type`
   - `compute_lvalue_address` → `compute_place_address`
   - `codegen_lvalue_load` / `_typed` → `codegen_place_load` / `_typed`
   - `resolve_lvalue_for_writeback` → `resolve_place_for_writeback`
   - `infer_lvalue` → `infer_place`
   - `lvalue_ty` → `place_ty`
   - `lvalue_root_reads` → `place_root_reads`
7. All variable names: `lhs_lvalue` → `lhs_place`, etc.
8. All doc comments: "lvalue" → "place" (where referring to the concept)
9. `src/resolve/resolver.rs`: new `current_self_kind: Option<HirSelfKind>`
   field; set to `Trait`/`Impl` when resolving trait/impl item paths;
   `resolve_path` uses it for `Self` resolution

**Why this matters**: Aligns implementation with design doc (06-mir.md §4
calls it `Place`), eliminates vocabulary mismatch with borrowck internals
(`PlacePath`, `PlaceRoot`), and matches modern rustc naming (post-RFC-1211).

**Test impact**: 0 (983/983 tests pass — pure refactoring, no test changes).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.4 (Stage 3.67, 2026-07-22)

P2 cleanup round. Completes the `HirSelfKind` work from Stage 3.66
(body-level context threading), eliminates the `&mut Rodeo` smell in
`resolve_crate`, and fixes 11 `Span::DUMMY` placeholders in `parser.rs`.

**Fixes applied in this round**:
1. `src/resolve/resolver.rs`: `resolve_all_paths` builds
   `HashMap<DefId, HirSelfKind>` and sets `current_self_kind` before
   each `resolve_body` call — body-level `Self` resolution now accurate
2. `src/resolve/resolver.rs`: `resolve_crate` signature changed from
   `&mut Rodeo` to `&Rodeo` (resolver is now pure read-only consumer)
3. `src/lexer/reader.rs`: lexer now interns keyword strings at
   tokenization time (`self.interner.get_or_intern(text)` before
   returning keyword tokens) — eliminates the need for resolver to
   pre-intern keywords
4. `src/parser/parser.rs`: 11 `Span::DUMMY` placeholders replaced with
   `kw_span` (keyword span captured before `self.bump()` in
   `parse_const`, `parse_static`, `parse_struct`, `parse_enum`,
   `parse_impl`, `parse_type_alias`)
5. `src/driver.rs` + 4 test files: `resolve_crate(&mut hir, &interner)`
   (was `&mut interner`)

**Test impact**: 0 (983/983 tests pass — pure refactoring).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.5 (Stage 3.68, 2026-07-22)

Visibility checking infrastructure round. Lays the groundwork for
Stage 1.3 Phase E1 (visibility enforcement) by collecting visibility
metadata and adding a check hook.

**Fixes applied in this round**:
1. `src/resolve/resolver.rs`: new `def_visibility: HashMap<DefId, Visibility>`
   field on `Resolver` — populated during `build_module_tree` for all
   item kinds (Fn, Const, Static, Struct, Enum, Trait, TypeAlias, Mod, Use)
2. `src/resolve/resolver.rs`: new `check_visibility(def_id, span)` method
   — called from `resolve_path` when resolving to `Res::Def` in both
   value and type namespaces. Currently a stub (returns `Ok(())`) —
   real enforcement deferred to Stage 4 (needs nested module support)
3. `src/resolve/resolver.rs`: public `def_visibility(def_id)` accessor
   for testing
4. `tests/v0/stage1/plan/hir_resolution_tests.rs`: +1 new test
   `visibility_metadata_collected_for_fn` — verifies `pub fn` →
   `Visibility::Public`, `fn` → `Visibility::Private`

**Test impact**: +1 (984/984 tests pass — was 983).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.6 (Stage 5.36, 2026-07-23)

Stage 5.36 stdlib trait method signatures round. Adds the first stdlib-scoped
public API surface since v1.5: a static method-signature registry for builtin
traits, exposed via 5 free-function query APIs.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibTraitMethod` | struct | `<Noun><Noun><Noun>` |
| `StdlibSelfKind` | enum | `<Noun><Noun><Noun>` |
| `stdlib_trait_methods` | free fn | `<noun>_<noun>_<noun>` |
| `stdlib_trait_method_count` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `find_stdlib_trait_method` | free fn | `find_<noun>_<noun>_<noun>` |
| `is_stdlib_trait_method` | free fn | `is_<noun>_<noun>_<noun>` |
| `stdlib_traits_with_method` | free fn | `<noun>_<noun>_with_<noun>` |

**Field naming**: `name` / `self_kind` / `param_count` / `return_kind` /
`is_unsafe` — all follow `<noun>_<noun>` or `is_<adj>` patterns.

**Design decisions**:
1. Per-op const tables (Add/Sub/Mul/...) instead of shared placeholder with
   runtime name override — ensures `StdlibTraitMethod.name` field always
   matches the trait's actual method name. Avoids the smell of returning a
   `&StdlibTraitMethod` whose `.name` doesn't match the queried method name.
2. `stdlib_traits_with_method()` uses a local `ALL_REGISTERED_TRAITS` const
   (mirrors the match arms in `stdlib_trait_methods()`) instead of importing
   `traits::builtin::BUILTIN_TRAIT_NAMES` — keeps `stdlib.rs` self-contained
   per §16 (no backwards dependency on the `traits` module).
3. Markers return `Some(&[])` (not `None`) so callers can distinguish
   "trait in registry but no methods" from "trait not in registry at all".

**§16 compliance**: `StdlibTraitMethod` uses `StdlibTypeKind` (stdlib-internal)
— no `mir::ty` reference, no circular dependency.

**Test impact**: +24 (1106 → 1130).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.7 (Stage 5.37, 2026-07-23)

Stage 5.37 stdlib vtable slot layout round. Adds the final static-prep API
surface for dyn Trait MIR lowering — deterministic vtable slot indexing
for stdlib traits.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibVtableSlot` | struct | `<Noun><Noun><Noun>` |
| `stdlib_trait_method_index` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `stdlib_vtable_layout` | free fn | `<noun>_<noun>_<noun>` |
| `stdlib_vtable_slot_count` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `is_stdlib_marker_trait` | free fn | `is_<noun>_<adj>_<noun>` |
| `stdlib_traits_with_vtable` | free fn | `<noun>_<noun>_with_<noun>` |

**Field naming**: `slot_index` (`<noun>_<noun>`) + `method` (`<noun>`) —
both comply.

**Design decisions**:
1. Slot index derived from `stdlib_trait_methods()` slice position (0-based)
   — not from a HashMap — so the same trait always returns the same slot
   order. Determinism is required for codegen: the vtable global's element
   count and the method-call byte offset must be stable across runs.
2. Three distinct return states for `stdlib_vtable_slot_count`:
   - `Some(0)` — marker trait (registered, no methods)
   - `Some(n)` — trait with n methods
   - `None` — trait not in registry at all
   This trichotomy lets codegen distinguish "skip emitting vtable" (marker)
   from "trait doesn't exist" (unknown).
3. `is_stdlib_marker_trait` returns false for unknown traits. Not registered
   ≠ marker — keeping these distinct avoids accidental "treat unknown as
   marker" bugs in codegen.
4. `StdlibVtableSlot` carries `&'static StdlibTraitMethod` (zero-copy ref
   to the existing static table) — no allocation per query, no lifetime
   management burden on callers.

**§16 compliance**: `StdlibVtableSlot` uses `StdlibTraitMethod` (stdlib-
internal) — no `mir::ty` / `codegen::EmitType` reference, no circular
dependency.

**Test impact**: +22 (1130 → 1152).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.8 (Stage 5.38, 2026-07-23)

Stage 5.38 stdlib vtable byte size + pointer-width-aware layout round.
Adds the final arithmetic helper API surface before dyn Trait MIR
lowering — translates slot indices into byte offsets that codegen can
directly use in LLVM IR emission.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibPointerWidth` | enum | `<Noun><Noun><Noun>` |
| `StdlibPointerWidth::Pointer32` | variant | `<Noun><Digits>` |
| `StdlibPointerWidth::Pointer64` | variant | `<Noun><Digits>` |
| `StdlibPointerWidth::byte_size` | method (const fn) | `<noun>_<noun>` |
| `stdlib_pointer_width_bytes` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `stdlib_vtable_byte_size` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `stdlib_vtable_method_offset` | free fn | `<noun>_<noun>_<noun>_<noun>` |

**Design decisions**:
1. `StdlibPointerWidth` is an enum, not a `u32` raw width — gives type
   safety (callers can't pass 5 or 16 by accident) and lets the compiler
   exhaustively match in `byte_size()`.
2. `byte_size()` is `const fn` — usable in const context for compile-time
   fixed vtable size computation (e.g. `const CLONE_VTABLE_SIZE_64: u64 =
   stdlib_vtable_byte_size("Clone", StdlibPointerWidth::Pointer64).unwrap();`).
3. Three-state return consistent with Stage 5.37:
   `Some(0)` (marker) / `Some(n)` (registered with n bytes) / `None`
   (unknown trait) — codegen distinguishes "skip vtable" from "trait
   doesn't exist".
4. Compositional design — `vtable_byte_size` and `method_offset` build
   on Stage 5.37's `slot_count` and `slot_index` rather than recomputing.
   Single source of truth for slot numbering.
5. Cross-check test in `stdlib_vtable_size_tests.rs` verifies
   `method_offset < vtable_byte_size` across 7 (trait, method) pairs ×
   2 pointer widths — this is the core safety invariant typeck will
   enforce at runtime in Stage 5.40+.

**§16 compliance**: All new APIs use only `StdlibPointerWidth` (stdlib-
internal) + existing `stdlib_vtable_slot_count` / `stdlib_trait_method_index`.
No `mir::ty` / `codegen::EmitType` reference, no circular dependency.

**Test impact**: +20 (1152 → 1172).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.9 (Stage 5.39, 2026-07-23)

Stage 5.39 stdlib vtable construction planner round. Adds the "last mile"
static planner that combines trait method signatures + slot indexing +
impl coverage into a single ordered plan codegen can consume in one pass.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibVtablePlanEntry` | struct | `<Noun><Noun><Noun><Noun>` |
| `StdlibVtablePlan` | struct | `<Noun><Noun><Noun>` |
| `StdlibVtablePlan::is_complete` | method | `<noun>_<adj>` |
| `StdlibVtablePlan::missing_methods` | method | `<adj>_<noun>` |
| `stdlib_vtable_plan` | free fn | `<noun>_<noun>_<noun>` |
| `stdlib_vtable_plan_entry_count` | free fn | `<noun>_<noun>_<noun>_<noun>_<noun>` |
| `stdlib_vtable_plan_is_complete` | free fn | `<noun>_<noun>_<noun>_<adj>` |
| `stdlib_vtable_plan_missing_methods` | free fn | `<noun>_<noun>_<noun>_<adj>_<noun>` |

**Field naming**: `slot_index` (`<noun>_<noun>`) + `method_name`
(`<noun>_<noun>`) + `provided` (`<adj>`, bool field) + `trait_name`
(`<noun>_<noun>`) + `entries` (`<noun>`) — all comply.

**Design decisions**:
1. `StdlibVtablePlan` is a plain struct (not an enum) — the plan either
   exists (`Some(plan)`) or the trait is unknown (`None`). This keeps the
   return type simple and avoids a 4-variant enum where 3 variants would
   be redundant with the `entries.is_empty()` + `is_complete()` checks.
2. `provided: bool` per entry — simple flag, not an enum like
   `ProvidedKind::Yes/No/Inherited`. Dyn Trait codegen only needs to know
   "do I emit the impl symbol or a null/stub?" — a bool captures this
   exactly. Inherited methods (from supertraits) are not modeled here
   because Landin's stdlib traits don't have inheritance (markers don't
   declare methods; Eq is empty).
3. Markers return empty plan with `is_complete() == true` (vacuously
   complete) — consistent with Stage 5.37/5.38's three-state convention
   where markers are `Some` with zero slots, not `None`.
4. Extra names in `provided_method_names` silently ignored (tolerant
   design) — an impl block may declare methods for multiple traits, and
   the caller may pass the union without filtering. Strictness would
   force callers to pre-filter, which is error-prone.
5. `StdlibVtablePlan` derives `PartialEq`/`Eq` — usable for test
   assertions and future plan-cache deduplication (codegen may memoize
   plans per (trait, impl_type) pair).
6. `stdlib_vtable_plan_entry_count()` is a non-allocating shortcut —
   delegates to `stdlib_vtable_slot_count()` without constructing the
   entries Vec. Useful when only the count is needed (e.g. pre-sizing
   buffers).
7. **5-noun function name** `stdlib_vtable_plan_entry_count` — long but
   unambiguous. The pattern `<noun>_<noun>_<noun>_<noun>_<noun>` is
   permitted by §23 when each noun adds a meaningful scope qualifier
   (stdlib → vtable → plan → entry → count). Splitting into a method
   (`plan.entry_count()`) was considered but rejected because the query
   doesn't require a `StdlibVtablePlan` value — it answers "how many
   entries *would* a plan for this trait have?"

**§16 compliance**: `StdlibVtablePlan` / `StdlibVtablePlanEntry` use only
`&'static str` + `Vec<>` + scalars — no `mir::ty` / `codegen::EmitType` /
`traits::TraitResolver` reference, no circular dependency.

**Test impact**: +18 (1172 → 1190).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.10 (Stage 5.40, 2026-07-23)

Stage 5.40 stdlib vtable symbol name planner round. Extracts LLVM
symbol-name formatting logic from codegen into pure stdlib functions —
the last static-prep step before codegen vtable emission refactor
(Stage 5.41+).

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_vtable_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` |
| `stdlib_dynptr_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` |
| `stdlib_data_global_name` | free fn | `<noun>_<noun>_<adj>_<noun>` |
| `stdlib_impl_method_symbol` | free fn | `<noun>_<noun>_<noun>_<noun>` |
| `stdlib_vtable_method_symbols` | free fn | `<noun>_<noun>_<noun>_<noun>` |

**Design decisions**:
1. **Byte-for-byte equivalence with codegen**: each function's output
   matches the corresponding codegen `format!()` call exactly. Two tests
   (`test_stdlib_vtable_global_name_match_codegen` and
   `test_stdlib_vtable_method_symbols_match_codegen_format`) explicitly
   cross-check by formatting the same string via `format!()` and asserting
   equality — guarantees Stage 5.41+ refactor is behavior-equivalent.
2. **No new types** — all 5 new symbols are free functions returning
   `String` / `Vec<String>`. There's no need for a `StdlibVtableSymbol`
   struct because the output is consumed directly by codegen as LLVM IR
   text (no further structured querying needed).
3. **`stdlib_vtable_method_symbols` composition**: combines Stage 5.39
   `stdlib_vtable_plan()` + `stdlib_impl_method_symbol()` per-entry, with
   `provided=false` → `"null"` literal string. Codegen consumes the
   returned `Vec<String>` directly to emit
   `@.vtable.<trait>.<type> = ... [n x ptr] [...]`.
4. **Markers return `Some(vec![])`** — consistent with Stage 5.37/5.38/5.39
   three-state convention.
5. **Extra provided names silently ignored** — same tolerant design as
   Stage 5.39 (impls may implement multiple traits' methods).
6. **`global_name` vs `symbol`**: the `_global_name` suffix is used for
   LLVM globals (`@.vtable.*`, `@.dynptr.*`, `@.data.*`), while `_symbol`
   is used for function symbols (`landin_*`). This matches existing
   codegen vocabulary and avoids ambiguity.

**§16 compliance**: All new APIs input `&str`, output `String` /
`Vec<String>`. No `mir::ty` / `codegen::EmitType` / `traits::TraitResolver`
reference, no circular dependency. Pure functions, callable from any stage.

**Test impact**: +16 (1190 → 1206).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.11 (Stage 5.41, 2026-07-23)

Stage 5.41 stdlib vtable emission plan (aggregate) round. Adds a single-call
aggregate struct that returns everything codegen needs to emit one
`@.vtable.<trait>.<type>` global. Stage 5.42+ will replace codegen's 5
separate stdlib calls with one `stdlib_vtable_emission()` call.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibVtableEmission` | struct | `<Noun><Noun><Noun>` |
| `stdlib_vtable_emission` | free fn | `<noun>_<noun>_<noun>` |
| `stdlib_vtable_emissions_for_traits` | free fn | `<noun>_<noun>_<noun>_<prep>_<noun>` |

**Field naming (9 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `trait_name` | `&'static str` | `<noun>_<noun>` |
| `type_name` | `String` | `<noun>_<noun>` |
| `global_name` | `String` | `<noun>_<noun>` |
| `method_symbols` | `Vec<String>` | `<noun>_<noun>` |
| `slot_count` | `u32` | `<noun>_<noun>` |
| `byte_size_32` | `u64` | `<noun>_<noun>_<digits>` |
| `byte_size_64` | `u64` | `<noun>_<noun>_<digits>` |
| `is_marker` | `bool` | `is_<adj>` |
| `is_complete` | `bool` | `is_<adj>` |

**Design decisions**:
1. **Aggregate struct, not multiple return values**: a 9-field struct is
   clearer than a 9-tuple and lets codegen use field names
   (`e.global_name`, `e.byte_size_64`) instead of positional access
   (`e.3`, `e.6`). Future field additions are non-breaking (callers using
   field names don't need to change).
2. **`byte_size_32` / `byte_size_64` rather than a single `byte_size` +
   `StdlibPointerWidth` parameter**: pre-computes both widths so codegen
   can pick the right one based on target without re-calling. Avoids
   passing the width through every call site.
3. **`is_marker` + `is_complete` as precomputed bools**: codegen often
   needs to skip markers or warn on incomplete impls — precomputing these
   flags avoids re-deriving from `slot_count` / `method_symbols` at every
   consumer site.
4. **Batch query `stdlib_vtable_emissions_for_traits`** uses the
   `<noun>_<noun>_<noun>_<prep>_<noun>` pattern
   (`emissions_for_traits`). The `for_traits` preposition phrase makes
   the "batch over a trait list" semantics explicit, distinguishing it
   from the singular `stdlib_vtable_emission`.
5. **Unknown traits silently skipped in batch**: the caller may pass a
   mixed list of stdlib trait names + user-defined trait names (which
   aren't in the stdlib registry). Strictness here would force callers
   to pre-filter, which is error-prone.
6. **`StdlibVtableEmission` derives `PartialEq`/`Eq`**: usable for test
   assertions and future emission-cache deduplication (codegen may
   memoize emissions per (trait, impl_type) pair).

**§16 compliance**: struct uses only `&'static str` + `String` +
`Vec<String>` + scalars — no `mir::ty` / `codegen::EmitType` /
`traits::TraitResolver` reference, no circular dependency.

**Test impact**: +17 (1206 → 1223).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.12 (Stage 5.42, 2026-07-23)

Stage 5.42 stdlib vtable emission summary round. Adds project-level
aggregate statistics — the last static-analysis step before codegen
modification. Triggers §25 deep review #4 (10 sub-stages since review #3).

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibVtableEmissionSummary` | struct | `<Noun><Noun><Noun><Noun>` |
| `stdlib_vtable_emission_summary` | free fn | `<noun>_<noun>_<noun>_<noun>` |

**Field naming (8 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `total_emissions` | `u32` | `<adj>_<noun>` |
| `marker_count` | `u32` | `<noun>_<noun>` |
| `complete_count` | `u32` | `<adj>_<noun>` |
| `incomplete_count` | `u32` | `<adj>_<noun>` |
| `total_slots` | `u32` | `<adj>_<noun>` |
| `total_byte_size_32` | `u64` | `<adj>_<noun>_<noun>_<digits>` |
| `total_byte_size_64` | `u64` | `<adj>_<noun>_<noun>_<digits>` |
| `trait_names` | `Vec<&'static str>` | `<noun>_<noun>` |

**Design decisions**:
1. **Project-level aggregate, not per-emission**: this struct summarizes a
   *list* of emissions, not a single one. The `total_*` prefix on count
   fields makes this unambiguous (`total_emissions` vs `emission_count`
   which could be misread as "count of one emission").
2. **`total_byte_size_32` / `total_byte_size_64`** (with `total_` prefix)
   distinguishes from Stage 5.41's per-emission `byte_size_32` / `byte_size_64`.
   Consistent prefix convention: aggregated fields get `total_` prefix.
3. **`trait_names` dedup preserves first-seen order**: deterministic output
   for diagnostics. Alternative would be alphabetical sort, but first-seen
   order preserves the caller's intent (e.g. if caller passes traits in
   impl-declaration order, the summary reflects that).
4. **No new query function for individual stats**: the summary struct is
   cheap to construct (O(n) once) and all fields are public — callers
   access fields directly (`s.total_slots`, `s.incomplete_count`). Adding
   per-field query functions would be over-engineering.
5. **`StdlibVtableEmissionSummary` derives `PartialEq`/`Eq`**: usable for
   test assertions and future summary-cache deduplication.

**§16 compliance**: struct uses only `&'static str` + `Vec<>` + scalars —
no `mir::ty` / `codegen::EmitType` / `traits::TraitResolver` reference,
no circular dependency.

**§25 deep review #4**: triggered at this stage (10 sub-stages since
review #3 at Stage 5.32). 7-dimension audit in
`docs/develop/v0/stage-5/deep-review-r91.md`. Verdict: 5/5 GO, 0 P0/P1,
2 P2 blockers deferred to Stage 6+.

**Test impact**: +13 (1223 → 1236).
**Clippy impact**: 0 (0 warnings; fixed 1 `cloned_ref_to_slice_refs`
warning in test).
**Fmt impact**: clean.

### v1.13 (Stage 5.43, 2026-07-23)

Stage 5.43 codegen vtable emission helper round. **First Stage 5 sub-stage
modifying `src/codegen/`** — adds a new free function that produces LLVM IR
text from a `StdlibVtableEmission`.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_vtable_global_from_emission` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<prep>_<noun>` |

**Design decisions**:
1. **`emit_` prefix** consistent with the rest of the codegen module
   (`emit_vtables`, `emit_dyn_trait_ptrs`, `emit_fat_ptr_type`,
   `emit_fat_ptr_type`). All codegen free functions that produce LLVM IR
   text use this prefix.
2. **`_from_emission` suffix** distinguishes this from the existing
   `emit_vtable_global` (a method on the `Emitter` trait). The suffix
   makes the input type explicit — callers know they need to pass a
   `StdlibVtableEmission`, not a `(global_name, method_symbols)` pair.
3. **"先并行、后委托" strategy**: the new function exists in parallel to
   `TextEmitter::emit_vtable_global()` — no existing path modified. This
   makes the change independently reviewable and revertable. Stage 5.44+
   will refactor `TextEmitter::emit_vtable_global()` to delegate here,
   eliminating the duplicated LLVM IR formatting logic.
4. **"null" handling**: the new function detects `"null"` strings in
   `method_symbols` and emits `ptr null` (no `@` prefix). This is needed
   because `StdlibVtableEmission.method_symbols` may contain `"null"`
   entries (from `stdlib_vtable_method_symbols()` for missing slots),
   while `TextEmitter::emit_vtable_global()` is only called with real
   symbols (from `emit_vtables()` → `VtableEntry.fn_name`). The new
   function is designed to consume `StdlibVtableEmission` directly, so
   it must handle the "null" case.
5. **Cross-check tests**: `test_emit_vtable_global_from_emission_match_text_emitter`
   + `_marker` variant construct `StdlibVtableEmission` with real symbols,
   call both the free function and `TextEmitter::emit_vtable_global()`,
   assert free fn output appears verbatim in TextEmitter output. This is
   the safety net for Stage 5.44+ refactor — guarantees behavior
   equivalence when `TextEmitter::emit_vtable_global()` delegates here.

**§16 compliance**: function takes `&StdlibVtableEmission` (stdlib-internal
type), returns `String`. No `mir::ty` / `traits::TraitResolver` / `Emitter`
trait reference, no circular dependency.

**Test impact**: +13 (1236 → 1249).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.14 (Stage 5.44, 2026-07-23)

Stage 5.44 codegen vtable global text bridge round. Adds the bridge
function between Stage 5.43's high-level `emit_vtable_global_from_emission()`
and Stage 5.45's `TextEmitter::emit_vtable_global()` delegation refactor.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_vtable_global_text` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<noun>` |

**Design decisions**:
1. **Bridge function strategy**: Stage 5.43 added high-level
   `emit_vtable_global_from_emission(&StdlibVtableEmission)`. Stage 5.44
   adds low-level `emit_vtable_global_text(&str, &[String])` with the
   **exact same parameter signature** as `TextEmitter::emit_vtable_global()`.
   Stage 5.45 will:
   - Make `emit_vtable_global_from_emission()` internally call
     `emit_vtable_global_text()` (extracting fields from the emission struct)
   - Make `TextEmitter::emit_vtable_global()` delegate to
     `emit_vtable_global_text()` (trivial body change, same signature)
   Three-step refactor, each independently reviewable.
2. **`_text` suffix**: distinguishes this free function (returns LLVM IR
   text as `String`) from the trait method `emit_vtable_global` (side
   effect: pushes to `self.globals`). When Stage 5.45 makes the trait
   method delegate here, the naming asymmetry will visually remind readers
   that the free function is the "pure" version.
3. **Parameter signature match with trait method**: `emit_vtable_global_text(
   global_name: &str, method_symbols: &[String])` matches
   `Emitter::emit_vtable_global(&self, global_name: &str, method_symbols:
   &[String])` exactly (minus `&self`). This makes Stage 5.45 delegation
   a one-line body change: `self.globals.push(emit_vtable_global_text(
   global_name, method_symbols)); global_name.to_string()`.
4. **"null" handling consistency**: both Stage 5.43 and 5.44 free functions
   handle `"null"` → `ptr null`. TextEmitter's current path doesn't (would
   emit `ptr @null`), but `emit_vtables()` never passes "null" — only real
   symbols from `VtableEntry.fn_name`. Stage 5.45 delegation will fix this
   latent bug as a side effect.
5. **Divergence documentation test**:
   `test_emit_vtable_global_text_null_path_diverges_from_text_emitter`
   explicitly documents that the free fn handles null correctly while
   TextEmitter's current path doesn't. This is not a failure — it's a
   known issue that Stage 5.45 will resolve. Documenting it in a test
   ensures we don't forget.

**§16 compliance**: pure function, input `(&str, &[String])`, output
`String`. No `mir::ty` / `traits::TraitResolver` / `Emitter` /
`StdlibVtableEmission` reference, no circular dependency.

**Test impact**: +12 (1249 → 1261).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.15 (Stage 5.45, 2026-07-23)

Stage 5.45 codegen vtable emission batch helper round. Adds the batch
version of Stage 5.44's `emit_vtable_global_text()` — takes a slice of
`StdlibVtableGlobalSpec` and returns `Vec<String>`.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibVtableGlobalSpec` | struct (in `codegen`) | `<Noun><Noun><Noun><Noun>` |
| `emit_vtable_globals_batch` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<noun>` |

**Field naming (2 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `global_name` | `String` | `<noun>_<noun>` |
| `method_symbols` | `Vec<String>` | `<noun>_<noun>` |

**Design decisions**:
1. **Batch vs individual**: `emit_vtable_globals_batch()` is the batch
   counterpart of Stage 5.44's `emit_vtable_global_text()`. Avoids per-
   iteration function call overhead in `emit_vtables()` loop (Stage 5.46
   refactor will construct spec list once, call batch helper, push all IR
   lines to emitter in one pass).
2. **`StdlibVtableGlobalSpec` struct** (not two parallel slices): packages
   `(global_name, method_symbols)` as a struct. More idiomatic Rust —
   callers construct spec list with `vec![StdlibVtableGlobalSpec { ... }, ...]`
   syntax. Derives `PartialEq`/`Eq` for test assertions.
3. **`_batch` suffix** indicates batch version; `_globals` (plural)
   distinguishes from Stage 5.44's `emit_vtable_global_text` (singular).
   Consistent plural/singular convention across the codegen vtable API
   family.
4. **Order preserved, no dedup**: output order matches input order;
   duplicate specs produce duplicate IR lines. Dedup is caller's
   responsibility — `emit_vtables()` achieves uniqueness via
   TraitResolver.vtables HashMap keys. Adding dedup here would be
   over-engineering (O(n²) or HashMap allocation) for a responsibility
   that already belongs to the caller.
5. **Cross-check test**: `test_emit_vtable_globals_batch_matches_individual`
   verifies batch output == calling `emit_vtable_global_text()` per spec
   and collecting. Safety net for Stage 5.46 refactor.

**§16 compliance**: struct uses only `String` + `Vec<String>` — no
`mir::ty` / `traits::TraitResolver` / `Emitter` / `StdlibVtableEmission`
reference, no circular dependency.

**Test impact**: +12 (1261 → 1273).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.16 (Stage 5.46, 2026-07-23)

Stage 5.46 codegen vtable spec builder round. Pure-function extraction of
the spec-construction logic currently inlined in `emit_vtables()` (Stage 5.6).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `build_vtable_global_specs` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<noun>` |

**Design decisions**:
1. **`build_` prefix** (not `emit_`): indicates a constructor function
   (input data → output data, no side effects). This distinguishes it from
   the `emit_*` family which produce LLVM IR text or push to an emitter.
   `build_vtable_global_specs()` returns `Vec<StdlibVtableGlobalSpec>` —
   no IR text, no emitter mutation.
2. **`_specs` (plural)**: indicates multiple specs returned. Consistent
   with the plural/singular convention across the codegen vtable API
   family (`emit_vtable_global_text` singular vs `emit_vtable_globals_batch`
   plural).
3. **Same input parameters as `emit_vtables()`**: takes `&TraitResolver` +
   `&Rodeo` (minus emitter). This makes Stage 5.47 delegation a trivial
   body change — `emit_vtables()` will become:
   ```rust
   let specs = build_vtable_global_specs(trait_resolver, interner);
   let ir_lines = emit_vtable_globals_batch(&specs);
   for line in ir_lines { emitter.emit_raw_global(&line); }
   ```
4. **Byte-for-byte equivalence**: `test_build_vtable_global_specs_match_emit_vtables_inline`
   manually inlines the `emit_vtables()` construction logic and asserts
   set equality with the builder output. Safety net for Stage 5.47 refactor.
5. **HashMap order non-determinism**: `TraitResolver.vtables` is a HashMap,
   so iteration order is non-deterministic. Tests use set comparison
   (`.contains()` / `.iter().any()`) instead of positional assertions.
   The builder itself preserves HashMap iteration order (no sorting) —
   Stage 5.47's `emit_vtables()` refactor will inherit this
   non-determinism, which is acceptable because LLVM IR global definitions
   can appear in any order.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` (same as
`emit_vtables()`), returns `Vec<StdlibVtableGlobalSpec>`. No `mir::ty` /
`Emitter` reference, no circular dependency.

**Test impact**: +12 (1273 → 1285).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.17 (Stage 5.47, 2026-07-23)

Stage 5.47 codegen vtable emission orchestrator round. Adds the orchestrator
that composes Stage 5.46's `build_vtable_global_specs()` + per-spec
`Emitter::emit_vtable_global()` calls. Behavior identical to `emit_vtables()`
(Stage 5.6) inline loop.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_vtables_from_resolver` | free fn (in `codegen`) | `<verb>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. **`emit_` prefix** (not `build_`): indicates side-effect (push to emitter).
   This distinguishes it from Stage 5.46's `build_vtable_global_specs()`
   (pure function, no side effects). The orchestrator is the "pure + side-effect
   combination" version of `emit_vtables()` current inline loop.
2. **`_from_resolver` suffix**: indicates the input source (TraitResolver).
   Consistent with the `_from_emission` suffix in Stage 5.43's
   `emit_vtable_global_from_emission()`. The `_from_*` convention makes the
   input type explicit in the function name.
3. **Same input parameters as `emit_vtables()`**: takes `&TraitResolver` +
   `&Rodeo` + `&mut dyn Emitter` (identical signature minus the name). This
   makes Stage 5.48 delegation a trivial one-liner body change:
   ```rust
   pub fn emit_vtables(resolver, interner, emitter) {
       emit_vtables_from_resolver(resolver, interner, emitter)
   }
   ```
4. **Not using batch helper this round**: `Emitter::emit_vtable_global()`
   currently receives `(global_name, method_symbols)`, not pre-formatted IR
   text. Stage 5.48 will delegate `TextEmitter::emit_vtable_global()` to
   `emit_vtable_global_text()` (Stage 5.44), after which the orchestrator
   can use `emit_vtable_globals_batch()` (Stage 5.45) for direct IR text
   push. For now, the orchestrator uses the existing trait method signature
   to maintain behavior equivalence.
5. **Behavior-equivalence cross-check tests**:
   `test_emit_vtables_from_resolver_match_emit_vtables` + `_multi` call
   both `emit_vtables()` and `emit_vtables_from_resolver()` on the same
   TraitResolver + interner + TextEmitter, assert outputs are identical.
   Safety net for Stage 5.48 delegation refactor.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`
(same as `emit_vtables()`). No `mir::ty` reference, no circular dependency.

**Test impact**: +13 (1285 → 1298).
**Clippy impact**: 0 (0 warnings; fixed 1 unused import).
**Fmt impact**: clean.

### v1.18 (Stage 5.48, 2026-07-23)

Stage 5.48 codegen dynptr global text helper round. Adds the **dynptr
counterpart** of Stage 5.44's `emit_vtable_global_text()`.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dynptr_global_text` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<noun>` |

**Design decisions**:
1. **dynptr counterpart of Stage 5.44**: Stage 5.44 added
   `emit_vtable_global_text()` (vtable global pure function), Stage 5.48
   adds `emit_dynptr_global_text()` (dynptr global pure function). Naming
   symmetric (vtable → dynptr), design pattern identical — both are
   pure-function counterparts of `TextEmitter` trait methods, both take
   the same parameters as the trait method (minus `&self`), both produce
   byte-for-byte identical LLVM IR.
2. **`_text` suffix** consistent with Stage 5.44's `emit_vtable_global_text`.
   Indicates the function returns LLVM IR text as `String`, distinguishing
   it from the trait method's side-effect version
   (`TextEmitter::emit_dyn_trait_const` pushes to `self.globals`).
3. **Parameter signature match with trait method**:
   `emit_dynptr_global_text(global_name, data_symbol, vtable_symbol)` matches
   `Emitter::emit_dyn_trait_const(&self, global_name, data_symbol,
   vtable_symbol)` exactly (minus `&self`). Stage 5.49 delegation is a
   one-line body change:
   ```rust
   fn emit_dyn_trait_const(&mut self, global_name, data_symbol, vtable_symbol) -> EmitValue {
       self.globals.push(emit_dynptr_global_text(global_name, data_symbol, vtable_symbol));
       global_name.to_string()
   }
   ```
4. **Cross-check test**: `test_emit_dynptr_global_text_match_text_emitter`
   constructs (global_name, data_symbol, vtable_symbol), calls both the free
   function and `TextEmitter::emit_dyn_trait_const()`, asserts free fn output
   appears verbatim in TextEmitter output. Safety net for Stage 5.49 refactor.
5. **Symmetric naming convention** across the codegen dyn API family:
   - `emit_vtable_global_text` (Stage 5.44) — vtable global IR text
   - `emit_dynptr_global_text` (Stage 5.48) — dynptr global IR text
   - Future: `emit_vtable_global_from_emission` / `emit_dynptr_global_from_emission`
     if needed for high-level API symmetry

**§16 compliance**: pure function, input `(&str, &str, &str)`, output
`String`. No `mir::ty` / `traits::TraitResolver` / `Emitter` /
`StdlibVtableEmission` reference, no circular dependency.

**Test impact**: +12 (1298 → 1310).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.19 (Stage 5.49, 2026-07-23)

Stage 5.49 codegen dynptr spec builder round. Adds the **dynptr counterpart**
of Stage 5.46's `build_vtable_global_specs()`. Pure-function extraction of
the spec-construction logic currently inlined in `emit_dyn_trait_ptrs()`
(Stage 5.7).

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibDynptrGlobalSpec` | struct (in `codegen`) | `<Noun><Noun><Noun><Noun>` |
| `build_dynptr_global_specs` | free fn (in `codegen`) | `<verb>_<noun>_<adj>_<noun>` |

**Field naming (3 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `global_name` | `String` | `<noun>_<noun>` |
| `data_symbol` | `String` | `<noun>_<noun>` |
| `vtable_symbol` | `String` | `<noun>_<noun>` |

**Design decisions**:
1. **dynptr counterpart of Stage 5.46**: Stage 5.46 added
   `build_vtable_global_specs()` (vtable spec builder), Stage 5.49 adds
   `build_dynptr_global_specs()` (dynptr spec builder). Naming symmetric
   (vtable → dynptr), design pattern identical — both are pure-function
   extractions of `emit_*()` inline construction logic, both take
   `(&TraitResolver, &Rodeo)`, both return `Vec<Stdlib*GlobalSpec>`.
2. **`StdlibDynptrGlobalSpec` struct** (dynptr counterpart of Stage 5.45's
   `StdlibVtableGlobalSpec`): packages the three inputs needed by
   `emit_dynptr_global_text()` (Stage 5.48) — `(global_name, data_symbol,
   vtable_symbol)`. The vtable counterpart packages `(global_name,
   method_symbols)` — different fields because vtable and dynptr globals
   have different LLVM IR shapes.
3. **`build_` prefix** (not `emit_`): indicates a constructor function
   (input data → output data, no side effects). Consistent with Stage 5.46's
   `build_vtable_global_specs()`. Distinguishes from the `emit_*` family
   which produce side effects (push to emitter).
4. **`_specs` (plural)**: indicates multiple specs returned. Consistent
   with the plural/singular convention across the codegen vtable+dynptr API
   family.
5. **Symmetric naming convention** across the codegen dyn API family:
   - vtable: `StdlibVtableGlobalSpec` (5.45) + `build_vtable_global_specs` (5.46)
   - dynptr: `StdlibDynptrGlobalSpec` (5.49) + `build_dynptr_global_specs` (5.49)
   - The `Vtable`/`Dynptr` noun in the type/fn name makes the global kind
     explicit, avoiding ambiguity.
6. **Byte-for-byte equivalence**: `test_build_dynptr_global_specs_match_emit_dyn_trait_ptrs`
   manually inlines the `emit_dyn_trait_ptrs()` construction logic and asserts
   set equality with the builder output. Safety net for Stage 5.50 refactor.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` (same as
`emit_dyn_trait_ptrs()`), returns `Vec<StdlibDynptrGlobalSpec>`. No
`mir::ty` / `Emitter` reference, no circular dependency.

**Test impact**: +12 (1310 → 1322).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.20 (Stage 5.50, 2026-07-23)

Stage 5.50 codegen dynptr emission orchestrator round. Adds the **dynptr
counterpart** of Stage 5.47's `emit_vtables_from_resolver()`. Orchestrator
that composes Stage 5.49's `build_dynptr_global_specs()` + per-spec
`Emitter::emit_dyn_trait_const()` calls.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dynptrs_from_resolver` | free fn (in `codegen`) | `<verb>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. **dynptr counterpart of Stage 5.47**: Stage 5.47 added
   `emit_vtables_from_resolver()` (vtable orchestrator), Stage 5.50 adds
   `emit_dynptrs_from_resolver()` (dynptr orchestrator). Naming symmetric
   (vtables → dynptrs), design pattern identical — both are "pure-function +
   side-effect combination" versions of `emit_*()` current inline loops,
   both take `(&TraitResolver, &Rodeo, &mut dyn Emitter)`.
2. **`emit_` prefix** (not `build_`): indicates side-effect (push to emitter).
   Consistent with Stage 5.47's `emit_vtables_from_resolver()`. Distinguishes
   from Stage 5.49's `build_dynptr_global_specs()` (pure function, no side
   effects).
3. **`_from_resolver` suffix**: indicates the input source (TraitResolver).
   Consistent with Stage 5.47's `emit_vtables_from_resolver()`.
4. **Same input parameters as `emit_dyn_trait_ptrs()`**: takes
   `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter` (identical signature
   minus the name). Stage 5.51 delegation is a trivial one-liner body change:
   ```rust
   pub fn emit_dyn_trait_ptrs(resolver, interner, emitter) {
       emit_dynptrs_from_resolver(resolver, interner, emitter)
   }
   ```
5. **Behavior-equivalence cross-check tests**:
   `test_emit_dynptrs_from_resolver_match_emit_dyn_trait_ptrs` + `_multi`
   call both `emit_dyn_trait_ptrs()` and `emit_dynptrs_from_resolver()` on
   the same TraitResolver + interner + TextEmitter, assert outputs are
   identical. Safety net for Stage 5.51 delegation refactor.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`
(same as `emit_dyn_trait_ptrs()`). No `mir::ty` reference, no circular dependency.

**Test impact**: +12 (1322 → 1334).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.21 (Stage 5.51, 2026-07-23)

Stage 5.51 codegen vtable + dynptr combined emission orchestrator round.
Adds the **single entry point** for emitting all trait-dispatch globals
(vtable + dynptr) in one call.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_vtables_and_dynptrs_from_resolver` | free fn (in `codegen`) | `<verb>_<noun>_<conj>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. **`_and_` conjunction** connects the two noun phrases (vtables + dynptrs).
   This is the first codegen API name to use `_and_` — justified because the
   function genuinely does two distinct things (emit vtable globals AND emit
   dynptr globals), and the name makes this explicit. Alternative
   `emit_trait_dispatch_globals_from_resolver` was considered but rejected
   because "trait_dispatch_globals" is vaguer than "vtables_and_dynptrs".
2. **`emit_` prefix** consistent with the rest of the codegen orchestrator
   family (`emit_vtables_from_resolver`, `emit_dynptrs_from_resolver`).
   Indicates side-effect (push to emitter).
3. **`_from_resolver` suffix** consistent with Stage 5.47 + Stage 5.50
   orchestrators. Indicates the input source (TraitResolver).
4. **Compositional**: internally calls Stage 5.47 `emit_vtables_from_resolver()`
   + Stage 5.50 `emit_dynptrs_from_resolver()`. Single source of truth — no
   duplicated logic. If the underlying orchestrators change behavior, the
   combined orchestrator automatically inherits the change.
5. **Behavior-equivalence cross-check test**:
   `test_emit_vtables_and_dynptrs_match_separate_calls` calls both the
   combined orchestrator and the separate `emit_vtables()` +
   `emit_dyn_trait_ptrs()` pair on the same inputs, asserts outputs are
   identical. Safety net for Stage 5.52 driver refactor.
6. **Order guarantee**: vtable globals emitted before dynptr globals (because
   `emit_vtables_from_resolver` is called first). This matches the existing
   driver call order. Verified by `test_emit_vtables_and_dynptrs_order`.
7. **Counting subtlety in tests**: `@.vtable.` appears both in vtable global
   definitions AND in dynptr initializers (`ptr @.vtable.X.Y`). Tests count
   global *definitions* (lines starting with `@.vtable.` + `private
   unnamed_addr constant`) rather than raw substring matches, to avoid
   double-counting.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` + `&mut dyn Emitter`
(same as `emit_vtables()` + `emit_dyn_trait_ptrs()`). No `mir::ty` reference,
no circular dependency.

**Test impact**: +12 (1334 → 1346).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.22 (Stage 5.52, 2026-07-23)

Stage 5.52 codegen trait-dispatch emission summary round. Adds the
**codegen counterpart** of Stage 5.42's `stdlib_vtable_emission_summary()`,
computed directly from `TraitResolver`.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `CodegenTraitDispatchEmissionSummary` | struct (in `codegen`) | `<Noun><Noun><Noun><Noun><Noun>` |
| `build_trait_dispatch_emission_summary` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |

**Field naming (6 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `vtable_count` | `u32` | `<noun>_<noun>` |
| `dynptr_count` | `u32` | `<noun>_<noun>` |
| `total_global_count` | `u32` | `<adj>_<noun>_<noun>` |
| `trait_names` | `Vec<String>` | `<noun>_<noun>` |
| `type_names` | `Vec<String>` | `<noun>_<noun>` |
| `total_method_slots` | `u32` | `<adj>_<noun>_<noun>` |

**Design decisions**:
1. **codegen counterpart of Stage 5.42**: Stage 5.42 added
   `stdlib_vtable_emission_summary()` (computed from `StdlibVtableEmission`
   list, for stdlib API layer), Stage 5.52 adds
   `build_trait_dispatch_emission_summary()` (computed directly from
   `TraitResolver`, for codegen diagnostic layer). The two are complementary
   — different input sources, different use cases, but same aggregate-statistics
   purpose.
2. **`Codegen` prefix** (not `Stdlib`): distinguishes from Stage 5.42's
   `StdlibVtableEmissionSummary`. Makes the layer (codegen vs stdlib) explicit
   in the type name. Consistent with the `Stdlib*` / `Codegen*` prefix
   convention.
3. **`String` (not `&'static str`)** for `trait_names` / `type_names`:
   unlike stdlib summary (which uses `&'static str` for stdlib-registered
   trait names), codegen summary uses `String` because trait/type names come
   from the interner at runtime (user-defined traits/types), not from static
   stdlib tables.
4. **`build_` prefix** (not `emit_`): indicates a constructor function
   (input data → output data, no side effects). Consistent with Stage 5.46's
   `build_vtable_global_specs()` and Stage 5.49's `build_dynptr_global_specs()`.
5. **`_summary` suffix**: indicates the function returns a summary struct
   (not individual specs). Consistent with Stage 5.42's
   `stdlib_vtable_emission_summary()`.
6. **Deduplication**: `trait_names` and `type_names` are deduplicated — same
   trait on multiple types produces one trait name; same type with multiple
   traits produces one type name. This avoids double-counting in diagnostics.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` (same as
`emit_vtables()`), returns `CodegenTraitDispatchEmissionSummary`. No
`mir::ty` / `Emitter` reference, no circular dependency.

**Test impact**: +14 (1346 → 1360).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.23 (Stage 5.53, 2026-07-23)

Stage 5.53 codegen trait-dispatch emission plan (final aggregate) round.
Adds the **final aggregate API** that returns vtable_specs + dynptr_specs +
summary in one call. Composes Stage 5.46 + Stage 5.49 + Stage 5.52 builders.

**New public symbols (all §23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `CodegenTraitDispatchEmissionPlan` | struct (in `codegen`) | `<Noun><Noun><Noun><Noun><Noun>` |
| `build_trait_dispatch_emission_plan` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |

**Field naming (3 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `vtable_specs` | `Vec<StdlibVtableGlobalSpec>` | `<noun>_<noun>` |
| `dynptr_specs` | `Vec<StdlibDynptrGlobalSpec>` | `<noun>_<noun>` |
| `summary` | `CodegenTraitDispatchEmissionSummary` | `<noun>` |

**Design decisions**:
1. **Final aggregate API**: `build_trait_dispatch_emission_plan()` is the
   one-call API that returns everything codegen needs to emit all
   trait-dispatch globals. Stage 5.54 driver refactor becomes a clean 4-liner:
   build plan, iterate vtable_specs, iterate dynptr_specs, print summary.
2. **Compositional**: internally calls Stage 5.46 `build_vtable_global_specs()`
   + Stage 5.49 `build_dynptr_global_specs()` + Stage 5.52
   `build_trait_dispatch_emission_summary()`. Single source of truth — no
   duplicated logic. If any underlying builder changes behavior, the plan
   automatically inherits the change.
3. **`Codegen` prefix** (not `Stdlib`): distinguishes from stdlib's
   `StdlibVtablePlan` (Stage 5.39). Makes the layer (codegen vs stdlib)
   explicit in the type name. Consistent with the `Stdlib*` / `Codegen*`
   prefix convention.
4. **`build_` prefix** (not `emit_`): indicates a constructor function
   (input data → output data, no side effects). Consistent with Stage 5.46's
   `build_vtable_global_specs()` and Stage 5.49's `build_dynptr_global_specs()`.
5. **`_plan` suffix**: indicates the function returns a plan struct (not
   individual specs). Consistent with Stage 5.39's `stdlib_vtable_plan()`.
   The plan struct is the natural unit for "everything needed to do X" —
   caller gets one value, accesses fields, doesn't need to coordinate
   multiple separate calls.
6. **Behavior-equivalence cross-check test**:
   `test_build_trait_dispatch_emission_plan_match_separate_calls` calls both
   the plan and the three separate builders on the same inputs, asserts
   fields are identical (summary direct equality, specs set equality due to
   HashMap order). Safety net for Stage 5.54 driver refactor.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` (same as
`emit_vtables()`), returns `CodegenTraitDispatchEmissionPlan`. No
`mir::ty` / `Emitter` reference, no circular dependency.

**Test impact**: +12 (1360 → 1372).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.24 (Stage 5.54, 2026-07-23)

Stage 5.54 codegen trait-dispatch emission orchestrator (plan-based) round.
Adds the **first plan-based orchestrator** — consumes a
`CodegenTraitDispatchEmissionPlan` (Stage 5.53) rather than a resolver.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_trait_dispatch_globals_from_plan` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. **First plan-based orchestrator**: previous orchestrators (Stage 5.47,
   5.50, 5.51) take `(&TraitResolver, &Rodeo, &mut dyn Emitter)` — they
   combine "build specs" + "emit" in one call. Stage 5.54 takes
   `(&CodegenTraitDispatchEmissionPlan, &mut dyn Emitter)` — it separates
   "build plan" (Stage 5.53) from "emit from plan". This separation lets
   callers inspect/modify the plan before emission (e.g. for diagnostics,
   caching, or partial emission).
2. **`emit_` prefix** (not `build_`): indicates side-effect (push to emitter).
   Consistent with Stage 5.47/5.50/5.51 orchestrators.
3. **`_from_plan` suffix** (not `_from_resolver`): indicates the input source
   is a plan (not a resolver). Distinguishes from Stage 5.51's
   `emit_vtables_and_dynptrs_from_resolver`. The `_from_*` convention makes
   the input type explicit in the function name.
4. **`_globals` (plural)** in the function name: indicates multiple globals
   are emitted (vtable + dynptr). Consistent with the plural/singular
   convention.
5. **Decoupling from resolver**: the plan-based signature
   `(&CodegenTraitDispatchEmissionPlan, &mut dyn Emitter)` decouples the
   orchestrator from `TraitResolver` / `Rodeo`. Callers can construct plans
   from any source (not just TraitResolver) — e.g. from a cached plan, from
   a deserialized plan, or from a test fixture. This is a design improvement
   over resolver-based orchestrators.
6. **Behavior-equivalence cross-check test**:
   `test_emit_trait_dispatch_globals_from_plan_match_resolver_orchestrator`
   calls both the plan-based orchestrator and the resolver-based orchestrator
   (Stage 5.51) on the same resolver, asserts outputs are identical. Safety
   net for Stage 5.55 driver refactor.

**§16 compliance**: function takes `&CodegenTraitDispatchEmissionPlan` +
`&mut dyn Emitter`. No `mir::ty` / `TraitResolver` / `Rodeo` reference, no
circular dependency.

**Test impact**: +12 (1372 → 1384).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.25 (Stage 5.55, 2026-07-23)

Stage 5.55 codegen trait-dispatch emission text batch (plan-based) round.
Adds the **plan-based counterpart** of Stage 5.45's
`emit_vtable_globals_batch()`, extended to vtable + dynptr. Generates all
LLVM IR text WITHOUT needing an Emitter.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_trait_dispatch_globals_text_batch` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` |

**Design decisions**:
1. **plan-based counterpart of Stage 5.45**: Stage 5.45 added
   `emit_vtable_globals_batch()` (vtable only, input
   `&[StdlibVtableGlobalSpec]`), Stage 5.55 adds
   `emit_trait_dispatch_globals_text_batch()` (vtable + dynptr, input
   `&CodegenTraitDispatchEmissionPlan`). Both return `Vec<String>` — no
   Emitter needed. The `_text_batch` suffix is consistent across both.
2. **`emit_` prefix** (not `build_`): indicates the function produces output
   (LLVM IR text), even though it has no side effects (no Emitter push).
   This is a slight naming tension — `emit_` usually implies side effects,
   but here it means "produce IR text". The `_text` suffix clarifies that
   the output is text (not emitter mutation). Alternative `build_*_text_batch`
   was considered but rejected for consistency with Stage 5.45's
   `emit_vtable_globals_batch` naming.
3. **No Emitter needed**: the function works without any `Emitter` trait
   object. Useful for:
   - Testing (assert IR text directly, no Emitter construction)
   - Future codegen paths that push pre-formatted text to emitter.globals
   - Diagnostics (inspect IR lines before emission)
4. **Behavior-equivalence cross-check test**:
   `test_emit_trait_dispatch_globals_text_batch_match_orchestrator` calls
   both the text batch and the orchestrator (Stage 5.54, via Emitter) on
   the same plan, asserts each text line appears in the emitter output.
5. **Order guarantee**: vtable lines first, then dynptr lines (matching
   Stage 5.54 order).

**§16 compliance**: function takes `&CodegenTraitDispatchEmissionPlan`,
returns `Vec<String>`. No `mir::ty` / `Emitter` / `TraitResolver` / `Rodeo`
reference, no circular dependency.

**Test impact**: +12 (1384 → 1396).
**Clippy impact**: 0 (0 warnings; fixed 1 `doc_lazy_continuation` by
rephrasing "vtable + dynptr" → "vtable and dynptr" in doc comment).
**Fmt impact**: clean.

### v1.26 (Stage 5.56, 2026-07-23)

Stage 5.56 codegen trait-dispatch emission text batch from resolver round.
Adds the **convenience entry point** — one call from resolver to all
trait-dispatch IR text.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_trait_dispatch_globals_text_batch_from_resolver` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. **Convenience entry point**: single function from `(&TraitResolver, &Rodeo)`
   to `Vec<String>`. Composes Stage 5.53 `build_trait_dispatch_emission_plan()`
   + Stage 5.55 `emit_trait_dispatch_globals_text_batch()`. Callers who don't
   need to inspect the plan can use this one-liner instead of the two-step
   approach.
2. **`_from_resolver` suffix** (not `_from_plan`): indicates the input source
   is a resolver (not a plan). Distinguishes from Stage 5.55's
   `emit_trait_dispatch_globals_text_batch` (plan-based). The `_from_*`
   convention makes the input type explicit in the function name.
3. **Naming tension with `_from_resolver` suffix**: the function name is long
   (`emit_trait_dispatch_globals_text_batch_from_resolver` — 7 words). This is
   justified because:
   - Each word adds a meaningful scope qualifier (emit → trait_dispatch →
     globals → text → batch → from → resolver)
   - Shorter alternatives (`emit_all_trait_dispatch_text(r, i)` or
     `emit_trait_dispatch_text(r, i)`) were considered but rejected for
     consistency with the Stage 5.55 naming pattern
4. **Two behavior-equivalence cross-check tests**:
   - `test_match_separate_emit_vtables_and_dyn_trait_ptrs` — vs existing
     `emit_vtables()` + `emit_dyn_trait_ptrs()` via Emitter
   - `test_match_plan_based_text_batch` — vs Stage 5.55 plan-based approach
   Both verify the convenience entry produces identical IR to alternative paths.
5. **No Emitter needed**: works without any Emitter trait object. The
   `_text_batch` suffix (consistent with Stage 5.45 + Stage 5.55) indicates
   this.

**§16 compliance**: function takes `&TraitResolver` + `&Rodeo` (same as
`emit_vtables()`), returns `Vec<String>`. No `mir::ty` / `Emitter` reference,
no circular dependency.

**Test impact**: +12 (1396 → 1408).
**Clippy impact**: 0 (0 warnings; fixed 1 unused import).
**Fmt impact**: clean.

### v1.27 (Stage 5.57, 2026-07-23)

Stage 5.57 TextEmitter::emit_vtable_global delegation round. **First
existing-path modification** in Stage 5 — replaces trait method body with
delegation to a free function.

**No new public symbols** — only modifies existing `TextEmitter::emit_vtable_global()`
trait method body.

**Design decisions**:
1. **First existing-path modification**: 5.36-5.56 all added parallel free
   functions without touching existing code. Stage 5.57 is the first to
   modify an existing trait method body — replacing inline `format!` logic
   with a delegation call to Stage 5.44's `emit_vtable_global_text()`.
2. **Behavior equivalence (non-null paths)**: the delegated free function
   produces byte-for-byte identical IR to the old inline code on non-null
   paths. Guaranteed by Stage 5.44's 14 cross-check tests.
3. **Null-handling bug fix**: the old inline code would emit `ptr @null` for
   "null" strings (because it unconditionally prepended `@` to every symbol).
   The free function correctly detects "null" and emits `ptr null` (no `@`).
   This is a latent bug fix — `emit_vtables()` never passes "null" symbols
   (only real symbols from `VtableEntry.fn_name`), so the bug was never
   triggered in practice. But the delegation makes the code correct for all
   inputs.
4. **No regression**: all 1408 existing tests pass + 10 new = 1418 total.
   `test_text_emitter_vtable_global_delegation_no_regression` explicitly
   verifies that `emit_vtables()` (which internally calls
   `emit_vtable_global()`) still produces correct output after delegation.
5. **§16 compliance**: `TextEmitter` calls `crate::codegen::emit_vtable_global_text()`
   (same-module free function). No cross-module dependency issue.

**Test impact**: +10 (1408 → 1418).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.28 (Stage 5.58, 2026-07-23)

Stage 5.58 TextEmitter::emit_dyn_trait_const delegation round. Second
existing-path modification — replaces trait method body with delegation to
a free function.

**No new public symbols** — only modifies existing `TextEmitter::emit_dyn_trait_const()`
trait method body.

**Design decisions**:
1. **Second existing-path modification**: follows the same pattern as Stage
   5.57 (vtable delegation). `TextEmitter::emit_dyn_trait_const()` method
   body replaced with delegation to Stage 5.48's `emit_dynptr_global_text()`.
2. **Behavior equivalence (all paths)**: dynptr globals have no null-handling
   issue (unlike vtable globals), so all paths are byte-for-byte identical
   to the old inline code.
3. **No regression**: all 1418 existing tests pass + 10 new = 1428 total.
4. **§16 compliance**: `TextEmitter` calls `crate::codegen::emit_dynptr_global_text()`
   (same-module free function). No cross-module dependency issue.

**Test impact**: +10 (1418 → 1428).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.29 (Stage 5.59, 2026-07-23)

Stage 5.59 emit_vtables delegation round. Third existing-path modification.

**No new public symbols** — only modifies existing `emit_vtables()` function body.

**Design decisions**: Same pattern as Stage 5.57/5.58 — one-liner delegation
to a free function (Stage 5.47 `emit_vtables_from_resolver()`). Behavior-
equivalent (verified by Stage 5.47 cross-check tests). No regression.

**Test impact**: +7 (1428 → 1435).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.30 (Stage 5.60, 2026-07-23)

Stage 5.60 emit_dyn_trait_ptrs delegation round. **Fourth and final
existing-path modification**. Codegen delegation complete.

**No new public symbols** — only modifies existing `emit_dyn_trait_ptrs()`
function body.

**Design decisions**: Same pattern as Stage 5.57/5.58/5.59 — one-liner
delegation to a free function (Stage 5.50 `emit_dynptrs_from_resolver()`).
Behavior-equivalent (verified by Stage 5.50 cross-check tests). No regression.

**Milestone**: Codegen trait-dispatch emission delegation complete (5.57-5.60).
All four existing codegen paths now delegate to free functions:
- `TextEmitter::emit_vtable_global()` → `emit_vtable_global_text()` (5.44)
- `TextEmitter::emit_dyn_trait_const()` → `emit_dynptr_global_text()` (5.48)
- `emit_vtables()` → `emit_vtables_from_resolver()` (5.47)
- `emit_dyn_trait_ptrs()` → `emit_dynptrs_from_resolver()` (5.50)

Codegen trait-dispatch emission logic is **fully centralized** in free functions.
`TextEmitter` and `emit_*()` are now thin wrappers that delegate + push.

**Test impact**: +7 (1435 → 1442).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.31 (Stage 5.61, 2026-07-23)

Stage 5.61 DynTraitFatPtr MIR-level representation round. **Start of dyn
Trait MIR lowering** — the core Stage 5 goal.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `DynTraitFatPtr` | struct (in `mir`) | `<Noun><Noun><Noun>` |

**Field naming (5 fields, all §23-compliant)**:

| Field | Type | Naming pattern |
|-------|------|----------------|
| `trait_name` | `String` | `<noun>_<noun>` |
| `type_name` | `String` | `<noun>_<noun>` |
| `data_symbol` | `String` | `<noun>_<noun>` |
| `vtable_symbol` | `String` | `<noun>_<noun>` |
| `dynptr_symbol` | `String` | `<noun>_<noun>` |

**Design decisions**:
1. **New MIR module**: `src/mir/dyn_trait.rs` — first new file in the `mir/`
   module since Stage 3. Placed alongside `ty.rs`, `place.rs`, `body.rs`.
2. **`DynTraitFatPtr` follows `<Noun><Noun><Noun>`**: `Dyn` (qualifier) +
   `Trait` (domain) + `FatPtr` (kind). Consistent with Rust's own
   `DynTrait` naming + the "FatPtr" terminology used in the codegen module
   (`emit_fat_ptr_type`).
3. **`new()` constructor auto-computes LLVM symbols**: the constructor takes
   `(trait_name, type_name)` and computes `data_symbol`, `vtable_symbol`,
   `dynptr_symbol` using the same naming convention as the codegen module.
   This avoids duplication — callers don't need to know the LLVM symbol
   format.
4. **`is_marker()` method**: checks if the trait is a marker (Copy/Send/
   Sync/Sized/Unpin/Eq). Marker traits have empty vtables. Uses the same
   marker list as `stdlib::is_stdlib_marker_trait()`.
5. **§16 compliance**: uses only `String` — no `mir::ty` / `codegen::EmitType`
   / `traits::TraitResolver` reference, no circular dependency. The struct
   is a pure data type that can be constructed and queried without any
   dependency on other compiler stages.

**Test impact**: +9 (1442 → 1451).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.32 (Stage 5.62, 2026-07-23)

Stage 5.62 build_dyn_trait_fat_ptrs_from_resolver round. Bridge function
connecting DynTraitFatPtr (MIR) with TraitResolver (data source).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `build_dyn_trait_fat_ptrs_from_resolver` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Design decisions**: Same `build_*_from_resolver` pattern as Stage 5.46/5.49.
Bridges MIR representation with resolver data. §16 compliant (no circular dep).

**Test impact**: +8 (1451 → 1459).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.33 (Stage 5.63, 2026-07-23)

Stage 5.63 emit_dyn_trait_fat_ptr_text round. Conversion function bridging
DynTraitFatPtr (MIR) with codegen text output.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_fat_ptr_text` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |

**Test impact**: +8 (1459 → 1467).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.34 (Stage 5.64, 2026-07-23)

Stage 5.64 emit_dyn_trait_fat_ptrs_text_batch round. Batch version of
Stage 5.63. **Dyn Trait fat ptr infrastructure complete (5.61-5.64)**.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_fat_ptrs_text_batch` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` |

**Test impact**: +8 (1467 → 1475).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.35 (Stage 5.65, 2026-07-23)

Stage 5.65 emit_dyn_trait_fat_ptrs_text_batch_from_resolver round.
Convenience entry point composing Stage 5.62 + 5.64.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_fat_ptrs_text_batch_from_resolver` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Test impact**: +8 (1475 → 1483).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.36 (Stage 5.66, 2026-07-23)

Stage 5.66 DynTraitMethodCall MIR representation round. **Last infrastructure
piece** before actual method call MIR lowering.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `DynTraitMethodCall` | struct (in `mir`) | `<Noun><Noun><Noun>` |

**Test impact**: +10 (1483 → 1493).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.37 (Stage 5.67, 2026-07-24)

Stage 5.67 emit_dyn_trait_method_call_text round. First substantive dyn
Trait method call lowering.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_method_call_text` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |

**Test impact**: +10 (1493 → 1503).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.38 (Stage 5.68, 2026-07-24)

Stage 5.68 build_dyn_trait_method_calls_from_fat_ptrs round. Bridge function
connecting stdlib trait method index with DynTraitMethodCall.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `build_dyn_trait_method_calls_from_fat_ptrs` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` |

**Test impact**: +10 (1503 → 1513).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.39 (Stage 5.69, 2026-07-24)

Stage 5.69 emit_dyn_trait_method_calls_text_batch round. Batch version of
Stage 5.67.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_method_calls_text_batch` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>` |

**Test impact**: +8 (1513 → 1521).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.40 (Stage 5.70, 2026-07-24)

Stage 5.70 emit_dyn_trait_method_calls_text_batch_from_resolver round.
Convenience entry point composing Stage 5.62 + 5.68 + 5.69.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_method_calls_text_batch_from_resolver` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Test impact**: +8 (1521 → 1529).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.41 (Stage 5.71, 2026-07-24)

Stage 5.71 DynTraitMIRSummary round. Project-level summary of dyn Trait MIR data.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `DynTraitMIRSummary` | struct (in `mir`) | `<Noun><Noun><Noun><Noun>` |
| `build_dyn_trait_mir_summary` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>` |

**Test impact**: +9 (1529 → 1538).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.42 (Stage 5.72, 2026-07-24)

Stage 5.72 build_dyn_trait_mir_summary_from_resolver round. Convenience
entry point composing Stage 5.62 + 5.68 + 5.71.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `build_dyn_trait_mir_summary_from_resolver` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Test impact**: +8 (1538 → 1546).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.43 (Stage 5.73, 2026-07-24)

Stage 5.73 DynTraitMIRPlan round. Final aggregate API combining fat_ptrs +
method_calls + summary.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `DynTraitMIRPlan` | struct (in `mir`) | `<Noun><Noun><Noun><Noun>` |
| `build_dyn_trait_mir_plan` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>` |
| `build_dyn_trait_mir_plan_from_resolver` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Test impact**: +9 (1546 → 1555).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.44 (Stage 5.74, 2026-07-24)

Stage 5.74 emit_dyn_trait_mir_plan_text round. Complete IR text generator.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_mir_plan_text` | free fn (in `mir`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |

**Test impact**: +8 (1555 → 1563).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.45 (Stage 5.75, 2026-07-24)

Stage 5.75 find_dyn_trait_method_call_in_plan round. FIRST query API on
`DynTraitMIRPlan` — single-point lookup of a `DynTraitMethodCall` by
`(trait_name, type_name, method_name)`. All prior dyn Trait MIR APIs
(5.61-5.74) were whole-plan builders / emitters; Stage 5.75 is the first
single-point lookup, enabling `mir/lower/` integration.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `find_dyn_trait_method_call_in_plan` | free fn (in `mir`) | `find_<noun>_<noun>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. Helper-verb `find_` prefix per §8.1, mirroring `find_stdlib_trait_method`
   from v1.6 (Stage 5.36) — establishes the same "lookup-style" convention
   across stdlib + mir layers.
2. First-match-wins semantics — when multiple `DynTraitMethodCall` entries
   share the same `(trait, type, method)` triple, the first one is returned.
   This is uncommon (upstream construction normally produces unique triples)
   but is documented and tested.
3. Case-sensitive exact string equality — no fuzzy matching, no normalization.
   This matches the strictness of `find_stdlib_trait_method` and avoids
   silent type-resolution surprises at MIR-lower time.
4. Pure read function: `(&DynTraitMIRPlan, &str, &str, &str) ->
   Option<&DynTraitMethodCall>`. No mutation, no side effects. §16-compliant:
   data flow stays entirely within `mir::dyn_trait`.

**§16 compliance**: Pure read; no new dependencies introduced. The function
lives in `src/mir/dyn_trait.rs` alongside the `DynTraitMIRPlan` it queries.

**Test impact**: +12 (1563 → 1575).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.46 (Stage 5.76, 2026-07-24)

Stage 5.76 MirLowerCtxt dyn_trait_plan field + setter/getter round. First
mir/lower integration step — context wiring only. Adds a
`dyn_trait_plan: Option<DynTraitMIRPlan>` field to `MirLowerCtxt` plus a
`set_dyn_trait_plan()` setter and `dyn_trait_plan()` getter. No lowering
logic changes (those land in Stage 5.77+).

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `MirLowerCtxt::set_dyn_trait_plan` | method (in `mir::lower`) | `<verb>_<noun>_<noun>_<noun>` (setter) |
| `MirLowerCtxt::dyn_trait_plan` | method (in `mir::lower`) | `<noun>_<noun>_<noun>` (getter, no `get_` prefix) |
| `MirLowerCtxt.dyn_trait_plan` | pub field (in `mir::lower`) | `<noun>_<noun>_<noun>` |

**Design decisions**:
1. Setter takes owned `DynTraitMIRPlan` (by value); context holds ownership.
   This mirrors the established pattern for context-attached data (e.g.,
   `cx.hir = Some(hir)` in `lower_hir_body_to_mir_full`).
2. Getter returns `Option<&DynTraitMIRPlan>` — read-only reference, no
   cloning. Callers can pattern-match on `Some(plan)` to get a `&DynTraitMIRPlan`
   and then call `find_dyn_trait_method_call_in_plan()` (Stage 5.75) for
   per-method-call lookup.
3. **No `unset_dyn_trait_plan` method** — once a plan is attached, it stays
   for the lifetime of the lowering context. This is consistent with the
   `hir` field semantics (also `Option`, also set once at construction).
   Avoids footguns where a caller unsets mid-lower and breaks invariants.
4. Getter uses Rust C-GETTER convention — no `get_` prefix. Field name and
   getter name are the same (`dyn_trait_plan`), which is the rust-api-guidelines
   pattern for accessor methods.
5. Pub field — caller can read `cx.dyn_trait_plan` directly OR via the
   getter. Both work. The getter exists for future trait-based abstraction
   (e.g., if `MirLowerCtxt` ever implements a `LowerContext` trait).

**§16 compliance**: `DynTraitMIRPlan` is defined in `mir::dyn_trait` (Stage
5.73). `MirLowerCtxt` lives in `mir::lower`. Data flow: driver builds plan
upstream via `build_dyn_trait_mir_plan_from_resolver()` → passes plan by
value to `cx.set_dyn_trait_plan()` → `mir::lower` reads via
`cx.dyn_trait_plan()`. `MirLowerCtxt` does not own a `TraitResolver`. No
circular dependency; data flows one way (driver → cx → lower).

**Test impact**: +11 (1575 → 1586).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.47 (Stage 5.77, 2026-07-24)

Stage 5.77 find_dyn_trait_method_call_in_plan_by_method round. Fuzzy lookup
variant of Stage 5.75's exact lookup — looks up a `DynTraitMethodCall` in a
`DynTraitMIRPlan` by `method_name` only (no trait/type required).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `find_dyn_trait_method_call_in_plan_by_method` | free fn (in `mir`) | `find_<noun>_<noun>_<noun>_<prep>_<noun>_<prep>_<noun>` |

**Design decisions**:
1. `_by_method` suffix — Rust API-guidelines convention for field-filter
   functions (mirrors `iter_by`, `get_by`). Distinguishes from 5.75's
   `_in_plan` (no suffix) which uses the full triple.
2. First-match-wins semantics — when multiple `DynTraitMethodCall` entries
   share the same `method_name`, the first one is returned. At MIR-lower
   time we cannot disambiguate trait/type, so this is intentional. The
   caller (Stage 5.78+) should treat the result as a candidate.
3. Case-sensitive exact string equality — same strictness as 5.75.
4. Pure read function: `(&DynTraitMIRPlan, &str) -> Option<&DynTraitMethodCall>`.
   No mutation, no side effects. §16-compliant: data flow stays entirely
   within `mir::dyn_trait`.

**§16 compliance**: Pure read; no new dependencies introduced. Same module
as Stage 5.75's `find_dyn_trait_method_call_in_plan`.

**Test impact**: +12 (1586 → 1598).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.48 (Stage 5.78, 2026-07-24)

Stage 5.78 HirExprKind::MethodCall dyn Trait integration round. FIRST
real `mir/lower` integration of dyn Trait data. Adds a
`MirBody.dyn_trait_calls: Vec<DynTraitMethodCall>` side-table + a
`build_dyn_trait_call_terminator()` helper, and modifies the
`HirExprKind::MethodCall` branch to use them when `cx.dyn_trait_plan()`
returns `Some` and the method_name matches.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `build_dyn_trait_call_terminator` | free fn (in `mir::lower`) | `<verb>_<noun>_<noun>_<noun>_<noun>` |
| `MirBody.dyn_trait_calls` | pub field (in `mir::body`) | `<noun>_<noun>_<noun>` (plural) |

**Design decisions**:
1. **Side-table pattern** (§16-compliant): MIR carries the dyn Trait
   call info as data on `MirBody.dyn_trait_calls`. The corresponding
   `Terminator::Call`'s `func` operand is a marker `Const{ty: Error,
   val: Int(index)}` where `index` is the side-table entry position.
   Codegen (Stage 5.79+) detects this marker and emits a vtable indirect
   call. This avoids needing a new `Operand` variant or `ConstVal` variant
   — the marker convention is internal between mir::lower and codegen.
2. **`build_` prefix** per §8.1 — same family as `build_dyn_trait_mir_plan`
   (Stage 5.73), `build_dyn_trait_fat_ptrs_from_resolver` (Stage 5.62),
   `build_dyn_trait_method_calls_from_fat_ptrs` (Stage 5.68). All
   "construct an IR object from a dyn Trait source" helpers use `build_`.
3. **Borrow-checker workaround**: the `HirExprKind::MethodCall` branch
   clones the matched `DynTraitMethodCall` out of the immutable borrow
   scope (`cx.dyn_trait_plan()` returns `Option<&DynTraitMIRPlan>`)
   before mutably borrowing `cx` via `build_dyn_trait_call_terminator`.
   This is the standard Rust "read-then-mutate" pattern on the same struct.
4. **Backward compatibility**: when `cx.dyn_trait_plan()` is `None` (the
   default — no plan attached) OR when method_name doesn't match, the
   branch falls through to the legacy Stage 2.1 placeholder path. All
   1598 pre-existing tests pass unchanged.
5. **Side-table initialized empty** in `MirBody::new()` — zero overhead
   for bodies without dyn Trait calls (the common case).

**§16 compliance**: `DynTraitMethodCall` defined in `mir::dyn_trait`.
`MirBody` lives in `mir::body`. `build_dyn_trait_call_terminator` lives
in `mir::lower`. Data flow: `mir::dyn_trait` → `mir::lower` →
`mir::body` → codegen (Stage 5.79+). Single-directional, no circular
dependency. Codegen doesn't need to query HIR or TraitResolver — MIR
carries all dyn Trait info as data.

**Test impact**: +13 (1598 → 1611).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.49 (Stage 5.79, 2026-07-24)

Stage 5.79 codegen dyn Trait vtable indirect call round. FIRST codegen
integration of dyn Trait data. Adds `emit_dyn_trait_method_call()` to the
`Emitter` trait (+ `TextEmitter` impl) and `codegen_dyn_trait_call()` free
function. Modifies `codegen_terminator`'s `Terminator::Call` branch to
detect the Stage 5.78 marker (`Const{ty: Error, val: Int(index)}`) and
dispatch to the dyn Trait path.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `emit_dyn_trait_method_call` | Emitter trait method + TextEmitter impl | `<verb>_<noun>_<noun>_<noun>_<noun>` |
| `codegen_dyn_trait_call` | free fn (in `codegen`) | `<verb>_<noun>_<noun>_<noun>` |

**Design decisions**:
1. **`emit_` prefix** per §8.1 codegen emit convention — same family as
   `emit_call`, `emit_load`, `emit_gep_field`, etc. The new method emits
   a 4-instruction LLVM IR sequence (getelementptr + 2 loads + indirect call).
2. **`codegen_` prefix** per §8.1 codegen top-level entry convention — same
   family as `codegen_terminator`, `codegen_operand`, `codegen_place_load`.
   The new function reads `mir.dyn_trait_calls[index]` and dispatches to
   `emitter.emit_dyn_trait_method_call`.
3. **Three-condition marker detection** in `codegen_terminator`'s
   `Terminator::Call` branch: (a) `func` is `Operand::Constant`, (b)
   `c.ty.kind` is `TyKind::Error`, (c) `c.val` is `ConstVal::Int(idx)`
   with `idx < mir.dyn_trait_calls.len()`. All three must hold — otherwise
   falls through to legacy direct-call path. Backward-compatible: all
   1611 pre-existing tests pass unchanged.
4. **Return type placeholder**: `codegen_dyn_trait_call` uses `EmitType::I32`
   as the return type because MIR doesn't carry typeck-resolved return types
   for dyn Trait calls yet. Future stages can refine this by extending the
   `DynTraitMethodCall` struct (Stage 5.66) with a `return_ty` field.
5. **Panics on out-of-bounds index** — the caller (codegen_terminator) is
   responsible for bounds-checking before invoking. This matches the
   "contract" pattern used by `codegen_operand` / `codegen_place_load`.

**§16 compliance**: `DynTraitMethodCall` defined in `mir::dyn_trait`.
`MirBody.dyn_trait_calls` side-table in `mir::body`. New codegen functions
in `codegen::emitter` + `codegen::text_emitter` + `codegen::mod`. Data
flow: `mir::body` → `codegen` → LLVM IR text. Single-directional, no
circular dependency. Codegen doesn't query HIR or TraitResolver — MIR
carries all dyn Trait info as data.

**Test impact**: +15 (1611 → 1626).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.50 (Stage 5.80, 2026-07-24)

Stage 5.80 driver dyn Trait plan integration round. END-TO-END driver
integration — the driver auto-builds `DynTraitMIRPlan` from
`TraitResolver` and passes it to each body's lowering via the new
`lower_hir_body_to_mir_full_with_dyn_trait_plan()` entry point. This
activates Stage 5.78 (MethodCall dyn Trait path) + Stage 5.79 (codegen
vtable indirect call) in the normal compile flow.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `lower_hir_body_to_mir_full_with_dyn_trait_plan` | free fn (in `mir::lower`) | `<verb>_<noun>_<noun>_<noun>_<prep>_<noun>_<noun>_<noun>` |

**Design decisions**:
1. **`_with_dyn_trait_plan` suffix** — Rust API-guidelines convention for
   "extended variant with additional feature" (mirrors `Vec::with_capacity`,
   `HashMap::with_hasher`). The new function is the `_full` variant
   extended with an optional `DynTraitMIRPlan` parameter.
2. **Backward compatibility via delegation** — the original
   `lower_hir_body_to_mir_full` now delegates to the new function with
   `plan = None`. All existing callers see identical behavior; all 1626
   pre-existing tests pass unchanged.
3. **`Option<&DynTraitMIRPlan>` parameter** — passing by reference avoids
   moving the plan; the lower clones once per body when attaching via
   `set_dyn_trait_plan(plan.clone())`. The clone cost is acceptable
   (plan is small — typically a few hundred bytes).
4. **Driver refactor**: `trait_resolver` building (Stage 5.2 + 5.8 +
   5.26 + collect) moved from after the per-body loop to before it. This
   is necessary because the plan must be available at lowering time.
   `validate_impls` remains in its original position (after the loop) —
   it doesn't affect lowering, only reports errors.
5. **Plan built once, reused per body** — the driver constructs the plan
   once via `build_dyn_trait_mir_plan_from_resolver(&trait_resolver, &interner)`
   before the loop, then passes `Some(&plan)` to each body's lowering.

**§16 compliance**: The driver is the sole orchestrator that connects
`TraitResolver` (Stage 5.2) to `mir::lower` (Stage 2.1) via the plan
data structure. `MirLowerCtxt` does not own a `TraitResolver` — it
receives the plan as data via `set_dyn_trait_plan`. Data flow:
driver → plan → cx → lower → mir::body side-table → codegen.

**Milestone**: dyn Trait MIR lowering → codegen pipeline is now ACTIVE
end-to-end in the normal compile flow. Stages 5.78 + 5.79 + 5.80 together
complete the pipeline:
- 5.78: lower writes side-table + Const marker
- 5.79: codegen detects marker, emits vtable indirect call IR
- 5.80: driver auto-builds plan, passes to lower

**Test impact**: +11 (1626 → 1637).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.51 (Stage 5.81, 2026-07-24)

Stage 5.81 Deep Review #5 round. §25 阶段末尾深度审查，覆盖 Stage 5.43-5.80
（38 个子阶段）。Documentation-only stage — 无新代码，无新公开符号。

**审查范围**: v1.44-v1.50 共 7 个版本条目（Stage 5.74-5.80），所有新符号 §23 合规。

**审查结论**: 5/5 GO → PASS

**关键发现**:
1. 🎉 dyn Trait MIR lowering → codegen pipeline 端到端激活
2. TD-014（L5 trait dispatch vtable）正式 CLOSE
3. 0 P0 / 0 P1 / 3 P2 阻塞项
4. §16/§23 完全合规
5. 测试覆盖 1637（+401 since r91, +32.4%）
6. CI/CD 持续零警告、零错误、fmt 清洁

**新增技术债**:
- TD-016 (P3): dyn Trait return type I32 placeholder — 未来 stage 扩展
  DynTraitMethodCall 加 return_ty 字段
- TD-017 (P3): codegen/mod.rs 2398 LOC — Stage 6+ 视增长情况拆分

**Test impact**: 0 (documentation-only stage).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.52 (Stage 5.82, 2026-07-24)

Stage 5.82 TD-016 dyn Trait return type refinement round. Close TD-016 —
add `return_kind: StdlibTypeKind` field to `DynTraitMethodCall`, propagate
from `StdlibTraitMethod.return_kind`, add `stdlib_type_kind_to_emit_type()`
converter, use in `codegen_dyn_trait_call`.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_type_kind_to_emit_type` | free fn (in `codegen`) | `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` |
| `DynTraitMethodCall.return_kind` | pub field (in `mir::dyn_trait`) | `<noun>_<noun>` |

**Design decisions**:
1. **`stdlib_type_kind_to_emit_type` naming** — follows the translation
   ladder convention per §8.2, mirroring `mir_type_to_emit_type` and
   `emit_type_to_llvm_str`. The `_to_` infix clearly indicates "convert
   from X to Y".
2. **Breaking change to `DynTraitMethodCall::new` / `from_fat_ptr`** —
   added `return_kind` parameter. All 12 test files + 1 source file
   updated. Default value `StdlibTypeKind::Unit` used for existing test
   cases (matches original I32 placeholder behavior for void methods).
3. **Type mapping** — integer types map by width (I8/U8/Bool/Char → I8,
   etc.), floats map directly, Unit/Never → Void, AllocType/StdType/Str/
   Unknown → OpaquePtr (dyn Trait receivers are fat pointers; method
   returns of these types are ptr-sized).
4. **§16 compliance** — data flows one way: `stdlib::StdlibTraitMethod.
   return_kind` → `mir::dyn_trait::DynTraitMethodCall.return_kind` →
   `codegen::stdlib_type_kind_to_emit_type` → `EmitType`. No circular
   dependency.

**TD-016 status**: CLOSED.

**Test impact**: +23 (1637 → 1660).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.53 (Stage 5.83, 2026-07-24)

Stage 5.83 dyn Trait end-to-end integration tests round. Test-only stage —
no new public symbols, no code changes. Adds 16 e2e tests verifying the
full dyn Trait pipeline (Stages 5.78-5.82 integration).

**New public symbols**: None (test-only stage).

**Test coverage**:
- Pipeline stage 1 (MIR side-table): 3 tests
- Pipeline stage 2 (codegen IR): 4 tests
- Pipeline stage 3 (vtable indirect call): 3 tests
- Pipeline stage 4 (return_kind e2e): 3 tests
- Robustness: 3 tests

**§16 compliance**: Tests use only public API (`compile` + `codegen_crate`
+ `result.mirs`). No internal data structure access.

**Test impact**: +16 (1660 → 1676).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.54 (Stage 5.84, 2026-07-24)

Stage 5.84 dyn Trait param type refinement round. Symmetric to Stage 5.82's
return_kind — add `param_kinds` field to `StdlibTraitMethod` and
`DynTraitMethodCall` for precise parameter type emission in codegen.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `StdlibTraitMethod.param_kinds` | pub field (in `stdlib`) | `<noun>_<noun>` (plural) |
| `DynTraitMethodCall.param_kinds` | pub field (in `mir::dyn_trait`) | `<noun>_<noun>` (plural) |

**Design decisions**:
1. **`&'static [StdlibTypeKind]`** for StdlibTraitMethod — keeps the `Copy`
   + `&'static` design intact. Uses `EMPTY_PARAM_KINDS` const for zero-param
   methods (avoids `&[] as &[T]` which doesn't work in const context).
2. **`Vec<StdlibTypeKind>`** for DynTraitMethodCall — owned, consistent with
   existing String fields. Cloned from `&'static [StdlibTypeKind]` via
   `.to_vec()` in `build_dyn_trait_method_calls_from_fat_ptrs`.
3. **Breaking change** to `DynTraitMethodCall::new` / `from_fat_ptr` — added
   `param_kinds` parameter. All 14 test files + 1 source file + 1 struct
   literal test updated. Default `vec![]` used for zero-param methods.
4. **Codegen integration** — `codegen_dyn_trait_call` now uses
   `call_info.param_kinds[i-1]` for precise arg types (self at index 0 →
   OpaquePtr, explicit args use param_kinds). Falls back to
   `detect_operand_type` when param_kinds is exhausted.
5. **§16 compliance** — data flows one way: `stdlib::StdlibTraitMethod.
   param_kinds` → `mir::dyn_trait::DynTraitMethodCall.param_kinds` →
   `codegen::stdlib_type_kind_to_emit_type` → `EmitType`. No circular
   dependency.
6. **Naming symmetry** — `param_kinds` mirrors `return_kind` (Stage 5.82).
   Both use `<noun>_<noun>` plural/singular pattern appropriately
   (param_kinds is plural Vec/slice, return_kind is singular value).

**Test impact**: +14 (1676 → 1690).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.55 (Stage 5.85, 2026-07-24)

Stage 5.85 is_stdlib_trait query round. Add trait-level membership query
`is_stdlib_trait()` — complements existing `is_stdlib_marker_trait`
(marker-only) and `is_stdlib_trait_method` (method-level) with a unified
trait-level check.

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `is_stdlib_trait` | free fn (in `stdlib`) | `is_<noun>_<noun>` |

**Design decisions**:
1. **`is_` prefix** per §8.1 helper-verb convention — same family as
   `is_stdlib_marker_trait` (v1.6) and `is_stdlib_trait_method` (v1.6).
   All "membership check" functions use `is_` prefix.
2. **Unified trait-level check** — covers both marker traits (return
   `Some(&[])` from `stdlib_trait_methods`) and method traits (return
   `Some(&[...])`). Implementation: `stdlib_trait_methods(trait_name).is_some()`.
3. **Pure read function** — reuses existing `stdlib_trait_methods`, no new
   dependencies. §16-compliant: data flow stays within `stdlib`.
4. **Complements existing queries**:
   - `is_stdlib_marker_trait` — marker-only (Copy/Send/Sync/Sized/Unpin/Eq)
   - `is_stdlib_trait_method` — method-level (trait, method) pair
   - `is_stdlib_trait` (new) — trait-level (any stdlib trait)

**§16 compliance**: Pure read; no new dependencies introduced. Same module
as `is_stdlib_marker_trait` / `is_stdlib_trait_method`.

**Test impact**: +24 (1690 → 1714).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.56 (Stage 5.86, 2026-07-24)

Stage 5.86 stdlib_trait_count + stdlib_all_traits convenience queries round.
Add two convenience functions for stdlib trait enumeration, and extract the
duplicated `ALL_REGISTERED_TRAITS` constant to module level as `STDLIB_TRAITS`.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_count` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>` |
| `stdlib_all_traits` | free fn (in `stdlib`) | `<noun>_<adj>_<noun>` |

**Design decisions**:
1. **`stdlib_trait_count` naming** — mirrors `stdlib_trait_method_count`
   (v1.6). Both are "count of X" queries returning `usize`.
2. **`stdlib_all_traits` naming** — `all_` prefix per Rust API-guidelines
   convention for "return everything" queries (mirrors `Vec::all`,
   `HashMap::all_keys` patterns).
3. **DRY refactoring** — extracted module-level `STDLIB_TRAITS: &[&str]`
   constant (47 trait names). Previously `ALL_REGISTERED_TRAITS` was
   duplicated as a local constant in both `stdlib_traits_with_method` and
   `stdlib_traits_with_vtable` (~110 lines of duplication). Now single
   source of truth.
4. **`stdlib_trait_count` avoids Vec allocation** — returns `STDLIB_TRAITS.len()`
   directly, useful for capacity hints and sanity checks without allocating.
5. **§16 compliance** — pure read functions, reuse existing constant, no
   new dependencies. Data flow stays within `stdlib`.

**Test impact**: +17 (1714 → 1731).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.57 (Stage 5.87, 2026-07-24)

Stage 5.87 stdlib_marker_traits query round. Add batch query returning all
stdlib marker trait names (Copy/Send/Sync/Sized/Unpin/Eq). Symmetric with
`stdlib_traits_with_vtable` (returns traits with methods).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_marker_traits` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>` (plural) |

**Design decisions**:
1. **Plural noun naming** — `stdlib_marker_traits` mirrors `stdlib_traits_with_vtable`
   (v1.7). Both are "return subset of traits matching filter" queries using
   the `<noun>_<noun>_<noun>` plural pattern.
2. **Batch complement to single-trait query** — `is_stdlib_marker_trait`
   (v1.6) checks one trait; `stdlib_marker_traits` returns all markers at once.
3. **Implementation** — filter `STDLIB_TRAITS` (Stage 5.86 module constant)
   by `is_stdlib_marker_trait`. Pure functional, no side effects.
4. **§16 compliance** — pure read, reuses existing `STDLIB_TRAITS` +
   `is_stdlib_marker_trait`, no new dependencies.
5. **Milestone** — this stage brings the test module count to 100
   (98 → 100 with this stage's 2 new test modules).

**Test impact**: +18 (1731 → 1749).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.58 (Stage 5.88, 2026-07-24)

Stage 5.88 stdlib_arithmetic_traits semantic group query round. First
semantic category query — returns all stdlib arithmetic operator trait
names (10 binary + 10 assign = 20 traits).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_arithmetic_traits` | free fn (in `stdlib`) | `<noun>_<adj>_<noun>` (plural) |

**Design decisions**:
1. **Semantic group naming** — `stdlib_arithmetic_traits` follows the
   `<noun>_<adj>_<noun>` plural pattern, mirroring `stdlib_marker_traits`
   (v1.57). Both are "return subset of traits by category" queries.
2. **First semantic category** — arithmetic operators are a clear semantic
   category useful for operator overloading detection, type inference, and
   codegen decisions. Future stages may add more categories (core/io/iter).
3. **Fixed `&'static` slice** — uses a local `ARITHMETIC_TRAITS: &[&str]`
   const (not derived from `STDLIB_TRAITS` filtering) because the arithmetic
   category is a specific enumerated list, not a predicate-based filter.
4. **20 traits** — 10 binary ops (Add/Sub/Mul/Div/Rem/BitAnd/BitOr/BitXor/
   Shl/Shr) + 10 assign variants (AddAssign/.../ShrAssign).
5. **§16 compliance** — pure read, `&'static` slice, no new dependencies.

**Test impact**: +20 (1749 → 1769).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.59 (Stage 5.89, 2026-07-24)

Stage 5.89 stdlib_core_traits semantic group query round. Second semantic
category query — returns all stdlib core trait names (13 traits covering
lifecycle/formatting/comparison/dereference/iteration).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_core_traits` | free fn (in `stdlib`) | `<noun>_<adj>_<noun>` (plural) |

**Design decisions**:
1. **Semantic group naming** — `stdlib_core_traits` follows the
   `<noun>_<adj>_<noun>` plural pattern, mirroring `stdlib_arithmetic_traits`
   (v1.58). Both are "return subset of traits by category" queries.
2. **Second semantic category** — core traits are the most commonly used
   traits for everyday programming (Clone/Drop/Default/Display/Debug/
   PartialEq/PartialOrd/Ord/Hash/Deref/DerefMut/IntoIterator/Iterator).
3. **Fixed `&'static` slice** — uses a local `CORE_TRAITS: &[&str]` const,
   consistent with `stdlib_arithmetic_traits` design (enumerated list, not
   predicate-based filter).
4. **13 traits** — organized by subcategory: lifecycle (3) + formatting (2)
   + comparison (4) + dereference (2) + iteration (2).
5. **§16 compliance** — pure read, `&'static` slice, no new dependencies.
6. **Disjoint from other groups** — core traits are disjoint from marker
   traits (5.87) and arithmetic traits (5.88), verified by tests.

**Test impact**: +22 (1769 → 1791).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.60 (Stage 5.90, 2026-07-24)

Stage 5.90 stdlib_io_traits + stdlib_unary_traits semantic group queries round.
Two small semantic category queries — io (Read/Write) and unary (Neg/Not).
**Completes the semantic category series** covering all stdlib trait categories.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_io_traits` | free fn (in `stdlib`) | `<noun>_<adj>_<noun>` (plural) |
| `stdlib_unary_traits` | free fn (in `stdlib`) | `<noun>_<adj>_<noun>` (plural) |

**Design decisions**:
1. **Two small categories in one stage** — io (2 traits) and unary (2 traits)
   are both small enough to combine. Completes the semantic series efficiently.
2. **`<noun>_<adj>_<noun>` plural naming** — mirrors `stdlib_core_traits` (v1.59)
   and `stdlib_arithmetic_traits` (v1.58). All semantic group queries use the
   same pattern for consistency.
3. **Fixed `&'static` slices** — `IO_TRAITS: &[&str] = &["Read", "Write"]` and
   `UNARY_TRAITS: &[&str] = &["Neg", "Not"]`. Consistent with prior stages.
4. **§16 compliance** — pure read, `&'static` slices, no new dependencies.
5. **Semantic series complete** — 5 categories (marker/arithmetic/core/io/unary)
   covering 43 traits total. All stdlib traits now have semantic group coverage.

**Milestone**: Semantic group query series COMPLETE. All 43 stdlib traits
across 5 categories now have batch query functions.

**Test impact**: +21 (1791 → 1812).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.61 (Stage 5.91, 2026-07-24)

Stage 5.91 Deep Review #6 round. §25 阶段末尾深度审查，覆盖 Stage 5.81-5.90
（10 个子阶段）。Documentation-only stage — 无新代码，无新公开符号。

**审查范围**: v1.51-v1.60 共 10 个版本条目（Stage 5.81-5.90），所有新符号 §23 合规。

**审查结论**: 5/5 GO → PASS

**关键发现**:
1. 🎉 dyn Trait 类型精化完成 (TD-016 CLOSED)
2. 🎉 语义分组查询系列完成 (5 categories, 43 traits)
3. 0 P0 / 0 P1 / 3 P2 阻塞项
4. §16/§23 完全合规
5. 测试覆盖 1812（+175 since r100, +10.7%）
6. CI/CD 持续零警告、零错误、fmt 清洁

**新增技术债**:
- TD-018 (P3): dyn Trait 仅支持 stdlib traits — Stage 6+ 扩展到用户自定义 trait

**Test impact**: 0 (documentation-only stage).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.62 (Stage 5.92, 2026-07-24)

Stage 5.92 param_kinds data accuracy refinement round. Fix Stage 5.84's
param_kinds data for 3 methods whose parameters are std types (Formatter,
Hasher) rather than `&Self`. No new public symbols — data-only correction.

**New public symbols**: None (data-only refinement).

**Corrections**:
- Display::fmt: param_kinds [AllocType] → [StdType] (Formatter is std type)
- Debug::fmt: param_kinds [AllocType] → [StdType] (Formatter is std type)
- Hash::hash: param_kinds [AllocType] → [StdType] (Hasher is std type)

**Design decisions**:
1. **Data accuracy** — Stage 5.84's Python script defaulted all param types
   to `AllocType`. This is correct for `&Self` params but wrong for std type
   params (Formatter, Hasher). This stage fixes the 3 affected methods.
2. **No API changes** — only static table data correction. No new symbols,
   no breaking changes.
3. **§16 compliance** — only data correction, no new dependencies.
4. **Backward compatible** — param_count unchanged, only param_kinds values
   are more precise. All existing tests pass unchanged.

**Test impact**: +8 (1812 → 1820).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.63 (Stage 5.93, 2026-07-24)

Stage 5.93 stdlib_trait_method accessors round. Add two convenience accessor
functions for direct field access on stdlib trait methods.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_method_return_kind` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<noun>_<noun>` |
| `stdlib_trait_method_param_kinds` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<noun>_<noun>` (plural) |

**Design decisions**:
1. **Convenience accessors** — thin wrappers over `find_stdlib_trait_method`
   that return specific fields. Eliminates the two-step `find(...)?.field`
   pattern with one-step `stdlib_trait_method_<field>(...)` calls.
2. **Naming mirrors existing family** — `stdlib_trait_method_return_kind` /
   `stdlib_trait_method_param_kinds` follow the same `<noun>_<noun>_<noun>_<noun>_<noun>`
   pattern as `stdlib_trait_method_count` / `stdlib_trait_method_index` (v1.6).
3. **`param_kinds` is plural** — returns `&'static [StdlibTypeKind]` (a slice),
   so the plural form is appropriate. `return_kind` is singular (returns a
   single `StdlibTypeKind`).
4. **§16 compliance** — pure read, thin wrappers, no new dependencies.

**Test impact**: +12 (1820 → 1832).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.64 (Stage 5.94, 2026-07-24)

Stage 5.94 stdlib_trait_method remaining field accessors round. Add 3
remaining field accessors (self_kind, param_count, is_unsafe) to complete
full StdlibTraitMethod field accessor coverage.

**New public symbols (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_method_self_kind` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<noun>_<noun>` |
| `stdlib_trait_method_param_count` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<noun>_<noun>` |
| `stdlib_trait_method_is_unsafe` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<noun>_<is_adj>` |

**Design decisions**:
1. **3 remaining accessors** — self_kind, param_count, is_unsafe complete
   the set of 5 queryable StdlibTraitMethod fields. (name is a query
   parameter, not a field accessor.)
2. **`is_unsafe` uses `is_<adj>` pattern** — per §8.1 helper-verb convention.
   Consistent with existing `is_unsafe` field name and other `is_` queries.
3. **Thin wrappers** — all 3 are `find_stdlib_trait_method(...).map(|m| m.field)`.
   Consistent with Stage 5.93's return_kind/param_kinds design.
4. **§16 compliance** — pure read, thin wrappers, no new dependencies.

**Milestone**: Full StdlibTraitMethod field accessor coverage complete.
All 5 queryable fields now have dedicated convenience accessors.

**Test impact**: +14 (1832 → 1846).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.65 (Stage 5.95, 2026-07-24)

Stage 5.95 stdlib_trait_methods_by_self_kind reverse query round. Add reverse
query returning all (trait, method) pairs with a given self_kind. Complements
the forward query `stdlib_trait_method_self_kind` (Stage 5.94).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_methods_by_self_kind` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` (plural) |

**Design decisions**:
1. **Reverse query** — given a self_kind, find all matching (trait, method)
   pairs. Complements the forward query `stdlib_trait_method_self_kind`
   (v1.64) which queries a single method's self_kind.
2. **`_by_self_kind` suffix** — Rust API-guidelines field-filter convention,
   mirroring `find_dyn_trait_method_call_in_plan_by_method` from v1.47.
3. **Returns `Vec<(&'static str, &'static str)>`** — (trait_name, method_name)
   pairs, all `&'static` since they come from the static method tables.
4. **§16 compliance** — pure read, reuses `STDLIB_TRAITS` +
   `stdlib_trait_methods`, no new dependencies.

**Test impact**: +11 (1846 → 1857).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.66 (Stage 5.96, 2026-07-24)

Stage 5.96 stdlib_trait_methods_by_return_kind reverse query round. Add reverse
query returning all (trait, method) pairs with a given return_kind. Symmetric
with `stdlib_trait_methods_by_self_kind` (v1.65, by self_kind).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_methods_by_return_kind` | free fn (in `stdlib`) | `<noun>_<noun>_<noun>_<prep>_<noun>_<noun>` (plural) |

**Design decisions**:
1. **Symmetric with 5.95** — `_by_return_kind` mirrors `_by_self_kind` (v1.65).
   Both are reverse queries filtering by a specific StdlibTraitMethod field.
2. **§16 compliance** — pure read, reuses `STDLIB_TRAITS` +
   `stdlib_trait_methods`, no new dependencies.

**Test impact**: +10 (1857 → 1867).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.67 (Stage 5.97, 2026-07-24)

Stage 5.97 Deep Review #7 round. §25 阶段末尾深度审查，覆盖 Stage 5.91-5.96
（6 个子阶段）。Documentation-only stage — 无新代码，无新公开符号。

**审查结论**: 5/5 GO → PASS

**关键发现**: stdlib trait method 查询 API 全面覆盖完成。

**Test impact**: 0 (documentation-only stage).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.68 (Stage 5.98, 2026-07-24)

Stage 5.98 stdlib_trait_methods_by_is_unsafe reverse query round. Add reverse
query returning all (trait, method) pairs matching a given is_unsafe flag.
**Completes the reverse query series** (3 dimensions: self_kind/return_kind/is_unsafe).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_methods_by_is_unsafe` | free fn (in `stdlib`) | `<noun>×3_<prep>_<is_adj>` (plural) |

**Design decisions**:
1. **Completes reverse query series** — 3 dimensions: self_kind (v1.65),
   return_kind (v1.66), is_unsafe (v1.68). All 3 are reverse queries filtering
   by a specific StdlibTraitMethod field.
2. **§16 compliance** — pure read, reuses `STDLIB_TRAITS` +
   `stdlib_trait_methods`, no new dependencies.

**Test impact**: +7 (1867 → 1874).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.69 (Stage 5.99, 2026-07-24)

Stage 5.99 stdlib_trait_methods_by_param_count reverse query round. **Stage 5
final sub-stage.** Add the fourth and final reverse query dimension —
param_count. Completes the reverse query series (4 dimensions).

**New public symbol (§23-compliant)**:

| Symbol | Kind | Naming pattern |
|--------|------|----------------|
| `stdlib_trait_methods_by_param_count` | free fn (in `stdlib`) | `<noun>×3_<prep>_<noun>×2` (plural) |

**Design decisions**:
1. **Fourth and final reverse query** — completes the series: self_kind (v1.65),
   return_kind (v1.66), is_unsafe (v1.68), param_count (v1.69). All 4 queryable
   StdlibTraitMethod fields now have reverse queries.
2. **§16 compliance** — pure read, reuses `STDLIB_TRAITS` + `stdlib_trait_methods`.

**🎉 Stage 5 Complete (5.1-5.99, 99 sub-stages)**

**Test impact**: +7 (1874 → 1881).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.70 (Stage 6.1, 2026-07-24)

Stage 6.1 mir/lower ADT layout split round. **Stage 6 begins!** First TD-011
repayment step — extract ADT layout functions from mir/lower/mod.rs into
mir/lower/adt_layout.rs.

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `lower_hir_ty_to_mir_ty`: `pub fn` → `pub(crate) fn` (needed by adt_layout.rs)
- `populate_adt_layouts`: `fn` → `pub(crate) fn` in new adt_layout.rs module
- No new public API — pure internal module reorganization

**Design decisions**:
1. **Behavior-equivalent refactoring** — all 1881 tests pass unchanged
2. **§16 compliance** — adt_layout.rs has single-direction dependencies
3. **pub(crate) visibility** — `lower_hir_ty_to_mir_ty` only needs to be
   accessible within the crate (from adt_layout.rs), not externally

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.71 (Stage 6.2, 2026-07-24)

Stage 6.2 mir/lower closure_capture split round. Continue TD-011 repayment —
extract closure capture functions into mir/lower/closure_capture.rs.

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `collect_captured_locals`: `fn` → `pub(crate) fn` in new closure_capture.rs
- `collect_block_captured`: `fn` → `pub(crate) fn` in new closure_capture.rs
- No new public API — pure internal module reorganization

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.72 (Stage 6.3, 2026-07-24)

Stage 6.3 mir/lower pattern_bindings split round. Continue TD-011 repayment —
extract pattern binding functions into mir/lower/pattern_bindings.rs.

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `pat_mutability`, `collect_pat_bindings_for_mir`, `lower_enum_variant_pattern_bindings`,
  `compute_enum_payload_starting_idx`, `collect_pat_hir_ids`: `fn` → `pub(crate) fn`
  in new pattern_bindings.rs module
- `resolve_enum_variant`: `fn` → `pub(crate) fn` (needed by pattern_bindings.rs)

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.73 (Stage 6.4, 2026-07-24)

Stage 6.4 mir/lower overflow_assert split round. Continue TD-011 repayment —
extract overflow/assert helper functions into mir/lower/overflow_assert.rs.

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `is_overflowable_op`, `emit_overflow_assert`, `emit_div_by_zero_assert`:
  `fn` → `pub(crate) fn` in new overflow_assert.rs module

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.74 (Stage 6.5, 2026-07-24)

Stage 6.5 mir/lower field_resolution split round. Continue TD-011 repayment —
extract field resolution helper functions into mir/lower/field_resolution.rs.

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `resolve_field_type`, `resolve_field_index`, `find_receiver_struct_def_id`,
  `resolve_index_element_type`, `resolve_adt_field_tys`:
  `fn` → `pub(crate) fn` in new field_resolution.rs module

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.75 (Stage 6.6, 2026-07-24)

Stage 6.6 mir/lower control_flow split round. Continue TD-011 repayment —
extract control flow lowering functions into mir/lower/control_flow.rs.
**🎉 mod.rs below 2000 LOC!**

**New public symbols**: None (internal refactoring, behavior-equivalent).

**Changes**:
- `lower_short_circuit`, `lower_deref_expr`, `lower_block`, `lower_if`,
  `lower_match`: `fn` → `pub(crate) fn` in new control_flow.rs module

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.76 (Stage 6.7, 2026-07-24)

Stage 6.7 codegen trait_dispatch architectural split round. **Architectural**
extraction from codegen/mod.rs — not just size reduction but scientific module
boundary design per single responsibility principle.

**New public symbols**: None (all re-exported from trait_dispatch via mod.rs).

**Changes**:
- 16 functions + 4 structs moved from codegen/mod.rs to codegen/trait_dispatch.rs
- All re-exported via `pub use trait_dispatch::{...}` for backward compatibility
- No new public API — pure architectural reorganization

**Architectural rationale**: Single responsibility principle.
- mod.rs = MIR→LLVM IR translation (consumes MirBody)
- trait_dispatch.rs = vtable/dynptr global generation (consumes TraitResolver)

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.77 (Stage 6.8, 2026-07-24)

Stage 6.8 codegen mir_translation architectural split round. Completes codegen
5-module architecture. Extract MIR type/place/operand translation helpers
into codegen/mir_translation.rs.

**New public symbols**: None (all re-exported from mir_translation via mod.rs).

**Changes**:
- 9 functions moved from codegen/mod.rs to codegen/mir_translation.rs
- `pub use` for public functions, `pub(crate) use` for internal helpers
- No new public API — pure architectural reorganization

**Architectural rationale**: Single responsibility — each module has one clear purpose.

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.78 (Stage 6.9, 2026-07-24)

Stage 6.9 stdlib 3-domain architectural split round. Convert single-file
stdlib.rs (2383 LOC) into 3-module directory structure following single
responsibility principle.

**New public symbols**: None (all re-exported via `pub use *::*`).

**Architectural rationale**: 3 data domains separated:
- mod.rs = type world (base)
- trait_methods.rs = trait method queries (depends on mod.rs)
- vtable_layout.rs = vtable layout + emission (depends on mod.rs + trait_methods.rs)

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

### v1.79 (Stage 6.10, 2026-07-25)

Stage 6.10 mir/lower expr_operand architectural split round, triggered by
user's explicit request: "重新分析 mir/lower" + "文件的拆分不是说只为了
缩小体积，还有需要符合架构设计需求、科学合理划分、其实本质上就只
组织结构的设计".

This round performs an **architectural re-analysis** of `mir/lower/mod.rs`
(1980 LOC) and identifies 4 responsibility domains:

| Domain | LOC | Responsibility |
|--------|-----|----------------|
| A: Context infrastructure | 432 | MirLowerCtxt struct + impl |
| B: Body entry points | 230 | lower_hir_body_to_mir* + aliases |
| C: HIR→MIR type conversion | 89 | const_eval_array_len + lower_hir_ty_to_mir_ty |
| **D: Expression lowering algorithm** | **1212** | lower_expr_to_operand + 3 helpers |

Domain D (61.4% of mod.rs) is the largest mixed responsibility. It contains
4 functions that together form the "HIR expression → MIR operand/terminator"
algorithm and interact with MirLowerCtxt only through its public API.

**New public symbols**: None (pure architectural reorganization).

**Changes**:
- Created `src/mir/lower/expr_operand.rs` (1275 LOC) hosting 4 functions:
  - `pub fn build_dyn_trait_call_terminator` (public API, re-exported)
  - `pub(crate) fn lower_expr_to_operand` (used by mod.rs + sibling modules)
  - `pub(crate) fn lower_expr_to_place` (used only within expr_operand)
  - `pub(crate) fn resolve_enum_variant` (used by adt_layout/control_flow)
- Updated `mod.rs` re-exports:
  ```rust
  pub use expr_operand::build_dyn_trait_call_terminator;
  pub(crate) use expr_operand::{lower_expr_to_operand, resolve_enum_variant};
  ```
- Removed unused imports from mod.rs (`DynTraitMethodCall`,
  `find_dyn_trait_method_call_in_plan_by_method`)
- Zero call-site changes for sibling modules (control_flow.rs,
  pattern_bindings.rs continue using `super::lower_expr_to_operand` etc.)

**Architectural rationale**: Single responsibility principle.
- mod.rs = MirLowerCtxt context + body entry points + type conversion
  utilities (skeleton)
- expr_operand.rs = HIR expression → MIR operand/terminator algorithm
  (algorithm core)

Data flow is unidirectional:
mod.rs → expr_operand → MirLowerCtxt → {adt_layout, closure_capture,
control_flow, field_resolution, overflow_assert, pattern_bindings}.
No circular dependency.

**Module naming**: `expr_operand` follows the `<noun>_<noun>` pattern set
by sibling modules (`adt_layout`, `closure_capture`, `pattern_bindings`).

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-011 cumulative**: mod.rs 3346 → 772 LOC (-76.9% across 7 splits).

### v1.80 (Stage 6.11, 2026-07-25)

Stage 6.11 process governance protocol round. **No code changes** — pure
process documentation + design-writeback. Bumps process version v3.20 →
v3.21 with three new protocols:

- §13.4 阶段开始时的设计对齐 (Stage-start design alignment)
- §14.4 重构即架构设计 (Refactoring as architecture design, 6 judgments J1-J6)
- §25.8 阶段末尾设计回写协议 (Stage-end design-writeback, 4 deviation types B1-B4)

**New public symbols**: None (no code changes).

**Changes**:
- docs/stage-committee-process.md: v3.20 → v3.21 (+416 LOC: §13.4 + §14.4 + §25.8 + §28.4)
- docs/lang-design/06-mir.md: +§14 实现状态 (B1/B3/B4 偏差清单 + dyn Trait lowering 算法补写)
- docs/lang-design/07-codegen.md: +§14 实现扩展 (Trait dispatch codegen 子系统补写)
- Cargo.toml: version 0.12.9 → 0.13.0 (process major version bump)
- No source code changes — 1881 tests pass unchanged

**Architectural rationale**: Per §25.8 (new), design docs must be kept in
sync with implementation. This round performs lightweight writeback to
06-mir.md + 07-codegen.md (full writeback reserved for Stage 6 end).

**Test impact**: 0 (no code changes).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean (no code changes).

### v1.81 (Stage 6.12, 2026-07-25)

Stage 6.12 parser.rs architectural split round. Per v3.21 §13.4 (stage-start
design alignment with 02-grammar.md §3.1-§3.7) + §14.4 (refactoring as
architecture design, J1-J6 judgments).

Splits `src/parser/parser.rs` (3112 LOC, project's largest file) into 7
sub-modules, each owning one parsing category aligned with 02-grammar.md §3.

**New public symbols**: None (pure architectural reorganization).

**Changes**:
- Created 7 new sub-modules under `src/parser/`:
  - `path.rs` (268 LOC) — PathContext + path parsing
  - `generics.rs` (274 LOC) — generics + bounds + where + params + return type
  - `ty.rs` (254 LOC) — type parsing
  - `expr.rs` (1028 LOC) — expression Pratt parsing + ExprSpan trait
  - `pat.rs` (318 LOC) — pattern parsing
  - `stmt.rs` (104 LOC) — block + let statement
  - `items.rs` (780 LOC) — 16 item-parsing functions + ty_to_path helper
- `parser.rs`: 3112 → 263 LOC (-91.5%, -2849 LOC)
- All `mod xxx;` declarations in `src/parser/mod.rs`
- Visibility: struct fields + cursor methods + parse_* methods all `pub(super)`;
  `parse_crate` remains `pub` (only public entry)
- Behavior-equivalent — all 1881 tests pass unchanged

**Architectural rationale**: Per §14.4 J1, new structure maps 1:1 to
02-grammar.md §3.1-§3.7. Per J2, each module owns one parse category.
Per J3, data flows mod.rs → items.rs → {generics, ty, path, expr, pat, stmt}.
Per J6, all modules in 104-1028 LOC range.

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-022**: parser.rs LOC — introduced and immediately closed in this stage.

### v1.82 (Stage 6.13, 2026-07-25)

Stage 6.13 lexer/reader.rs architectural split round. Per v3.21 §13.4
(stage-start design alignment with 02-grammar.md §1) + §14.4 (refactoring
as architecture design, J1-J6 judgments).

Splits `src/lexer/reader.rs` (1537 LOC) into 4 sub-modules, each owning
one lexical category aligned with 02-grammar.md §1 productions.

**New public symbols**: None (pure architectural reorganization).

**Changes**:
- Created 4 new sub-modules under `src/lexer/`:
  - `ident.rs` (123 LOC) — lex_raw_identifier + lex_ident + is_ident_start_byte (§1.3+§1.4)
  - `number.rs` (303 LOC) — lex_number + lex_hex/oct/bin + try_lex_number_suffix (§1.5+§1.6)
  - `string.rs` (486 LOC) — 10 char/string functions + escape (§1.7)
  - `operators.rs` (372 LOC) — lex_doc_comment + 14 lex_<op> functions (§1.1+§1.8)
- `reader.rs`: 1537 → 349 LOC (-77.3%, -1188 LOC)
  - Retains: Lexer struct + cursor methods (pub(super)) + skip_trivia + next_token (pub) + LexError
- All `mod xxx;` declarations in `src/lexer/mod.rs` (sibling to `reader.rs`)
- Visibility: struct fields + cursor methods + lex_* methods all `pub(super)`;
  `next_token` remains `pub` (only public entry)
- Behavior-equivalent — all 1881 tests pass unchanged

**Architectural rationale**: Per §14.4 J1, new structure maps to 02-grammar.md
§1 lexical categories (9 sub-sections aggregated to 4 cohesive modules).
Per J2, each module owns one lexical category. Per J3, data flows
reader.rs → {ident, number, string, operators}. Per J6, all modules in
123-486 LOC range.

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-023**: lexer/reader.rs LOC — introduced and immediately closed in this stage.

### v1.83 (Stage 6.14, 2026-07-25)

Stage 6.14 borrowck/mod.rs architectural split round. Per v3.21 §13.4
(stage-start design alignment with 04-ownership-borrowing.md §4) + §14.4
(refactoring as architecture design, J1-J6 judgments).

Splits `src/borrowck/mod.rs` (1452 LOC) into 3 sub-modules, each owning
one analysis responsibility aligned with 04-ownership-borrowing.md §4
NLL algorithm structure.

**New public symbols**: None (all re-exported from sub-modules via mod.rs).

**Changes**:
- Created 3 new sub-modules under `src/borrowck/`:
  - `liveness.rs` (109 LOC) — LastUseMap + compute_last_use_map + 5 read-collection helpers (§4.3)
  - `copy_semantics.rs` (124 LOC) — ty_is_copy + ty_is_copy_with_resolver + ty_is_copy_unified (§4.5 related)
  - `place_path.rs` (112 LOC) — PlacePath + PlaceRoot + ProjElem + impl PlacePath (§4 data structures)
- `mod.rs`: 1452 → 1146 LOC (-21%, -306 LOC; ~550 LOC code + ~600 LOC tests)
  - Retains: BorrowChecker struct + impl + check_mir_body/check_crate entry points + tests
- mod.rs `pub use` re-exports all public symbols from sub-modules for backward compat:
  - `pub use copy_semantics::{ty_is_copy, ty_is_copy_unified, ty_is_copy_with_resolver};`
  - `pub use liveness::{compute_last_use_map, LastUseMap};`
  - `pub use place_path::{PlacePath, PlaceRoot, ProjElem};`
- Behavior-equivalent — all 1881 tests pass unchanged

**Architectural rationale**: Per §14.4 J1, new structure maps to
04-ownership-borrowing.md §4 NLL algorithm stages. Per J2, each module
owns one analysis responsibility. Per J3, data flows mod.rs → {liveness,
copy_semantics, place_path}. Per J6, all modules in 109-124 LOC range
(mod.rs retains 1146 LOC due to ~600 LOC of tests).

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-024**: borrowck/mod.rs LOC — introduced and immediately closed in this stage.

### v1.84 (Stage 6.15, 2026-07-25)

Stage 6.15 typeck/checker.rs architectural split round. Per v3.21 §13.4
(stage-start design alignment with 03-type-system.md §4+§8) + §14.4
(refactoring as architecture design, J1-J6 judgments).

Splits `src/typeck/checker.rs` (1320 LOC) into 2 sub-modules, each owning
one typeck responsibility aligned with 03-type-system.md §4 (type inference
data structures) + §8 (Subtyping rules).

**New public symbols**: None (all re-exported from sub-modules via mod.rs).

**Changes**:
- Created 2 new sub-modules under `src/typeck/`:
  - `tables.rs` (78 LOC) — TypeckResults + FieldTyTable + FnSigTable (§4 data structures)
  - `predicates.rs` (132 LOC) — 6 type predicates + can_coerce (§8 Subtyping)
- `checker.rs`: 1320 → 1160 LOC (-12%, -160 LOC)
  - Retains: TypeChecker struct + impl + check_mir_body/check_crate entry points + tests
- mod.rs `pub use` re-exports public symbols from sub-modules for backward compat:
  - `pub use tables::{FieldTyTable, FnSigTable, TypeckResults};`
- checker.rs imports predicates via `use super::predicates::{...}`
- Behavior-equivalent — all 1881 tests pass unchanged

**Architectural rationale**: Per §14.4 J1, new structure maps to
03-type-system.md §4 (data structures) + §8 (Subtyping). Per J2, each
module owns one typeck responsibility. Per J3, data flows
checker.rs → {tables, predicates}. Per J6, all modules in 78-132 LOC range.

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-025**: typeck/checker.rs LOC — introduced and immediately closed in this stage.

### v1.85 (Stage 6.16, 2026-07-25)

Stage 6.16 resolve/resolver.rs architectural split round. Per v3.21 §13.4
(stage-start design alignment with 01-language-specification.md §6.2) +
§14.4 (refactoring as architecture design, J1-J6 judgments).

Splits `src/resolve/resolver.rs` (1131 LOC) into 3 sub-modules, each owning
one resolution phase aligned with 01-language-specification.md §6.2 解析顺序.

**New public symbols**: None (pure architectural reorganization).

**Changes**:
- Created 3 new sub-modules under `src/resolve/`:
  - `primitives.rs` (32 LOC) — lookup_prim_ty (primitive type lookup table)
  - `module_build.rs` (470 LOC) — build_module_tree + collect_item_registration + build_child_module + item_def_id + resolve_uses + resolve_use_tree + resolve_use_leaf + resolve_use_glob + lookup_use_path_target + check_visibility (§6.2 pass 1-3)
  - `path_resolve.rs` (577 LOC) — resolve_all_paths + resolve_owner_paths + resolve_item_paths + resolve_generics_paths + resolve_ty_paths + resolve_hir_path + resolve_path + resolve_body + collect_pat_bindings + resolve_expr + resolve_block (§6.2 pass 4-5)
- `resolver.rs`: 1131 → 154 LOC (-86.4%, -977 LOC)
  - Retains: Resolver struct + new + resolve orchestrator + into_errors + name_to_string + path_to_string + def_visibility + current_module + resolve_crate entry
- All `mod xxx;` declarations in `src/resolve/mod.rs`
- Visibility: struct fields + cursor methods + resolve_* methods all `pub(super)`;
  `resolve_crate` remains `pub` (only public entry)
- Behavior-equivalent — all 1881 tests pass unchanged

**Architectural rationale**: Per §14.4 J1, new structure maps to
01-language-specification.md §6.2 解析顺序 (pass 1-3 → module_build;
pass 4-5 → path_resolve; helpers → primitives). Per J2, each module owns
one resolution phase. Per J3, data flows resolver.rs → {module_build,
path_resolve, primitives}. Per J6, all modules in 32-577 LOC range.

**Test impact**: 0 (behavior-equivalent).
**Clippy impact**: 0 (0 warnings).
**Fmt impact**: clean.

**TD-026**: resolve/resolver.rs LOC — introduced and immediately closed in this stage.
