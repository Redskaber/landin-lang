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
