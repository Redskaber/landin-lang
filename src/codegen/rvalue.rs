//! MIR rvalue → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `Rvalue::Use`, `BinaryOp`, `UnaryOp`, `Ref`, `Aggregate`, etc.

// Stage 16.42: Removed `#[allow(unused_imports)]` — fixed the underlying
// unused imports instead. Per §1.0 原則 5 "去除兼容思维".
use super::mir_translation::detect_operand_type;
use super::*;
use crate::mir::place::*;
pub(crate) fn codegen_rvalue(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    rv: &Rvalue,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) -> EmitValue {
    match rv {
        Rvalue::Use(op) => codegen_operand(
            emitter,
            mir,
            op,
            interner,
            layouts,
            mono_layouts,
            fn_name_by_def_id,
        ),
        Rvalue::BinaryOp(op, a, b) => {
            let a_val = codegen_operand(
                emitter,
                mir,
                a,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            let b_val = codegen_operand(
                emitter,
                mir,
                b,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            // Stage 18.109 (S10 fix): For Div/Rem, ensure operand values are
            // stored to their local allocas before the DivisionByZero assert
            // reads them. DCE may have removed the constant-assignment
            // statements (e.g., `local 4 = Use(Constant(4))`), so the alloca
            // is uninitialized when the assert does `load %loc_4`.
            //
            // Fix: if the operand is Copy(local), explicitly store the value
            // to the local's alloca here. This is idempotent (if already
            // stored, it just overwrites with the same value).
            //
            // Per §1.0 原則 4 "报错 > 静默": assert must read correct value.
            // Per §1.0 原則 6 "通用 > 特例": one fix for all Div/Rem operands.
            if matches!(op, BinOp::Div | BinOp::Rem) {
                store_operand_to_local(emitter, mir, a, &a_val, layouts, mono_layouts);
                store_operand_to_local(emitter, mir, b, &b_val, layouts, mono_layouts);
            }
            let ty = detect_operand_type(mir, a, layouts)
                .or(detect_operand_type(mir, b, layouts))
                .unwrap_or(EmitType::I32);

            // Stage 3.49 (L13 closure): fat pointers (`{ ptr, len }`) cannot
            // be compared with a single `icmp` — LLVM icmp doesn't support
            // aggregate types. For `==`/`!=`, we compare both fields:
            //   eq = (a.ptr == b.ptr) & (a.len == b.len)
            //   ne = (a.ptr != b.ptr) | (a.len != b.len)
            //
            // Stage 14.69: For &str (fat pointer with i8 pointee), use the
            // `__landin_str_eq` runtime function for CONTENT comparison.
            // Previously, this was a bitwise comparison (pointer + length),
            // which only worked for deduplicated string globals. For different
            // allocations of the same content (e.g., function parameter vs.
            // literal in function body), bitwise comparison returned false.
            //
            // For &[T] (fat pointer with non-i8 pointee), keep the bitwise
            // comparison (content comparison for arbitrary types requires
            // element-wise comparison, deferred to a future stage).
            //
            // Per §1.0 原则 5 "报错 > 静默": string equality now correctly
            // compares content, not just pointers.
            let (is_fat_ptr, ptr_field_ty) = match &ty {
                EmitType::Struct(fields) if fields.len() == 2 => {
                    let is_fp = fields[0].is_ptr() && fields[1] == EmitType::I64;
                    (is_fp, fields[0].clone())
                }
                _ => (false, EmitType::I32),
            };
            // Stage 14.69: Check if this is a &str (fat pointer to i8).
            // If so, use __landin_str_eq for content comparison.
            let is_str_fat_ptr = is_fat_ptr
                && matches!(ptr_field_ty, EmitType::Ptr(ref inner) if **inner == EmitType::I8);

            match op {
                BinOp::Eq => {
                    if is_str_fat_ptr {
                        // Stage 14.69: Use __landin_str_eq for content comparison.
                        // Call: __landin_str_eq(a.ptr, a.len, b.ptr, b.len) → i32 (0 or 1)
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let args: [(EmitType, &EmitValue); 4] = [
                            (EmitType::OpaquePtr, &a_ptr),
                            (EmitType::I64, &a_len),
                            (EmitType::OpaquePtr, &b_ptr),
                            (EmitType::I64, &b_len),
                        ];
                        // __landin_str_eq returns i32 (0 or 1) — already the
                        // correct type for the comparison result (zext'd i1 → i32).
                        // Skip the emit_zext below by returning early.
                        return emitter.emit_call("__landin_str_eq", &args, &EmitType::I32);
                    }
                    let cmp = if is_fat_ptr {
                        // Extract ptr (field 0) and len (field 1) from both,
                        // compare each, AND the results.
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let ptr_eq = emitter.emit_icmp("eq", &ptr_field_ty, &a_ptr, &b_ptr);
                        let len_eq = emitter.emit_icmp("eq", &EmitType::I64, &a_len, &b_len);
                        emitter.emit_and(&EmitType::I1, &ptr_eq, &len_eq)
                    } else if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oeq", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("eq", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Ne => {
                    if is_str_fat_ptr {
                        // Stage 14.69: Use __landin_str_eq, then NOT the result.
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let args: [(EmitType, &EmitValue); 4] = [
                            (EmitType::OpaquePtr, &a_ptr),
                            (EmitType::I64, &a_len),
                            (EmitType::OpaquePtr, &b_ptr),
                            (EmitType::I64, &b_len),
                        ];
                        let eq_result = emitter.emit_call("__landin_str_eq", &args, &EmitType::I32);
                        // NOT the result: ne = (eq == 0) ? 1 : 0
                        // eq_result is i32 (0 or 1). ne = 1 - eq_result.
                        // Use icmp eq with 0, then zext to i32.
                        let zero = "0".to_string();
                        let ne_i1 = emitter.emit_icmp("eq", &EmitType::I32, &eq_result, &zero);
                        return emitter.emit_zext(&EmitType::I1, &EmitType::I32, &ne_i1);
                    }
                    let cmp = if is_fat_ptr {
                        let a_ptr = emitter.emit_extractvalue(&ty, &a_val, 0);
                        let a_len = emitter.emit_extractvalue(&ty, &a_val, 1);
                        let b_ptr = emitter.emit_extractvalue(&ty, &b_val, 0);
                        let b_len = emitter.emit_extractvalue(&ty, &b_val, 1);
                        let ptr_ne = emitter.emit_icmp("ne", &ptr_field_ty, &a_ptr, &b_ptr);
                        let len_ne = emitter.emit_icmp("ne", &EmitType::I64, &a_len, &b_len);
                        emitter.emit_or(&EmitType::I1, &ptr_ne, &len_ne)
                    } else if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("one", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("ne", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Lt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("olt", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("slt", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Le => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ole", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sle", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Gt => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("ogt", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sgt", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::Ge => {
                    let cmp = if ty == EmitType::F64 || ty == EmitType::F32 {
                        emitter.emit_fcmp("oge", &ty, &a_val, &b_val)
                    } else {
                        emitter.emit_icmp("sge", &ty, &a_val, &b_val)
                    };
                    emitter.emit_zext(&EmitType::I1, &EmitType::I32, &cmp)
                }
                BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor
                    if ty == EmitType::F64 || ty == EmitType::F32 =>
                {
                    let int_ty = if ty == EmitType::F64 {
                        EmitType::I64
                    } else {
                        EmitType::I32
                    };
                    let a_int = emitter.emit_cast(&ty, &int_ty, &a_val);
                    let b_int = emitter.emit_cast(&ty, &int_ty, &b_val);
                    let result_int = emitter.emit_binop(*op, &int_ty, &a_int, &b_int);
                    emitter.emit_cast(&int_ty, &ty, &result_int)
                }
                _ => emitter.emit_binop(*op, &ty, &a_val, &b_val),
            }
        }
        Rvalue::UnaryOp(op, operand) => {
            let val = codegen_operand(
                emitter,
                mir,
                operand,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            let ty = detect_operand_type(mir, operand, layouts).unwrap_or(EmitType::I32);
            emitter.emit_unop(*op, &ty, &val)
        }
        Rvalue::Ref(_, _borrow_kind, lv) => {
            if let PlaceKind::Local(id) = &lv.kind {
                if let Some(ptr) = emitter.local_ptr(id.0).cloned() {
                    return ptr;
                }
            }
            "0".to_string()
        }
        Rvalue::Aggregate(AggregateKind::Tuple, operands) => {
            if operands.is_empty() {
                "0".to_string()
            } else if operands.len() == 1 {
                codegen_operand(
                    emitter,
                    mir,
                    &operands[0],
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                )
            } else {
                let field_tys: Vec<EmitType> = operands
                    .iter()
                    .map(|op| detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32))
                    .collect();
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(
                        emitter,
                        mir,
                        op,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                    );
                    let val_ty = &field_tys[i];
                    agg = emitter.emit_insertvalue(&agg_ty, &agg, val_ty, &val, i as u32);
                }
                agg
            }
        }
        Rvalue::Aggregate(AggregateKind::Array(elem_ty), operands) => {
            if operands.is_empty() {
                return "0".to_string();
            }
            // Stage 14.44: Use mir_type_to_emit_type_WITH_LAYOUTS (not the
            // legacy mir_type_to_emit_type which doesn't know about AdtLayouts).
            // For arrays of structs, the legacy function returns I32 (fallback),
            // causing the array type to be [N x i32] instead of [N x { i32, i32 }].
            // This made insertvalue insert a struct value into an i32 array →
            // invalid IR + empty object file (silent failure).
            //
            // Stage 14.44b: If elem_ty is Infer (MIR lower uses fresh_infer_ty
            // because typeck doesn't fully propagate element types), fall back
            // to detecting the type from the first operand. This handles
            // arrays of structs like `[Point { x: 1, y: 2 }, Point { x: 3, y: 4 }]`
            // where elem_ty is Infer but the operands are Adt values.
            let elem_emit_ty = {
                let from_elem_ty =
                    mir_type_to_emit_type_with_layouts_and_mono(elem_ty, layouts, mono_layouts);
                if matches!(from_elem_ty, EmitType::I32) && !operands.is_empty() {
                    // elem_ty might be Infer → try detecting from first operand
                    if let Some(detected) = detect_operand_type(mir, &operands[0], layouts) {
                        detected
                    } else {
                        from_elem_ty
                    }
                } else {
                    from_elem_ty
                }
            };
            let n = operands.len() as u64;
            let agg_ty = EmitType::array_of(elem_emit_ty.clone(), n);
            let mut agg = "undef".to_string();
            for (i, op) in operands.iter().enumerate() {
                let val = codegen_operand(
                    emitter,
                    mir,
                    op,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                );
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &elem_emit_ty, &val, i as u32);
            }
            agg
        }
        Rvalue::Aggregate(AggregateKind::Adt(def_id, variant, _substs, field_tys), operands) => {
            if operands.is_empty() {
                return "0".to_string();
            }
            // Stage 3.48 (L-ENUM-UNION): for enum variants, compute the
            // correct starting field_idx in the flat storage layout.
            // The storage is `{ discr, variant_0_fields..., variant_1_fields..., ... }`
            // (flattened — unit variants contribute no fields). The starting
            // field_idx for variant V's payload = 1 + sum(field_counts of
            // variants 0..V-1) — but only counting this variant's own fields
            // starting from that offset. (See `mir_type_to_emit_type_with_layouts`
            // for the layout definition.)
            //
            // For struct (AdtLayout::Struct), variant_idx is always 0 and the
            // storage is just the struct's fields (no discriminant). The legacy
            // path (no AdtLayout) also treats it as a flat struct.
            use crate::mir::body::AdtLayout;

            let layout = layouts.get(def_id);
            let is_enum = matches!(layout, Some(AdtLayout::Enum { .. }));

            if is_enum {
                // Enum variant construction.
                // Look up the full storage type from the Adt layout.
                let storage_ty = mir_type_to_emit_type_with_layouts_and_mono(
                    &crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Adt(
                            *def_id,
                            Vec::<crate::mir::ty::Ty>::new().into(),
                        ),
                        crate::session::Span::DUMMY,
                    ),
                    layouts,
                    mono_layouts,
                );
                // Compute the starting field_idx for this variant's payload.
                // = 1 (for discriminant) + sum(field_counts of variants 0..V-1)
                let variant_idx = *variant;
                let starting_field_idx = if let Some(AdtLayout::Enum {
                    variant_payloads, ..
                }) = layout
                {
                    let mut idx = 1u32; // skip discriminant
                    for (i, payload) in variant_payloads.iter().enumerate() {
                        if i as u32 >= variant_idx {
                            break;
                        }
                        idx += payload.len() as u32;
                    }
                    idx
                } else {
                    1 // fallback (shouldn't reach here for enum)
                };

                let mut agg = "undef".to_string();
                // Operand 0 is always the discriminant (prepended by MIR lower
                // for enum variants — see `lower_expr_to_operand`'s Call path).
                // Insert it at field 0 of the storage.
                let discr_op = &operands[0];
                let discr_val = codegen_operand(
                    emitter,
                    mir,
                    discr_op,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                );
                let discr_ty = detect_operand_type(mir, discr_op, layouts).unwrap_or(EmitType::I32);
                agg = emitter.emit_insertvalue(&storage_ty, &agg, &discr_ty, &discr_val, 0);

                // Remaining operands are the variant's payload fields, inserted
                // starting at `starting_field_idx`.
                // `field_tys` from AggregateKind includes the discriminant as
                // element 0 (per `resolve_enum_variant`), so payload field i
                // is at `field_tys[i+1]`.
                for (i, op) in operands.iter().enumerate().skip(1) {
                    let val = codegen_operand(
                        emitter,
                        mir,
                        op,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                    );
                    // field_tys[i] is this operand's type (field_tys[0]=discr,
                    // field_tys[1]=payload_field_0, ...).
                    let val_ty = field_tys
                        .get(i)
                        .map(mir_type_to_emit_type)
                        .unwrap_or_else(|| {
                            detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32)
                        });
                    let target_idx = starting_field_idx + (i as u32 - 1);
                    agg = emitter.emit_insertvalue(&storage_ty, &agg, &val_ty, &val, target_idx);
                }
                agg
            } else {
                // Struct construction (or test-context fallback without layout).
                // Legacy path: flat struct, operands at 0..N.
                // Stage 14.22: Use mir_type_to_emit_type_with_layouts (not
                // mir_type_to_emit_type) to correctly resolve nested Adt types.
                // Was: mir_type_to_emit_type returned I32 for Adt, causing
                // insertvalue to use wrong type for struct fields.
                let field_tys: Vec<EmitType> = if field_tys.is_empty() {
                    operands
                        .iter()
                        .map(|op| detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32))
                        .collect()
                } else {
                    field_tys
                        .iter()
                        .map(|t| {
                            mir_type_to_emit_type_with_layouts_and_mono(t, layouts, mono_layouts)
                        })
                        .collect()
                };
                let agg_ty = EmitType::Struct(field_tys.clone());
                let mut agg = "undef".to_string();
                for (i, op) in operands.iter().enumerate() {
                    let val = codegen_operand(
                        emitter,
                        mir,
                        op,
                        interner,
                        layouts,
                        mono_layouts,
                        fn_name_by_def_id,
                    );
                    let val_ty = field_tys.get(i).cloned().unwrap_or(EmitType::I32);
                    agg = emitter.emit_insertvalue(&agg_ty, &agg, &val_ty, &val, i as u32);
                }
                agg
            }
        }
        // Stage 13.3a (TD-030): Closure struct construction.
        // Aggregate(AggregateKind::Closure(def_id, substs), operands) constructs
        // a closure struct value with one field per capture. The `substs`
        // vector carries the capture field types (matching `TyKind::Closure`).
        // The `operands` vector carries the capture values (in field order).
        //
        // Codegen: emit the closure struct type as `{ field_tys... }`, then
        // `insertvalue` each capture operand at its field index. Mirrors the
        // `AggregateKind::Adt` struct path (lines 603-622 above).
        //
        // Per `07-codegen.md` §8.1: "每个闭包字面量生成一个唯一的匿名 struct" —
        // the closure struct is an anonymous struct with one field per capture.
        // The struct type is computed by `mir_type_to_emit_type` from
        // `TyKind::Closure(_, substs)` (see `emitter.rs:487-490`).
        Rvalue::Aggregate(AggregateKind::Closure(_def_id, substs), operands) => {
            if operands.is_empty() {
                // Empty closure (no captures) — emit an empty struct value.
                return "0".to_string();
            }
            // Build the closure struct type from the capture field types.
            //
            // Stage 14.82 (GAP-7 partial fix): use
            // `mir_type_to_emit_type_with_layouts` (NOT the legacy
            // `mir_type_to_emit_type`) so that `Adt(Point)` capture types
            // resolve to their actual LLVM struct type (e.g. `{ i32, i32 }`)
            // instead of falling back to `EmitType::I32`. Was: closures
            // capturing structs crashed LLVM verification with
            // `Invalid InsertValueInst operands!` because the closure
            // struct was typed `{ i32 }` but the operand was `{ i32, i32 }`.
            //
            // Per §1.0 原則 5 "报错 > 静默": the legacy fallback silently
            // produced wrong LLVM types, manifesting as a backend crash
            // instead of a clear compiler error. Using the layouts-aware
            // variant surfaces the correct type.
            let field_tys: Vec<EmitType> = substs
                .iter()
                .map(|ty| mir_type_to_emit_type_with_layouts_and_mono(ty, layouts, mono_layouts))
                .collect();
            let agg_ty = EmitType::Struct(field_tys.clone());
            let mut agg = "undef".to_string();
            for (i, op) in operands.iter().enumerate() {
                let val = codegen_operand(
                    emitter,
                    mir,
                    op,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                );
                let val_ty = field_tys.get(i).cloned().unwrap_or(EmitType::I32);
                agg = emitter.emit_insertvalue(&agg_ty, &agg, &val_ty, &val, i as u32);
            }
            agg
        }
        Rvalue::Cast(_, op, target_ty) => {
            let val = codegen_operand(
                emitter,
                mir,
                op,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            let src_ty = detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32);
            let dst_ty = mir_type_to_emit_type(target_ty);
            emitter.emit_cast(&src_ty, &dst_ty, &val)
        }
        // Stage 14.103 (SH-7 fix): BinaryOp2 is used for Range expressions
        // (start..end). For v0.1, ranges are only used in for-loop iterators
        // and are desugared before codegen — they should never reach here.
        //
        // Stage 18.75 P0-5: Previously the catch-all silently returned "0",
        // producing wrong code without any warning. Now we emit a visible
        // error message to stderr so the user knows something went wrong.
        // Per §1.0 原则 4 "报错 > 静默": never silently produce wrong code.
        //
        // TODO (v0.2): Make codegen return CodegenResult<String> so this
        // can propagate as a proper CodegenError through CompileErrors.codegen.
        Rvalue::BinaryOp2(_, _, _) => {
            eprintln!(
                "warning: range expression reached codegen (should have been desugared) — \
                 producing fallback value 0"
            );
            "0".to_string()
        }
    }
}

/// Stage 18.109 (S10 fix): Store an operand's value to its local alloca.
///
/// If the operand is `Copy(Place::local(id))` or `Move(Place::local(id))`,
/// this function stores `val` to the local's alloca pointer. This ensures
/// that subsequent loads (e.g., DivisionByZero assert) read the correct value
/// even when DCE has removed the original constant-assignment statement.
///
/// Per §23: `store_operand_to_local` follows `<verb>_<noun>_<prep>_<noun>` pattern.
/// Per §16: codegen-internal helper, no cross-stage access.
fn store_operand_to_local(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    operand: &Operand,
    val: &EmitValue,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
) {
    use crate::codegen::mir_translation::types::mir_type_to_emit_type_with_layouts_and_mono;
    use crate::mir::place::{Operand, PlaceKind};
    if let Operand::Copy(place) | Operand::Move(place) = operand {
        if let PlaceKind::Local(id) = &place.kind {
            // Get the local's type.
            let default_ty = crate::mir::ty::Ty::new(
                crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
                crate::session::Span::DUMMY,
            );
            let local_ty = mir
                .local_decls
                .get(id.0 as usize)
                .map(|ld| &ld.ty)
                .unwrap_or(&default_ty);
            let emit_ty =
                mir_type_to_emit_type_with_layouts_and_mono(local_ty, layouts, mono_layouts);
            // Store the value to the local's alloca.
            if emit_ty != EmitType::Void {
                if let Some(ptr) = emitter.local_ptr(id.0).cloned() {
                    emitter.emit_store(&emit_ty, val, &ptr);
                }
            }
            // Also update the emitter's cached local value.
            emitter.set_local(id.0, val.clone());
        }
    }
}
