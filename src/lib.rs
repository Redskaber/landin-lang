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
//!   Stage 5.17 (v0.11.16): Vtable method resolution —
//!     `resolve_vtable_method()` + `vtable_method_names()` +
//!     `vtable_has_method()` for single-entry-point method dispatch
//!     resolution (combines find_vtable + entry lookup).
//!   Stage 5.18 (v0.11.17): Trait coherence checking —
//!     `CoherenceError` struct + `check_coherence()` +
//!     `has_coherence_error()` + `coherence_error_count()` for detecting
//!     conflicting impls (multiple `impl Trait for Type` for same pair).
//!   Stage 5.19 (v0.11.18): Trait impl completeness check —
//!     `impl_covers_trait()` + `missing_impl_methods()` +
//!     `missing_method_count()` for detecting incomplete impls (missing
//!     methods that the trait declares but the impl doesn't provide).
//!   Stage 5.20 (v0.11.19): Trait impl validation report —
//!     `ImplValidationReport` + `IncompleteImpl` structs +
//!     `validate_impls()` + `impls_are_valid()` + `all_impls_complete()`
//!     for single-pass validation of all impls (coherence + completeness).
//!   Stage 5.22 (v0.11.20): Driver validation integration —
//!     `validate_impls()` wired into driver; `CompileErrors.trait_errors`
//!     field added; coherence + completeness errors reported to user.
//!   Stage 5.24 (v0.11.22): Mini-cargo MVP —
//!     `ProjectManifest` + `BuildConfig` + `BuildResult` structs +
//!     `parse_manifest()` + `load_manifest()` + `build_project()` for
//!     project-level build orchestration via public `compile()` API.
//!   Stage 5.25 (v0.11.23): Stdlib MVP —
//!     `src/stdlib.rs` module: core types (i8-i128/u8-u128/f32/f64/bool/char/
//!     str/()/Never) + ops traits (Add/Sub/Mul/.../PartialEq/Ord/...) +
//!     convert traits (From/Into/AsRef/...) + iter traits (Iterator/...) +
//!     `StdlibPrelude` + `register_stdlib()` + `default_prelude()`.
//!   Stage 5.26 (v0.11.24): Driver stdlib integration —
//!     `register_stdlib()` wired into driver; `CompileResult.stdlib_prelude`
//!     field added; all stdlib types + traits auto-interned.
//!   Stage 5.28 (v0.11.25): Stdlib alloc layer —
//!     `STDLIB_ALLOC_TYPES` (Box/Vec/String/HashMap/Rc/Arc/...) +
//!     `STDLIB_ALLOC_TRAITS` (Display/Debug/Deref/Default/Hash) added;
//!     `register_stdlib()` + `all_stdlib_type_names()` +
//!     `all_stdlib_trait_names()` + `StdlibPrelude` extended.
//!   Stage 5.30 (v0.11.27): Stdlib std layer —
//!     `STDLIB_STD_TYPES` (File/Path/TcpStream/Thread/Mutex/Result/Option/...)
//!     + `STDLIB_STD_TRAITS` (Read/Write/Seek/Error/Termination) added;
//!     `StdlibLayer::Std` variant added; `register_stdlib()` +
//!     `all_stdlib_type_names()` + `all_stdlib_trait_names()` +
//!     `layer_for_name()` + `names_for_layer()` extended.
//!   Stage 5.31 (v0.11.28): Stdlib facade —
//!     `StdlibFacade` struct: `type_count()` + `trait_count()` +
//!     `type_count_for_layer()` + `layer_count()` + `is_stdlib_name()` +
//!     `summary()` for aggregate stdlib statistics + queries.
//!   Stage 5.33 (v0.11.29): Stdlib facade integration —
//!     `CompileResult.stdlib_facade` field added; `StdlibFacade` available
//!     to downstream stages for aggregate stdlib statistics + queries.
//!   Stage 5.34 (v0.11.30): Stdlib type resolution —
//!     `StdlibTypeKind` enum (I8-I128/U8-U128/F32/F64/Bool/Char/Str/Unit/
//!     Never/AllocType/StdType/Unknown) + `resolve_stdlib_type()` +
//!     `is_primitive_type()` + `integer_bit_width()` + `is_signed_integer()`
//!     + `is_unsigned_integer()` + `is_float_type()` for type name → kind
//!     mapping without mir::ty circular dependency.
//!   Stage 5.35 (v0.11.31): Stdlib type layout —
//!     `type_size_bytes()` + `type_alignment_bytes()` + `is_zero_sized_type()`
//!     + `type_description()` for primitive type size/alignment/ZST/desc.
//!   Stage 5.36 (v0.11.32): Stdlib trait method signatures —
//!     `StdlibTraitMethod` + `StdlibSelfKind` (4 receiver kinds) +
//!     static method tables for 25+ stdlib traits (markers empty, Clone 2
//!     methods, Drop/Default/Display/Debug/PartialEq/PartialOrd/Ord/Hash/
//!     Deref/DerefMut/IntoIterator/Iterator/Read/Write/Neg/Not + 10 binary
//!     arithmetic ops + 10 assign ops) + `stdlib_trait_methods()` +
//!     `stdlib_trait_method_count()` + `find_stdlib_trait_method()` +
//!     `is_stdlib_trait_method()` + `stdlib_traits_with_method()` for
//!     trait-method signature queries — prereq for dyn Trait MIR lowering
//!     and typeck trait-bound solving.
//!   Stage 5.37 (v0.11.33): Stdlib vtable slot layout —
//!     `StdlibVtableSlot` struct + `stdlib_trait_method_index()` +
//!     `stdlib_vtable_layout()` + `stdlib_vtable_slot_count()` +
//!     `is_stdlib_marker_trait()` + `stdlib_traits_with_vtable()` for
//!     deterministic vtable slot indexing — last static-prep step before
//!     dyn Trait MIR lowering (codegen will use these to emit
//!     `@.vtable.<trait>.<type>` globals with the correct element count
//!     and compute method call byte offsets).
//!   Stage 5.38 (v0.11.34): Stdlib vtable byte size + pointer-width-aware
//!     layout helpers — `StdlibPointerWidth` enum (Pointer32/Pointer64) +
//!     `byte_size()` method + `stdlib_pointer_width_bytes()` +
//!     `stdlib_vtable_byte_size(trait, width)` +
//!     `stdlib_vtable_method_offset(trait, method, width)` for translating
//!     slot indices into byte offsets that codegen can directly use in
//!     `alloca` / `getelementptr` calculations.
//!   Stage 5.39 (v0.11.35): Stdlib vtable construction planner —
//!     `StdlibVtablePlanEntry` + `StdlibVtablePlan` structs +
//!     `stdlib_vtable_plan(trait, provided_methods)` +
//!     `stdlib_vtable_plan_entry_count(trait)` +
//!     `stdlib_vtable_plan_is_complete(&plan)` +
//!     `stdlib_vtable_plan_missing_methods(&plan)` for combining trait
//!     method signatures + slot indexing + impl coverage into a single
//!     ordered plan that codegen can consume in one pass (no need to
//!     re-derive slot order or provided-checking at codegen time).
//!   Stage 5.40 (v0.11.36): Stdlib vtable symbol name planner —
//!     `stdlib_vtable_global_name(trait, type)` +
//!     `stdlib_dynptr_global_name(trait, type)` +
//!     `stdlib_data_global_name(type)` +
//!     `stdlib_impl_method_symbol(type, method)` +
//!     `stdlib_vtable_method_symbols(trait, type, provided)` for
//!     extracting LLVM symbol-name formatting logic from codegen into
//!     pure stdlib functions (matches existing codegen `format!` calls
//!     byte-for-byte — Stage 5.41+ will replace codegen's `format!`
//!     with these planner functions, behavior-equivalent).
//!   Stage 5.41 (v0.11.37): Stdlib vtable emission plan (aggregate) —
//!     `StdlibVtableEmission` struct (trait_name + type_name +
//!     global_name + method_symbols + slot_count + byte_size_32/64 +
//!     is_marker + is_complete) + `stdlib_vtable_emission(trait, type,
//!     provided)` + `stdlib_vtable_emissions_for_traits(traits, type,
//!     provided)` for single-call aggregation of everything codegen
//!     needs to emit `@.vtable.<trait>.<type>` global. Stage 5.42+
//!     will replace codegen's 5 separate stdlib calls with one
//!     `stdlib_vtable_emission()` call — codegen becomes simpler.
//!   Stage 5.42 (v0.11.38): Stdlib vtable emission summary + deep
//!     review #4 — `StdlibVtableEmissionSummary` struct (total_emissions
//!     + marker_count + complete_count + incomplete_count + total_slots
//!     + total_byte_size_32/64 + trait_names) +
//!     `stdlib_vtable_emission_summary(&[StdlibVtableEmission])` for
//!     project-level vtable statistics. §25 deep review #4 triggered
//!     (Stage 5.33-5.42 = 10 sub-stages since review #3).
//!   Stage 5.43 (v0.11.39): Codegen vtable emission helper — new free fn
//!     `emit_vtable_global_from_emission(&StdlibVtableEmission) -> String`
//!     in `src/codegen/mod.rs` — pure-function counterpart of
//!     `TextEmitter::emit_vtable_global()` producing byte-for-byte identical
//!     LLVM IR. **First Stage 5 sub-stage modifying codegen** — but does
//!     NOT modify existing emission path (`emit_vtables()` +
//!     `TextEmitter::emit_vtable_global()` unchanged). Stage 5.44+ will
//!     refactor `TextEmitter::emit_vtable_global()` to delegate here.
//!   Stage 5.44 (v0.11.40): Codegen vtable global text bridge — new free fn
//!     `emit_vtable_global_text(global_name, method_symbols) -> String` in
//!     `src/codegen/mod.rs` — bridge function with the **exact same
//!     parameter signature** as `TextEmitter::emit_vtable_global()`. Sits
//!     between Stage 5.43's high-level `emit_vtable_global_from_emission()`
//!     and Stage 5.45's `TextEmitter::emit_vtable_global()` delegation
//!     refactor. Handles "null" symbol → `ptr null` literal.
//!   Stage 5.45 (v0.11.41): Codegen vtable emission batch helper — new
//!     `StdlibVtableGlobalSpec` struct (global_name + method_symbols) +
//!     `emit_vtable_globals_batch(&[StdlibVtableGlobalSpec]) -> Vec<String>`
//!     free fn in `src/codegen/mod.rs`. Batch version of Stage 5.44's
//!     `emit_vtable_global_text()`. Prepares for Stage 5.46 refactor where
//!     `emit_vtables()` will construct spec list once, call batch helper,
//!     and push all IR lines to emitter in one pass.
//!   Stage 5.46 (v0.11.42): Codegen vtable spec builder — new free fn
//!     `build_vtable_global_specs(&TraitResolver, &Rodeo) -> Vec<StdlibVtableGlobalSpec>`
//!     in `src/codegen/mod.rs`. Pure-function extraction of the
//!     spec-construction logic currently inlined in `emit_vtables()` (Stage
//!     5.6). Stage 5.47 will refactor `emit_vtables()` to call this builder
//!     + `emit_vtable_globals_batch()` + push all IR lines to emitter in
//!     one pass.
//!   Stage 5.47 (v0.11.43): Codegen vtable emission orchestrator — new
//!     free fn `emit_vtables_from_resolver(&TraitResolver, &Rodeo, &mut dyn Emitter)`
//!     in `src/codegen/mod.rs`. Composes Stage 5.46's
//!     `build_vtable_global_specs()` + per-spec `Emitter::emit_vtable_global()`
//!     calls. Behavior identical to `emit_vtables()` (Stage 5.6) inline loop
//!     — verified by cross-check test. Stage 5.48 will refactor `emit_vtables()`
//!     to delegate to this orchestrator (one-liner body).
//!   Stage 5.48 (v0.11.44): Codegen dynptr global text helper — new free fn
//!     `emit_dynptr_global_text(global_name, data_symbol, vtable_symbol) -> String`
//!     in `src/codegen/mod.rs`. Pure-function counterpart of
//!     `TextEmitter::emit_dyn_trait_const()` producing byte-for-byte identical
//!     LLVM IR. **dynptr counterpart** of Stage 5.44's
//!     `emit_vtable_global_text()`. Stage 5.49 will refactor
//!     `TextEmitter::emit_dyn_trait_const()` to delegate here.
//!   Stage 5.49 (v0.11.45): Codegen dynptr spec builder — new
//!     `StdlibDynptrGlobalSpec` struct (global_name + data_symbol +
//!     vtable_symbol) + `build_dynptr_global_specs(&TraitResolver, &Rodeo)`
//!     free fn in `src/codegen/mod.rs`. Pure-function extraction of the
//!     spec-construction logic currently inlined in `emit_dyn_trait_ptrs()`
//!     (Stage 5.7). **dynptr counterpart** of Stage 5.46's
//!     `build_vtable_global_specs()`. Stage 5.50 will refactor
//!     `emit_dyn_trait_ptrs()` to call this builder + per-spec
//!     `Emitter::emit_dyn_trait_const()` calls.
//!   Stage 5.50 (v0.11.46): Codegen dynptr emission orchestrator — new
//!     free fn `emit_dynptrs_from_resolver(&TraitResolver, &Rodeo, &mut dyn Emitter)`
//!     in `src/codegen/mod.rs`. Composes Stage 5.49's
//!     `build_dynptr_global_specs()` + per-spec `Emitter::emit_dyn_trait_const()`
//!     calls. Behavior identical to `emit_dyn_trait_ptrs()` (Stage 5.7) inline
//!     loop — verified by cross-check test. **dynptr counterpart** of Stage
//!     5.47's `emit_vtables_from_resolver()`. Stage 5.51 will refactor
//!     `emit_dyn_trait_ptrs()` to delegate to this orchestrator (one-liner body).
//!   Stage 5.51 (v0.11.47): Codegen vtable + dynptr combined emission
//!     orchestrator — new free fn
//!     `emit_vtables_and_dynptrs_from_resolver(&TraitResolver, &Rodeo, &mut dyn Emitter)`
//!     in `src/codegen/mod.rs`. Composes Stage 5.47's
//!     `emit_vtables_from_resolver()` + Stage 5.50's
//!     `emit_dynptrs_from_resolver()`. **Single entry point** for codegen to
//!     emit all trait-dispatch globals (vtable + dynptr). Stage 5.52 will
//!     refactor driver/codegen to call this combined orchestrator instead of
//!     separately calling `emit_vtables()` + `emit_dyn_trait_ptrs()`.
//!   Stage 5.52 (v0.11.48): Codegen trait-dispatch emission summary — new
//!     `CodegenTraitDispatchEmissionSummary` struct (vtable_count +
//!     dynptr_count + total_global_count + trait_names + type_names +
//!     total_method_slots) + `build_trait_dispatch_emission_summary(&TraitResolver, &Rodeo)`
//!     free fn in `src/codegen/mod.rs`. **codegen counterpart** of Stage
//!     5.42's `stdlib_vtable_emission_summary()`, but computed from
//!     TraitResolver. Stage 5.53 will use this for codegen diagnostic output.
//!   Stage 5.53 (v0.11.49): Codegen trait-dispatch emission plan (final
//!     aggregate) — new `CodegenTraitDispatchEmissionPlan` struct
//!     (vtable_specs + dynptr_specs + summary) +
//!     `build_trait_dispatch_emission_plan(&TraitResolver, &Rodeo)` free fn
//!     in `src/codegen/mod.rs`. **Final aggregate API** — one call returns
//!     everything codegen needs to emit all trait-dispatch globals. Composes
//!     Stage 5.46 `build_vtable_global_specs()` + Stage 5.49
//!     `build_dynptr_global_specs()` + Stage 5.52
//!     `build_trait_dispatch_emission_summary()`. Stage 5.54 driver refactor
//!     will call this plan once.
//!   Stage 5.54 (v0.11.50): Codegen trait-dispatch emission orchestrator
//!     (plan-based) — new free fn
//!     `emit_trait_dispatch_globals_from_plan(&CodegenTraitDispatchEmissionPlan, &mut dyn Emitter)`
//!     in `src/codegen/mod.rs`. **First plan-based orchestrator** — emits
//!     all trait-dispatch globals by iterating the plan's vtable_specs +
//!     dynptr_specs. Behavior identical to
//!     `emit_vtables_and_dynptrs_from_resolver()` (Stage 5.51) when given
//!     the plan from the same resolver. Stage 5.55 driver refactor will
//!     call `build_trait_dispatch_emission_plan()` + this orchestrator.
//!   Stage 5.55 (v0.11.51): Codegen trait-dispatch emission text batch
//!     (plan-based) — new free fn
//!     `emit_trait_dispatch_globals_text_batch(&CodegenTraitDispatchEmissionPlan) -> Vec<String>`
//!     in `src/codegen/mod.rs`. **plan-based counterpart** of Stage 5.45's
//!     `emit_vtable_globals_batch()`, extended to vtable + dynptr. Generates
//!     all LLVM IR text WITHOUT needing an Emitter trait object — useful for
//!     testing + future codegen paths that push pre-formatted text.
//!   Stage 5.56 (v0.11.52): Codegen trait-dispatch emission text batch from
//!     resolver — new free fn
//!     `emit_trait_dispatch_globals_text_batch_from_resolver(&TraitResolver, &Rodeo) -> Vec<String>`
//!     in `src/codegen/mod.rs`. **Convenience entry point** — one call from
//!     resolver to all trait-dispatch IR text (no Emitter, no separate plan
//!     step). Composes Stage 5.53 `build_trait_dispatch_emission_plan()` +
//!     Stage 5.55 `emit_trait_dispatch_globals_text_batch()`. Final piece
//!     before Stage 5.57 driver delegation.
//!   Stage 5.57 (v0.11.53): TextEmitter::emit_vtable_global delegation —
//!     **first existing-path modification**. `TextEmitter::emit_vtable_global()`
//!     method body replaced with delegation to Stage 5.44's
//!     `emit_vtable_global_text()` free function. Behavior-equivalent on
//!     non-null paths (14 cross-check tests); fixes latent null-handling
//!     bug (old inline code emitted `ptr @null`, new code emits `ptr null`).
//!   Stage 5.58 (v0.11.54): TextEmitter::emit_dyn_trait_const delegation —
//!     `TextEmitter::emit_dyn_trait_const()` method body replaced with
//!     delegation to Stage 5.48's `emit_dynptr_global_text()` free function.
//!     Behavior-equivalent (all paths byte-for-byte identical). Second
//!     existing-path modification.
//!   Stage 5.59 (v0.11.55): emit_vtables delegation — `emit_vtables()`
//!     function body replaced with one-liner delegation to
//!     `emit_vtables_from_resolver()` (Stage 5.47). Third existing-path
//!     modification. Behavior-equivalent (verified by Stage 5.47 cross-check).
//!   Next: Stage 5.60+ (emit_dyn_trait_ptrs delegation, then dyn Trait MIR lowering).
//! See `docs/develop/v0/api-naming-standard.md` for the API naming standard.

pub mod ast;
pub mod borrowck;
pub mod cargo;
pub mod codegen;
pub mod diagnostics;
pub mod driver;
pub mod hir;
pub mod lexer;
pub mod mir;
pub mod parser;
pub mod resolve;
pub mod session;
pub mod stdlib;
pub mod traits;
pub mod typeck;

// Stage 3.61: Clear public API surface — re-export the intended entry points.
// Stage 3.63: Naming standardized per docs/develop/v0/api-naming-standard.md.
// Stage 3.64: Re-export codegen Emitter trait + impls for pluggability
// (allows third-party LLVM-IR backends to implement `Emitter` and call
// `codegen_from_mir` directly).
pub use cargo::{build_project, BuildConfig, BuildResult, ProjectManifest};
pub use codegen::{
    build_dynptr_global_specs, build_trait_dispatch_emission_plan,
    build_trait_dispatch_emission_summary, build_vtable_global_specs, codegen_crate,
    emit_dyn_trait_ptr_type, emit_dyn_trait_ptrs, emit_dynptr_global_text,
    emit_dynptrs_from_resolver, emit_trait_dispatch_globals_from_plan,
    emit_trait_dispatch_globals_text_batch, emit_trait_dispatch_globals_text_batch_from_resolver,
    emit_vtable_global_from_emission, emit_vtable_global_text, emit_vtable_globals_batch,
    emit_vtables, emit_vtables_and_dynptrs_from_resolver, emit_vtables_from_resolver,
    CodegenTraitDispatchEmissionPlan, CodegenTraitDispatchEmissionSummary, EmitType, EmitValue,
    Emitter, StdlibDynptrGlobalSpec, StdlibVtableGlobalSpec, TextEmitter,
};
pub use driver::{compile, CompileErrors, CompileResult};
pub use stdlib::{
    default_prelude, find_stdlib_trait_method, integer_bit_width, is_float_type, is_primitive_type,
    is_signed_integer, is_stdlib_marker_trait, is_stdlib_trait_method, is_unsigned_integer,
    is_zero_sized_type, register_stdlib, resolve_stdlib_type, stdlib_data_global_name,
    stdlib_dynptr_global_name, stdlib_impl_method_symbol, stdlib_pointer_width_bytes,
    stdlib_trait_method_count, stdlib_trait_method_index, stdlib_trait_methods,
    stdlib_traits_with_method, stdlib_traits_with_vtable, stdlib_vtable_byte_size,
    stdlib_vtable_emission, stdlib_vtable_emission_summary, stdlib_vtable_emissions_for_traits,
    stdlib_vtable_global_name, stdlib_vtable_layout, stdlib_vtable_method_offset,
    stdlib_vtable_method_symbols, stdlib_vtable_plan, stdlib_vtable_plan_entry_count,
    stdlib_vtable_plan_is_complete, stdlib_vtable_plan_missing_methods, stdlib_vtable_slot_count,
    type_alignment_bytes, type_description, type_size_bytes, StdlibFacade, StdlibLayer,
    StdlibPointerWidth, StdlibPrelude, StdlibSelfKind, StdlibTraitMethod, StdlibTypeKind,
    StdlibVtableEmission, StdlibVtableEmissionSummary, StdlibVtablePlan, StdlibVtablePlanEntry,
    StdlibVtableSlot,
};
pub use traits::{
    extract_impl_self_ty_name, is_primitive_copy_kind, CoherenceError, ImplValidationReport,
    IncompleteImpl, TraitResolver, BUILTIN_DEF_ID_BASE, BUILTIN_PRIMITIVE_COPY_KINDS,
    BUILTIN_TRAIT_NAMES,
};
