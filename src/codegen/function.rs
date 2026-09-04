//! Stage 16.76 MUV-2: Per-function codegen orchestrator.
//!
//! Contains:
//! - `codegen_from_mir`: iterate over MirBody list and call codegen_function
//! - `codegen_synthesized_closure_functions`: emit synthesized closure call fns
//! - `codegen_function`: emit a single LLVM function from a MirBody
//! - `get_call_dest_type`: helper to override local type for Call destinations
//!
//! Extraction from `codegen/mod.rs` per §13.4 J2 (single responsibility).
//! Per `docs/lang-design/07-codegen.md` §4 (MIR → LLVM IR mapping).

use crate::codegen::emitter::{EmitType, Emitter};
use crate::codegen::error::CodegenResult;
use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts_and_mono;
use crate::codegen::statement::codegen_statement;
use crate::codegen::terminator::codegen_terminator;
use crate::mir::body::MirBody;
use lasso::Rodeo;

/// Stage 18.103 (TD-MONO-CODEGEN): Emit specialized functions for MonoItem::Fn.
///
/// For each generic function instantiation collected by `collect_mono_items`,
/// this function:
/// 1. Finds the generic MIR body (by DefId)
/// 2. Substitutes Param types with concrete substs via `substitute_mir_body`
/// 3. Computes a specialized name via `mono_item_name` (e.g., `landin_id_i32`)
/// 4. Emits the specialized function via `codegen_function`
///
/// # Design Simplification (S4)
///
/// **S4**: Only `MonoItem::Fn` is handled. `MonoItem::Type` (generic struct/enum
/// layouts) are already handled by `build_mono_layouts` + `mono_layouts` map.
/// `MonoItem::Closure` is handled by `codegen_synthesized_closure_functions`.
///
/// **Impact**: Generic closure monomorphization is not handled here (closures
/// use the synthesized call function path).
/// **Fix plan**: v0.2 Phase 2 — if closure monomorphization is needed, add
/// MonoItem::Closure handling.
///
/// Per §23: `codegen_mono_functions` follows `<verb>_<adj>_<noun>` pattern.
/// Per §16: reads MIR + fn_sigs + fn_name_by_def_id (data, no HIR).
/// Per §1.0 原則 6 "通用 > 特例": one pass for all MonoItem::Fn.
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<()>` to
/// propagate codegen errors from `codegen_function`.
///
/// Per §2 原则 9 (正确>妥协): full Result propagation.
#[allow(clippy::too_many_arguments)]
pub fn codegen_mono_functions(
    mirs: &[MirBody],
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    mono_layouts: &crate::mir::MonoLayoutMap,
    emitter: &mut dyn Emitter,
    trait_method_map: &crate::mir::monomorphize::TraitMethodResolutionMap,
) -> CodegenResult<()> {
    use crate::mir::collect_mono_items;
    use crate::mir::monomorphize::{build_mono_item_names, mono_item_name, MonoItem};
    use crate::mir::substitute_mir_body;

    // Collect all MonoItems from MIR bodies.
    let mono_items = collect_mono_items(mirs);

    // Stage 18.104 (S5 fix): type_name_by_def_id is now pre-computed in the
    // driver and passed in (was rebuilt from HIR here — violated §16 no-HIR-in-codegen).
    // Per §16: codegen reads pre-computed data, no HIR access.

    // Build MonoItem → specialized name map.
    let mono_names = build_mono_item_names(
        &mono_items,
        fn_name_by_def_id,
        type_name_by_def_id,
        interner,
    );

    // For each MonoItem::Fn, emit a specialized function.
    for item in &mono_items {
        if let MonoItem::Fn { def_id, substs } = item {
            // Skip if substs are empty (non-generic or already handled).
            if substs.is_empty() {
                continue;
            }

            // Find the generic MIR body by DefId.
            let generic_mir = mirs.iter().find(|mir| mir.def_id == Some(*def_id));
            let generic_mir = match generic_mir {
                Some(mir) => mir,
                None => continue, // MIR body not found (shouldn't happen)
            };

            // Substitute Param types with concrete substs.
            let mut specialized_mir = substitute_mir_body(generic_mir, substs);

            // Stage 68 (v0.8 — TD-IMPL-TRAIT-MONO-RESOLUTION): Re-resolve trait
            // method calls in the specialized MIR. After substitution, Param(N)
            // types are replaced with concrete types. Trait method calls that
            // were resolved to the trait declaration method (no body) are now
            // re-resolved to the concrete impl method (has body) using the
            // pre-computed TraitMethodResolutionMap.
            re_resolve_trait_method_calls(
                &mut specialized_mir,
                substs,
                trait_method_map,
                fn_name_by_def_id,
                interner,
            );

            // Get the specialized function name.
            let specialized_name = mono_names.get(item).cloned().unwrap_or_else(|| {
                // Fallback: use mono_item_name directly.
                let base = fn_name_by_def_id
                    .get(def_id)
                    .cloned()
                    .unwrap_or_else(|| format!("fn_{}", def_id.as_u32()));
                mono_item_name(item, &base, type_name_by_def_id, interner)
            });

            // Get the sig for param_count + is_void + abi.
            let sig = match fn_sigs.get(def_id) {
                Some(sig) => sig.clone(),
                None => continue, // No sig (shouldn't happen)
            };

            // Emit the specialized function.
            // Per §1.0 原則 6 "通用 > 特例": reuse codegen_function (same path
            // as non-generic functions, just with substituted MIR + mangled name).
            // Stage 33.1: Pass type_name_by_def_id so call sites can mangle
            // user-defined types correctly (was: not passed, causing Adt_N
            // fallback + linker errors).
            crate::codegen::function::codegen_function(
                emitter,
                &specialized_name,
                &specialized_mir,
                fn_name_by_def_id,
                fn_sigs,
                sig.inputs.len(),
                interner,
                &specialized_mir.adt_layouts,
                Some(mono_layouts),
                matches!(sig.output.kind, crate::mir::ty::TyKind::Tuple(ref tys) if tys.is_empty()),
                crate::ast::Abi::Landin,
                type_name_by_def_id,
            )?;
        }
    }
    Ok(())
}

/// (no HIR, no re-lowering, no re-typeck).
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<()>`.
/// Stage 33.1: Added type_name_by_def_id param for call-site mangle.
/// Stage 100 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 1 fix): Added
/// `user_item_count` param to skip prelude generic function bodies. Prelude
/// generic functions (e.g., `Option::map<T,U>`, `Box::new<T>`, `Vec::push<T>`)
/// have MIR containing `TyKind::Param` types that shouldn't be codegen'd —
/// only instantiated `MonoItem::Fn` (handled by `codegen_mono_functions`)
/// should be emitted. Skipping them eliminates the 1360+ Param fallback
/// warnings that cause non-deterministic SIGSEGV/SIGABRT in cargo test.
///
/// Per §1.0 原則 6 (通解 > 特解): one skip rule for all prelude items.
/// Per §1.0 原則 9 (正确 > 妥协): generic defs don't emit, only instances.
/// Per §16: user_item_count is pre-computed (no HIR access in codegen).
#[allow(clippy::too_many_arguments)]
pub fn codegen_from_mir(
    mirs: &[MirBody],
    body_metas: &[crate::driver::BodyMeta],
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    mono_layouts: &crate::mir::MonoLayoutMap,
    emitter: &mut dyn Emitter,
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
    // Stage 92 (v0.8 — TD-GENERIC-TRAIT-METHOD-MANGLING): Pass
    // trait_method_map so non-generic functions (like main) can re-resolve
    // trait method calls to concrete impl methods.
    trait_method_map: &crate::mir::monomorphize::TraitMethodResolutionMap,
    // Stage 100 (v0.10 — TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 1):
    // user_item_count — boundary between user items (DefId 0..N-1) and
    // prelude items (DefId N..). Prelude generic function bodies are
    // skipped here (only MonoItem::Fn instances are emitted, via
    // codegen_mono_functions).
    user_item_count: usize,
    // Stage 100: collected MonoItems — used to check if a prelude generic
    // function has any MonoItem::Fn instantiation. If it does, the generic
    // def body is still emitted (codegen_operand may reference it via
    // generic def name when FnDef type substs are empty).
    collected_mono_items: &[crate::mir::monomorphize::MonoItem],
) -> CodegenResult<()> {
    for (mir, meta) in mirs.iter().zip(body_metas.iter()) {
        // Stage 100 (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 1 fix):
        // Skip prelude generic function bodies that have NO MonoItem::Fn
        // instantiation. These bodies have Param types that shouldn't be
        // codegen'd — only instantiated MonoItem::Fn (handled by
        // codegen_mono_functions) should be emitted.
        //
        // Skip condition: DefId >= user_item_count (prelude item) AND
        // MIR body contains Param type (generic function) AND
        // no MonoItem::Fn instantiation exists for this DefId.
        //
        // If a MonoItem::Fn instantiation exists, the generic def body is
        // still emitted (because codegen_operand may reference it via
        // generic def name when substs are empty in FnDef type — a separate
        // issue tracked for Stage 101 codegen_operand substs mangling fix).
        //
        // Per §1.0 原則 6 (通解 > 特解): one skip rule for all prelude items.
        // Per §1.0 原則 9 (正确 > 妥协): generic defs don't emit, only instances.
        // Per §1.0 原則 4 (报错 > 静默): don't silently fallback Param to i32.
        if let Some(def_id) = mir.def_id {
            if def_id.as_u32() as usize >= user_item_count
                && mir_body_contains_param_type(mir)
                && !mono_items_contains_fn_for_def_id(collected_mono_items, def_id)
            {
                // Prelude generic function with no instantiation — skip codegen.
                continue;
            }
        }

        // Stage 92 (v0.8 — TD-GENERIC-TRAIT-METHOD-MANGLING): Re-resolve
        // trait method calls in ALL functions (not just generic ones). Non-
        // generic functions (like main) may contain generic trait method
        // calls (e.g., `From::<i32>::from(42)`) that need re-resolution to
        // the concrete impl method.
        //
        // Before Stage 92: re_resolve_trait_method_calls was only called in
        // codegen_mono_functions (for generic functions with non-empty
        // substs). Non-generic functions' trait method calls were left as
        // trait declaration DefIds → call sites used mangled names like
        // `fn_0_i32` (trait decl DefId 0 + substs) instead of
        // `landin_Wrapper_from` (the concrete impl method).
        //
        // Per §12 (最优 > 最小): root-cause fix — re-resolve in
        // codegen_from_mir too (the path that handles ALL functions).
        // Per §1.0 原則 6 (通解 > 特解): one re-resolution path for all
        // functions (generic + non-generic).
        // Per §1.0 原則 4 (报错 > 静默): avoid linker errors from wrong
        // mangled names.
        let mut mir_resolved = mir.clone();
        re_resolve_trait_method_calls(
            &mut mir_resolved,
            &[],
            trait_method_map,
            fn_name_by_def_id,
            interner,
        );

        // Stage 18.348 (P2 soundness fix): Pre-codegen diagnostic —
        // check for unresolved type kinds (Param/Infer/Error/Projection)
        // in type-relevant positions. For non-generic functions (handled
        // here in codegen_from_mir), Param types are real errors because
        // monomorphization doesn't substitute them (they're already
        // supposed to be concrete).
        //
        // Per §1.0 原則 4 (报错 > 静默): report unresolved types instead of
        // silently mapping them to EmitType::I32.
        // Per §1.0 原則 6 (通解 > 特解): one param_check for all functions.
        // Per §20 (iterative audit): same class as Stage 18.347 (Param leak).
        let type_errors = crate::mir::param_check::check_unresolved_types(&mir_resolved);
        if !type_errors.is_empty() {
            // Emit a warning to stderr (non-fatal — codegen continues
            // with potentially wrong types, but the user sees the error).
            // Per §1.0 原則 4 (报错 > 静默): user MUST see the error.
            for err in &type_errors {
                eprintln!(
                    "warning: unresolved type in `{}`: {}",
                    meta.fn_name, err.message
                );
            }
        }

        codegen_function(
            emitter,
            &meta.fn_name,
            &mir_resolved,
            fn_name_by_def_id,
            fn_sigs,
            meta.param_count,
            interner,
            // Stage 15.8: Arc<AdtLayouts> auto-derefs to &AdtLayouts.
            // Per clippy::explicit_auto_deref, use &mir_resolved.adt_layouts (auto-deref).
            &mir_resolved.adt_layouts,
            Some(mono_layouts),
            meta.is_void,
            meta.abi,
            type_name_by_def_id,
        )?;
    }
    Ok(())
}

/// Stage 100: Helper — check if any MonoItem::Fn instantiation exists for
/// the given DefId. Used by `codegen_from_mir` to decide whether to skip
/// a prelude generic function body.
///
/// Per §1.0 原則 6 (通解 > 特解): one check for all MonoItem kinds.
/// Per §23: `mono_items_contains_fn_for_def_id` follows
/// `<noun>_<noun>_<verb>_<prep>_<noun>` pattern.
fn mono_items_contains_fn_for_def_id(
    mono_items: &[crate::mir::monomorphize::MonoItem],
    def_id: crate::hir::DefId,
) -> bool {
    mono_items.iter().any(|item| match item {
        crate::mir::monomorphize::MonoItem::Fn {
            def_id: item_def_id,
            ..
        } => *item_def_id == def_id,
        _ => false,
    })
}

/// Stage 100 (v0.10 — TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH Layer 1):
/// Check if a MirBody contains any `TyKind::Param` type in type-relevant
/// positions (local_decls, statements, terminators). Used by
/// `codegen_from_mir` to detect prelude generic function bodies that
/// should be skipped (only their MonoItem::Fn instances should be emitted).
///
/// Per §1.0 原則 6 (通解 > 特解): one walker for all MIR positions.
/// Per §16: reads MIR only (no HIR access).
/// Per §23: `mir_body_contains_param_type` follows
/// `<noun>_<noun>_<verb>_<noun>` pattern.
fn mir_body_contains_param_type(mir: &MirBody) -> bool {
    // 1. Check local declarations.
    for local_decl in &mir.local_decls {
        if type_contains_param(&local_decl.ty.kind) {
            return true;
        }
    }

    // 2. Check basic blocks (statements + terminators).
    for block in &mir.basic_blocks {
        for stmt in &block.statements {
            if statement_contains_param(&stmt.kind) {
                return true;
            }
        }
        if terminator_contains_param(&block.terminator.kind) {
            return true;
        }
    }

    false
}

/// Stage 100: Helper — check if a TyKind contains Param (recursively).
fn type_contains_param(kind: &crate::mir::ty::TyKind) -> bool {
    use crate::mir::ty::TyKind;
    match kind {
        TyKind::Param(_) => true,
        TyKind::Adt(_, substs) | TyKind::FnDef(_, substs) | TyKind::Closure(_, substs) => {
            substs.iter().any(|t| type_contains_param(&t.kind))
        }
        TyKind::Ref(_, _, inner) | TyKind::RawPtr(_, inner) | TyKind::Slice(inner) => {
            type_contains_param(&inner.kind)
        }
        TyKind::Array(inner, _) => type_contains_param(&inner.kind),
        TyKind::Tuple(tys) => tys.iter().any(|t| type_contains_param(&t.kind)),
        TyKind::FnPtr(sig) => {
            sig.inputs.iter().any(|t| type_contains_param(&t.kind))
                || type_contains_param(&sig.output.kind)
        }
        TyKind::Projection(_, substs) => substs.iter().any(|t| type_contains_param(&t.kind)),
        _ => false,
    }
}

/// Stage 100: Helper — check if a StatementKind contains Param types.
fn statement_contains_param(stmt: &crate::mir::body::StatementKind) -> bool {
    use crate::mir::body::StatementKind;
    use crate::mir::place::{AggregateKind, Rvalue};
    if let StatementKind::Assign(boxed) = stmt {
        let (_, rvalue) = &**boxed;
        match rvalue {
            Rvalue::Use(op) | Rvalue::UnaryOp(_, op) => operand_contains_param(op),
            Rvalue::BinaryOp(_, a, b) | Rvalue::BinaryOp2(_, a, b) => {
                operand_contains_param(a) || operand_contains_param(b)
            }
            Rvalue::Cast(_, op, ty) => operand_contains_param(op) || type_contains_param(&ty.kind),
            Rvalue::Aggregate(kind, operands) => {
                if let AggregateKind::Adt(_, _, substs, field_tys) = kind {
                    if substs.iter().any(|t| type_contains_param(&t.kind)) {
                        return true;
                    }
                    if field_tys.iter().any(|t| type_contains_param(&t.kind)) {
                        return true;
                    }
                }
                operands.iter().any(operand_contains_param)
            }
            _ => false,
        }
    } else {
        false
    }
}

/// Stage 100: Helper — check if an Operand contains Param types.
fn operand_contains_param(op: &crate::mir::place::Operand) -> bool {
    match op {
        crate::mir::place::Operand::Constant(const_val) => type_contains_param(&const_val.ty.kind),
        _ => false,
    }
}

/// Stage 100: Helper — check if a TerminatorKind contains Param types.
fn terminator_contains_param(term: &crate::mir::body::TerminatorKind) -> bool {
    use crate::mir::body::TerminatorKind;
    match term {
        TerminatorKind::Call { func, args, .. } => {
            operand_contains_param(func) || args.iter().any(operand_contains_param)
        }
        TerminatorKind::SwitchInt { discr, .. } => operand_contains_param(discr),
        TerminatorKind::Assert { cond, .. } => operand_contains_param(cond),
        _ => false,
    }
}

/// Stage 16.16 (Task 10 Steps 3+4): Emit LLVM functions for synthesized
/// closure `call` functions.
///
/// Each MirBody in `synthesized_closure_mir_bodies` represents a closure's
/// synthesized `call` function. The function name is resolved from
/// `fn_name_by_def_id` by matching the MirBody's DefId (stored in the
/// closure struct's type).
///
/// Since the synthesized MIR bodies don't have BodyMeta entries, we
/// synthesize the metadata here: param_count = captures + params + 1 (self),
/// is_void = false (closures return a value), abi = Landin.
///
/// Per §16: codegen reads MirBody + fn_name_by_def_id (data only, no HIR).
/// Per §23: `codegen_synthesized_closure_functions` follows
/// `<verb>_<adj>_<noun>_<noun>` pattern.
///
/// Stage 16.35: Removed incorrect `#[cfg(feature = "llvm-backend")]` gate.
/// This function is fully backend-agnostic (operates on `&mut dyn Emitter`),
/// so it must be available for the text-only build too. The gate was a bug
/// that broke `cargo check` without `--features llvm-backend`.
/// Stage 18.151 (TD-CODEGEN-RESULT): Returns `CodegenResult<()>`.
pub(crate) fn codegen_synthesized_closure_functions(
    synthesized_mirs: &[MirBody],
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    mono_layouts: &crate::mir::MonoLayoutMap,
    emitter: &mut dyn Emitter,
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
) -> CodegenResult<()> {
    for mir in synthesized_mirs {
        // Stage 16.17: Use the DefId stored on MirBody (set during
        // build_synthesized_closure_mir_body) to resolve the function name.
        // This replaces the fragile string-pattern search from Stage 16.16.
        let def_id = match mir.def_id {
            Some(id) => id,
            None => continue, // Skip MIR bodies without DefId (shouldn't happen)
        };

        let fn_name = match fn_name_by_def_id.get(&def_id) {
            Some(name) => name.clone(),
            None => continue, // Skip if name not registered (shouldn't happen)
        };

        // Stage 16.29 (通解 — fix hardcoded param_count):
        // The previous code hardcoded `param_count = 2` (self + 1 param)
        // — a 特解 (special-case) that breaks for closures with 0 params
        // (e.g., `|| 42`) or 2+ params (e.g., `|x, y| x + y`).
        //
        // The 通解 (general solution) is to read the actual param_count
        // from `fn_sigs[def_id].inputs.len()`. The driver now populates
        // `fn_sig_table` with the resolved sig (after closure typeck),
        // so `inputs.len()` = 1 (self) + N (closure params) = correct
        // param_count for codegen.
        //
        // Per §1.0 原則 6 "通用 > 特例": one source of truth (fn_sigs)
        // for the param_count, not a hardcoded constant.
        // Per §16: codegen reads MirBody + fn_sigs (data only, no HIR).
        let param_count = fn_sigs
            .get(&def_id)
            .map(|sig| sig.inputs.len())
            .unwrap_or(1); // Defensive: self only (shouldn't happen)

        let meta = crate::driver::BodyMeta {
            fn_name: fn_name.clone(),
            is_void: false, // Closures return a value
            param_count,
            abi: crate::ast::Abi::Landin,
        };
        codegen_function(
            emitter,
            &meta.fn_name,
            mir,
            fn_name_by_def_id,
            fn_sigs,
            meta.param_count,
            interner,
            &mir.adt_layouts,
            Some(mono_layouts),
            meta.is_void,
            meta.abi,
            type_name_by_def_id,
        )?;
    }
    Ok(())
}

/// Stage 18.151 (TD-CODEGEN-RESULT): `codegen_function` now returns
/// `CodegenResult<()>` to propagate errors from `codegen_statement` and
/// `codegen_terminator`.
///
/// Per §2 原则 9 (正确>妥协): full Result propagation, no `unwrap()` stubs.
#[allow(clippy::too_many_arguments)]
pub(crate) fn codegen_function(
    emitter: &mut dyn Emitter,
    name: &str,
    mir: &MirBody,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    param_count: usize,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    is_void: bool,
    abi: crate::ast::Abi,
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
) -> CodegenResult<()> {
    // The entry point `fn main()` is codegen'd as `landin_main` and is called
    // by the C wrapper which declares `extern int landin_main(void)`.
    // Per Rust convention: `fn main()` without explicit return type returns `()`.
    // The C wrapper reads the return value, so for `()` return we emit `ret i32 0`.
    // For `fn main() -> i32 { N }` we emit `ret i32 N`.
    //
    // The `is_entry` flag is set by the driver for the `fn main()` function.
    // This replaces the old `name == "landin_main"` string comparison (Stage 13.26).
    let is_entry = name == "landin_main";

    let ret_ty = if is_void {
        if is_entry {
            // Entry point with `()` return → emit i32 (C wrapper reads it as 0)
            EmitType::I32
        } else {
            EmitType::Void
        }
    } else if mir.local_decls.is_empty() {
        if is_entry {
            EmitType::I32
        } else {
            EmitType::Void
        }
    } else {
        match &mir.local_decls[0].ty.kind {
            crate::mir::ty::TyKind::Tuple(tys) if tys.is_empty() => {
                if is_entry {
                    EmitType::I32
                } else {
                    EmitType::Void
                }
            }
            _ => mir_type_to_emit_type_with_layouts_and_mono(
                &mir.local_decls[0].ty,
                layouts,
                mono_layouts,
            ),
        }
    };

    let params: Vec<(EmitType, String, u32)> = (0..param_count)
        .filter_map(|i| {
            let local_idx = i + 1;
            let ty = mir
                .local_decls
                .get(local_idx)
                .map(|ld| {
                    // Stage 16.21: For synthesized closure functions,
                    // the `self` parameter (local_idx=1) has a Closure
                    // type. Codegen should pass it as OpaquePtr (pointer)
                    // instead of by-value struct, because:
                    // 1. The call site passes the closure struct by address
                    // 2. The function body accesses captures via GEP from
                    //    the pointer
                    // This matches how regular struct params are handled
                    // (Adt types are passed as OpaquePtr in codegen).
                    if local_idx == 1 && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
                    {
                        EmitType::OpaquePtr
                    } else {
                        mir_type_to_emit_type_with_layouts_and_mono(&ld.ty, layouts, mono_layouts)
                    }
                })
                .unwrap_or(EmitType::I32);
            // Stage 18.335 (P1 soundness fix): Skip ZST params (EmitType::Void).
            // LLVM IR requires first-class types for function parameters; `void`
            // is only allowed as a function *return* type. Without this filter,
            // `fn foo(u: ())` produces `define void @foo(void %arg0)` which
            // `llvm-as` rejects with "void type only allowed for function results".
            //
            // Mirrors rustc_codegen_llvm: ZST params are elided from the LLVM
            // signature entirely (Rust ABI doesn't pass them in registers/memory).
            //
            // Per §1.0 原則 6 (通解 > 特解): ZST elision is the GENERIC pattern
            // for all ZST params, not a special-case per param type.
            // Per §1.0 原則 9 (正确 > 妥协): correct ABI > pragmatic placeholder.
            // Per §20 (iterative audit): found via §20 Round 4 audit after
            // Stages 18.332/18.333/18.334 fixed sret/byval/TextEmitter IR.
            if ty == EmitType::Void {
                return None;
            }
            // Keep both the LLVM arg index `i` (for arg naming) and the MIR
            // local_idx `i + 1` (for alloca lookup). After filtering Void params,
            // these two values can diverge — we need both.
            Some((ty, format!("%arg{}", i), local_idx as u32))
        })
        .collect();

    let param_refs: Vec<(EmitType, &str)> = params
        .iter()
        .map(|(t, n, _)| (t.clone(), n.as_str()))
        .collect();

    // Stage 8.3: Add ABI attributes after the function definition.
    // For C ABI: no special attribute needed (C is the default in LLVM).
    // For Landin ABI: add `cc 64` (custom calling convention placeholder).
    // In MVP, both ABIs use the same LLVM calling convention (C), so no
    // attribute is emitted. Future: Landin ABI could use a custom CC.
    let _ = abi; // ABI is tracked but not yet differentiated in codegen
    emitter.emit_function_begin(name, &param_refs, &ret_ty);

    for (i, ld) in mir.local_decls.iter().enumerate() {
        // Stage 16.21: For the `self` parameter (local_idx=1) in synthesized
        // closure functions, use OpaquePtr for the alloca type. This matches
        // how the parameter is passed (as ptr). Other Closure-typed locals
        // (e.g., the closure struct in the caller) keep their original type.
        let is_self_param = i == 1
            && matches!(ld.ty.kind, crate::mir::ty::TyKind::Closure(_, _))
            && mir.def_id.is_some();
        let ty = if is_self_param {
            EmitType::OpaquePtr
        } else {
            mir_type_to_emit_type_with_layouts_and_mono(&ld.ty, layouts, mono_layouts)
        };
        // Stage 14.36: If this local is the destination of a Call terminator,
        // override its type with the callee's return type from fn_sigs. This
        // fixes struct-returning method calls where the local's type is
        // Infer→i32 after typeck writeback but the actual value is a struct.
        let ty = if let Some(override_ty) = call_dest_type(mir, i, fn_sigs, layouts, mono_layouts) {
            override_ty
        } else {
            ty
        };
        // Stage 18.335 (P1 soundness fix): Move the Void check to AFTER the
        // call_dest_type override. Was: checked BEFORE the override, so a
        // local whose declared type is non-void but whose callee returns
        // `()` would still produce `emit_alloca(&Void, ...)` → invalid IR.
        //
        // Per §2.2 (根因思维): the root cause was the check ordering — the
        // override can introduce Void that the original check missed.
        // Per §20 (iterative audit): found via §20 Round 4 audit.
        if ty == EmitType::Void {
            continue;
        }
        let ptr_name = format!("%loc_{}", i);
        let ptr = emitter.emit_alloca(&ty, &ptr_name);
        emitter.set_local_ptr(i as u32, ptr);
    }

    for (ty, arg_name, local_idx) in params.iter() {
        let local_idx = *local_idx;
        if let Some(ptr) = emitter.local_ptr(local_idx).cloned() {
            // Stage 18.333 (P1 soundness fix): For byval params, the LLVM
            // param value is a `ptr` (the caller's stack slot), NOT the
            // struct itself. We must LOAD the struct from the pointer first,
            // then store it to the local alloca.
            //
            // Without this load, the IR would be:
            //   store { i64, i64, i64 } %arg0, ptr %loc_1   ; %arg0 is ptr!
            // This is invalid IR (LLVM would reject "stored value and pointer
            // base type do not match"). The previous code "worked" only
            // because LLVM silently truncated or coerced — producing garbage.
            //
            // Per §1.0 原則 4 (报错 > 静默): emit an explicit load to make
            // the value type match the alloca type.
            // Per §20 (iterative audit): same root cause as sret; same fix
            // pattern (load value from ABI pointer before storing to local).
            if ty.needs_byval() {
                let loaded = emitter.emit_load(ty, &arg_name.to_string());
                emitter.emit_store(ty, &loaded, &ptr);
            } else {
                emitter.emit_store(ty, arg_name, &ptr);
            }
        }
    }

    // Stage 18.329: Emit `br label %bb0` to terminate the entry block.
    //
    // **Design boundary** (per LLVM Language Reference):
    // - The entry block (containing alloca) MUST end with a terminator.
    // - Without a terminator, LLVM produces invalid IR → segfault.
    // - Both TextEmitter and LLVMSysEmitter need this `br`.
    //
    // Stage 18.327 added this `br`. Stage 18.329 temporarily removed it,
    // but that caused both_news to regress (101/500 segfault). Restoring.
    //
    // The LLVMSysEmitter's `emit_block("bb0")` reuse mechanism works correctly
    // with the `br`: after `emit_br("bb0")`, the entry block is terminated.
    // `emit_block("bb0")` then creates a NEW block (not reusing entry, because
    // entry already has a terminator). This is the correct behavior.
    //
    // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
    if !mir.basic_blocks.is_empty() {
        emitter.emit_br("bb0");
    }

    for (bb_idx, bb) in mir.basic_blocks.iter().enumerate() {
        let label = format!("bb{}", bb_idx);
        emitter.emit_block(&label);
        for stmt in &bb.statements {
            codegen_statement(
                emitter,
                mir,
                stmt,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            )?;
        }
        codegen_terminator(
            emitter,
            mir,
            &bb.terminator,
            &ret_ty,
            fn_name_by_def_id,
            fn_sigs,
            interner,
            layouts,
            mono_layouts,
            type_name_by_def_id,
        )?;
    }

    // Stage 13.12 + Stage 13.13: println! output is now emitted INLINE
    // via StatementKind::Println in codegen_statement (see Println arm
    // below). The Stage 13.12 side-table approach (a Vec<String> field
    // on MirBody + a separate helper function emitted after
    // emit_function_end + a weak-symbol trick in the C wrapper) was
    // REMOVED in Stage 13.13 because it broke output ordering for loops
    // and conditionals — the helper ran BEFORE landin_main(), so all
    // prints appeared before the program body.
    //
    // Stage 15.6 (cleanup): The MirBody.println_messages field itself
    // was removed in Stage 14.x (no longer declared on MirBody). This
    // comment retained as historical context for the inline-emission
    // design decision.
    emitter.emit_function_end();
    Ok(())
}

/// Stage 14.36: Check if a local is the destination of a Call terminator,
/// and if so, return the callee's return type from fn_sigs. This overrides
/// the local's declared type (which may be Infer→i32 after typeck writeback)
/// with the actual return type (e.g. struct { i32, i32 }), ensuring the
/// alloca has the correct size for struct-returning method calls.
pub(crate) fn call_dest_type(
    mir: &MirBody,
    local_idx: usize,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
) -> Option<EmitType> {
    for bb in &mir.basic_blocks {
        if let crate::mir::body::TerminatorKind::Call {
            func, destination, ..
        } = &bb.terminator.kind
        {
            if let crate::mir::place::PlaceKind::Local(id) = &destination.kind {
                if id.0 as usize == local_idx {
                    // This local is a Call destination — get callee's DefId
                    let callee_def_id = if let crate::mir::place::Operand::Constant(c) = func {
                        // Stage 18.375 (TD-AS-CAST-TRUNCATION): use try_from + expect
                        // instead of `as u32`. Per §1.0 原則 1 (内存安全决不能妥协):
                        // silent truncation could mask corrupted ConstVal. Per §2 原则 3:
                        // expect documents the FnDef invariant.
                        match &c.val {
                            crate::mir::ty::ConstVal::Uint(n) => Some(crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Uint must fit u32"),
                            )),
                            crate::mir::ty::ConstVal::Int(n) => Some(crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Int must fit u32"),
                            )),
                            _ => None,
                        }
                    } else if let crate::mir::place::Operand::Copy(lv)
                    | crate::mir::place::Operand::Move(lv) = func
                    {
                        if let crate::mir::place::PlaceKind::Local(id) = &lv.kind {
                            mir.local_decls.get(id.0 as usize).and_then(|ld| {
                                if let crate::mir::ty::TyKind::FnDef(did, _) = &ld.ty.kind {
                                    Some(*did)
                                } else {
                                    None
                                }
                            })
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    if let Some(did) = callee_def_id {
                        if let Some(sig) = fn_sigs.get(&did) {
                            return Some(mir_type_to_emit_type_with_layouts_and_mono(
                                &sig.output,
                                layouts,
                                mono_layouts,
                            ));
                        }
                    }
                }
            }
        }
    }
    None
}

// Stage 68 (v0.8 — TD-IMPL-TRAIT-MONO-RESOLUTION): Trait method re-resolution
// during monomorphization. After substitute_mir_body replaces Param(N) types
// with concrete types, trait method calls still point to the trait declaration
// method (no body). This function re-resolves them to the concrete impl method.
//
// Per §12 (最优 > 最小): root-cause fix — re-resolve after substitution.
// Per §16 (codegen is HIR-free): uses pre-computed map, no HIR access.
fn re_resolve_trait_method_calls(
    mir: &mut crate::mir::body::MirBody,
    substs: &[crate::mir::ty::Ty],
    trait_method_map: &crate::mir::monomorphize::TraitMethodResolutionMap,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    interner: &Rodeo,
) {
    use crate::mir::body::TerminatorKind;
    use crate::mir::place::Operand;
    use crate::mir::ty::TyKind;

    let _ = (substs, interner); // Reserved for future use.

    // Stage 92 (v0.8 — TD-GENERIC-TRAIT-METHOD-MANGLING): Removed the
    // `if substs.is_empty() { return; }` guard. Non-generic functions
    // (called from codegen_from_mir with empty substs) may still contain
    // generic trait method calls (e.g., `From::<i32>::from(42)`) that
    // need re-resolution. The guard was too aggressive — it skipped
    // re-resolution for ALL non-generic functions, including main.
    //
    // Per §12 (最优 > 最小): root-cause fix — always try re-resolution,
    // regardless of the function's own substs.
    // Per §1.0 原則 6 (通解 > 特解): one re-resolution path for all
    // functions (generic + non-generic).

    // Clone local_decls to avoid borrow conflict with the mutable iteration
    // over basic_blocks. This is O(n) but only runs on generic functions.
    let local_decls = mir.local_decls.clone();

    for bb in &mut mir.basic_blocks {
        let TerminatorKind::Call { func, args, .. } = &mut bb.terminator.kind else {
            continue;
        };
        let Operand::Constant(c) = func else {
            continue;
        };
        let TyKind::FnDef(trait_method_def_id, _) = &c.ty.kind else {
            continue;
        };

        // Get the receiver type (first arg or first input local).
        let receiver_ty = get_receiver_type(&local_decls, args);
        let type_name = receiver_ty
            .as_ref()
            .map(|ty| get_concrete_type_name(ty, interner))
            .unwrap_or_default();

        // Look up in the trait method map.
        // Stage 92: If type_name is empty (static trait method — no receiver),
        // fall back to lookup_by_trait_method. If that also fails (call site's
        // DefId differs from trait decl's DefId due to turbofish), try
        // matching by method name via fn_name_by_def_id.
        let impl_method_def_id = if type_name.is_empty() {
            // Static trait method (no receiver) — try DefId-only lookup,
            // then fall back to name-based matching.
            let result = trait_method_map.lookup_by_trait_method(*trait_method_def_id);
            if result.is_some() {
                result
            } else {
                let call_name = fn_name_by_def_id.get(trait_method_def_id);
                if let Some(call_name) = call_name {
                    trait_method_map.lookup_by_method_name(call_name, fn_name_by_def_id)
                } else {
                    None
                }
            }
        } else {
            // Instance method — try (DefId, type_name) lookup first.
            let result = trait_method_map.lookup(*trait_method_def_id, &type_name);
            if result.is_some() {
                result
            } else {
                // Stage 92: If (DefId, type_name) lookup fails, the type_name
                // might be the arg type (e.g., i32) instead of Self type
                // (e.g., Wrapper) — this happens for static trait methods
                // where get_receiver_type returns the first arg's type.
                // Fall back to DefId-only lookup (ignores type_name).
                let fallback = trait_method_map.lookup_by_trait_method(*trait_method_def_id);
                if fallback.is_some() {
                    fallback
                } else {
                    // Last resort: name-based matching.
                    let call_name = fn_name_by_def_id.get(trait_method_def_id);
                    if let Some(call_name) = call_name {
                        trait_method_map.lookup_by_method_name(call_name, fn_name_by_def_id)
                    } else {
                        None
                    }
                }
            }
        };
        let Some(impl_method_def_id) = impl_method_def_id else {
            continue;
        };

        // Found the concrete impl method! Replace the func operand.
        c.ty = crate::mir::ty::Ty::new(
            TyKind::FnDef(impl_method_def_id, Vec::new().into()),
            crate::session::Span::DUMMY,
        );
        c.val = crate::mir::place::ConstVal::Uint(impl_method_def_id.as_u32() as u128);
    }
}

/// Get the receiver type from the Call terminator's args or the first input local.
fn get_receiver_type(
    local_decls: &[crate::mir::body::LocalDecl],
    args: &[crate::mir::place::Operand],
) -> Option<crate::mir::ty::Ty> {
    use crate::mir::place::{Operand, PlaceKind};

    if !args.is_empty() {
        match &args[0] {
            Operand::Copy(place) | Operand::Move(place) => {
                let PlaceKind::Local(id) = &place.kind else {
                    return None;
                };
                let idx = id.0 as usize;
                if idx < local_decls.len() {
                    return Some(local_decls[idx].ty.clone());
                }
            }
            Operand::Constant(c) => return Some(c.ty.clone()),
        }
    }

    // No args — the receiver might be the first input local.
    if local_decls.len() > 1 {
        return Some(local_decls[1].ty.clone());
    }
    None
}

/// Get the source-language name of a MIR type as a string.
fn get_concrete_type_name(ty: &crate::mir::ty::Ty, _interner: &Rodeo) -> String {
    use crate::mir::ty::TyKind;
    match &ty.kind {
        TyKind::Int(int_ty) => {
            use crate::ast::IntTy;
            match int_ty {
                IntTy::I8 => "i8",
                IntTy::I16 => "i16",
                IntTy::I32 => "i32",
                IntTy::I64 => "i64",
                IntTy::I128 => "i128",
                IntTy::Isize => "isize",
            }
            .to_string()
        }
        TyKind::Uint(uint_ty) => {
            use crate::ast::UintTy;
            match uint_ty {
                UintTy::U8 => "u8",
                UintTy::U16 => "u16",
                UintTy::U32 => "u32",
                UintTy::U64 => "u64",
                UintTy::U128 => "u128",
                UintTy::Usize => "usize",
            }
            .to_string()
        }
        TyKind::Bool => "bool".to_string(),
        TyKind::Str => "str".to_string(),
        TyKind::Char => "char".to_string(),
        TyKind::Float(float_ty) => {
            use crate::ast::FloatTy;
            match float_ty {
                FloatTy::F32 => "f32",
                FloatTy::F64 => "f64",
            }
            .to_string()
        }
        TyKind::Ref(_, _, inner) => get_concrete_type_name(inner, _interner),
        _ => String::new(),
    }
}
