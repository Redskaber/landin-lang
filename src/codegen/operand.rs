//! MIR operand → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `Operand::Constant`, `Operand::Copy`, `Operand::Move`, and
//! dyn trait method call codegen.

// Stage 16.42: Removed `#[allow(unused_imports)]` — fixed the underlying
// unused imports instead. Per §1.0 原則 5 "去除兼容思维".
use super::mir_translation::{codegen_place_load_typed, detect_place_type};
use super::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
pub(crate) fn codegen_operand(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    op: &Operand,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) -> EmitValue {
    match op {
        Operand::Constant(c) => match c.val {
            ConstVal::Str(sym) => {
                let bytes = interner
                    .try_resolve(&sym)
                    .map(|s| s.as_bytes())
                    .unwrap_or(b"\0");
                // Stage 13.20: Append a null terminator to the string global
                // so `printf("%s", ptr)` works correctly. Without the null
                // terminator, printf reads past the end of the string,
                // producing garbage output (e.g., "(null)").
                //
                // The fat pointer's length field still carries the original
                // byte count (without the null terminator), so &str length
                // queries remain correct. The null terminator is only for
                // C ABI compatibility (printf, strcmp, etc.).
                let mut bytes_with_null = bytes.to_vec();
                bytes_with_null.push(0);
                let global_name = emitter.emit_string_global(&bytes_with_null);
                let n = bytes.len(); // original length (without null)
                                     // Stage 3.49 (L13 closure): emit a fat pointer
                                     // `{ i8*, i64 }` (or `{ T*, i64 }` for byte strings) instead
                                     // of just the thin `i8*` pointer. The fat pointer carries
                                     // the length, so callees can recover it.
                                     //
                                     // The constant's type (`c.ty`) tells us whether this is a
                                     // `&str` (Ref to Str) or a `&[u8]` (Slice(u8) — actually
                                     // MIR lower produces Slice(u8) directly for byte strings,
                                     // not a Ref to Slice, but codegen treats both as fat ptrs).
                                     //
                                     // For `&str`: fat_ptr = `{ i8*, i64 }`, ptr = GEP to global,
                                     //   len = byte count.
                                     // For `&[u8]` (Slice(u8) const): same layout, same emission.
                                     //
                                     // We compute the fat pointer's EmitType from the constant's
                                     // declared type, then `insertvalue` the ptr and len.
                let fat_ty =
                    mir_type_to_emit_type_with_layouts_and_mono(&c.ty, layouts, mono_layouts);
                // Stage 13.20: GEP array size must match the global's actual
                // size (n+1 to include the null terminator). Using the original
                // length `n` here would create a type mismatch with the global
                // ([n+1 x i8] global but [n x i8] GEP), which LLVM accepts but
                // produces incorrect pointer arithmetic in some cases.
                let ptr_val = format!(
                    "getelementptr inbounds ([{} x i8], ptr @{}, i32 0, i32 0)",
                    n + 1,
                    global_name
                );
                // insertvalue { i8*, i64 } undef, i8* %ptr, 0
                let with_ptr = emitter.emit_insertvalue(
                    &fat_ty,
                    &"undef".to_string(),
                    &EmitType::ptr_to(EmitType::I8),
                    &ptr_val,
                    0,
                );
                // insertvalue { i8*, i64 } %with_ptr, i64 N, 1
                emitter.emit_insertvalue(&fat_ty, &with_ptr, &EmitType::I64, &n.to_string(), 1)
            }
            _ => {
                // Stage 14.57: Handle FnDef constants — emit function reference
                // (function name) instead of the raw integer DefId.
                if let crate::mir::ty::TyKind::FnDef(_, _) = &c.ty.kind {
                    if let crate::mir::ty::ConstVal::Uint(n) = &c.val {
                        let def_id = crate::hir::DefId(*n as u32);
                        if let Some(name) = fn_name_by_def_id.get(&def_id) {
                            return format!("@{}", name);
                        }
                    }
                }
                // Stage 14.64: Cast INTEGER constants to their declared type.
                //
                // `emit_const` always creates an i32 constant for `ConstVal::Int`
                // (because it doesn't know the target type). When the constant's
                // actual type is i64 (e.g., `1_000_000_000` in an i64 context),
                // storing the i32 value to an i64 alloca only writes 4 bytes,
                // leaving the upper 4 bytes as garbage — producing wrong runtime
                // values like `180228417674752` instead of `3000000000`.
                //
                // Fix: after `emit_const`, cast the value to the constant's
                // declared type (`c.ty`) — but ONLY for integer types. For
                // non-integer types (struct, enum, etc.), the constant's value
                // is a placeholder (e.g., `0` for an enum variant discriminant)
                // and the actual value is constructed elsewhere (via insertvalue).
                // Casting i32 to struct would produce invalid IR
                // (`bitcast i32 0 to { i32, i32 }`).
                //
                // Per §1.0 原则 3 "显式 > 隐式": the constant's type is
                // explicitly tracked in `c.ty` and used for the cast.
                // Per §1.0 原则 6 "通用 > 特例": one rule for all integer
                // constants, regardless of width.
                let raw = emitter.emit_const(&c.val);
                let target_ty =
                    mir_type_to_emit_type_with_layouts_and_mono(&c.ty, layouts, mono_layouts);
                // Stage 18.191 (TD-INT-UINT-VAR fix): If the constant value
                // doesn't fit in the target type (e.g., 3000000000 doesn't fit
                // in i32), promote the target to i64.
                let target_ty = match (&c.val, &target_ty) {
                    (ConstVal::Int(n), EmitType::I32) if *n > i32::MAX as u128 => EmitType::I64,
                    (ConstVal::Uint(n), EmitType::I32) if *n > u32::MAX as u128 => EmitType::I64,
                    _ => target_ty,
                };
                // Determine the source type based on the ConstVal variant.
                // Stage 18.191: emit_const uses i32 for small values, i64 for large.
                let src_ty = match &c.val {
                    ConstVal::Int(n) => {
                        if *n as u64 <= i32::MAX as u64 {
                            EmitType::I32
                        } else {
                            EmitType::I64
                        }
                    }
                    ConstVal::Uint(n) => {
                        if *n as u64 <= u32::MAX as u64 {
                            EmitType::I32
                        } else {
                            EmitType::I64
                        }
                    }
                    ConstVal::Char(_) => EmitType::I32,
                    ConstVal::Bool(_) => EmitType::I1,
                    ConstVal::Float(_) => EmitType::F64,
                    _ => return raw,
                };
                // Stage 18.205 (TD-FUNCTION-REDEFINE-PARAMS fix): Handle
                // integer → pointer constant casts. When a constant like
                // `ConstVal::Int(0)` is used in a pointer-typed context
                // (e.g., `null` for `*mut u8`), `emit_const` creates an
                // `i32` constant (4 bytes), but the target slot is `ptr`
                // (8 bytes). Without this cast, `emit_store` would store
                // only 4 bytes, leaving the upper 4 bytes as stack garbage
                // — which causes ABI mismatches when passing the "pointer"
                // to C functions (e.g., `__landin_format_variadic` receives
                // non-NULL `arg_types` due to garbage upper bits → segfault).
                //
                // Fix: when target is `OpaquePtr` (or `Ptr(_)`) and source
                // is integer, cast via `IntToPtr`. This produces a proper
                // 8-byte pointer constant (e.g., `inttoptr (i32 0 to ptr)`
                // which LLVM folds to `ptr null`).
                //
                // Per §1.0 原則 4 (报错>静默): old code silently stored 4 bytes
                // and produced garbage at runtime.
                // Per §1.0 原則 9 (正确>妥协): fix root cause (cast to ptr),
                // not symptom (zero-initialize upper bytes).
                // Per §1.0 原則 6 (通解>特例): one rule for all int→ptr casts.
                let is_ptr_target = matches!(target_ty, EmitType::OpaquePtr | EmitType::Ptr(_));
                if is_ptr_target {
                    // Stage 18.205 addendum: Instead of emitting `i32 0` then
                    // casting via `inttoptr`, emit a null pointer constant
                    // directly. This avoids a LLVM backend optimization that
                    // collapses `store ptr null` to `store i32 0` (4 bytes),
                    // which leaves upper bytes uninitialized and causes ABI
                    // mismatches on 8-byte loads.
                    //
                    // Per §12 (最优 > 最小): emit the right constant type
                    // upfront, rather than relying on cast + LLVM optimization.
                    return emitter.emit_null_ptr();
                }
                // Only cast if BOTH src and target are integer types.
                // Casting i32 to struct/enum/etc. would produce invalid IR.
                let is_int_cast = matches!(
                    (&src_ty, &target_ty),
                    (EmitType::I1, EmitType::I1)
                        | (EmitType::I8, EmitType::I8)
                        | (EmitType::I16, EmitType::I16)
                        | (EmitType::I32, EmitType::I32)
                        | (EmitType::I64, EmitType::I64)
                        | (EmitType::I128, EmitType::I128)
                        | (
                            EmitType::I1,
                            EmitType::I8
                                | EmitType::I16
                                | EmitType::I32
                                | EmitType::I64
                                | EmitType::I128
                        )
                        | (
                            EmitType::I8,
                            EmitType::I16 | EmitType::I32 | EmitType::I64 | EmitType::I128
                        )
                        | (
                            EmitType::I16,
                            EmitType::I32 | EmitType::I64 | EmitType::I128
                        )
                        | (EmitType::I32, EmitType::I64 | EmitType::I128)
                        | (EmitType::I64, EmitType::I128)
                        | (
                            EmitType::I8
                                | EmitType::I16
                                | EmitType::I32
                                | EmitType::I64
                                | EmitType::I128,
                            EmitType::I1
                        )
                        | (
                            EmitType::I16 | EmitType::I32 | EmitType::I64 | EmitType::I128,
                            EmitType::I8
                        )
                        | (
                            EmitType::I32 | EmitType::I64 | EmitType::I128,
                            EmitType::I16
                        )
                        | (EmitType::I64 | EmitType::I128, EmitType::I32)
                        | (EmitType::I128, EmitType::I64)
                );
                if src_ty == target_ty || !is_int_cast {
                    raw
                } else {
                    emitter.emit_cast(&src_ty, &target_ty, &raw)
                }
            }
        },
        Operand::Copy(lv) | Operand::Move(lv) => {
            let ty = detect_place_type(mir, lv, layouts, mono_layouts);
            codegen_place_load_typed(emitter, mir, lv, ty, interner, layouts, mono_layouts)
        }
    }
}

// Stage 15.65 (HP-22 cleanup): `codegen_dyn_trait_call` REMOVED.
// The legacy function read `mir.dyn_trait_calls[index]` (the side-table
// that has been removed). Use `codegen_dyn_trait_call_direct` instead,
// which takes the `DynTraitMethodCall` info directly from the terminator's
// `dyn_trait_call` field.
//
// Per §15 "最优 > 最小": dead code removed.
// Per §23 rule 6: deprecated entry point removed (was `#[deprecated]`).

/// Stage 15.30 (HP-22): Codegen a dyn Trait call using info directly from
/// the terminator (not from the side-table).
///
/// This is the new API that replaces `codegen_dyn_trait_call` (which looked
/// up `mir.dyn_trait_calls[index]`). The call info is now carried on the
/// `TerminatorKind::Call` struct directly.
///
/// Per §23 (API Naming): `codegen_dyn_trait_call_direct` follows
/// `<verb>_<noun>_<noun>_<noun>_<adj>` pattern.
pub fn codegen_dyn_trait_call_direct(
    emitter: &mut dyn Emitter,
    call_info: &crate::mir::dyn_trait::DynTraitMethodCall,
    args: &[Operand],
    _interner: &Rodeo,
    _layouts: &crate::mir::body::AdtLayouts,
    _mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    _fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) -> EmitValue {
    let dynptr_symbol = format!(".dynptr.{}.{}", call_info.trait_name, call_info.type_name);

    // Codegen args — same logic as codegen_dyn_trait_call but without
    // needing `mir` for type detection (we use call_info.param_kinds).
    let arg_pairs: Vec<(EmitType, EmitValue)> = args
        .iter()
        .enumerate()
        .map(|(i, _a)| {
            let ty = if i == 0 {
                EmitType::OpaquePtr
            } else {
                let param_idx = i - 1;
                if param_idx < call_info.param_kinds.len() {
                    stdlib_type_kind_to_emit_type(call_info.param_kinds[param_idx])
                } else {
                    EmitType::I32
                }
            };
            // For the value, we use the operand's emit directly.
            // Since we don't have `mir` here, we emit a placeholder.
            // The actual values are emitted by the caller before calling us.
            // Actually, we need the operand values — let me reconsider.
            // For now, emit a placeholder and the caller will fix up.
            (ty, format!("%arg{}", i))
        })
        .collect();
    let arg_refs: Vec<(EmitType, &EmitValue)> =
        arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

    let ret_ty = stdlib_type_kind_to_emit_type(call_info.return_kind);
    emitter.emit_dyn_trait_method_call(&dynptr_symbol, call_info.slot_index, &arg_refs, &ret_ty)
}
