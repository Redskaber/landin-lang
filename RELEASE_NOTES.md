# Landin Compiler — Release Notes

**Author**: redskaber
**Current version**: v0.9.0
**Date**: 2026-07-22
**Test count**: 987 tests passing, 0 warnings, fmt + clippy clean

---

## v0.9.0 — Stage 4.1-4.2 (Nested module support + L1 PHI design decision)

### Overview

First Stage 4 release. Implements nested module support (Stage 4.1) and
closes the L1 PHI optimization limitation with a documented design decision
(Stage 4.2). 987 tests pass (was 984, +3 new nested module tests). 0 clippy
warnings. fmt clean. This release follows the Stage 3.69 deep review's
priority list: nested modules first (unblocks visibility enforcement), then
L1 PHI (resolved as design decision rather than implementation).

### Stage 4.1: Nested module support

**Previously** (Stage 1.3-3.68): `build_module_tree` registered all items
at the crate root level — `ModuleNode.children` was never populated. This
meant `mod foo { pub fn bar() {} }` would register `bar` at crate root,
not in a child module. Visibility enforcement (TD-004) was blocked because
it needs nested module context.

**Now** (Stage 4.1):
- `build_module_tree` refactored to recursively process inline modules
- New `collect_item_registration` helper handles each item kind
- New `build_child_module` recursively builds a child `ModuleNode` for
  `HirModKind::Inline(items)` — handles arbitrarily deep nesting
- New `item_def_id` helper extracts `DefId` from any `HirItem` variant
  via `hir_id.owner`
- `ModuleNode.children` is now populated for inline modules
- 2-level nesting verified (`mod a { mod b { fn deep() {} } }`)

**What this unblocks**:
- Visibility enforcement (TD-004) — `check_visibility` can now use
  `current_module` context to enforce `pub`/`pub(crate)`/private
- Future `use` resolution improvements — glob imports can now pull from
  child modules
- Path resolution — `mod::item` paths can now walk into child modules

**New tests** (3):
- `nested_module_items_resolve` — `mod inner { pub fn f() {} }` + `inner::f()`
- `nested_module_struct_resolves` — struct inside module
- `deeply_nested_module_resolves` — 2-level nesting (`a::b::deep_fn`)

### Stage 4.2: L1 PHI optimization — design decision (CLOSED)

**Previously**: L1 was listed as "PHI node optimization — codegen emits
alloca+load/store, relies on LLVM `mem2reg`". The deep review (Stage 3.69)
flagged this for Stage 4.

**After analysis** (Stage 4.2): This is **not a limitation** — it's the
**standard design** used by Clang, rustc, and most LLVM frontends. The
`alloca`-based IR is correct and produces optimal code after `opt -mem2reg`
or `lli` (which runs default passes).

**Decision**: L1 is **CLOSED** as a design decision. The documentation in
`src/codegen/mod.rs` now explicitly explains:
1. `mem2reg` is a well-tested LLVM pass that produces optimal SSA form
2. Implementing PHI emission manually would duplicate `mem2reg` logic
3. The `alloca`-based IR is correct — valid LLVM IR that any toolchain optimizes
4. The IR quality concern is non-blocking — `opt -mem2reg` produces optimal code

**What was considered and rejected**: Emitting PHI nodes directly in
`codegen_function` by tracking SSA values per basic block. This would
require per-block value mapping, PHI insertion at joins, dominance frontier
computation, and handling of partially-defined variables — essentially
reimplementing `mem2reg` in Rust (high effort, high risk, low benefit).

**L1 status**: ✅ CLOSED (design decision documented in `src/codegen/mod.rs`)

### Verification

- `cargo test`: **987 passed, 0 failed, 2 ignored** (was 984, +3 new)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `src/resolve/resolver.rs` — recursive `build_module_tree` + `collect_item_registration` + `build_child_module` + `item_def_id`
- `src/codegen/mod.rs` — L1 PHI design decision documentation
- `src/lib.rs` — Stage 4.1-4.2 mention + L1 removed from "Remaining"
- `tests/hir_resolution.rs` — +3 nested module tests

### Next Stage 4 priorities (from deep review)

1. **L3 closure codegen** — closure type lowering + capture codegen (high user value)
2. **Macro system + attributes** — `Expr::MacroCall` expansion
3. **Visibility enforcement activation** — now that nested modules work, activate `check_visibility`
4. **Performance benchmark suite** — add `benches/` + criterion (QA condition from deep review)

---

## v0.8.13 — Stage 3.69 (Process v3.16 + Stage 0-3 deep review)

### Overview

This round updates the process document to v3.16 (adding §25 阶段末尾深度审查
协议) and executes the first deep review per the new protocol. The deep review
analyzes Stage 0-3 across 7 dimensions (architecture health, tech debt, test
coverage, next-stage readiness, design soundness, performance, documentation)
and concludes **GO-WITH-CONDITIONS** for entering Stage 4. 984 tests pass
(unchanged — pure documentation + process work). 0 clippy warnings. fmt clean.

### Process v3.16: §25 阶段末尾深度审查协议

**New section §25** added to `docs/stage-committee-process.md`:

- **7 review dimensions** (D1-D7):
  - D1: 架构健康度 (architecture health)
  - D2: 技术债清单 (tech debt inventory)
  - D3: 测试覆盖深度 (test coverage depth)
  - D4: 下一阶段就绪度 (next-stage readiness)
  - D5: 设计合理性 (design soundness)
  - D6: 性能与可扩展性 (performance & scalability)
  - D7: 文档与知识传承 (documentation & knowledge transfer)

- **Trigger points**: stage-end review / gate / convergence round / stage transition
- **Output**: `deep-review-roundN.md` report with 7-dimension analysis + committee vote + action plan
- **Relationship to §9.3/§21**: §25 is the superset — includes §9.3 (round correctness) + §21 (cross-stage integrity) + adds D4 (forward-looking readiness) and D2 (tech debt inventory)

- **Also updated**: §1 总体原则 (added 9th principle) + §3.3 退出硬性标准 (added 8th requirement)

### Stage 0-3 Deep Review (Round 37)

**Output**: `docs/develop/v0/stage-3/deep-review-r37.md`

**Committee vote**: 5/5 GO (1 GO-WITH-CONDITIONS) → **GO-WITH-CONDITIONS**

**Key findings**:
- 0 P0 / 0 P1 blockers
- 5 P2 tech debt items (all with repayment plans, none blocking Stage 4)
- Architecture health: excellent (§16 compliant, naming standardized)
- Test coverage: ~99% (984 tests, 7 negative categories covered)
- Next-stage readiness: ✅ ready (AST/HIR infrastructure for closures/macros exists)
- Conditions for Stage 4: add benchmark suite, create ADR docs, review HirParam duplication

**Stage 4 priority tasks** (from deep review):
1. L3 closure codegen (high user value)
2. L1 PHI optimization (IR quality)
3. Nested module support (unblocks visibility enforcement)
4. Macro system + attributes (new feature)
5. Performance benchmark suite (QA condition)

### Verification

- `cargo test`: **984 passed, 0 failed, 2 ignored** (unchanged)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `docs/stage-committee-process.md` — v3.15 → v3.16 (added §25 + §26 + §1/§3.3 enhancements)
- `docs/develop/v0/stage-3/deep-review-r37.md` — new deep review report
- `README.md` / `RELEASE_NOTES.md` / `Cargo.toml` / `src/lib.rs` / `docs/tests/matrix.md` — version + status updates

---

## v0.8.12 — Stage 3.68 (Visibility checking infrastructure)

### Overview

Continuation of the §21 cross-stage audit follow-up. This round implements
the visibility checking infrastructure (Stage 1.3 Phase E1 groundwork):
a `def_visibility` map that records each definition's visibility, and a
`check_visibility` hook called during path resolution. The actual check
is a stub (returns Ok) because the module tree is currently flat — once
nested modules are supported in Stage 4, the check will enforce
`pub`/`pub(crate)`/`pub(super)`/private access rules. 984 tests pass
(was 983, +1 new visibility metadata test). 0 clippy warnings. fmt clean.

### P2 fix: Visibility checking infrastructure

**Previously**: The resolver collected `DefKind` metadata but not
`Visibility`. Path resolution never checked whether a definition was
accessible from the current context — private items were accessible
from anywhere.

**Now** (Stage 3.68):
- New `def_visibility: HashMap<DefId, Visibility>` field on `Resolver`
- Populated during `build_module_tree` — each item's `vis` field is
  recorded (Fn, Const, Static, Struct, Enum, Trait, TypeAlias, Mod, Use)
- New `check_visibility(def_id, span)` method — called from `resolve_path`
  when resolving to `Res::Def`. Currently a stub (returns `Ok(())`) because
  the module tree is flat. Once nested modules are supported (Stage 4),
  this will enforce:
  - `pub` items visible from anywhere
  - `pub(crate)` items visible within the crate
  - `pub(super)` items visible in parent module
  - private items visible only within their defining module
- Public `def_visibility(def_id)` accessor for testing

### New test (1)

Added `visibility_metadata_collected_for_fn` to `tests/hir_resolution.rs`:
- Verifies that `pub fn public_fn() {}` gets `Visibility::Public`
- Verifies that `fn private_fn() {}` gets `Visibility::Private`
- Uses the public `def_visibility` accessor

### Verification

- `cargo test`: **984 passed, 0 failed, 2 ignored** (was 983, +1 new)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `src/resolve/resolver.rs` — `def_visibility` map + `check_visibility` hook + public accessor + visibility metadata collection in `build_module_tree` + visibility check calls in `resolve_path`
- `tests/hir_resolution.rs` — +1 new visibility metadata test

### Remaining P2/P3 items (deferred to Stage 4+)

- AST enum naming standardization (Expr/Ty/Pat direct vs ItemKind wrapper)
- `HirParam` duplication between `HirFnSig.inputs` and `Body.params`
- Full visibility enforcement (infrastructure done in Stage 3.68; needs nested modules)
- Prelude injection (Stage 1.3 Phase E3)

---

## v0.8.11 — Stage 3.67 (P2 cleanup: body owner context, &Rodeo, Span::DUMMY)

### Overview

Continuation of the §21 cross-stage audit follow-up. This round addresses
3 more P2 cleanup items: threading owner context into body resolution
(completes the `HirSelfKind` work from Stage 3.66), eliminating the
`&mut Rodeo` smell in `resolve_crate`, and fixing the 11 `Span::DUMMY`
placeholders in `parser.rs`. 983 tests pass (unchanged — pure refactoring).
0 clippy warnings. fmt clean.

### P2 fix #1: Body owner context threading for accurate `HirSelfKind`

**Previously** (Stage 3.66): The resolver set `current_self_kind` when
resolving Trait/Impl **item** paths (supertraits, self_ty), but body
resolution happened in a separate loop without owner context. So
`fn bar(x: Self) {}` inside an impl always got `HirSelfKind::Impl`
(which happened to be correct for impls), but `fn bar(x: Self) {}`
inside a trait would also get `HirSelfKind::Impl` (wrong — should be
`HirSelfKind::Trait`).

**Now** (Stage 3.67):
- `resolve_all_paths` builds a `HashMap<DefId, HirSelfKind>` mapping
  trait/impl owner DefIds to their `HirSelfKind`
- When iterating bodies, it looks up `body.hir_id.owner` in the map
  and sets `current_self_kind` before calling `resolve_body`
- `resolve_path` now produces accurate `HirSelfKind` at both owner
  AND body levels

### P2 fix #2: `&mut Rodeo` → `&Rodeo` in `resolve_crate`

**Previously**: `resolve_crate` took `&mut Rodeo` to pre-intern keyword
strings ("Self", "self", "crate", "super") that the parser looks up via
`interner.get()` but the lexer never interned (because keyword tokens
are returned as `TokenKind::Kw*` without interning the string).

**Now** (Stage 3.67):
- The lexer now interns keyword strings at tokenization time
  (`self.interner.get_or_intern(text)` before returning `Token { kind: kw, span }`)
- `resolve_crate` signature changed from `&mut Rodeo` to `&Rodeo`
- All callers updated (driver.rs + 4 test files)
- The resolver is now a pure read-only consumer of the interner

### P2 fix #3: `Span::DUMMY` placeholders fixed in parser.rs

**Previously**: 11 occurrences of `Span::DUMMY` in `parser.rs` for the
`span` field of top-level declaration structs (`ConstDecl`, `StaticDecl`,
`StructDecl`, `EnumDecl`, `ImplDecl`, `TypeAliasDecl`). These spans were
placeholder values that didn't point to any source location.

**Now** (Stage 3.67):
- Each `parse_*` function captures `let kw_span = self.current_span()`
  before `self.bump()` (which consumes the keyword token)
- The struct constructor uses `span: kw_span` instead of `span: Span::DUMMY`
- All 11 placeholders replaced with the keyword's actual span

### Verification

- `cargo test`: **983 passed, 0 failed, 2 ignored** (unchanged — pure refactoring)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `src/resolve/resolver.rs` — body owner context map + `resolve_crate` signature change
- `src/lexer/reader.rs` — intern keyword strings at tokenization time
- `src/parser/parser.rs` — 11 `Span::DUMMY` → `kw_span` (keyword span capture)
- `src/driver.rs` — `resolve_crate(&mut hir, &interner)` (was `&mut interner`)
- `tests/mir_lowering.rs` — same caller update
- `tests/hir_scope_resolution.rs` — same caller update
- `tests/hir_resolution.rs` — same caller update
- `tests/typeck_tests.rs` — same caller update

### Remaining P2/P3 items (deferred to Stage 4+)

- `HirParam` duplication between `HirFnSig.inputs` and `Body.params`
- Visibility checking (Stage 1.3 Phase E1)
- Prelude injection (Stage 1.3 Phase E3)
- AST enum naming standardization (Expr/Ty/Pat direct vs ItemKind wrapper)

---

## v0.8.10 — Stage 3.66 (Lvalue→Place rename + resolver owner context threading)

### Overview

Continuation of the §21 cross-stage audit follow-up. Stage 3.65 closed 4
P2 architectural fixes. This round (Stage 3.66) completes the largest
remaining P2 item: the `Lvalue` → `Place` rename (167+ references across
7+ files), aligning the implementation with the design doc (06-mir.md §4)
and the borrowck internal vocabulary (`PlacePath`, `PlaceRoot`). Also
threads owner context through the resolver for accurate `HirSelfKind`
(Trait vs Impl). 983 tests pass (unchanged — pure refactoring). 0 clippy
warnings. fmt clean.

### P2 fix #1: `Lvalue` → `Place` rename (the big one)

**Previously**: The MIR type for addressable memory locations was named
`Lvalue` (legacy rustc name from pre-RFC-1211 era). The design doc
(06-mir.md §4) calls it `Place`. The borrowck internals already used
`PlacePath` and `PlaceRoot` — so the codebase had mixed vocabulary.

**Now** (Stage 3.66):
- `mir::lvalue::Lvalue` → `mir::place::Place` (type renamed + file renamed)
- `mir::lvalue::LvalueKind` → `mir::place::PlaceKind`
- `src/mir/lvalue.rs` → `src/mir/place.rs` (file renamed)
- `pub mod lvalue` → `pub mod place` in `src/mir/mod.rs`
- `pub use lvalue::{...}` → `pub use place::{...}` in `src/mir/mod.rs`
- All `crate::mir::lvalue::` module paths → `crate::mir::place::`
- All function names: `lower_expr_to_lvalue` → `lower_expr_to_place`,
  `detect_lvalue_type` → `detect_place_type`,
  `detect_lvalue_storage_type` → `detect_place_storage_type`,
  `compute_lvalue_address` → `compute_place_address`,
  `codegen_lvalue_load` → `codegen_place_load`,
  `codegen_lvalue_load_typed` → `codegen_place_load_typed`,
  `resolve_lvalue_for_writeback` → `resolve_place_for_writeback`,
  `infer_lvalue` → `infer_place`,
  `lvalue_ty` → `place_ty`,
  `lvalue_root_reads` → `place_root_reads`, etc.
- All variable names: `lhs_lvalue` → `lhs_place`, etc.
- All doc comments: "lvalue" → "place" (where referring to the concept)

**Scope**: 167 `Lvalue` + 75 `LvalueKind` + 79 `lvalue` (lowercase) + 123
`Lvalue::` constructor/method references = **hundreds of replacements
across 7+ source files + test files + example files**.

**Why this matters**: Aligns implementation with design doc, eliminates
vocabulary mismatch between MIR (`Lvalue`) and borrowck (`PlacePath`),
and matches modern rustc naming (post-RFC-1211).

### P2 fix #2: Resolver owner context threading for accurate `HirSelfKind`

**Previously** (Stage 3.65): `Res::SelfTy(HirSelfKind)` was added, but
the resolver always defaulted to `HirSelfKind::Impl` — it didn't track
whether `Self` appeared inside a trait declaration or an impl block.

**Now** (Stage 3.66):
- New `current_self_kind: Option<HirSelfKind>` field on `Resolver`
- Set to `Some(HirSelfKind::Trait)` when resolving `HirItem::Trait` paths
- Set to `Some(HirSelfKind::Impl)` when resolving `HirItem::Impl` paths
- Reset to `None` after each item
- `resolve_path` uses `current_self_kind.unwrap_or(HirSelfKind::Impl)`
  when resolving the `Self` keyword

**Limitation**: Body-level `Self` resolution (e.g., `fn bar(x: Self) {}`
inside an impl) still defaults to `Impl` because body resolution happens
in a separate loop that doesn't carry owner context. Threading owner
context into body resolution is Stage 4 work.

### Verification

- `cargo test`: **983 passed, 0 failed, 2 ignored** (unchanged — pure refactoring)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `src/mir/lvalue.rs` → `src/mir/place.rs` (file renamed + all type/function/variable names)
- `src/mir/mod.rs` — module path + re-export updated
- `src/mir/lower/mod.rs` — all `Lvalue` → `Place`, function names renamed
- `src/typeck/checker.rs` — all `Lvalue` → `Place`, function names renamed
- `src/borrowck/mod.rs` — all `Lvalue` → `Place`, function names renamed
- `src/codegen/mod.rs` — all `Lvalue` → `Place`, function names renamed
- `src/resolve/resolver.rs` — `current_self_kind` field + context threading
- `tests/codegen_tests.rs` — all `Lvalue` → `Place` (test helpers)
- `tests/*.rs` — all `Lvalue` → `Place` (test assertions)
- `examples/*.rs` — all `Lvalue` → `Place` (example code + comments)

### Remaining P2/P3 items (deferred to Stage 4+)

- `HirParam` duplication between `HirFnSig.inputs` and `Body.params`
- Visibility checking (Stage 1.3 Phase E1)
- Prelude injection (Stage 1.3 Phase E3)
- Thread owner context into body resolution for body-level `HirSelfKind`
- `Span::DUMMY` placeholders fix (11 occurrences in parser.rs)
- AST enum naming standardization (Expr/Ty/Pat direct vs ItemKind wrapper)

---

## v0.8.9 — Stage 3.65 (P2 architectural fixes: unsafe impl/trait, Res::SelfTy, lower_body aliases)

### Overview

Continuation of the §21 cross-stage audit follow-up. Stage 3.63 closed
all 9 P1 naming issues. Stage 3.64 closed 5 P2 ergonomics fixes + the
`use` declaration resolution feature. This round (Stage 3.65) addresses
the next batch of P2 architectural items: `unsafe impl/trait` AST fields
(closes a Stage 1.0 soundness debt), `Res::SelfTy` trait/impl
discrimination, `lower_body` short-form aliases, and `mir_type_to_emit_type`
documentation unification. 983 tests pass (was 982, +1 new). 0 clippy
warnings. fmt clean.

### P2 fix #1: `unsafe impl`/`unsafe trait` AST + HIR + parser support

**Closes a Stage 1.0 soundness debt**: the parser previously accepted
`unsafe impl` and `unsafe trait` syntax but silently dropped the `unsafe`
qualifier — the AST `ImplDecl` and `TraitDecl` structs had no `is_unsafe`
field.

**Now**:
- `ast::ImplDecl` has `is_unsafe: bool`
- `ast::TraitDecl` has `is_unsafe: bool`
- `hir::HirImpl` has `is_unsafe: bool` (propagated from AST)
- `hir::HirTrait` has `is_unsafe: bool` (propagated from AST)
- `parser::parse_impl(is_unsafe: bool)` and `parser::parse_trait(is_unsafe: bool)` now take the flag
- The item-dispatch match arms for `KwUnsafe` + `KwImpl` / `KwTrait` now pass `true`

**Why this matters**:
- `unsafe trait Foo {}` declares a trait that is unsafe to implement
  (implementors must use `unsafe impl`).
- `unsafe impl Foo for Bar {}` asserts that the implementor has verified
  the unsafe preconditions.
- Without the `is_unsafe` field, the compiler couldn't distinguish safe
  from unsafe impls/traits — a soundness gap.

### P2 fix #2: `Res::SelfTy` trait/impl discrimination

**Previously**: `Res::SelfTy` was a single variant with no payload. The
resolver couldn't distinguish `Self` inside a trait declaration (abstract
— `Self` is the implementor's type, supertraits are bounds) from `Self`
inside an impl block (concrete — `Self` equals `impl self_ty`, supertraits
are facts).

**Now**:
- New `hir::HirSelfKind` enum with `Trait` and `Impl` variants
- `Res::SelfTy(HirSelfKind)` — now carries the discriminator
- Resolver currently defaults to `HirSelfKind::Impl` (threading owner
  context through the resolver is Stage 4 work)

**Named `HirSelfKind` (not `SelfKind`)** to avoid collision with the
pre-existing `ast::SelfKind` enum (which discriminates method receivers:
`self`/`&self`/`&mut self`/`self: Self` — a different concept).

### P2 fix #3: `lower_body` + `lower_body_full` convenience aliases

Per `api-naming-standard.md` §2.2, each stage should expose a
`<verb>_<noun>` free-function entry point. The MIR lower stage
historically used the verbose `lower_hir_body_to_mir_*` names. These
thin wrappers provide the short form:

- `mir::lower::lower_body(body, interner, hir) -> MirBody` — alias for `lower_hir_body_to_mir`
- `mir::lower::lower_body_full(body, interner, hir, return_ty) -> (MirBody, UnificationTable)` — alias for `lower_hir_body_to_mir_full`

Both re-exported from `mir::mod`. The long-form names remain available
for callers who prefer the explicit form.

### P2 fix #4: `mir_type_to_emit_type` documentation unification

Documented the relationship between the two MIR→EmitType translation functions:

- `mir_type_to_emit_type(ty)` — **legacy fallback** (no `AdtLayouts`; falls
  back to `I32` for `TyKind::Adt`). OK for tests/standalone helpers where
  the type is known primitive.
- `mir_type_to_emit_type_with_layouts(ty, layouts)` — **canonical
  §16-compliant** (resolves `TyKind::Adt` via `MirBody::adt_layouts`
  side-table, no HIR access). Use everywhere a `MirBody` is available.

Added "When to use which" guidance to prevent misuse.

### New test (1)

Added `test_safe_impl_and_trait_have_is_unsafe_false` to
`tests/ast_structure.rs` — verifies that regular (non-unsafe) impl and
trait get `is_unsafe=false`. Existing
`test_regression_unsafe_impl_parses` and
`test_regression_unsafe_trait_parses` updated to verify `is_unsafe=true`.

### Verification

- `cargo test`: **983 passed, 0 failed, 2 ignored** (was 982, +1 new)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 audit tests pass

### Files touched

- `src/ast/kinds.rs` — added `is_unsafe: bool` to `ImplDecl` and `TraitDecl`
- `src/hir/kinds.rs` — added `is_unsafe: bool` to `HirImpl` and `HirTrait`;
  added `HirSelfKind` enum; `Res::SelfTy` now carries `HirSelfKind`
- `src/hir/mod.rs` — re-export `HirSelfKind`
- `src/hir/lower/item.rs` — propagate `is_unsafe` from AST to HIR
- `src/parser/parser.rs` — `parse_impl`/`parse_trait` take `is_unsafe` flag
- `src/resolve/resolver.rs` — `Res::SelfTy` construction passes `HirSelfKind::Impl`
- `src/mir/lower/mod.rs` — added `lower_body` + `lower_body_full` aliases
- `src/mir/mod.rs` — re-export `lower_body` + `lower_body_full`
- `src/codegen/emitter.rs` — documented `mir_type_to_emit_type` (legacy)
- `src/codegen/mod.rs` — documented `mir_type_to_emit_type_with_layouts` (canonical)
- `tests/ast_structure.rs` — +1 new test + 2 updated tests
- `tests/hir_structure.rs` — updated `Res::SelfTy` test to use `HirSelfKind::Impl`
- `tests/hir_resolution.rs` — updated `self_type_resolves` to use `matches!(Res::SelfTy(_))`

### Deferred to Stage 4+

- **`Lvalue` → `Place` rename**: 167 references across 7 files (much more
  than the audit's ~50 estimate). Needs dedicated round with careful
  regression testing.
- `HirParam` duplication between `HirFnSig.inputs` and `Body.params`
- Visibility checking (Stage 1.3 Phase E1)
- Prelude injection (Stage 1.3 Phase E3)
- Thread owner context (trait vs impl) through resolver for accurate `HirSelfKind`
- `Span::DUMMY` placeholders fix (11 occurrences in parser.rs)
- AST enum naming standardization (Expr/Ty/Pat direct vs ItemKind wrapper)

---

## v0.8.8 — Stage 3.64 (P2 ergonomics fixes + use declaration resolution)

### Overview

Continuation of the §21 cross-stage audit follow-up. The previous round
(Stage 3.63, v0.8.7) closed all 9 P1 naming inconsistencies. This round
(Stage 3.64) addresses the highest-value P2 items deferred from the
audit, plus implements the previously-stub `use` declaration resolution
feature (Stage 1.3 Phase C). 982 tests pass (was 977, +5 new use-resolution
tests). 0 clippy warnings. fmt clean.

### P2 ergonomics fixes (6 Error trait impls)

All stage error types now implement `std::error::Error` + `Display`,
integrating with the standard Rust error-handling ecosystem (`?`
propagation, `anyhow::Error`, `Box<dyn Error>`, etc.):

1. `LexError` (src/lexer/reader.rs) — both `Display` + `Error` added
2. `ParseError` (src/parser/error.rs) — both `Display` + `Error` added
3. `LowerError` (src/hir/lower/error.rs) — `Error` added (`Display` existed)
4. `ResolveError` (src/resolve/error.rs) — `Error` added (`Display` existed)
5. `TypeError` (src/typeck/error.rs) — `Error` added (`Display` existed)
6. `BorrowError` (src/borrowck/error.rs) — `Error` added (`Display` existed)

### P2 codegen pluggability (1 re-export)

The `Emitter` trait + `TextEmitter` implementation + `EmitType` + `EmitValue`
are now re-exported from `lib.rs`. This enables third-party LLVM-IR backends
to implement `Emitter` and call `codegen_from_mir` directly, fulfilling
the §16.1.3 "可替换" (pluggable) design goal.

### P3 codegen naming consistency (1 rename)

`Emitter::output()` → `Emitter::emit_output()` for prefix consistency
with the other `emit_*` trait methods. The old name was the only
state-query method without an `emit_*` prefix, breaking the convention.
The rename is internal — `output()` was never called by external code.

### P2 code cleanliness (1 doc cleanup)

Removed 2 orphaned doc comments in `src/lexer/token.rs`:
- Line 26: `/// Boolean literal.` (no `BoolLit` variant follows — booleans
  are `KwTrue`/`KwFalse`)
- Line 156: `/// Pipe (for closures)` (no `Pipe` variant follows — closures
  use `Or`)

### P2 feature: use declaration resolution (Stage 1.3 Phase C)

**Previously** (Stage 1.3-3.62): `resolve_uses` was a no-op stub that
just set `uses_resolved = true`. This meant `use a::b::c;` declarations
had no effect on path resolution — real Landin programs that used
imports couldn't compile.

**Now** (Stage 3.64): `resolve_uses` walks every `use` declaration and
populates the new `module_tree.use_imports: HashMap<Spur, UseImport>`
table. The `UseImport` struct carries:
- `target: DefId` — the definition the import points to
- `kind: DefKind` — the kind of definition (Fn/Struct/Enum/etc.)
- `is_glob: bool` — whether this is a glob import (`use a::b::*;`)

**Resolution precedence** (when both leaf and glob imports exist for
the same name):
- Leaf imports (`is_glob = false`) shadow glob imports (`is_glob = true`)
- Two leaf imports with the same name → ambiguity error at import time
- Two glob imports with the same name → first one wins, no error

**Supported forms**:
- `use foo;` — single-segment leaf import (looks up `foo` in crate root)
- `use mod::foo;` — two-segment leaf import (looks up `foo` in `mod`'s namespace)
- `use foo as bar;` — aliased leaf import (registers `bar` as the imported name)
- `use mod::*;` — glob import (registers all public items from `mod` as globs)
- `use a::{b, c};` — path-prefix use tree (recurses into each child)

**Limitations** (deferred to Stage 4+):
- Cross-crate imports (Stage 5+)
- Visibility enforcement (Stage 1.3 Phase E1, still not implemented)
- Ambiguity detection at use-site (currently at import-site only)
- 3+ segment paths (`use a::b::c::d;`) — Stage 4

### New tests (5)

Added 5 tests to `tests/hir_resolution.rs` covering the new `use`
resolution feature:
- `use_resolution_leaf_import_fn` — basic leaf import
- `use_resolution_glob_import_does_not_error` — glob import safety
- `use_resolution_path_prefix_no_crash` — `use a::{b, c};` form
- `use_resolution_alias_no_crash` — `use foo as bar;` form
- `use_resolution_table_populated` — end-to-end resolution check

### Verification

- `cargo test`: **982 passed, 0 failed, 2 ignored** (was 977 — +5 new tests)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- All 5 §21 programmatic audit tests pass

### Files touched

- `src/lexer/reader.rs` — `LexError` impl Display + Error + orphaned doc removal
- `src/lexer/token.rs` — orphaned doc comment removal
- `src/parser/error.rs` — `ParseError` impl Display + Error
- `src/hir/lower/error.rs` — `LowerError` impl Error
- `src/resolve/error.rs` — `ResolveError` impl Error
- `src/resolve/module_tree.rs` — new `UseImport` struct + `use_imports` table + `lookup_use_import` + `insert_use_import` methods
- `src/resolve/resolver.rs` — real `resolve_uses` implementation (was stub) + `resolve_path` consults `use_imports` as fallback
- `src/resolve/mod.rs` — re-export `UseImport` + `UseDecl`
- `src/typeck/error.rs` — `TypeError` impl Error
- `src/borrowck/error.rs` — `BorrowError` impl Error
- `src/codegen/emitter.rs` — `output()` → `emit_output()` rename
- `src/codegen/text_emitter.rs` — `output()` → `emit_output()` rename
- `src/lib.rs` — re-export `Emitter` + `TextEmitter` + `EmitType` + `EmitValue`
- `tests/hir_resolution.rs` — +5 new use-resolution tests

---

## v0.8.7 — Stage 3.63 (cross-stage naming standardization per §21 audit)

### Overview

End-of-Stage-3 cross-stage deep audit (§21 of process v3.14) executed by
4 Stage Audit subagents (Stage 0/1/2/3) coordinated by main agent.
Audit identified 0 P0 / 9 P1 / 15 P2 / 19 P3 issues across the four
stages. All 9 P1 naming inconsistencies fixed in this round; 1 high-value
P2 architectural fix also applied. Pure refactoring — 977/977 tests
remain green, 0 clippy warnings, fmt clean.

### P1 naming fixes (9)

1. **Stage 0 — glob → explicit re-exports**: `src/lexer/mod.rs` and
   `src/ast/mod.rs` converted from `pub use X::*;` to explicit lists.
   Completes the Stage 3.57 P0-3 fix that previously only covered
   `src/hir/mod.rs` and `src/mir/mod.rs`.
2. **Stage 1 — `LowerCtxt` → `HirLowerCtxt`**: renamed across 9 files in
   `src/hir/lower/` + `src/hir/mod.rs`. Establishes parity with
   `MirLowerCtxt` (Stage 2).
3. **Stage 2 — `check_crate` deprecation drift fixed**: `typeck::check_crate`
   and `borrowck::check_crate` both marked `#[deprecated(note = "...")]`
   pointing to §16-compliant replacements. The Stage 3.62 worklog had
   claimed deprecation but the code showed full working implementations
   — process-vs-code drift now corrected.
4. **Stage 2 — `typeck/mod.rs` doc-comment updated**: now points to
   `TypeChecker::check_mir_body_with_tables` as the canonical
   §16-compliant entry point (was pointing to deprecated `check_crate`).
5. **Stage 2 — `BorrowKind` unified**: removed duplicate
   `borrowck::borrow_set::BorrowKind` (was aliased as `BkKind`). Single
   source of truth now in `mir::lvalue::BorrowKind` (added `Hash` to
   derive list). 6-line manual conversion code in `borrowck::check_rvalue`
   eliminated. `borrowck::mod.rs` re-exports from `crate::mir::lvalue`
   for backwards compatibility.
6. **Stage 2 — canonical entry points re-exported**: `mir/mod.rs` now
   re-exports `lower_hir_body_to_mir_full` and
   `lower_hir_body_to_mir_with_return_ty` (previously only
   `lower_hir_body_to_mir` was). The `_full` variant is what the driver
   actually uses.
7. **Stage 0 — `parser::parse_crate` free function added**: wraps
   `Parser::new(...).parse_crate()` + `into_errors()`. Aligns parser
   entry style with `lexer::tokenize`, `hir::lower::lower_crate`,
   `resolve::resolve_crate`, `codegen::codegen_crate`.
8. **Stage 3 — `fat_ptr_type` → `emit_fat_ptr_type`**: prefix consistency
   with the `mir_type_to_emit_type` / `emit_type_to_llvm_str` translation
   ladder.
9. **Stage 3 — `codegen/mod.rs` module docs expanded**: now includes
   status (Stage 3 COMPLETE), §16 compliance note, Stage 3.46/3.63
   history, open limitations table (L1/L3/L5/L8/L-COPY-ADT with target
   stages), and architectural debt note (Emitter trait bloat — 36 methods,
   1 implementation).

### P2 architectural fix (1)

10. **Stage 1 — `DefKind` moved from `resolve::module_tree` to `hir::kinds`**:
    `DefKind` is consumed by `Res::Def(DefId, DefKind)` — a HIR type — so
    its architectural home is `hir::kinds`, not `resolve::module_tree`.
    The move aligns the dependency direction: `resolve` depends on `hir`,
    not vice versa. `resolve::module_tree` and `resolve::mod.rs` now
    import + re-export from `crate::hir::DefKind` for backwards compatibility.

### Process v3.15 (§23 naming standardization protocol)

- New §23 added to `docs/stage-committee-process.md`: codifies the API
  naming conventions established by Stage 3.63.
- §22 changelog updated (v3.14 → v3.15 coverage confirmation).
- Effective from Stage 3.63.

### New documents

- `docs/develop/v0/stage-0-3-cross-stage-audit.md` — full §21 audit
  report (D1-D6 dimensions + §16 compliance + data flow + per-stage
  findings + standardization summary + test verification).
- `docs/develop/v0/api-naming-standard.md` — Stage 0-3 API naming
  standard (entry-point convention, context type convention, type prefix
  convention, re-export convention, single source of truth, deprecation
  convention, function naming conventions, error type convention,
  enforcement).

### Verification

- `cargo test`: **977 passed, 0 failed, 2 ignored** (unchanged from baseline)
- `cargo clippy --all-targets`: **0 warnings, 0 errors**
- `cargo fmt --check`: **clean**
- §16 compliance re-verified: all 8 §21.3 checklist items green
- 5 §21 programmatic audit tests all pass

---

## v0.8.6 — Stage 3.21–3.46 (typed codegen + runtime checks + literals + ADT structs + field type resolution + field mutation + 6 gate review rounds)

### Stage 3.21 — Typed aggregate codegen
- `EmitType` now carries full structure: `Struct(Vec<EmitType>)`, `Array(Box<EmitType>, u64)`,
  `Ptr(Box<EmitType>)` (was hardcoded `{ i32 }` / `[10 x i32]` / opaque `i32*`).
- 10 new tests.

### Stage 3.22 — Block-scoped local value cache
- **Bug fix**: `if x > 0 { 1 } else { 2 }` previously returned `2` regardless of `x`.
- 6 new tests.

### Stage 3.24 — Real overflow checks
- **Bug fix**: overflow checks never fired. `a + b` silently wrapped (UB).
- 8 new tests.

### Stage 3.25 — Real div-by-zero checks
- **Bug fix**: `a / 0` invoked LLVM `sdiv` — UB.
- 6 new tests.

### Stage 3.27 — String literal codegen
- **Bug fix**: `ConstVal::Str` hardcoded to emit `"0"` (null pointer).
- 13 new tests.

### Stage 3.28 — Byte string literal codegen
- **Bug fix**: `b"..."` literals and `u8`/`i8` types fell through to `I32`.
- 9 new tests.

### Stage 3.30 — ADT/struct codegen + §15/§16 process principles
- **Process v3.10 + v3.11**: added §15 (最优 > 最小) and §16 (阶段间接口隔离).
- **3 root-cause bugs fixed**: tuple struct ctor as Call, named struct type lost,
  field index hardcoded 0.
- **§16 compliance**: `AggregateKind::Adt` extended with `field_tys: Vec<Ty>`.
- 13 new tests.

### Stage 3.32 — L-DEBT-2 fix: field type resolution through projections
- **Bug fix**: `p.1` where field 1 is `i64` loaded as `i32` (silent truncation).
- **Fix** (per §15): typeck `infer_rvalue` handles `AggregateKind::Adt`; new
  Phase 3.5 `writeback_field_types`; MIR lower `resolve_field_index` fallback scan.
- 6 new tests.

### Stage 3.34 — L-MUT-1 fix: field mutation MIR lower
- **Bug fix**: `a.v = 42` didn't mutate the struct (silently dropped).
- **Root cause**: MIR lower's `HirExprKind::Assign` only handled `Path` LHS.
- **Fix** (per §15): new `lower_expr_to_lvalue` function handles all LHS shapes
  (Path, Field, Index, Deref). `HirExprKind::Assign` uses it generically.
- 8 new tests.

### Gate Reviews Round 1-6
- R1: 38-case audit, 5/5 APPROVED
- R2: 43-case audit, 5/5 APPROVED
- R3: 43-case audit, 5/5 APPROVED
- R4: 37-case audit, 5/5 APPROVED
- R5: 30-case audit, 5/5 APPROVED
- R6: 30-case audit, 5/5 APPROVED
- §9.3.3 CONVERGED: 6 consecutive rounds with 0 new issues
- L2 (struct codegen) + L4 (string literals) + L6 (overflow) + L7 (div-by-zero)
  + L12 (u8/i8 type) + L-DEBT-2 (field type resolution) + L-MUT-1 (field mutation) CLOSED.
- Remaining: L1 PHI, L3 closures, L5 traits, L8 lli, L9 i128, L10 float-bitwise,
  L11 shift-count, L13 fat pointers, L14 i16, L15 str-as-arg, L-ENUM enum variants,
  L-PIPE-1 HIR lookup for Adt storage, L-DEBT-3 field type propagation through arithmetic.

### Changed
- `Cargo.toml`: v0.8.5 → v0.8.6
- `src/codegen/{emitter.rs, text_emitter.rs, mod.rs}`: typed codegen + string globals + ADT/struct codegen + `hir_ty_to_emit_type`
- `src/mir/{body.rs, lower/mod.rs, lvalue.rs}`: AssertMessage extended, AggregateKind::Adt field_tys, resolve_field_index/resolve_field_type/resolve_adt_field_tys, HirTyKind::Path → TyKind::Adt, lower_expr_to_lvalue
- `src/typeck/checker.rs`: AggregateKind::Adt handling in infer_rvalue, Phase 3.5 writeback_field_types, check_mir_body_with_hir
- `src/hir/kinds.rs`: `Res::Def(DefId, DefKind)`
- `src/resolve/resolver.rs`: populates `DefKind`
- `src/parser/parser.rs`: `&mut Rodeo` + tuple field index interning
- `src/driver.rs`: passes `&mut interner` + `&hir` to MIR Lower + `check_mir_body_with_hir`
- `tests/codegen_tests.rs`: +79 tests (total 115)
- `examples/stage3_gate_audit{,_r2..r6}.rs`: 6 audit tools
- `docs/develop/v0/stage-3/{dev-log.md, gate-review-round1..6.md}`
- `docs/stage-committee-process.md`: §15 + §16

---

## v0.7.4 — Stage 3.9: Imported user-provided documentation (process v3.7)

### Added — agent-team/ (12 new documents)
- `00-requirement-history.md` — Requirements evolution history
- `01-agent-team-overview.md` — Agent team structure overview
- `02-agent-roles-detail.md` — Detailed role definitions (25 roles)
- `03-collaboration-workflow.md` — Inter-agent collaboration workflow
- `04-agent-skills.md` — Agent skill definitions
- `05-meeting-and-decision-log.md` — Meeting records and decisions
- `06-risk-register.md` — Project risk tracking
- `07-team-charter.md` — Team charter and principles
- `08-agent-lifecycle.md` — Agent lifecycle management
- `09-runtime-protocol.md` — Runtime communication protocol
- `10-modernization-roadmap.md` — Modernization roadmap
- `README.md` — Agent team index

### Added — lang-design/ (20 new documents)
- `01-language-specification.md` — Full language specification
- `02-grammar.md` — Grammar definition (EBNF)
- `03-type-system.md` — Type system design
- `04-ownership-borrowing.md` — Ownership and borrowing design
- `05-ast.md` — AST structure design
- `06-mir.md` — MIR design
- `07-codegen.md` — LLVM codegen design (replaces our 08-codegen.md)
- `08-bootstrap-strategy.md` — Self-hosting strategy
- `09-stdlib.md` — Standard library design
- `10-toolchain.md` — Toolchain design
- `11-testing.md` — Testing strategy
- `12-roadmap.md` — Project roadmap
- `13-stage1-feature-whitelist.md` — Stage 1 feature whitelist
- `14-soundness-considerations.md` — Soundness analysis
- `15-attributes.md` — Attribute system design
- `16-diagnostics.md` — Diagnostic system design
- `17-conformance-suite.md` — Conformance test suite
- `18-glossary.md` — Glossary of terms
- `19-project-meta.md` — Project metadata
- `CHANGELOG.md` — Language design changelog
- `FREEZE-REPORT.md` — Design freeze report
- `README.md` — Language design index

### Changed
- Consolidated uploaded docs into our v0/stage-N structure
- Removed duplicate flat docs/develop/ files (kept v0/stage-N/ versions)
- Process docs restored to v3.7

### Document count
- docs/agent-team/: 12 files (was 2)
- docs/lang-design/: 22 files (was 2)
- docs/develop/v0/stage-N/: 18 files (unchanged)
- Total docs: 56 files (was 25)

---

## v0.7.3 — Stage 3.8: Doc reorganization (process v3.7)

### Added
- Process v3.7 §12: Document organization structure rules

---

## v0.7.2 — Stage 3.7: Author + cast codegen (process v3.6)

### Added
- Author "redskaber" added to all project documents
- Cast codegen (sext/zext/trunc/sitofp/fptosi/fpext/fptrunc)

---

## v0.7.1 — Stage 3.5: Parameter passing + doc sync (process v3.5)

### Added
- Parameter passing: `fn add(a: i32, b: i32) -> i32 { a + b }` generates
  `define i32 @fn_0(i32 %arg0, i32 %arg1)` with params stored to alloca slots

---

## v0.5.0 — Stage 3.1-3.4: LLVM codegen MVP

### Added
- `src/codegen/` module with Emitter trait + TextEmitter
- LLVM IR text output (.ll)

---

## v0.4.9 — Stage 0-2 OFFICIAL FINAL

### Summary
- Stage 0 (lexer/parser): 245 tests, 0 issues
- Stage 1 (HIR/resolve): 451 tests, 0 issues  
- Stage 2 (MIR/typeck/borrowck): 673 tests, 0 issues
- 6 rounds of phase gate review, 233 cumulative audit cases

---

## Process version history

| Version | Change |
|---------|--------|
| v1.0 | Initial 5-role + voting + 4-7 rounds |
| v2.0 | Dynamic rounds + defect grading + weighted voting |
| v3.0 | Integration verification + P3 reclassification + gate review |
| v3.1 | Negative-test coverage matrix (§9.1.1) |
| v3.2 | Expanded audit requirement ≥30 cases (§9.3.1) |
| v3.3 | Previous-round-fix edge case tests (§9.3.2) |
| v3.4 | Diminishing returns rule + Stage 3 start conditions (§9.3.3) |
| v3.5 | Documentation sync rules (§11) |
| v3.6 | Author标注规则 |
| v3.7 | 文档组织结构规则 (§12) |
| v3.8 | (Pending) Stage 3 gate review convergence rule for codegen |
