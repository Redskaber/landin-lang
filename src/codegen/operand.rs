//! MIR operand → LLVM IR codegen.
//!
//! Extracted from codegen/mod.rs per Stage 13.28 codegen reorganization.
//! Handles `Operand::Constant`, `Operand::Copy`, `Operand::Move`, and
//! dyn trait method call codegen.

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
pub(crate) fn codegen_operand(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    op: &Operand,
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
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
                let fat_ty = mir_type_to_emit_type_with_layouts(&c.ty, layouts);
                // Stage 13.20: GEP array size must match the global's actual
                // size (n+1 to include the null terminator). Using the original
                // length `n` here would create a type mismatch with the global
                // ([n+1 x i8] global but [n x i8] GEP), which LLVM accepts but
                // produces incorrect pointer arithmetic in some cases.
                let ptr_val = format!(
                    "getelementptr inbounds ([{} x i8], [{} x i8]* @{}, i32 0, i32 0)",
                    n + 1,
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
            _ => emitter.emit_const(&c.val),
        },
        Operand::Copy(lv) | Operand::Move(lv) => {
            let ty = detect_place_type(mir, lv, layouts);
            codegen_place_load_typed(emitter, mir, lv, ty, interner, layouts)
        }
    }
}

#[allow(clippy::only_used_in_recursion)]
/// Stage 5.79: Codegen a dyn Trait method call.
///
/// Reads `mir.dyn_trait_calls[index]` to get the
/// `(trait, type, method, slot_index, param_count)` info, computes the
/// dynptr symbol (`.dynptr.<trait>.<type>`), and calls
/// `emitter.emit_dyn_trait_method_call()` with the slot_index + args.
///
/// # Arguments
///
/// - `emitter`: the codegen emitter (text or other backend)
/// - `mir`: the MIR body (carries `dyn_trait_calls` side-table)
/// - `index`: the side-table index (from the `Const{val: Int(index)}` marker)
/// - `args`: the call's argument operands (self first, then explicit args)
/// - `interner`: the string interner (for type detection)
/// - `layouts`: ADT layouts (for type detection)
///
/// # Returns
///
/// The `EmitValue` (LLVM register name) holding the call result.
///
/// # Panics
///
/// Panics if `index` is out of bounds for `mir.dyn_trait_calls`. The
/// caller (`codegen_terminator`'s `Terminator::Call` branch) is
/// responsible for bounds-checking before invoking this function.
///
/// # §16 compliance
///
/// Reads `mir::body::MirBody.dyn_trait_calls` (side-table populated by
/// Stage 5.78's `build_dyn_trait_call_terminator`). No HIR or
/// TraitResolver queries — MIR carries all dyn Trait info as data.
/// Data flow: `mir::body` → `codegen` → LLVM IR text. Single-directional.
///
/// # §23 compliance
///
/// `codegen_dyn_trait_call` follows the `<verb>_<noun>_<noun>_<noun>`
/// pattern (helper-verb `codegen_` prefix per §8.1, mirrors
/// `codegen_terminator` / `codegen_operand`).
pub fn codegen_dyn_trait_call(
    emitter: &mut dyn Emitter,
    mir: &MirBody,
    index: u128,
    args: &[Operand],
    interner: &Rodeo,
    layouts: &crate::mir::body::AdtLayouts,
) -> EmitValue {
    let call_info = &mir.dyn_trait_calls[index as usize];
    let dynptr_symbol = format!(".dynptr.{}.{}", call_info.trait_name, call_info.type_name);

    // Codegen the args (self first, then explicit args — already ordered
    // by `build_dyn_trait_call_terminator`).
    //
    // Stage 5.84: use param_kinds for precise arg types. The args list is
    // [self, arg0, arg1, ...] — self is at index 0 (always OpaquePtr since
    // it's a fat pointer), explicit args start at index 1 and use
    // param_kinds[i-1] for their EmitType. Falls back to detect_operand_type
    // when param_kinds is exhausted or unavailable.
    let arg_pairs: Vec<(EmitType, EmitValue)> = args
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let ty = if i == 0 {
                // self — always a fat pointer (OpaquePtr)
                EmitType::OpaquePtr
            } else {
                // Explicit arg — use param_kinds[i-1] if available
                let param_idx = i - 1;
                if param_idx < call_info.param_kinds.len() {
                    stdlib_type_kind_to_emit_type(call_info.param_kinds[param_idx])
                } else {
                    // Fallback to operand type detection
                    detect_operand_type(mir, a, layouts).unwrap_or(EmitType::I32)
                }
            };
            let val = codegen_operand(emitter, mir, a, interner, layouts);
            (ty, val)
        })
        .collect();
    let arg_refs: Vec<(EmitType, &EmitValue)> =
        arg_pairs.iter().map(|(t, v)| (t.clone(), v)).collect();

    // Stage 5.82: use return_kind for precise return type (TD-016 closure).
    // Previously (Stage 5.79), this used EmitType::I32 as a placeholder.
    // Now we convert call_info.return_kind (populated from
    // StdlibTraitMethod.return_kind via build_dyn_trait_method_calls_from_fat_ptrs)
    // to the correct EmitType.
    let ret_ty = stdlib_type_kind_to_emit_type(call_info.return_kind);
    emitter.emit_dyn_trait_method_call(&dynptr_symbol, call_info.slot_index, &arg_refs, &ret_ty)
}
