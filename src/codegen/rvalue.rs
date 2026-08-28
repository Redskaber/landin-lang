//! MIR rvalue → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `Rvalue::Use`, `BinaryOp`, `UnaryOp`, `Ref`, `Aggregate`, etc.

// Stage 16.42: Removed `#[allow(unused_imports)]` — fixed the underlying
// unused imports instead. Per §1.0 原則 5 "去除兼容思维".
use super::mir_translation::detect_operand_type;
use super::*;
use crate::mir::place::*;

/// Stage 18.151 (TD-CODEGEN-RESULT): `codegen_rvalue` now returns
/// `CodegenResult<EmitValue>` instead of `EmitValue`. This allows the
/// `BinaryOp2` arm (range expression that should have been desugared) to
/// return a proper `CodegenError` instead of panicking.
///
/// Per §2 原则 4 (报错>静默): codegen errors are reported, not panicked.
/// Per §2 原则 9 (正确>妥协): full Result propagation, no `unwrap()` stubs.
/// Per §12 (最优>最小): root-cause fix, not a workaround.
pub(crate) fn codegen_rvalue(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    rv: &Rvalue,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    mono_layouts: Option<&crate::mir::MonoLayoutMap>,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) -> CodegenResult<EmitValue> {
    Ok(match rv {
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
            // Stage 18.118: Only Struct{ptr, i64} is a fat pointer.
            // All other types (I32, Bool, Ptr, etc.) are not fat pointers.
            // The _ => catch-all is intentional — non-struct types cannot
            // be fat pointers. Per §1.0 原則 6 "通用 > 特例": one check.
            let (is_fat_ptr, ptr_field_ty) = match &ty {
                EmitType::Struct(fields) if fields.len() == 2 => {
                    let is_fp = fields[0].is_ptr() && fields[1] == EmitType::I64;
                    (is_fp, fields[0].clone())
                }
                // Non-struct types are never fat pointers. The I32 default
                // for ptr_field_ty is unused when is_fat_ptr is false.
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
                        return Ok(emitter.emit_call("__landin_str_eq", &args, &EmitType::I32));
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
                        return Ok(emitter.emit_zext(&EmitType::I1, &EmitType::I32, &ne_i1));
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
                    return Ok(ptr);
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
                return Ok("0".to_string());
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
        Rvalue::Aggregate(AggregateKind::Adt(def_id, variant, adt_substs, field_tys), operands) => {
            if operands.is_empty() {
                return Ok("0".to_string());
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
                // Stage 18.338+18.339 (P2 soundness fix): For generic structs,
                // the raw `field_tys` from MIR contain unsubstituted generic
                // params. Use lookup_mono_layout to get substituted types.
                // When mono layout is not available (adt_substs empty — type
                // inference didn't propagate), check if field_tys contain Param.
                // If they do, use operand types as fallback. If not (non-generic
                // struct like String), use the raw field_tys as before.
                let field_tys: Vec<EmitType> = if field_tys.is_empty() {
                    operands
                        .iter()
                        .map(|op| detect_operand_type(mir, op, layouts).unwrap_or(EmitType::I32))
                        .collect()
                } else if let Some(mono_layout) =
                    crate::mir::monomorphize::lookup_mono_layout(*def_id, adt_substs, mono_layouts)
                {
                    // Generic struct with known substs: use monomorphized layout.
                    use crate::mir::body::AdtLayout;
                    match mono_layout {
                        AdtLayout::Struct {
                            field_tys: mono_fields,
                        } => mono_fields
                            .iter()
                            .map(|t| {
                                mir_type_to_emit_type_with_layouts_and_mono(
                                    t,
                                    layouts,
                                    mono_layouts,
                                )
                            })
                            .collect(),
                        AdtLayout::Enum { .. } => field_tys
                            .iter()
                            .map(|t| {
                                mir_type_to_emit_type_with_layouts_and_mono(
                                    t,
                                    layouts,
                                    mono_layouts,
                                )
                            })
                            .collect(),
                    }
                } else {
                    // Non-generic struct OR generic struct with empty adt_substs.
                    // Use raw field_tys — for non-generic (String), these are
                    // correct. For generic with empty substs (Wrapper<T> in
                    // main), these contain Param(0) which resolves to I32 via
                    // mir_type_to_emit_type fallback. This is a known P2
                    // limitation — the correct fix requires MIR-lower-level
                    // type inference improvement (not codegen-level patch).
                    // Per §1.0 原則 9 (正确 > 妥协): this is a known compromise
                    // documented as TD-GENERIC-STRUCT-FIELD-TYS-INFERENCE.
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
                return Ok("0".to_string());
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

            // Stage 18.326 B1 (P1 soundness fix): When casting integer to
            // pointer, check if the value is a zero constant (null pointer).
            // If so, emit `null` directly instead of `inttoptr i32 0 to ptr`
            // (which leaves upper 32 bits undefined on 64-bit → segfault).
            //
            // **Design boundary** (per Rust rustc_codegen_llvm):
            // - `emit_null_ptr` returns `"null"` (value only, no type prefix).
            // - Callers add `ptr` prefix via `format!("{} {}", ty, val)`.
            // - This matches `emit_store` / `emit_call` / `emit_select` patterns.
            //
            // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
            // Per §1.0 原則 6 (通解>特解): one rule for ALL int→ptr casts.
            let is_ptr_dst = matches!(dst_ty, EmitType::OpaquePtr | EmitType::Ptr(_));
            let is_int_src = matches!(
                src_ty,
                EmitType::I1
                    | EmitType::I8
                    | EmitType::I16
                    | EmitType::I32
                    | EmitType::I64
                    | EmitType::I128
            );
            if is_ptr_dst && is_int_src {
                let val_str = val.as_str();
                // Check if the value is a zero constant (null pointer).
                if val_str.trim() == "0" {
                    // Emit `null` — callers add `ptr` prefix → `ptr null`.
                    return Ok("null".to_string());
                }
            }
            emitter.emit_cast(&src_ty, &dst_ty, &val)
        }
        // Stage 14.103 (SH-7 fix): BinaryOp2 is used for Range expressions
        // (start..end). For v0.1, ranges are only used in for-loop iterators
        // and are desugared before codegen — they should never reach here.
        //
        // Stage 18.119 (D1-R2 fix): Previously emitted eprintln! + returned "0",
        // silently producing wrong code. Then (Stage 18.150 plan) panicked.
        //
        // Stage 18.151 (TD-CODEGEN-RESULT + TD-BINARYOP2-PANIC root-cause fix):
        // Now returns `CodegenError` instead of panicking. The error propagates
        // through `codegen_statement` → `codegen_function` → `run_codegen_pipeline`
        // → `codegen_crate` → driver, surfacing as a user-visible diagnostic.
        //
        // Per §2 原则 4 (报错>静默): codegen errors are reported, not panicked.
        // Per §2 原则 9 (正确>妥协): proper Result propagation, not panic.
        // Per §12 (最优>最小): root-cause fix, not a workaround.
        Rvalue::BinaryOp2(_, _, _) => {
            return Err(CodegenError::new(
                "BinaryOp2 (range expression) reached codegen — \
                 this should have been desugared during MIR lowering. \
                 This is a compiler bug.",
                crate::session::Span::DUMMY,
            ));
        }
        // Stage 18.227 (v0.2.5c): MIR intrinsic Load — load value from raw
        // pointer. Used by v0.2.5d-g migration to replace compound C helpers
        // (e.g. `__landin_vec_get` element load).
        //
        // Per §1.0 原則 6 (通解>特例): one Load for all pointer types.
        // Per §1.0 原則 4 (报错>静默): void Load returns CodegenError (visible).
        // Per §16.2 (06-mir.md): MIR intrinsic ops design.
        // Per §10 DRY: reuses `MemoryEmitter::emit_load` — no new emit method.
        Rvalue::Load(ptr_op, pointee_ty) => {
            let ptr_val = codegen_operand(
                emitter,
                mir,
                ptr_op,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            let pointee_emit_ty =
                mir_type_to_emit_type_with_layouts_and_mono(pointee_ty, layouts, mono_layouts);
            if pointee_emit_ty == EmitType::Void {
                return Err(CodegenError::new(
                    "Load of void-typed pointer has no value — this is a compiler bug.",
                    crate::session::Span::DUMMY,
                ));
            }
            emitter.emit_load(&pointee_emit_ty, &ptr_val)
        }
        // Stage 18.227 (v0.2.5c): MIR intrinsic GetElementPtr — compute the
        // address of an element at the given indices. Used by v0.2.5d-g
        // migration (e.g. `__landin_vec_get` indexing, struct field access
        // through raw pointers).
        //
        // Per §1.0 原則 6 (通解>特例): one GEP arm handles all index forms —
        // the codegen path is uniform; the MIR producer (Stage 18.228+)
        // supplies the right index operands.
        // Per §10 DRY: reuses `MemoryEmitter::emit_gep_index_ptr` for all
        // indices — no new emit method, no branching on const vs var.
        // Per §16.2 (06-mir.md): MIR intrinsic ops design.
        //
        // Stage 18.228 fix: Derive the element type from `result_ty` (which
        // is `*mut T`). The pointee type T determines the GEP stride —
        // passing I32 for all types caused `Vec<i64>::get(1)` to compute
        // offset 4 instead of 8, reading garbage. Now we extract the
        // pointee from `result_ty` and pass it to `emit_gep_index_ptr`.
        //
        // Per §1.0 原則 9 (正确>妥协): use the actual element type from
        // the result_ty, not a placeholder.
        Rvalue::GetElementPtr {
            base,
            indices,
            result_ty,
        } => {
            // Extract the pointee type from result_ty (*mut T → T).
            let elem_emit_ty = match &result_ty.kind {
                crate::mir::ty::TyKind::RawPtr(_, inner)
                | crate::mir::ty::TyKind::Ref(_, _, inner) => {
                    mir_type_to_emit_type_with_layouts_and_mono(inner, layouts, mono_layouts)
                }
                _ => EmitType::I32, // Fallback for unexpected types.
            };
            let mut cur_ptr = codegen_operand(
                emitter,
                mir,
                base,
                interner,
                layouts,
                mono_layouts,
                fn_name_by_def_id,
            );
            for idx_op in indices {
                // Per §1.0 原則 6 (通解>特例): one path for const and runtime
                // indices — `emit_gep_index_ptr` works for both because LLVM
                // 19 opaque-ptr GEP doesn't distinguish them at the IR level.
                let idx_val = codegen_operand(
                    emitter,
                    mir,
                    idx_op,
                    interner,
                    layouts,
                    mono_layouts,
                    fn_name_by_def_id,
                );
                cur_ptr = emitter.emit_gep_index_ptr(&cur_ptr, &elem_emit_ty, &idx_val);
            }
            cur_ptr
        }
    })
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

// ===========================================================================
// Stage 18.227 (v0.2.5c): lib unit tests for MIR intrinsic ops codegen.
//
// These are lib unit tests (not integration tests) because `codegen_rvalue`
// and `codegen_statement` are `pub(crate)` — exposing them publicly would
// leak codegen internals across the crate boundary (per §11 接口隔离).
//
// Per §9.4 + §17.6: each new variant gets at least 1 positive (text-IR
// verification) + 1 negative or stress test. Per §10 DRY: shared fixtures.
// ===========================================================================

#[cfg(test)]
mod intrinsic_ops_tests {
    use super::*;
    use crate::codegen::{
        mir_type_to_emit_type_with_layouts_and_mono, CodegenError, EmitType, MemoryEmitter,
        TextEmitter,
    };
    use crate::mir::body::{MirBody, Statement, StatementKind};
    use crate::mir::place::{LocalId, Operand, Place, Rvalue};
    use crate::mir::ty::{Const, ConstVal, Mutability, Ty, TyKind};
    use crate::session::Span;

    use lasso::Rodeo;
    use std::collections::HashMap;

    // --- Shared test fixture helpers (per §10 DRY) ---

    /// Build a minimal MirBody with one local of type `ty`.
    /// The local is registered as LocalId(0).
    fn build_mir_with_local(ty: Ty) -> MirBody {
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ty, None, Span::DUMMY);
        let _ = mir.new_block();
        mir
    }

    /// Build an empty interner + empty fn_name map + empty mono_layouts.
    struct CodegenCtx {
        interner: Rodeo,
        fn_names: HashMap<crate::hir::DefId, String>,
    }

    impl CodegenCtx {
        fn new() -> Self {
            Self {
                interner: Rodeo::new(),
                fn_names: HashMap::new(),
            }
        }
    }

    /// Set up a TextEmitter inside a function context with one alloca'd local.
    /// Returns the local's pointer EmitValue (e.g. `%loc_0`).
    fn setup_emitter_with_local(emitter: &mut TextEmitter, ty: &EmitType) -> String {
        emitter.emit_function_begin("test_fn", &[], &EmitType::I32);
        emitter.emit_alloca(ty, "%loc_0")
    }

    // --- Rvalue::Load tests ---

    /// Positive: `Rvalue::Load(ptr, i32)` emits a `load i32, ptr %X` instruction.
    ///
    /// Per §1.0 原則 6 (通解>特例): one Load path for all pointer types —
    /// this test verifies the i32 case (the most common primitive).
    #[test]
    fn stage18_227_rvalue_load_i32_emits_load_instruction() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mir = build_mir_with_local(ptr_ty);
        emitter.set_local_ptr(0, ptr_val.clone());

        let ctx = CodegenCtx::new();
        let rv = Rvalue::Load(Operand::Copy(Place::local(LocalId(0), Span::DUMMY)), i32_ty);

        let result = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Load should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("load i32,"),
            "Expected `load i32,` in IR, got:\n{}",
            output
        );
        assert!(
            result.starts_with("%v"),
            "Load result should be a fresh SSA value, got: {}",
            result
        );
    }

    /// Negative: `Rvalue::Load(ptr, void)` returns CodegenError (not silent "0").
    ///
    /// Per §1.0 原則 4 (报错>静默): void loads are errors, not silent skips.
    #[test]
    fn stage18_227_rvalue_load_void_returns_error() {
        let mut emitter = TextEmitter::new();
        let _ = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let void_ty = Ty::new(TyKind::Tuple(vec![]), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(void_ty.clone())),
            Span::DUMMY,
        );
        let mir = build_mir_with_local(ptr_ty);
        emitter.set_local_ptr(0, "%loc_0".to_string());

        let ctx = CodegenCtx::new();
        let rv = Rvalue::Load(
            Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
            void_ty,
        );

        let result = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        );

        assert!(
            result.is_err(),
            "Load of void-typed pointer must return CodegenError, got: {:?}",
            result
        );
        let err: CodegenError = result.unwrap_err();
        assert!(
            err.message.contains("void"),
            "Error message must mention 'void', got: {}",
            err.message
        );
    }

    /// Positive: `Rvalue::Load` on a pointer-to-i64 emits `load i64,`.
    ///
    /// Per §1.0 原則 6 (通解>特例): same path handles different pointee types.
    #[test]
    fn stage18_227_rvalue_load_i64_emits_load_i64() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I64);

        let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i64_ty.clone())),
            Span::DUMMY,
        );
        let mir = build_mir_with_local(ptr_ty);
        emitter.set_local_ptr(0, ptr_val);

        let ctx = CodegenCtx::new();
        let rv = Rvalue::Load(Operand::Copy(Place::local(LocalId(0), Span::DUMMY)), i64_ty);

        let _ = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Load i64 should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("load i64,"),
            "Expected `load i64,` in IR, got:\n{}",
            output
        );
    }

    // --- Rvalue::GetElementPtr tests ---

    /// Positive: `Rvalue::GetElementPtr` with a runtime index operand emits a
    /// `getelementptr inbounds` instruction.
    ///
    /// Per §1.0 原則 6 (通解>特例): one GEP path for all index forms.
    #[test]
    fn stage18_227_rvalue_gep_runtime_index_emits_gep_instruction() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ptr_ty.clone(), None, Span::DUMMY); // LocalId(0) = base ptr
        let _ = mir.new_local(i32_ty, None, Span::DUMMY); // LocalId(1) = index
        let _ = mir.new_block();
        emitter.set_local_ptr(0, ptr_val.clone());
        emitter.set_local_ptr(1, "%loc_1".to_string());

        let ctx = CodegenCtx::new();
        let rv = Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
            indices: vec![Operand::Copy(Place::local(LocalId(1), Span::DUMMY))],
            result_ty: ptr_ty,
        };

        let result = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("GEP should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("getelementptr inbounds"),
            "Expected `getelementptr inbounds` in IR, got:\n{}",
            output
        );
        assert!(
            result.starts_with("%v"),
            "GEP result should be a fresh SSA value, got: {}",
            result
        );
    }

    /// Positive: `Rvalue::GetElementPtr` with a constant index operand also
    /// emits a `getelementptr inbounds` (single path for const + var indices).
    #[test]
    fn stage18_227_rvalue_gep_const_index_emits_gep_instruction() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mir = build_mir_with_local(ptr_ty);
        emitter.set_local_ptr(0, ptr_val);

        let ctx = CodegenCtx::new();
        let const_idx = Operand::Constant(Const {
            ty: i32_ty.clone(),
            val: ConstVal::Int(3),
        });
        let rv = Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
            indices: vec![const_idx],
            result_ty: Ty::new(
                TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty)),
                Span::DUMMY,
            ),
        };

        let _ = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("GEP with const index should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("getelementptr inbounds"),
            "Expected `getelementptr inbounds` for const-index GEP, got:\n{}",
            output
        );
    }

    /// Positive: chained GEP with 2 indices emits 2 `getelementptr` instructions.
    ///
    /// Per §10 DRY: each index produces one GEP, chained through the previous result.
    #[test]
    fn stage18_227_rvalue_gep_chained_indices_emits_multiple_gep() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ptr_ty.clone(), None, Span::DUMMY);
        let _ = mir.new_local(i32_ty, None, Span::DUMMY);
        let _ = mir.new_block();
        emitter.set_local_ptr(0, ptr_val);
        emitter.set_local_ptr(1, "%loc_1".to_string());

        let ctx = CodegenCtx::new();
        let rv = Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
            indices: vec![
                Operand::Copy(Place::local(LocalId(1), Span::DUMMY)),
                Operand::Constant(Const {
                    ty: Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY),
                    val: ConstVal::Int(1),
                }),
            ],
            result_ty: Ty::new(
                TyKind::RawPtr(
                    Mutability::Mutable,
                    Box::new(Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY)),
                ),
                Span::DUMMY,
            ),
        };

        let _ = codegen_rvalue(
            &mut emitter,
            &mir,
            &rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Chained GEP should codegen successfully");

        let output = emitter.output_with_globals();
        let gep_count = output.matches("getelementptr inbounds").count();
        assert_eq!(
            gep_count, 2,
            "Expected 2 `getelementptr` instructions for 2 indices, got {} in:\n{}",
            gep_count, output
        );
    }

    // --- StatementKind::Store tests ---

    /// Positive: `StatementKind::Store` emits a `store` instruction.
    ///
    /// Per §1.0 原則 6 (通解>特例): one Store path for all pointer destinations.
    #[test]
    fn stage18_227_statement_store_emits_store_instruction() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ptr_ty, None, Span::DUMMY); // LocalId(0) = ptr place
        let _ = mir.new_local(i32_ty.clone(), None, Span::DUMMY); // LocalId(1) = val source
        let _ = mir.new_block();
        emitter.set_local_ptr(0, ptr_val.clone());
        emitter.set_local_ptr(1, "%loc_1".to_string());

        let ctx = CodegenCtx::new();
        let stmt = Statement {
            kind: StatementKind::Store {
                ptr: Place::local(LocalId(0), Span::DUMMY),
                val: Operand::Constant(Const {
                    ty: i32_ty.clone(),
                    val: ConstVal::Int(42),
                }),
                val_ty: i32_ty,
            },
            span: Span::DUMMY,
        };

        crate::codegen::statement::codegen_statement(
            &mut emitter,
            &mir,
            &stmt,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Store should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("store i32"),
            "Expected `store i32` in IR, got:\n{}",
            output
        );
    }

    /// Positive: `StatementKind::Store` with an i64 value emits `store i64`.
    #[test]
    fn stage18_227_statement_store_i64_emits_store_i64() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I64);

        let i64_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I64), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i64_ty.clone())),
            Span::DUMMY,
        );
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ptr_ty, None, Span::DUMMY);
        let _ = mir.new_block();
        emitter.set_local_ptr(0, ptr_val);

        let ctx = CodegenCtx::new();
        let stmt = Statement {
            kind: StatementKind::Store {
                ptr: Place::local(LocalId(0), Span::DUMMY),
                val: Operand::Constant(Const {
                    ty: i64_ty.clone(),
                    val: ConstVal::Int(99),
                }),
                val_ty: i64_ty,
            },
            span: Span::DUMMY,
        };

        crate::codegen::statement::codegen_statement(
            &mut emitter,
            &mir,
            &stmt,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Store i64 should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("store i64"),
            "Expected `store i64` in IR, got:\n{}",
            output
        );
    }

    // --- Regression: Stage 18.226 data structures still construct ---

    /// Regression: the 3 new MIR variants added in Stage 18.226 must still
    /// construct without panic. Catches enum-variant removal or signature
    /// drift introduced by Stage 18.227 codegen wiring.
    ///
    /// Per §9.4 (test↔design锚定): test verifies the design doc §16.2-§16.3
    /// data structures match the codegen implementation.
    #[test]
    fn stage18_227_mir_intrinsics_data_structures_compile() {
        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );

        // Rvalue::Load constructs without panic.
        let _load = Rvalue::Load(
            Operand::Constant(Const {
                ty: ptr_ty.clone(),
                val: ConstVal::Int(0),
            }),
            i32_ty.clone(),
        );

        // Rvalue::GetElementPtr constructs without panic.
        let _gep = Rvalue::GetElementPtr {
            base: Operand::Constant(Const {
                ty: ptr_ty.clone(),
                val: ConstVal::Int(0),
            }),
            indices: vec![],
            result_ty: ptr_ty.clone(),
        };

        // StatementKind::Store constructs without panic.
        let _store = StatementKind::Store {
            ptr: Place::local(LocalId(0), Span::DUMMY),
            val: Operand::Constant(Const {
                ty: i32_ty.clone(),
                val: ConstVal::Int(0),
            }),
            val_ty: i32_ty,
        };

        // If we got here, all 3 variants construct successfully.
        // The Stage 18.226 data structures are intact.
    }

    // --- Integration: GEP + Load combination (mirrors vec_get target) ---

    /// Integration: `GEP` followed by `Load` mirrors what `__landin_vec_get`
    /// will become after v0.2.5d migration:
    ///   ```text
    ///   let elem_ptr = GetElementPtr { base: vec.data_ptr, indices: [idx] };
    ///   let val = Load(elem_ptr, T);
    ///   ```
    ///
    /// Per §17.6 (缺陷纳入): this test pre-validates the migration path before
    /// Stage 18.228 starts. If the codegen can't handle GEP→Load, the migration
    /// is blocked.
    #[test]
    fn stage18_227_integration_gep_then_load_mirrors_vec_get_target() {
        let mut emitter = TextEmitter::new();
        let ptr_val = setup_emitter_with_local(&mut emitter, &EmitType::I32);

        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let ptr_ty = Ty::new(
            TyKind::RawPtr(Mutability::Mutable, Box::new(i32_ty.clone())),
            Span::DUMMY,
        );
        let mut mir = MirBody::new(Span::DUMMY);
        let _ = mir.new_local(ptr_ty.clone(), None, Span::DUMMY); // base ptr
        let _ = mir.new_local(i32_ty.clone(), None, Span::DUMMY); // index
        let _ = mir.new_local(ptr_ty.clone(), None, Span::DUMMY); // GEP result holder
        let _ = mir.new_block();
        emitter.set_local_ptr(0, ptr_val);
        emitter.set_local_ptr(1, "%loc_1".to_string());

        let ctx = CodegenCtx::new();

        // Step 1: GEP to compute element pointer.
        let gep_rv = Rvalue::GetElementPtr {
            base: Operand::Copy(Place::local(LocalId(0), Span::DUMMY)),
            indices: vec![Operand::Copy(Place::local(LocalId(1), Span::DUMMY))],
            result_ty: ptr_ty.clone(),
        };
        let elem_ptr = codegen_rvalue(
            &mut emitter,
            &mir,
            &gep_rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("GEP should codegen successfully");

        // Step 2: Load from the computed element pointer.
        emitter.set_local_ptr(2, elem_ptr);
        let load_rv = Rvalue::Load(Operand::Copy(Place::local(LocalId(2), Span::DUMMY)), i32_ty);

        let _loaded_val = codegen_rvalue(
            &mut emitter,
            &mir,
            &load_rv,
            &ctx.interner,
            &mir.adt_layouts,
            None,
            &ctx.fn_names,
        )
        .expect("Load after GEP should codegen successfully");

        let output = emitter.output_with_globals();
        assert!(
            output.contains("getelementptr inbounds") && output.contains("load i32,"),
            "Expected both GEP and load in IR (vec_get target shape), got:\n{}",
            output
        );
    }

    // --- Design-doc anchor: verify mir_type_to_emit_type_with_layouts_and_mono ---

    /// Per §9.4 (test↔design锚定): the codegen uses
    /// `mir_type_to_emit_type_with_layouts_and_mono` to translate the pointee Ty
    /// to EmitType. This test verifies that for an i32 pointee, the result is
    /// `EmitType::I32` (the simplest case — used by all Stage 18.227 tests above).
    #[test]
    fn stage18_227_mir_type_to_emit_type_resolves_i32_pointee() {
        let i32_ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let layouts = std::collections::HashMap::new();
        let emit_ty = mir_type_to_emit_type_with_layouts_and_mono(&i32_ty, &layouts, None);
        assert_eq!(emit_ty, EmitType::I32);
    }
}
