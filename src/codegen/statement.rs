//! MIR statement → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `StatementKind::Assign`, `StorageLive`, `StorageDead`, `Nop`,
//! `Deinit`, and `Println`.

#![allow(unused_imports)]
#[allow(unused_imports)]
use super::mir_translation::{
    codegen_place_load, codegen_place_load_typed, compute_place_address, detect_operand_type,
    detect_place_storage_type, detect_place_type, unwrap_fat_ptr_for_index,
};
use super::*;
use crate::mir::body::*;
use crate::mir::place::*;
use crate::mir::ty::ConstVal;
pub(crate) fn codegen_statement(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    stmt: &Statement,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
    fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,
) {
    match &stmt.kind {
        StatementKind::Assign(boxed) => {
            let (place, rvalue) = &**boxed;
            let mut val =
                codegen_rvalue(emitter, mir, rvalue, interner, layouts, fn_name_by_def_id);
            match &place.kind {
                PlaceKind::Local(id) => {
                    let default_ty = crate::mir::ty::Ty::new(
                        crate::mir::ty::TyKind::Int(crate::ast::IntTy::I32),
                        crate::session::Span::DUMMY,
                    );
                    let local_ty = mir
                        .local_decls
                        .get(id.0 as usize)
                        .map(|ld| &ld.ty)
                        .unwrap_or(&default_ty);
                    let ty = mir_type_to_emit_type_with_layouts(local_ty, layouts);
                    // Stage 14.64: Coerce comparison results to the local's type.
                    //
                    // Comparison ops (Eq/Ne/Lt/Le/Gt/Ge) in codegen_rvalue
                    // always zext the i1 result to i32. When the destination
                    // local is Bool (i1), storing i32 to i1 is a type mismatch
                    // that produces invalid LLVM IR (silently miscompiles at
                    // runtime — LLVM module verification doesn't catch it
                    // because the LLVMSysEmitter uses LLVMBuildStore which
                    // doesn't validate types as strictly as the textual IR
                    // verifier).
                    //
                    // Fix: when storing to an i1 local and the rvalue is a
                    // comparison, trunc the i32 value to i1 first.
                    //
                    // Per §1.0 原则 5 "报错 > 静默": this surfaces the type
                    // mismatch as a truncation rather than silently storing
                    // the wrong-sized value.
                    if ty == EmitType::I1 {
                        if let Rvalue::BinaryOp(op, _, _) = rvalue {
                            if matches!(
                                op,
                                BinOp::Eq
                                    | BinOp::Ne
                                    | BinOp::Lt
                                    | BinOp::Le
                                    | BinOp::Gt
                                    | BinOp::Ge
                            ) {
                                val = emitter.emit_cast(&EmitType::I32, &EmitType::I1, &val);
                            }
                        }
                    }
                    emitter.set_local(id.0, val.clone());
                    if ty != EmitType::Void {
                        if let Some(ptr) = emitter.get_local_ptr(id.0).cloned() {
                            emitter.emit_store(&ty, &val, &ptr);
                        }
                    }
                }
                PlaceKind::Projection(base, elem) => {
                    let ty = detect_operand_type(mir, &Operand::Copy(place.clone()), layouts)
                        .unwrap_or(EmitType::I32);
                    match elem {
                        ProjectionElem::Deref => {
                            // Stage 14.27: For `*ptr = val`, we need to load the
                            // POINTER from the base place (not the value it points to).
                            // Was: codegen_place_load loaded the pointed-to value
                            // (e.g. i32) instead of the pointer (e.g. i32*), causing
                            // `store i32 20, i32 %v2` (storing to a non-pointer).
                            // Fix: use codegen_place_load_typed with the pointer type.
                            let ptr_ty = detect_place_type(mir, base, layouts);
                            let ptr_val = codegen_place_load_typed(
                                emitter, mir, base, ptr_ty, interner, layouts,
                            );
                            emitter.emit_store(&ty, &val, &ptr_val);
                        }
                        ProjectionElem::Field(field_id, _) => {
                            // Stage 14.19 (GAP-31): Handle Deref+Field for store path.
                            // When base is a Deref (e.g. `(*self).field`), load the
                            // pointer from the inner base, then GEP through it.
                            //
                            // Stage 14.43: Handle nested Field projection for store path.
                            // When base is itself a Field projection (e.g., `self.inner.val`
                            // → Projection(Projection(Local(self), Field(inner)), Field(val))),
                            // we need the ADDRESS of the inner field, not its loaded value.
                            // Was: codegen_place_load loaded the inner struct value, then
                            // GEP-ed into it as if it were a pointer → invalid IR + LLVM
                            // "Cannot emit physreg copy instruction" error at JIT.
                            // Fix: recursively compute the address via compute_place_address.
                            let base_ptr = if let PlaceKind::Local(id) = &base.kind {
                                emitter
                                    .get_local_ptr(id.0)
                                    .cloned()
                                    .unwrap_or_else(|| "0".to_string())
                            } else if let PlaceKind::Projection(inner_base, ProjectionElem::Deref) =
                                &base.kind
                            {
                                // base is (*inner).field — load the pointer from inner_base
                                let ptr_ty = detect_place_type(mir, inner_base, layouts);
                                codegen_place_load_typed(
                                    emitter, mir, inner_base, ptr_ty, interner, layouts,
                                )
                            } else if let PlaceKind::Projection(_, ProjectionElem::Field(_, _)) =
                                &base.kind
                            {
                                // Stage 14.43: base is a nested Field projection
                                // (e.g., self.inner). Compute its address recursively.
                                compute_place_address(emitter, mir, base, interner, layouts)
                            } else {
                                codegen_place_load(emitter, mir, base, interner, layouts)
                            };
                            let struct_ty = detect_place_storage_type(mir, base, layouts);
                            let field_ptr =
                                emitter.emit_gep_field(&base_ptr, &struct_ty, field_id.0);
                            emitter.emit_store(&ty, &val, &field_ptr);
                        }
                        ProjectionElem::Index(idx) => {
                            // Stage 14.62: For store path, when base is a Ref (e.g., `&mut [i32; 3]`),
                            // load the reference value (the array pointer) instead of using
                            // the alloca pointer. Also extract Array type from Ref for GEP.
                            // Mirrors the load path fix from Stage 14.61.
                            let base_ty = detect_place_type(mir, base, layouts);
                            let base_ptr = if base_ty.is_ptr() {
                                // Ref to array — load the pointer value
                                codegen_place_load_typed(
                                    emitter, mir, base, base_ty, interner, layouts,
                                )
                            } else {
                                compute_place_address(emitter, mir, base, interner, layouts)
                            };
                            // Extract Array type from Ptr(Array) or Ref for GEP
                            let array_ty = {
                                let raw_ty = detect_place_storage_type(mir, base, layouts);
                                match &raw_ty {
                                    EmitType::Ptr(inner) => *inner.clone(),
                                    EmitType::OpaquePtr => {
                                        if let PlaceKind::Local(id) = &base.kind {
                                            if let Some(ld) = mir.local_decls.get(id.0 as usize) {
                                                if let crate::mir::ty::TyKind::Ref(_, _, inner) =
                                                    &ld.ty.kind
                                                {
                                                    mir_type_to_emit_type_with_layouts(
                                                        inner, layouts,
                                                    )
                                                } else {
                                                    raw_ty
                                                }
                                            } else {
                                                raw_ty
                                            }
                                        } else {
                                            raw_ty
                                        }
                                    }
                                    _ => raw_ty,
                                }
                            };
                            let idx_val = if let Some(v) = emitter.get_local(idx.0).cloned() {
                                v
                            } else if let Some(ptr) = emitter.get_local_ptr(idx.0).cloned() {
                                emitter.emit_load(&EmitType::I32, &ptr)
                            } else {
                                "0".to_string()
                            };
                            // Stage 3.51: same fat pointer unwrap as the load path.
                            let (gep_base, pointee_opt) =
                                unwrap_fat_ptr_for_index(emitter, &base_ptr, &array_ty);
                            let elem_ptr = match pointee_opt {
                                Some(elem_ty) => {
                                    emitter.emit_gep_index_ptr(&gep_base, &elem_ty, &idx_val)
                                }
                                None => emitter.emit_gep_index(&gep_base, &array_ty, &idx_val),
                            };
                            emitter.emit_store(&ty, &val, &elem_ptr);
                        }
                        _ => {}
                    }
                }
                PlaceKind::Static(_) => {}
            }
        }
        StatementKind::StorageLive(id) => {
            let _ = id;
        }
        StatementKind::StorageDead(_) => {}
        StatementKind::Nop | StatementKind::Deinit(_) => {}
        // Stage 13.13 + 13.14 + 13.16: Inline println! / print! / eprintln! / eprint!
        // statement with format args support.
        //
        // Stage 13.13: introduced the variant; routed both stdout and stderr
        // to `printf` (stderr flag captured but ignored — explicit deferral).
        // Stage 13.14: closed the deferral — stderr routes to __landin_eprint helper.
        // Stage 13.16: format args support — builds a C printf format string from
        // the Landin template (replacing `{}` with `%ld`/`%s`/`%d` based on arg
        // type) and emits `printf(c_fmt, c_args...)` with the correct types.
        //
        // The `msg` field is the format string template (with trailing "\n"
        // already appended if `newline == true`). The `args` field is the list
        // of MIR operands to substitute into `{}` placeholders, in order.
        //
        // Per `api-naming-standard.md` §8.1: helpers follow the
        // `__landin_<verb>_<noun>` pattern (matches `__landin_panic_*` siblings).
        StatementKind::Println {
            msg,
            args,
            newline,
            stderr,
        } => {
            let _ = newline; // already encoded in `msg` (trailing "\n")

            // Stage 13.16: Build the C printf format string by replacing
            // Landin `{}` placeholders with C conversion specifiers based
            // on each arg's type. Also codegen each arg operand to get its
            // LLVM value handle.
            //
            // Type mapping:
            //   - Integer (i8/i16/i32/i64/i128/u*/bool) → `%ld` (cast to i64)
            //   - Float (f32/f64) → `%f` (use double)
            //   - &str / &[u8] → `%s` (string pointer)
            //   - Other (struct, etc.) → `%s` with "<?>` placeholder (debug)
            //
            // We also collect the LLVM value handles for each arg, with
            // appropriate casting for the C ABI.
            let mut c_fmt = String::new();
            let mut c_arg_vals: Vec<(EmitType, EmitValue)> = Vec::new();

            // Iterate the msg template, replacing `{}` with the next arg's
            // conversion specifier.
            let mut chars = msg.chars().peekable();
            let mut arg_idx = 0usize;
            while let Some(c) = chars.next() {
                if c == '{' && chars.peek() == Some(&'}') {
                    // `{}` placeholder — substitute with next arg's conversion
                    chars.next(); // consume '}'
                    if arg_idx < args.len() {
                        let arg = &args[arg_idx];
                        arg_idx += 1;
                        // Detect the arg's type
                        let arg_ty =
                            detect_operand_type(mir, arg, layouts).unwrap_or(EmitType::I32);
                        // Codegen the operand to get its LLVM value
                        let arg_val = codegen_operand(
                            emitter,
                            mir,
                            arg,
                            interner,
                            layouts,
                            fn_name_by_def_id,
                        );
                        // Determine the C conversion specifier + cast
                        match &arg_ty {
                            EmitType::I1
                            | EmitType::I8
                            | EmitType::I16
                            | EmitType::I32
                            | EmitType::I64
                            | EmitType::I128 => {
                                // Integer → %ld (cast to i64 for portability)
                                // Stage 13.21: Use SIGN-EXTENSION (emit_cast) for signed
                                // integers so negative numbers print correctly. Before
                                // Stage 13.21, we used zext (zero-extension), which
                                // turned -5 (0xFFFFFFFB in i32) into 4294967291
                                // (0x00000000FFFFFFFB in i64) — a P0 bug for any
                                // program printing negative values.
                                //
                                // Stage 14.12 (GAP-18): Bool (i1) now prints as
                                // "true"/"false" instead of 1/0. Uses emit_select to
                                // choose between two string globals based on the bool
                                // value, then prints with %s. This matches Rust's
                                // Display impl for bool.
                                if arg_ty == EmitType::I1 {
                                    // Bool → "true" / "false" via select + %s
                                    let true_str = emitter.emit_string_global(b"true\0");
                                    let false_str = emitter.emit_string_global(b"false\0");
                                    let selected = emitter.emit_select(
                                        &EmitType::OpaquePtr,
                                        &arg_val,
                                        &true_str,
                                        &false_str,
                                    );
                                    c_fmt.push_str("%s");
                                    c_arg_vals.push((EmitType::OpaquePtr, selected));
                                } else if arg_ty != EmitType::I64 {
                                    // Use emit_cast which does sext for signed integers
                                    let cast_val =
                                        emitter.emit_cast(&arg_ty, &EmitType::I64, &arg_val);
                                    c_fmt.push_str("%ld");
                                    c_arg_vals.push((EmitType::I64, cast_val));
                                } else {
                                    c_fmt.push_str("%ld");
                                    c_arg_vals.push((EmitType::I64, arg_val));
                                }
                            }
                            EmitType::F32 | EmitType::F64 => {
                                // Float → %f (cast to double via emit_cast)
                                let cast_val = if arg_ty == EmitType::F32 {
                                    emitter.emit_cast(&EmitType::F32, &EmitType::F64, &arg_val)
                                } else {
                                    arg_val
                                };
                                c_fmt.push_str("%f");
                                c_arg_vals.push((EmitType::F64, cast_val));
                            }
                            EmitType::Ptr(inner) => {
                                // Stage 14.59: Distinguish &i32 (thin pointer to int)
                                // from &str (fat pointer struct). For &i32, dereference
                                // and print as integer. For other pointers, treat as %s.
                                let inner_ref = inner.as_ref();
                                if matches!(
                                    inner_ref,
                                    EmitType::I1
                                        | EmitType::I8
                                        | EmitType::I16
                                        | EmitType::I32
                                        | EmitType::I64
                                        | EmitType::I128
                                ) {
                                    // &i32 → load the value through the pointer, then print as int
                                    let loaded = emitter.emit_load(inner_ref, &arg_val);
                                    if *inner_ref == EmitType::I1 {
                                        let true_str = emitter.emit_string_global(b"true\0");
                                        let false_str = emitter.emit_string_global(b"false\0");
                                        let selected = emitter.emit_select(
                                            &EmitType::OpaquePtr,
                                            &loaded,
                                            &true_str,
                                            &false_str,
                                        );
                                        c_fmt.push_str("%s");
                                        c_arg_vals.push((EmitType::OpaquePtr, selected));
                                    } else if *inner_ref != EmitType::I64 {
                                        let cast_val =
                                            emitter.emit_cast(inner_ref, &EmitType::I64, &loaded);
                                        c_fmt.push_str("%ld");
                                        c_arg_vals.push((EmitType::I64, cast_val));
                                    } else {
                                        c_fmt.push_str("%ld");
                                        c_arg_vals.push((EmitType::I64, loaded));
                                    }
                                } else {
                                    // Other pointer → treat as %s
                                    c_fmt.push_str("%s");
                                    c_arg_vals.push((EmitType::OpaquePtr, arg_val));
                                }
                            }
                            EmitType::OpaquePtr => {
                                // Pointer → assume &str (fat pointer: {ptr, len})
                                // For simplicity, treat as %s with the pointer.
                                // (Full &str support requires extracting the data ptr.)
                                c_fmt.push_str("%s");
                                c_arg_vals.push((EmitType::OpaquePtr, arg_val));
                            }
                            EmitType::Struct(fields) if fields.len() == 2 => {
                                // Fat pointer (&str / &[T]): { data_ptr, len }
                                // Extract field 0 (data_ptr) for %s
                                let data_ptr = emitter.emit_extractvalue(&arg_ty, &arg_val, 0);
                                c_fmt.push_str("%s");
                                c_arg_vals.push((EmitType::OpaquePtr, data_ptr));
                            }
                            _ => {
                                // Unknown type — emit placeholder
                                c_fmt.push_str("%s");
                                c_arg_vals.push((
                                    EmitType::OpaquePtr,
                                    emitter.emit_string_global(b"(?)\0"),
                                ));
                            }
                        }
                    } else {
                        // More `{}` than args — leave the placeholder as-is
                        // (printf will read garbage from the stack, but this
                        // is a user error; we don't crash).
                        c_fmt.push_str("%ld");
                    }
                } else if c == '%' {
                    // Escape literal `%` for C printf
                    c_fmt.push_str("%%");
                } else {
                    // Regular character — copy to C format string
                    c_fmt.push(c);
                }
            }

            // Null-terminate the C format string
            c_fmt.push('\0');

            // Emit the C format string as a global
            let fmt_global = emitter.emit_string_global(c_fmt.as_bytes());

            // Build the args list for emit_call: first arg is the format string,
            // followed by the substituted arg values.
            let mut call_args: Vec<(EmitType, &EmitValue)> =
                Vec::with_capacity(1 + c_arg_vals.len());
            call_args.push((EmitType::OpaquePtr, &fmt_global));
            for (ty, val) in &c_arg_vals {
                call_args.push((ty.clone(), val));
            }

            if *stderr {
                // Stage 13.14 + 13.16: eprintln!/eprint! → __landin_eprintf helper.
                //
                // The C wrapper defines:
                //   void __landin_eprintf(const char* fmt, ...) { vfprintf(stderr, fmt, va_list); }
                //
                // This is a variadic helper that takes a printf-style format
                // string and args, routing output to stderr.
                emitter.emit_call("__landin_eprintf", &call_args, &EmitType::Void);
            } else {
                // Stage 13.13 + 13.16: println!/print! → printf(fmt, args...)
                // printf returns i32 (number of chars printed); we discard it.
                emitter.emit_call("printf", &call_args, &EmitType::I32);
            }
        }
    }
}
