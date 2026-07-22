//! Landin Compiler
//!
//! Stage 0 (v0.1.x): Lexer + Parser + AST — COMPLETE
//! Stage 1 (v0.2.x): HIR + Name Resolution — COMPLETE (Stage 3.64: `use` decl resolution;
//!   Stage 3.65: `unsafe impl/trait` AST fields + `Res::SelfTy` trait/impl discrimination)
//! Stage 2 (v0.4.x): MIR + Typeck + Borrowck — COMPLETE (Stage 3.65: `lower_body` aliases)
//! Stage 3 (v0.8.x): LLVM Codegen — COMPLETE (soundness-critical limitations closed)
//!   Stage 3.63 (v0.8.7): cross-stage naming standardization per §21 audit
//!     (9 P1 naming fixes + 1 P2 architectural fix; pure refactoring).
//!   Stage 3.64 (v0.8.8): P2 ergonomics fixes + use declaration resolution
//!     (6 Error trait impls + Emitter re-export + emit_output rename +
//!      basic use resolution: leaf/glob/path-prefix/alias).
//!   Stage 3.65 (v0.8.9): P2 architectural fixes
//!     (unsafe impl/trait AST+HIR+parser + Res::SelfTy HirSelfKind discrimination +
//!      lower_body aliases + mir_type_to_emit_type documentation).
//!   Stage 3.66 (v0.8.10): Lvalue → Place rename (167+ refs, aligns with design
//!     doc 06-mir.md §4 + borrowck vocabulary) + resolver owner context threading
//!     for accurate HirSelfKind (Trait vs Impl).
//!   Stage 3.67 (v0.8.11): P2 cleanup — body owner context threading (body-level
//!     HirSelfKind accurate) + &mut Rodeo → &Rodeo in resolve_crate (lexer now
//!     interns keywords) + Span::DUMMY placeholders fixed (11 occurrences in
//!     parser.rs → keyword spans).
//!   Stage 3.68 (v0.8.12): Visibility checking infrastructure — def_visibility
//!     map + check_visibility hook (stub, ready for Stage 4 nested modules).
//!   Stage 3.69 (v0.8.13): Process v3.16 (§25 阶段末尾深度审查协议) +
//!     Stage 0-3 deep review (GO-WITH-CONDITIONS for Stage 4).
//!   Stage 4.1-4.2 (v0.9.0): Nested module support (recursive build_module_tree
//!     + child ModuleNode) + L1 PHI optimization CLOSED (design decision: rely
//!     on LLVM mem2reg).
//!   Stage 4.3-4.4 (v0.9.1): Visibility enforcement activation (check_visibility
//!     implements pub/private/pub-restricted checks) + L3 closure lowering
//!     (AggregateKind::Closure + TyKind::Closure → empty struct; capture analysis
//!     deferred to Stage 4.5).
//!   Stage 4.5 (v0.9.2): Complete dev-logs for all stages (Stage 1 + Stage 2 +
//!     Stage 4 dev-logs created; Stage 0 + Stage 3 dev-logs updated with
//!     retroactive entries).
//!   Stage 4.6 (v0.9.3): Process v3.17 — §17 测试目录标准化与三阶段文档协议
//!     (开发轮/审查轮/深度审查轮) + standardized tests/ directory structure.
//!   Stage 4.7 (v0.9.4): L3 closure capture analysis — collect_captured_locals
//!     detects external variables referenced in closure body; captures populate
//!     closure struct fields + Aggregate operands.
//!   Stage 4.8 (v0.9.5): tests/ directory restructure — all 13 flat test files
//!     migrated to standardized tests/v0/stage{N}/plan/ per v3.17 §17.1.
//!   Stage 4.9 (v0.9.6): L3 closure call lowering — detect TyKind::Closure in
//!     Call lowering; simplified placeholder (full call deferred to Stage 4.10).
//!   Stage 4.10 (v0.9.7): Macro system — built-in macro expansion (println!,
//!     stringify!, assert!) in MIR lowering; MacroCall no longer produces Error.
//!   Stage 4.11 (v0.9.8): Performance benchmark suite (benches/compile_bench.rs,
//!     5 benchmarks) + Architecture Decision Records (ADR-001 to ADR-007).
//!     Closes deep review R37 conditions (QA benchmark + D7 documentation).
//!   Stage 4.12 (v0.9.9): Process v3.18 (worklog snapshot sync to docs/worklog/)
//!     + current_module tracking for visibility enforcement + 1000 tests milestone.
//!   Stage 4.13 (v0.10.0): Full closure call lowering — extract captures from
//!     closure struct + produce inferred-type result (inline body deferred to Stage 5).
//!   Stage 4.14 (v0.10.1): Stage 4 deep review (§25) — 7-dimension analysis,
//!     GO for Stage 5. Stage 4 COMPLETE.
//!   Cross-stage (v0.10.2): Stage 0-4 deep review (§21+§25) — pipeline 7-point
//!     verification, 16 tech debt items cataloged, GO for Stage 5.
//!   Stage 5.1 (v0.11.0): TraitResolver — collect trait definitions + impl blocks +
//!     build dispatch tables (ImplMap + MethodMap). `src/traits/mod.rs` created.
//!   Stage 5.2 (v0.11.1): TraitResolver integrated into driver pipeline —
//!     CompileResult.trait_resolver populated; fmt issues fixed.
//!   Stage 5.3 (v0.11.2): ty_is_copy_with_resolver — precise Copy detection
//!     using TraitResolver (Adt fallback until DefId→name map in Stage 5.4).
//!   Stage 5.4 (v0.11.3): DefId→name reverse map in TraitResolver —
//!     `type_by_def_id` populated for struct/enum/trait; `is_copy()` and
//!     `implements_by_def_id()` query methods; full Copy detection activated.
//!   Stage 5.5 (v0.11.4): Vtable data structures — `VtableEntry` + `Vtable`
//!     + `find_vtable()` query; vtables built during collect() for each trait impl.
//!   Stage 5.6 (v0.11.5): Vtable codegen emission — `VtableEntry.fn_name`
//!     carries the resolved LLVM symbol (`landin_<Type>_<method>`);
//!     `codegen::emit_vtables()` emits `@.vtable.<trait>.<type>` globals;
//!     `Emitter::emit_vtable_global()` added to the trait; driver `body_metas`
//!     extended to emit impl method bodies with matching naming. L5 trait
//!     dispatch foundation in place; `dyn Trait` fat-pointer construction
//!     deferred to Stage 5.7+.
//!   Stage 5.7 (v0.11.6): `dyn Trait` fat-pointer construction —
//!     `emit_dyn_trait_ptr_type()` returns `{ ptr, ptr }` EmitType;
//!     `Emitter::emit_dyn_trait_const()` emits `@.dynptr.<trait>.<type>`
//!     globals referencing data + vtable; `codegen::emit_dyn_trait_ptrs()`
//!     iterates TraitResolver to emit all dyn fat pointers. Foundation for
//!     `dyn Trait` value lowering; actual MIR→codegen wiring of `dyn` locals
//!     deferred to Stage 5.8+.
//!   Stage 5.8 (v0.11.7): Standard trait registry (stdlib MVP) —
//!     `BUILTIN_TRAIT_NAMES` constant + `register_builtin_traits()` method +
//!     `BuiltinTraits` map on TraitResolver; compiler now recognizes Copy,
//!     Clone, Drop, Sized, Send, Sync, etc. without user `trait Copy {}`
//!     definition. `is_builtin_trait()` + `find_builtin_trait()` query
//!     methods added.
//!   Stage 5.9 (v0.11.8): Builtin Copy activation — `is_copy_builtin()`
//!     method on TraitResolver (looks up builtin Copy automatically, no
//!     Spur parameter needed); `ty_is_copy_with_resolver` Adt branch now
//!     uses `is_copy_builtin()` with correct `false` fallback (was unsound
//!     `true`). `impl Copy for S` now works without `trait Copy {}`.
//!   Stage 5.10 (v0.11.9): Builtin Clone/Drop activation + generic
//!     `implements_builtin_trait()` — `is_clone_builtin()` +
//!     `is_drop_builtin()` methods (parallel to `is_copy_builtin`); generic
//!     `implements_builtin_trait(def_id, trait_name_str, interner)` for
//!     any builtin trait by name. Process spec v3.20 (§0.2 task routing +
//!     §1.1 env check + §1.2 acceptance check + §1.3 spec evolution).
//!   Stage 5.11 (v0.11.10): Primitive Copy auto-detection —
//!     `BUILTIN_PRIMITIVE_COPY_KINDS` constant (10 always-Copy TyKinds) +
//!     `is_primitive_copy_kind()` free function (string-based check, avoids
//!     mir↔traits circular dep). Foundation for stdlib MVP auto-Copy.
//!   Stage 5.12 (v0.11.11): Copy detection unification —
//!     `ty_is_copy_with_resolver` primitive branches now delegate to
//!     `is_primitive_copy_kind()` (single source of truth); new
//!     `ty_is_copy_unified()` entry point (preferred for new code).
//!   Stage 5.13 (v0.11.12): Trait impl statistics —
//!     `impl_count_for_type()` + `impl_count_for_trait()` +
//!     `builtin_trait_count()` + `traits_for_type()` query methods for
//!     diagnostics and typeck trait-bound solving.
//!   Stage 5.14 (v0.11.13): Trait method query API —
//!     `trait_methods()` + `impl_methods()` + `trait_has_method()` +
//!     `traits_with_method()` + `method_count_for_trait()` for method
//!     resolution and vtable method lookup.
//!   Stage 5.15 (v0.11.14): Trait hierarchy (supertraits) —
//!     `TraitInfo.supertraits` field populated from `HirTrait.supertraits`;
//!     `trait_supertraits()` + `trait_has_supertrait()` +
//!     `supertrait_count_for_trait()` query methods for hierarchy traversal.
//!   Stage 5.16 (v0.11.15): TraitResolver summary —
//!     `summary(&Rodeo) -> String` method generates human-readable state
//!     report (trait/impl/type/vtable/builtin counts + per-trait methods/
//!     supertraits + per-type impl list). For diagnostics + debugging.
//!   Next: Stage 5.17+ (dyn Trait MIR lowering, full stdlib, mini-cargo).
//! See `docs/develop/v0/api-naming-standard.md` for the API naming standard.

pub mod ast;
pub mod borrowck;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod traits;
pub mod typeck;

// Stage 3.61: Clear public API surface — re-export the intended entry points.
// Stage 3.63: Naming standardized per docs/develop/v0/api-naming-standard.md.
// Stage 3.64: Re-export codegen Emitter trait + impls for pluggability
// (allows third-party LLVM-IR backends to implement `Emitter` and call
// `codegen_from_mir` directly).
pub use codegen::{
    codegen_crate, emit_dyn_trait_ptr_type, emit_dyn_trait_ptrs, emit_vtables, EmitType, EmitValue,
    Emitter, TextEmitter,
};
pub use driver::{compile, CompileErrors, CompileResult};
pub use traits::{
    extract_impl_self_ty_name, is_primitive_copy_kind, TraitResolver, BUILTIN_DEF_ID_BASE,
    BUILTIN_PRIMITIVE_COPY_KINDS, BUILTIN_TRAIT_NAMES,
};
