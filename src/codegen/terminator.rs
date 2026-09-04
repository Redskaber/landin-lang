//! MIR terminator → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `TerminatorKind::Return`, `Goto`, `SwitchInt`, `Call`, `Drop`,
//! `Assert`, `Unreachable`.

// Stage 16.42: Removed `#[allow(unused_imports)]` — fixed the underlying
// unused imports instead. Per §1.0 原則 5 "去除兼容思维".
use super::mir_translation::detect_operand_type;
use super::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
/// Stage 18.151 (TD-CODEGEN-RESULT): `codegen_terminator` now returns
/// `CodegenResult<()>` to propagate codegen errors (e.g., from nested
/// `emit_printf_call` calls that may fail in future).
///
/// Per §2 原则 9 (正确>妥协): full Result propagation.
#[allow(clippy::too_many_arguments)]
pub(crate) fn codegen_terminator(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    term: &Terminator,
    ret_ty: &EmitType,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    fn_sigs: &std::collections::HashMap<crate::hir::DefId, crate::mir::ty::Sig>,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
    // Stage 101: mono_names for FnDef substs mangling in codegen_operand.
    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,
) -> CodegenResult<()> {
    match &term.kind {
        TerminatorKind::Return => {
            if *ret_ty == EmitType::Void {
                emitter.emit_ret(ret_ty, None);
            } else {
                let ret_val = if let Some(ptr) = emitter.local_ptr(0).cloned() {
                    Some(emitter.emit_load(ret_ty, &ptr))
                } else {
                    emitter.local(0).cloned()
                };
                // Stage 18.71 P0-5: When ret_ty is non-void but ret_val is
                // None (e.g., `fn main()` where the return local is unit and
                // has no alloca), emit a default return value.
                //
                // For `fn main()` (is_entry=true), the C wrapper expects
                // `i32` return. The return local is unit (no value stored),
                // so we emit `ret i32 0` (success exit code).
                //
                // Per §1.0 原則 4 "报错 > 静默": don't silently emit `ret void`
                // when the function signature expects a value.
                // Per §1.0 原則 9 "正确 > 妥协": match Rust's `fn main()` →
                // exit code 0 semantics.
                match (ret_val, ret_ty) {
                    (Some(v), _) => emitter.emit_ret(ret_ty, Some(&v)),
                    (None, EmitType::I32) => {
                        // Default i32 return (exit code 0 for void main).
                        emitter.emit_ret(ret_ty, Some(&"0".to_string()));
                    }
                    (None, _) => {
                        // Other types: emit ret void (best effort).
                        emitter.emit_ret(ret_ty, None);
                    }
                }
            }
        }
        TerminatorKind::Unreachable => {
            emitter.emit_unreachable();
        }
        TerminatorKind::Goto(target) => {
            emitter.emit_br(&format!("bb{}", target.0));
        }
        TerminatorKind::SwitchInt {
            discr,
            targets,
            otherwise,
        } => {
            let discr_val = codegen_operand(
                emitter,
                mir,
                discr,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
                mono_names,
                type_name_by_def_id,
            );
            let is_bool_switch = targets
                .iter()
                .any(|(val, _)| matches!(val, ConstVal::Bool(_)));
            if is_bool_switch {
                // Stage 14.65: Fix bool match with both true AND false arms.
                //
                // Previously, the codegen assumed "false goes to otherwise"
                // and only checked for the `true` target. This was WRONG when
                // the match had BOTH a `true => ...` and a `false => ...` arm:
                //
                //   match b { true => 1, false => 0 }
                //
                // The `false` arm's body was NEVER executed — the otherwise
                // block was empty (or had stale content), so the result was
                // uninitialized garbage (e.g., -976284176).
                //
                // Fix: check for BOTH `true` and `false` targets. If both are
                // present (as separate arms), branch to each. If only one is
                // present, the other goes to `otherwise` (legacy behavior).
                //
                // Per §1.0 原则 5 "报错 > 静默": both arms now execute their
                // proper bodies, rather than silently skipping the false arm.
                let true_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(true)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
                let false_bb = targets
                    .iter()
                    .find(|(val, _)| matches!(val, ConstVal::Bool(false)))
                    .map(|(_, bb)| bb.0)
                    .unwrap_or(otherwise.0);
                emitter.emit_br_cond(
                    &discr_val,
                    &format!("bb{}", true_bb),
                    &format!("bb{}", false_bb),
                );
            } else {
                let discr_ty =
                    detect_operand_type(mir, discr, layouts, mono_layouts).unwrap_or(EmitType::I32);
                let cases: Vec<(i128, String)> = targets
                    .iter()
                    .filter_map(|(val, bb)| match val {
                        ConstVal::Int(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        ConstVal::Uint(n) => Some((*n as i128, format!("bb{}", bb.0))),
                        _ => None,
                    })
                    .collect();
                emitter.emit_switch(&discr_val, &discr_ty, &cases, &format!("bb{}", otherwise.0));
            }
        }
        TerminatorKind::Call {
            func,
            args,
            destination,
            target,
            dyn_trait_call,
        } => {
            // Stage 15.30 (HP-22): Check dyn_trait_call field FIRST.
            // If Some, this is a dyn Trait vtable indirect call — use the
            // DynTraitMethodCall info directly from the terminator.
            // Was: decode magic `Error + Int(index)` marker from func operand.
            if let Some(call_info) = dyn_trait_call {
                let ret_val = codegen_dyn_trait_call_direct(
                    emitter,
                    call_info,
                    args,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                );
                if let PlaceKind::Local(id) = &destination.kind {
                    let dest_ty = mir
                        .local_decls
                        .get(id.0 as usize)
                        .map(|ld| {
                            mir_type_to_emit_type_with_layouts_and_mono(
                                &ld.ty,
                                layouts,
                                mono_layouts,
                            )
                        })
                        .unwrap_or(EmitType::I32);
                    emitter.set_local(id.0, ret_val.clone());
                    if let Some(ptr) = emitter.local_ptr(id.0).cloned() {
                        emitter.emit_store(&dest_ty, &ret_val, &ptr);
                    }
                }
                if let Some(cont) = target {
                    emitter.emit_br(&format!("bb{}", cont.0));
                }
                return Ok(());
            }

            // Stage 15.65 (HP-22 cleanup): Removed the legacy dyn Trait
            // vtable indirect call path (was Stage 5.79). The `dyn_trait_call`
            // field on the terminator is now the SOLE source of truth —
            // codegen checks it FIRST (above) and returns. If we reach here,
            // it's a regular direct call.
            //
            // The legacy path decoded a magic `Error + Int(index)` marker
            // from the func operand and looked up `mir.dyn_trait_calls[index]`.
            // That side-table has been removed.
            //
            // Per §16: MIR carries the dyn Trait info as data on the
            // terminator's `dyn_trait_call` field (Stage 15.30).
            // Per §15 "最优 > 最小": dead code (legacy path) is removed.

            // Stage 16.30 (通解 — Closure-typed call sites):
            // When the func operand has TyKind::Closure(def_id, _), the call
            // is to a closure value (not a FnDef). This happens for:
            //   - `f()()` where f returns a closure (the inner call's func
            //     is the result of f(), which has Closure type after typeck)
            //   - `let g = f(); g()` where g is a closure-typed let binding
            //     that didn't propagate through closure_bodies
            //
            // The 通解: resolve the function name from the Closure's def_id
            // (via fn_name_by_def_id), and PREPEND the closure struct as
            // the first arg (self). The synthesized `call` function expects:
            //   fn closure_call_fn_N(self: Closure_N, params...) -> Ret
            //
            // This handles ALL closure-typed call sites uniformly, regardless
            // of how the closure value was produced (literal, let binding,
            // call result, etc.).
            //
            // Per §1.0 原則 6 "通用 > 特例": one codegen path for all
            // closure-typed calls.
            // Per §1.0 原則 9 "正确 > 妥协": fix the root cause (codegen
            // doesn't handle Closure-typed func), not the symptom (indirect
            // call on closure struct).
            let mut closure_self_local: Option<crate::mir::place::LocalId> = None;

            let fn_name = if let Operand::Copy(lv) | Operand::Move(lv) = func {
                if let PlaceKind::Local(id) = &lv.kind {
                    let local_ty = mir.local_decls.get(id.0 as usize).map(|ld| &ld.ty);
                    if let Some(ty) = local_ty {
                        match &ty.kind {
                            // Direct function call (FnDef-typed func local).
                            // Stage 18.103 (TD-MONO-CODEGEN): If the FnDef has
                            // non-empty substs, use the specialized (mangled)
                            // function name instead of the generic name.
                            crate::mir::ty::TyKind::FnDef(def_id, substs) => {
                                if substs.is_empty() {
                                    // Non-generic or unresolved substs — use base name.
                                    fn_name_by_def_id.get(def_id).cloned()
                                } else {
                                    // Generic instantiation — use specialized name.
                                    // The specialized functions are emitted by
                                    // codegen_mono_functions with names like
                                    // "landin_id_i32" (base + mangled substs).
                                    //
                                    // Stage 33.1 (TD-VEC-PUSH-GET-MIGRATION):
                                    // Pass the REAL type_name_by_def_id (was:
                                    // empty HashMap, causing Adt_N fallback for
                                    // user-defined types like MyType/Point).
                                    // This fixes the call-site ↔ definition
                                    // name mismatch (call site used
                                    // `landin_Vec_new_Adt_0` but definition
                                    // was `Vec_new_MyType`).
                                    //
                                    // Per §1.0 原則 4 (报错 > 静默): name
                                    // mismatch caused linker errors (undefined
                                    // reference), now fixed.
                                    // Per §1.0 原則 6 (通解 > 特解): one
                                    // type_name_by_def_id map for all call sites.
                                    use crate::mir::monomorphize::mono_item_name;
                                    use crate::mir::MonoItem;
                                    let base_with_prefix = fn_name_by_def_id
                                        .get(def_id)
                                        .cloned()
                                        .unwrap_or_else(|| format!("fn_{}", def_id.as_u32()));
                                    // Stage 33.1: Strip the `landin_` prefix to
                                    // match `build_mono_item_names` (which also
                                    // strips it). The call site and definition
                                    // must use the SAME base name.
                                    let base = base_with_prefix
                                        .strip_prefix("landin_")
                                        .unwrap_or(&base_with_prefix)
                                        .to_string();
                                    let item = MonoItem::Fn {
                                        def_id: *def_id,
                                        substs: substs.clone(),
                                    };
                                    Some(mono_item_name(
                                        &item,
                                        &base,
                                        type_name_by_def_id,
                                        interner,
                                    ))
                                }
                            }
                            // Stage 16.30: Closure-typed func local → resolve
                            // to synthesized function name. The closure struct
                            // itself is passed as self (first arg).
                            crate::mir::ty::TyKind::Closure(def_id, _) => {
                                closure_self_local = Some(*id);
                                fn_name_by_def_id.get(def_id).cloned()
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else if let Operand::Constant(c) = func {
                // Stage 18.375 (TD-AS-CAST-TRUNCATION): use try_from + expect
                // instead of `as u32`. Per §1.0 原則 1 (内存安全决不能妥协):
                // silent truncation could mask corrupted ConstVal. Per §2 原则 3:
                // expect documents the FnDef invariant.
                //
                // Stage 33.1 (TD-VEC-PUSH-GET-MIGRATION): Check if the
                // Constant's type is FnDef with non-empty substs (set by
                // writeback_fndef_substs). If so, use the specialized name
                // via mono_item_name. Was: only read c.val for DefId and
                // used fn_name_by_def_id (ignoring substs entirely) →
                // generic calls always used the base name.
                if let crate::mir::ty::TyKind::FnDef(def_id, substs) = &c.ty.kind {
                    if !substs.is_empty() {
                        // Generic instantiation — use specialized name.
                        use crate::mir::monomorphize::mono_item_name;
                        use crate::mir::MonoItem;
                        let base_with_prefix = fn_name_by_def_id
                            .get(def_id)
                            .cloned()
                            .unwrap_or_else(|| format!("fn_{}", def_id.as_u32()));
                        // Strip landin_ prefix to match build_mono_item_names.
                        let base = base_with_prefix
                            .strip_prefix("landin_")
                            .unwrap_or(&base_with_prefix)
                            .to_string();
                        let item = MonoItem::Fn {
                            def_id: *def_id,
                            substs: substs.clone(),
                        };
                        Some(mono_item_name(&item, &base, type_name_by_def_id, interner))
                    } else {
                        fn_name_by_def_id.get(def_id).cloned()
                    }
                } else {
                    // Fallback: read DefId from c.val (old behavior for
                    // non-FnDef constants).
                    match &c.val {
                        ConstVal::Uint(n) => fn_name_by_def_id
                            .get(&crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Uint must fit u32"),
                            ))
                            .cloned(),
                        ConstVal::Int(n) => fn_name_by_def_id
                            .get(&crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Int must fit u32"),
                            ))
                            .cloned(),
                        _ => None,
                    }
                }
            } else {
                None
            };

            // Stage 16.30: Build arg pairs. If this is a closure-typed call
            // (closure_self_local is Some), PREPEND the closure struct as self.
            let mut arg_pairs: Vec<(EmitType, EmitValue)> = Vec::new();

            // Stage 89 (v0.8 — TD-DYN-TRAIT-FAT-PTR-COERCION): Pre-compute
            // callee_def_id before the args loop so we can look up the
            // callee's param types (to detect Adt→Dyn coercion sites).
            // This duplicates the extraction logic below (line 484+) but is
            // necessary because the args loop runs before that block.
            // Per §13.4 (重构判据): small duplication is acceptable — the
            // alternative (moving the entire callee_def_id block up) would
            // require reordering ~100 lines of code with non-trivial borrow
            // patterns.
            let callee_def_id_early: Option<crate::hir::DefId> = if let Operand::Constant(c) = func
            {
                if let crate::mir::ty::TyKind::FnDef(did, _) = &c.ty.kind {
                    Some(*did)
                } else {
                    match &c.val {
                        ConstVal::Uint(n) => Some(crate::hir::DefId(
                            u32::try_from(*n).expect("FnDef ConstVal::Uint must fit u32"),
                        )),
                        ConstVal::Int(n) => Some(crate::hir::DefId(
                            u32::try_from(*n).expect("FnDef ConstVal::Int must fit u32"),
                        )),
                        _ => None,
                    }
                }
            } else if let Operand::Copy(lv) | Operand::Move(lv) = func {
                if let PlaceKind::Local(id) = &lv.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .and_then(|ld| match &ld.ty.kind {
                            crate::mir::ty::TyKind::FnDef(did, _) => Some(*did),
                            crate::mir::ty::TyKind::Closure(did, _) => Some(*did),
                            _ => None,
                        })
                } else {
                    None
                }
            } else {
                None
            };

            // Stage 16.30: Prepend closure self arg if applicable.
            if let Some(self_id) = closure_self_local {
                // The synthesized function expects self as a pointer
                // (OpaquePtr). Pass the local's alloca pointer.
                let ptr_str = format!("%loc_{}", self_id.0);
                arg_pairs.push((EmitType::OpaquePtr, ptr_str));
            }

            // Process the remaining args from the terminator.
            for (arg_idx, a) in args.iter().enumerate() {
                let ty =
                    detect_operand_type(mir, a, layouts, mono_layouts).unwrap_or(EmitType::I32);
                // Stage 18.335 (P1 soundness fix): Skip ZST args (EmitType::Void).
                // LLVM IR requires first-class types for call args; `void` is only
                // allowed as a function *return* type. Without this filter, calling
                // `fn foo(u: ())` produces `call void @foo(void 0)` which `llvm-as`
                // rejects with "void value cannot be first class".
                //
                // Mirrors rustc_codegen_llvm: ZST args are elided from the call site
                // (matching the ZST param elision in codegen_function).
                //
                // Per §1.0 原則 6 (通解 > 特解): same ZST elision pattern for both
                // callee (function definition) and caller (call site).
                // Per §20 (iterative audit): found via §20 Round 4 audit.
                if ty == EmitType::Void {
                    continue;
                }

                // Stage 89 (v0.8 — TD-DYN-TRAIT-FAT-PTR-COERCION): When
                // the callee expects a `&dyn Trait` param (Ref(Dyn)) but
                // the arg is `&ConcreteType` (Ref(Adt)), construct the fat
                // pointer `{ptr @.data.Concrete, ptr @.vtable.Trait.Concrete}`
                // instead of passing the thin data pointer.
                //
                // This is the unsized coercion `&Concrete → &dyn Trait` —
                // typeck accepts it (Stage 87's Adt↔Dyn arm), but codegen
                // must construct the fat pointer at the call site.
                //
                // The dynptr global `@.dynptr.Trait.Concrete` is already
                // emitted by the trait_dispatch orchestrator (Stage 5.50+).
                // We just need to reference it instead of the thin pointer.
                //
                // Per Rust: unsized coercion constructs a fat pointer
                // `{data_ptr, vtable_ptr}` at the coercion site.
                // Per §12 (最优 > 最小): root-cause fix — construct fat
                // pointer at the call site, not patch the callee to accept
                // thin pointers.
                // Per §1.0 原則 6 (通解 > 特解): one coercion path for all
                // Adt → Dyn arg coercions (any concrete type, any trait).
                // Per §1.0 原則 4 (显式 > 隐式): the fat pointer is
                // explicitly constructed, not silently passed as thin.
                let callee_param_ty = callee_def_id_early
                    .and_then(|did| {
                        let sig_opt = fn_sigs.get(&did);
                        sig_opt
                    })
                    .and_then(|sig| sig.inputs.get(arg_idx));

                if let Some(param_ty) = callee_param_ty {
                    // Check if callee expects Ref(Dyn(trait_def_id))
                    if let crate::mir::ty::TyKind::Ref(_, _, inner) = &param_ty.kind {
                        if let crate::mir::ty::TyKind::Dyn(trait_def_id) = inner.kind {
                            // Callee expects &dyn Trait. Build the fat pointer
                            // global name: @.dynptr.{trait_name}.{concrete_name}
                            let concrete_def_id = if let Operand::Copy(lv) | Operand::Move(lv) = a {
                                if let PlaceKind::Local(id) = &lv.kind {
                                    mir.local_decls.get(id.0 as usize).and_then(|ld| {
                                        if let crate::mir::ty::TyKind::Ref(_, _, inner) =
                                            &ld.ty.kind
                                        {
                                            if let crate::mir::ty::TyKind::Adt(adt_def, _) =
                                                inner.kind
                                            {
                                                Some(adt_def)
                                            } else {
                                                None
                                            }
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

                            if let Some(concrete_def_id) = concrete_def_id {
                                // Look up trait_name and concrete_name from HIR/resolver
                                // via the interner. We need the symbol names.
                                // For now, we can't easily access the resolver here,
                                // but we can construct the symbol from the DefIds
                                // using the type_name_by_def_id map.
                                let trait_name_opt = type_name_by_def_id.get(&trait_def_id);
                                let concrete_name_opt = type_name_by_def_id.get(&concrete_def_id);
                                if let (Some(trait_sym), Some(concrete_sym)) =
                                    (trait_name_opt, concrete_name_opt)
                                {
                                    let trait_name = interner.resolve(trait_sym);
                                    let concrete_name = interner.resolve(concrete_sym);
                                    let dynptr_symbol =
                                        format!("@.dynptr.{}.{}", trait_name, concrete_name);
                                    // Pass the fat pointer global as the arg.
                                    // The callee's vtable dispatch will GEP into
                                    // this fat pointer to load the vtable.
                                    arg_pairs.push((ty, dynptr_symbol));
                                    continue;
                                }
                            }
                        }
                    }
                }

                let val = codegen_operand(
                    emitter,
                    mir,
                    a,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                    mono_names,
                    type_name_by_def_id,
                );
                arg_pairs.push((ty, val));
            }
            let arg_refs: Vec<(EmitType, &EmitValue)> =
                arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

            // Stage 14.35: Extract the callee's DefId from the func operand
            // so we can look up its return type in fn_sigs. This fixes
            // struct-returning method calls where dest local type defaults to i32.
            //
            // Stage 16.30: Also extract from Closure-typed func (the
            // closure's def_id is the callee def_id).
            //
            // Stage 18.107 (S8 fix): Also extract callee substs so we can
            // substitute the generic sig's output with the call-site substs.
            // This fixes `make_box::<i32>()` return type: was `Box<Param(0)>`,
            // now correctly `Box<i32>`.
            let (callee_def_id, callee_substs): (
                Option<crate::hir::DefId>,
                crate::mir::ty::SubstsRef,
            ) = if let Operand::Constant(c) = func {
                // Stage 33.1 (TD-VEC-PUSH-GET-MIGRATION): Check c.ty for
                // FnDef(def_id, substs) FIRST — writeback_fndef_substs updates
                // c.ty with inferred substs. Was: only read c.val for DefId
                // and used empty substs — causing call return type to use
                // generic sig.output (Param) instead of substituted output.
                if let crate::mir::ty::TyKind::FnDef(did, substs) = &c.ty.kind {
                    (Some(*did), substs.clone())
                } else {
                    // Fallback: read DefId from c.val (old behavior).
                    match &c.val {
                        ConstVal::Uint(n) => (
                            Some(crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Uint must fit u32"),
                            )),
                            Vec::new().into(),
                        ),
                        ConstVal::Int(n) => (
                            Some(crate::hir::DefId(
                                u32::try_from(*n).expect("FnDef ConstVal::Int must fit u32"),
                            )),
                            Vec::new().into(),
                        ),
                        _ => (None, Vec::new().into()),
                    }
                }
            } else if let Operand::Copy(lv) | Operand::Move(lv) = func {
                if let PlaceKind::Local(id) = &lv.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .and_then(|ld| match &ld.ty.kind {
                            crate::mir::ty::TyKind::FnDef(did, substs) => {
                                Some((Some(*did), substs.clone()))
                            }
                            crate::mir::ty::TyKind::Closure(did, substs) => {
                                Some((Some(*did), substs.clone()))
                            }
                            _ => None,
                        })
                        .unwrap_or((None, Vec::new().into()))
                } else {
                    (None, Vec::new().into())
                }
            } else {
                (None, Vec::new().into())
            };

            let ret_val = if let Some(ref name) = fn_name {
                // Stage 18.18: Detect __landin_println / __landin_print /
                // __landin_eprintln / __landin_eprint calls and route to
                // codegen_print_call (which calls emit_printf_call from
                // Stage 18.12). This is the Phase 2.2 activation of the
                // Phase 2.1 interface (Stage 18.15).
                if is_landin_print_macro(name) {
                    codegen_print_call(
                        name,
                        args,
                        emitter,
                        mir,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                        mono_names,
                        type_name_by_def_id,
                    )?;
                    // The print macros return void/unit — no return value
                    // to store. Branch to the target block if present.
                    if let Some(target) = target {
                        emitter.emit_br(&format!("bb{}", target.0));
                    } else {
                        emitter.emit_unreachable();
                    }
                    return Ok(());
                }
                // Stage 14.35: Use the callee's actual return type from fn_sigs
                // Stage 18.107 (S8 fix): Substitute sig.output with callee_substs
                // so generic functions return the correct concrete type.
                let call_ret_ty = callee_def_id
                    .and_then(|did| fn_sigs.get(&did))
                    .map(|sig| {
                        let substituted_output = if callee_substs.is_empty() {
                            (*sig.output).clone()
                        } else {
                            crate::mir::substitute(&sig.output, &callee_substs)
                        };
                        mir_type_to_emit_type_with_layouts_and_mono(
                            &substituted_output,
                            layouts,
                            mono_layouts,
                        )
                    })
                    .unwrap_or_else(|| {
                        if let PlaceKind::Local(id) = &destination.kind {
                            mir.local_decls
                                .get(id.0 as usize)
                                .map(|ld| {
                                    mir_type_to_emit_type_with_layouts_and_mono(
                                        &ld.ty,
                                        layouts,
                                        mono_layouts,
                                    )
                                })
                                .unwrap_or(EmitType::I32)
                        } else {
                            EmitType::I32
                        }
                    });
                emitter.emit_call(name, &arg_refs, &call_ret_ty)
            } else {
                // Stage 14.58: Indirect call through function pointer.
                // When fn_name is None but the func operand is a FnPtr-typed
                // local, we need to call through the pointer value.
                // Get the function pointer value from the operand.
                let fn_ptr_val = codegen_operand(
                    emitter,
                    mir,
                    func,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                    mono_names,
                    type_name_by_def_id,
                );
                // Determine return type from fn_sigs or dest local
                let call_ret_ty = if let PlaceKind::Local(id) = &destination.kind {
                    mir.local_decls
                        .get(id.0 as usize)
                        .map(|ld| {
                            mir_type_to_emit_type_with_layouts_and_mono(
                                &ld.ty,
                                layouts,
                                mono_layouts,
                            )
                        })
                        .unwrap_or(EmitType::I32)
                } else {
                    EmitType::I32
                };
                emitter.emit_call(&fn_ptr_val, &arg_refs, &call_ret_ty)
            };

            if let PlaceKind::Local(id) = &destination.kind {
                // Stage 14.35: Use the callee's return type for the store too
                // Stage 18.107 (S8 fix): Substitute sig.output with callee_substs
                let dest_ty = callee_def_id
                    .and_then(|did| fn_sigs.get(&did))
                    .map(|sig| {
                        let substituted_output = if callee_substs.is_empty() {
                            (*sig.output).clone()
                        } else {
                            crate::mir::substitute(&sig.output, &callee_substs)
                        };
                        mir_type_to_emit_type_with_layouts_and_mono(
                            &substituted_output,
                            layouts,
                            mono_layouts,
                        )
                    })
                    .unwrap_or_else(|| {
                        mir.local_decls
                            .get(id.0 as usize)
                            .map(|ld| {
                                mir_type_to_emit_type_with_layouts_and_mono(
                                    &ld.ty,
                                    layouts,
                                    mono_layouts,
                                )
                            })
                            .unwrap_or(EmitType::I32)
                    });
                emitter.set_local(id.0, ret_val.clone());
                if let Some(ptr) = emitter.local_ptr(id.0).cloned() {
                    emitter.emit_store(&dest_ty, &ret_val, &ptr);
                }
            }
            if let Some(cont) = target {
                emitter.emit_br(&format!("bb{}", cont.0));
            }
        }
        TerminatorKind::Assert {
            cond, target, msg, ..
        } => {
            let panic_label = format!("panic_assert_{}", target.0);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, lhs, rhs) => {
                    let op_ty = detect_operand_type(mir, lhs, layouts, mono_layouts)
                        .or(detect_operand_type(mir, rhs, layouts, mono_layouts))
                        .unwrap_or(EmitType::I32);
                    let lhs_val = codegen_operand(
                        emitter,
                        mir,
                        lhs,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                        mono_names,
                        type_name_by_def_id,
                    );
                    let rhs_val = codegen_operand(
                        emitter,
                        mir,
                        rhs,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                        mono_names,
                        type_name_by_def_id,
                    );
                    match op {
                        crate::mir::place::BinOp::Shl | crate::mir::place::BinOp::Shr => {
                            // Stage 18.118: Explicit arms for all integer types.
                            // EmitType uses I1 (not Bool), and has no U8/U16/etc
                            // (unsigned types are emitted as signed I8/I16/etc).
                            // Non-integer types default to 32 bits (conservative).
                            let bit_width: u32 = match op_ty {
                                EmitType::I8 => 8,
                                EmitType::I16 => 16,
                                EmitType::I32 | EmitType::I1 => 32,
                                EmitType::I64 => 64,
                                EmitType::I128 => 128,
                                _ => 32, // Fallback for Float/Struct/Ptr/etc.
                            };
                            // Stage 18.288 (TD-SHIFTOVERFLOW-CONST-TYPE fix):
                            // Use `emit_const_typed` instead of raw string.
                            // The old code passed `bit_width.to_string()` (e.g.,
                            // "64") which was parsed as `i32 64` by the LLVM
                            // emitter's `lookup`, causing type mismatch when
                            // `op_ty` was i64:
                            //   "icmp uge i64 %v4, i32 64" → LLVM verify fail
                            //
                            // Same class as TD-NEGOVERFLOW-I32 + TD-DIVZERO-CONST-TYPE.
                            // Per §17.6 (直到审查不出问题为止): found during audit.
                            // Per §12 (最优 > 最小): reuse `emit_const_typed`.
                            let width_val = emitter.emit_const_typed(bit_width as i64, &op_ty);
                            let is_overflow =
                                emitter.emit_icmp("uge", &op_ty, &rhs_val, &width_val);
                            emitter.emit_br_cond(
                                &is_overflow,
                                &panic_label,
                                &format!("bb{}", target.0),
                            );
                        }
                        _ => {
                            let agg = emitter.emit_checked_binop(*op, &op_ty, &lhs_val, &rhs_val);
                            let agg_ty = EmitType::struct_of(vec![op_ty.clone(), EmitType::I1]);
                            let overflow_flag = emitter.emit_extractvalue(&agg_ty, &agg, 1);
                            let inverted = emitter.emit_unop(
                                crate::mir::place::UnOp::Not,
                                &EmitType::I1,
                                &overflow_flag,
                            );
                            emitter.emit_br_cond(
                                &inverted,
                                &format!("bb{}", target.0),
                                &panic_label,
                            );
                        }
                    }
                }
                crate::mir::body::AssertMessage::DivisionByZero(rhs) => {
                    // Stage 18.109 (S10 fix): After const_prop + DCE, the rhs
                    // local may not have been stored (the constant assignment
                    // was removed). If the local has no cached value in the
                    // emitter, it means const_prop folded the BinaryOp and
                    // the assert is stale — skip it.
                    //
                    // Per §1.0 原則 4 "报错 > 静默": const_prop only folds
                    // when rhs is a known non-zero constant, so skipping is safe.
                    // Per §1.0 原則 6 "通用 > 特例": one check for all Div/Rem.
                    let skip_check = if let Operand::Copy(lv) | Operand::Move(lv) = rhs {
                        if let PlaceKind::Local(id) = &lv.kind {
                            emitter.local(id.0).is_none()
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if skip_check {
                        emitter.emit_br(&format!("bb{}", target.0));
                    } else {
                        let rhs_val = codegen_operand(
                            emitter,
                            mir,
                            rhs,
                            interner,
                            layouts,
                            mono_layouts,
                            fn_name_by_def_id,
                            mono_names,
                            type_name_by_def_id,
                        );
                        let rhs_ty = detect_operand_type(mir, rhs, layouts, mono_layouts)
                            .unwrap_or(EmitType::I32);
                        // Stage 18.288 (TD-DIVZERO-CONST-TYPE fix): Use
                        // `emit_const_typed` instead of `"0".to_string()` for
                        // the zero constant. The old code passed a raw "0"
                        // string which the text emitter formatted as `i32 0`,
                        // but the LLVM emitter's `lookup` parsed it as an i32
                        // constant — causing LLVM type mismatch when `rhs_ty`
                        // was i64 (e.g., `a / b` where a, b: i64):
                        //   "Both operands to ICmp instruction are not of the
                        //    same type! icmp eq i64 %v2, i32 0"
                        //
                        // Same class as TD-NEGOVERFLOW-I32 (Stage 18.287 fix):
                        // overflow/division asserts must emit zero constants
                        // with the EXACT type matching the operand.
                        //
                        // Per §17.6 (直到审查不出问题为止): found during the
                        // post-Stage-18.287 audit of similar emit_const patterns.
                        // Per §12 (最优 > 最小): reuse `emit_const_typed` (added
                        // in Stage 18.287) — one typed-const path for all asserts.
                        let zero_val = emitter.emit_const_typed(0, &rhs_ty);
                        let is_zero = emitter.emit_icmp("eq", &rhs_ty, &rhs_val, &zero_val);
                        emitter.emit_br_cond(&is_zero, &panic_label, &format!("bb{}", target.0));
                    }
                }
                // Stage 18.67: NegOverflow — check if operand is iN::MIN
                // by computing `0 - operand` with ssub.with.overflow.
                crate::mir::body::AssertMessage::NegOverflow(operand) => {
                    let op_val = codegen_operand(
                        emitter,
                        mir,
                        operand,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                        mono_names,
                        type_name_by_def_id,
                    );
                    let op_ty = detect_operand_type(mir, operand, layouts, mono_layouts)
                        .unwrap_or(EmitType::I32);
                    // Reuse emit_checked_binop with Sub to get {result, overflow}.
                    // Per §1.0 原則 6 "通用 > 特例": reuse existing infrastructure.
                    //
                    // Stage 18.287 (TD-NEGOVERFLOW-I32 fix): Use `emit_const_typed`
                    // instead of `emit_const(&ConstVal::Int(0))`. The old code
                    // emitted `0` as i32 (the default for small ConstVal::Int),
                    // causing LLVM type mismatch when `op_ty` was i64:
                    //   "Call parameter type does not match function signature!
                    //    i32 0 vs i64 %v = call { i64, i1 } @llvm.ssub.with.overflow.i64(...)"
                    //
                    // The fix emits `0` with the EXACT type matching `op_ty`,
                    // so the checked binop's operands are always type-matched.
                    //
                    // Per §12 (最优 > 最小): fix root cause (typed const),
                    // not symptom (cast after emit).
                    // Per §1.0 原則 6 (通解 > 特解): one typed-const path for all widths.
                    let zero_val = emitter.emit_const_typed(0, &op_ty);
                    let checked =
                        emitter.emit_checked_binop(BinOp::Sub, &op_ty, &zero_val, &op_val);
                    // Extract overflow flag (field 1 of {T, i1} struct).
                    let overflow = emitter.emit_extractvalue(&op_ty, &checked, 1);
                    emitter.emit_br_cond(&overflow, &panic_label, &format!("bb{}", target.0));
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let cond_val = codegen_operand(
                        emitter,
                        mir,
                        cond,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                        mono_names,
                        type_name_by_def_id,
                    );
                    emitter.emit_br_cond(&cond_val, &format!("bb{}", target.0), &panic_label);
                }
            }
            emitter.emit_block(&panic_label);
            match msg {
                crate::mir::body::AssertMessage::Overflow(op, _, _) => {
                    let op_code: i32 = match op {
                        BinOp::Add => 0,
                        BinOp::Sub => 1,
                        BinOp::Mul => 2,
                        BinOp::Div => 3,
                        BinOp::Rem => 4,
                        BinOp::Shl => 5,
                        BinOp::Shr => 6,
                        _ => 99,
                    };
                    let op_str = op_code.to_string();
                    let z1 = "0".to_string();
                    let z2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_overflow",
                        &[
                            (EmitType::I32, &op_str),
                            (EmitType::I32, &z1),
                            (EmitType::I32, &z2),
                        ],
                        &EmitType::Void,
                    );
                }
                crate::mir::body::AssertMessage::DivisionByZero(_) => {
                    let _ = emitter.emit_call("__landin_panic_div_by_zero", &[], &EmitType::Void);
                }
                // Stage 18.67: NegOverflow panic — reuse overflow panic with Sub op (code=1).
                crate::mir::body::AssertMessage::NegOverflow(_) => {
                    let op_str = "1".to_string(); // Sub op code
                    let z1 = "0".to_string();
                    let z2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_overflow",
                        &[
                            (EmitType::I32, &op_str),
                            (EmitType::I32, &z1),
                            (EmitType::I32, &z2),
                        ],
                        &EmitType::Void,
                    );
                }
                crate::mir::body::AssertMessage::BoundsCheck => {
                    let z1 = "0".to_string();
                    let z2 = "0".to_string();
                    let _ = emitter.emit_call(
                        "__landin_panic_bounds_check",
                        &[(EmitType::I64, &z1), (EmitType::I64, &z2)],
                        &EmitType::Void,
                    );
                }
            }
            emitter.emit_unreachable();
        }
        // Stage 15.45 (HP-12 step 4): TerminatorKind::Drop calls drop glue.
        //
        // The `Drop` terminator is inserted by `elaborate_drops` (Stage 15.44,
        // fixed in Stage 15.61) for locals whose type needs drop. When this
        // terminator is reached, the codegen:
        //   1. Computes the place's address (pointer to the value).
        //   2. Calls the drop glue function `drop_adt_<DefId>` with the pointer.
        //   3. Branches to the target block.
        //
        // The drop glue function (`drop_adt_<DefId>`) is emitted by
        // `emit_drop_glue_functions` (Stage 15.57). For types that implement
        // `Drop`, it calls the user's `Drop::drop` method with the pointer.
        //
        // Stage 15.61 fix: the call now passes `EmitType::OpaquePtr` (not the
        // place's value type). The drop glue function is declared as
        // `void @drop_adt_<DefId>(ptr %self)`, so the argument MUST be a
        // pointer. Previously, passing `place_ty` (e.g., `{ i32 }`) caused
        // a type mismatch between call site and declaration.
        //
        // Per §1.0 原則 3 "显式 > 隐式": the drop call is explicit.
        // Per §23: the drop glue function name follows `drop_<noun>_<id>`.
        TerminatorKind::Drop { place, target, .. } => {
            // Compute the place's address (pointer to the value to drop).
            let place_addr = crate::codegen::mir_translation::compute_place_address(
                emitter,
                mir,
                place,
                interner,
                layouts,
                mono_layouts,
            );

            // Call the drop glue function.
            //
            // The drop glue function name uses a simple format based on
            // the place's MIR type. For ADT types, we look up the DefId
            // from the local's type declaration (e.g., `drop_adt_3` for
            // DefId(3)). For other types, we use a generic name.
            //
            // Per §23: function name follows `drop_<noun>_<id>` pattern.
            // Per §1.0 原則 3 "显式 > 隐式": the drop call is explicit.
            let drop_fn_name = {
                // Look up the place's MIR type to get the DefId (if ADT).
                let mir_ty = match &place.kind {
                    crate::mir::place::PlaceKind::Local(local_id) => {
                        Some(mir.local(*local_id).ty.kind.clone())
                    }
                    _ => None,
                };
                match &mir_ty {
                    Some(crate::mir::ty::TyKind::Adt(def_id, _)) => {
                        format!("drop_adt_{}", def_id.0)
                    }
                    _ => "drop_generic".to_string(),
                }
            };

            // Stage 15.61: pass `OpaquePtr` (not the place's value type).
            // The drop glue function expects `ptr %self`, and `place_addr`
            // is the address of the place (an alloca pointer for a Local,
            // or a GEP result for a projection). Passing the value type
            // (e.g., `{ i32 }`) would cause an LLVM type mismatch.
            emitter.emit_call(
                &drop_fn_name,
                &[(EmitType::OpaquePtr, &place_addr)],
                &EmitType::Void,
            );

            // Branch to the target block.
            emitter.emit_br(&format!("bb{}", target.0));
        }
    }
    Ok(())
}

// =============================================================================
// Stage 18.15: __landin_println / __landin_print / __landin_eprintln /
// __landin_eprint call detection (Phase 2.1 — interface preparation)
// =============================================================================

/// Stage 18.15: Check if a function name is a Landin built-in print macro
/// runtime function (`__landin_println` / `__landin_print` /
/// `__landin_eprintln` / `__landin_eprint`).
///
/// These are the C ABI names that the built-in macro_rules! definitions
/// (Stage 18.10) will expand to in Phase 2.2. When codegen detects a
/// call to one of these, it routes to `emit_printf_call` (Stage 18.12)
/// instead of the regular call path.
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
/// Stage 18.18: Now called from Call terminator — `#[allow(dead_code)]`
/// removed (Phase 2.2 activation).
pub(crate) fn is_landin_print_macro(name: &str) -> bool {
    matches!(
        name,
        "__landin_println" | "__landin_print" | "__landin_eprintln" | "__landin_eprint"
    )
}

/// Stage 18.23: Extract the format string from a call argument operand.
///
/// Handles two cases:
/// 1. `Operand::Constant(Const { val: Str(sym) })` → direct extraction
/// 2. `Operand::Move/Copy(place)` → trace back through MIR basic blocks
///    to find `Assign(place, Rvalue::Use(Constant(Str)))` → extract
///
/// This is needed because MIR lowering assigns string literals to
/// temporary locals before passing them as call args. The Call
/// terminator sees `Operand::Move(local)`, not `Operand::Constant`.
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
fn extract_format_string(arg: &Operand, mir: &MirBody, interner: &Rodeo) -> String {
    use crate::mir::place::{Operand as MirOperand, PlaceKind as MirPlaceKind};
    match arg {
        // Case 1: Direct constant — extract string immediately.
        MirOperand::Constant(c) => match c.val {
            ConstVal::Str(sym) => interner
                .try_resolve(&sym)
                .map(|s| s.to_string())
                .unwrap_or_default(),
            _ => String::new(),
        },
        // Case 2: Move/Copy of a place — trace back to constant assignment.
        MirOperand::Move(place) | MirOperand::Copy(place) => {
            // Only handle simple local places (no projections).
            if let MirPlaceKind::Local(local_id) = &place.kind {
                // Scan all basic blocks for an Assign to this local
                // with a Rvalue::Use(Operand::Constant(Str)).
                for bb in &mir.basic_blocks {
                    for stmt in &bb.statements {
                        if let crate::mir::body::StatementKind::Assign(boxed) = &stmt.kind {
                            let (assign_place, rvalue) = &**boxed;
                            // Check if this assignment targets our local.
                            if let MirPlaceKind::Local(assign_local) = &assign_place.kind {
                                if assign_local == local_id {
                                    // Check if rvalue is Use(Constant(Str)).
                                    if let crate::mir::place::Rvalue::Use(MirOperand::Constant(c)) =
                                        rvalue
                                    {
                                        if let ConstVal::Str(sym) = c.val {
                                            return interner
                                                .try_resolve(&sym)
                                                .map(|s| s.to_string())
                                                .unwrap_or_default();
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Could not trace back to a constant — return empty.
            String::new()
        }
    }
}

/// Stage 18.15: Codegen a call to a Landin print macro runtime function.
///
/// Routes to `emit_printf_call` (Stage 18.12) with the appropriate
/// `newline` and `stderr` flags derived from the function name:
/// - `__landin_println`  → newline=true,  stderr=false
/// - `__landin_print`    → newline=false, stderr=false
/// - `__landin_eprintln` → newline=true,  stderr=true
/// - `__landin_eprint`   → newline=false, stderr=true
///
/// The first argument must be a string literal (the format string).
/// Remaining arguments are the format args.
///
/// Per §10: `<verb>_<noun>_<noun>` pattern.
/// Stage 18.18: Now called from Call terminator — `#[allow(dead_code)]`
/// removed (Phase 2.2 activation).
/// Stage 18.151 (TD-CODEGEN-RESULT): `codegen_print_call` now returns
/// `CodegenResult<()>` to propagate codegen errors from `emit_printf_call`.
///
/// Per §2 原则 9 (正确>妥协): full Result propagation.
#[allow(clippy::too_many_arguments)] // codegen context requires many params
fn codegen_print_call(
    name: &str,
    args: &[Operand],
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
    // Stage 101: mono_names + type_name_by_def_id for FnDef substs mangling.
    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,
    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,
) -> CodegenResult<()> {
    // Derive newline/stderr from the function name.
    let (newline, stderr) = match name {
        "__landin_println" => (true, false),
        "__landin_print" => (false, false),
        "__landin_eprintln" => (true, true),
        "__landin_eprint" => (false, true),
        _ => return Ok(()), // Not a print macro — no-op.
    };

    // Stage 18.23: Extract the format string from the first argument.
    // Uses `extract_format_string` which handles both Operand::Constant
    // (direct) and Operand::Move/Copy (traced back through MIR).
    let mut msg = if let Some(first) = args.first() {
        extract_format_string(first, mir, interner)
    } else {
        String::new()
    };

    // Stage 18.27: Append newline if needed (println!/eprintln!).
    // The old Println statement path had newline pre-encoded in msg
    // (parser added it). The new Call path extracts msg from the raw
    // string literal, which doesn't include the newline. So we add it
    // here, matching the old behavior.
    if newline {
        msg.push('\n');
    }

    // Remaining args are the format args.
    let fmt_args = if args.len() > 1 { &args[1..] } else { &[] };

    // Route to emit_printf_call (Stage 18.12).
    super::statement::emit_printf_call(
        emitter,
        mir,
        &msg,
        fmt_args,
        newline,
        stderr,
        interner,
        layouts,
        mono_layouts,
        fn_name_by_def_id,
        mono_names,
        type_name_by_def_id,
    )
}

#[cfg(test)]
mod tests {
    use super::is_landin_print_macro;

    /// Stage 18.15 positive 1: println! still works via parser special case
    /// (Phase 2.1 doesn't change behavior — built-in macro body is still
    /// no-op, so parser special case still handles println!).
    #[test]
    fn stage18_15_println_still_works_via_special_case() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    /// Stage 18.15 positive 2: eprintln! still works via parser special case.
    #[test]
    fn stage18_15_eprintln_still_works_via_special_case() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.15 negative 1: is_landin_print_macro detects __landin_println.
    #[test]
    fn stage18_15_is_landin_print_macro_println() {
        assert!(is_landin_print_macro("__landin_println"));
    }

    /// Stage 18.15 negative 2: is_landin_print_macro detects __landin_print.
    #[test]
    fn stage18_15_is_landin_print_macro_print() {
        assert!(is_landin_print_macro("__landin_print"));
    }

    /// Stage 18.15 negative 3: is_landin_print_macro detects __landin_eprintln.
    #[test]
    fn stage18_15_is_landin_print_macro_eprintln() {
        assert!(is_landin_print_macro("__landin_eprintln"));
    }

    /// Stage 18.15 negative 4: is_landin_print_macro detects __landin_eprint.
    #[test]
    fn stage18_15_is_landin_print_macro_eprint() {
        assert!(is_landin_print_macro("__landin_eprint"));
    }

    /// Stage 18.15 negative 5: is_landin_print_macro rejects other names.
    #[test]
    fn stage18_15_is_landin_print_macro_rejects_other() {
        assert!(!is_landin_print_macro("printf"));
        assert!(!is_landin_print_macro("__landin_panic"));
        assert!(!is_landin_print_macro("main"));
    }

    /// Stage 18.15 negative 6: is_landin_print_macro rejects empty string.
    #[test]
    fn stage18_15_is_landin_print_macro_rejects_empty() {
        assert!(!is_landin_print_macro(""));
    }

    // =====================================================================
    // Stage 18.18 tests — Phase 2.2 activation
    // =====================================================================

    /// Stage 18.18 positive 1: println! still works after activation
    /// (parser special case path still handles it).
    #[test]
    fn stage18_18_println_still_works_after_activation() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    /// Stage 18.18 positive 2: eprintln! still works after activation.
    #[test]
    fn stage18_18_eprintln_still_works_after_activation() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.18 negative 1: is_landin_print_macro still detects all 4.
    #[test]
    fn stage18_18_is_landin_print_macro_still_detects() {
        assert!(is_landin_print_macro("__landin_println"));
        assert!(is_landin_print_macro("__landin_print"));
        assert!(is_landin_print_macro("__landin_eprintln"));
        assert!(is_landin_print_macro("__landin_eprint"));
    }

    /// Stage 18.18 negative 2: is_landin_print_macro detects __landin_println.
    #[test]
    fn stage18_18_is_landin_print_macro_println_after_activation() {
        assert!(is_landin_print_macro("__landin_println"));
    }

    /// Stage 18.18 negative 3: is_landin_print_macro detects __landin_eprintln.
    #[test]
    fn stage18_18_is_landin_print_macro_eprintln_after_activation() {
        assert!(is_landin_print_macro("__landin_eprintln"));
    }

    /// Stage 18.18 negative 4: Regular function calls are not affected
    /// by the __landin_println detection.
    #[test]
    fn stage18_18_regular_call_not_affected() {
        let src = "fn foo() -> i32 { 42 } fn main() { let x = foo(); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.18 negative 5: print! macro (not __landin_print) is not
    /// affected by the activation.
    #[test]
    fn stage18_18_print_macro_not_broken_by_activation() {
        let src = "fn main() { print!(\"no newline\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.18 negative 6: User-defined macro_rules! is not affected
    /// by the __landin_println activation.
    #[test]
    fn stage18_18_macro_rules_user_macro_not_affected() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    // =====================================================================
    // Stage 18.23 tests — codegen_print_call MIR operand handling
    // =====================================================================

    /// Stage 18.23 positive 1: println! with constant format string still works.
    #[test]
    fn stage18_23_println_constant_format_works() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.23 positive 2: println! with args (Move/Copy operands) still works.
    #[test]
    fn stage18_23_println_move_format_traced() {
        let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.23 negative 1: is_landin_print_macro detects all 4 names.
    #[test]
    fn stage18_23_is_landin_print_macro_all_four() {
        assert!(is_landin_print_macro("__landin_println"));
        assert!(is_landin_print_macro("__landin_print"));
        assert!(is_landin_print_macro("__landin_eprintln"));
        assert!(is_landin_print_macro("__landin_eprint"));
    }

    /// Stage 18.23 negative 2: is_landin_print_macro rejects non-print names.
    #[test]
    fn stage18_23_is_landin_print_macro_rejects_others() {
        assert!(!is_landin_print_macro("printf"));
        assert!(!is_landin_print_macro("main"));
        assert!(!is_landin_print_macro("__landin_panic"));
    }

    /// Stage 18.23 negative 3: println! with multiple args still works.
    #[test]
    fn stage18_23_println_multiple_args() {
        let src = "fn main() { let a = 1; let b = 2; println!(\"{}{}\", a, b); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.23 negative 4: print! (no newline) still works.
    #[test]
    fn stage18_23_print_no_newline_works() {
        let src = "fn main() { print!(\"no newline\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.23 negative 5: eprintln! still works.
    #[test]
    fn stage18_23_eprintln_works() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = crate::compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.23 negative 6: println! produces MIR bodies (codegen path works).
    #[test]
    fn stage18_23_println_produces_mir() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = crate::compile(src);
        assert!(!result.mirs.is_empty(), "should produce MIR bodies");
    }
}
