//! Stage 13.5 MUV-2: LLVM C-API (llvm-sys) emitter.
//!
//! Implements the `Emitter` trait using the LLVM C API via `llvm-sys`.
//! Unlike `TextEmitter` (which emits textual `.ll` IR), this emitter
//! constructs an in-memory `LLVMModuleRef` directly via the C API.
//!
//! Stage 16.76 MUV-2: `build_fn_sigs_map` extracted to `function_sigs.rs`
//! (LLVM-only helper for forward-reference resolution).
//!
//! ## Bridging `EmitValue = String` and `LLVMValueRef`
//!
//! The `Emitter` trait uses `EmitValue = String` (e.g. `"%v3"`) so that
//! `TextEmitter` can render IR text. `LLVMValueRef` is a raw pointer
//! (`*mut LLVMValue`) which has no string form. To bridge these:
//!
//! - Every `emit_*` call assigns the produced `LLVMValueRef` a unique
//!   name of the form `"%vN"` (mirroring `TextEmitter`) and stores it
//!   in a `HashMap<String, LLVMValueRef>` so callers can pass the name
//!   back in via subsequent emit calls.
//! - Internally, `lookup(name)` resolves a `&EmitValue` to its
//!   `LLVMValueRef`. The named-lookup is exact; names like `"0"`,
//!   `"undef"`, or GEP-text (produced by `codegen_operand` for
//!   `&str` fat-pointer construction) are handled as ad-hoc constants.
//!
//! This integration is intentionally pragmatic — it is *not* a fully
//! correct lowering of every Landin construct (e.g. dyn-trait calls,
//! string fat-pointer GEPs are stubbed). The goal is to demonstrate
//! the LLVM C API integration compiles and produces a valid
//! `LLVMModuleRef` for the common path (allocas / loads / stores /
//! arithmetic / control flow).
//!
//! Per §16: this emitter is a pure MIR consumer — it only receives
//! `EmitValue` handles back from itself and never reaches into the
//! MIR / HIR.

#![cfg(feature = "llvm-backend")]

use crate::codegen::emitter::*;
use llvm_sys::analysis::{LLVMVerifierFailureAction, LLVMVerifyModule};
use llvm_sys::core::*;
use llvm_sys::prelude::*;
use llvm_sys::target_machine::*;
use std::collections::HashMap;
use std::ffi::CString;

// Stage 16.77 MUV-1: Import private helpers (cstr, is_float, parse_*, collect_cstring).
use helpers::*;

// Stage 16.76 MUV-2: LLVM-only function signature map builder.

// Stage 16.77 MUV-1: Backend file organization — 6 sub-trait impls split into separate files.
pub(crate) mod aggregate;
pub(crate) mod arithmetic;
pub(crate) mod function;
pub(crate) mod helpers;
pub(crate) mod local_state;
pub(crate) mod memory;
pub(crate) mod module;
#[cfg(test)]
mod tests;

pub(crate) mod function_sigs;

/// LLVM C-API emitter.
pub struct LLVMSysEmitter {
    ctx: LLVMContextRef,
    module: LLVMModuleRef,
    builder: LLVMBuilderRef,
    /// Counter for unique SSA register names ("v1", "v2", ...).
    next_val: u32,
    /// Counter for unique global string names (".str.0", ".str.1", ...).
    next_str: u32,
    /// Map: LLVM-side SSA name ("%vN") → LLVM value handle.
    values: HashMap<String, LLVMValueRef>,
    /// Map: local_id → alloca pointer handle.
    local_ptrs: HashMap<u32, EmitValue>,
    /// Map: local_id → cached value handle.
    locals: HashMap<u32, EmitValue>,
    /// The current function being built (used by emit_function_end).
    cur_fn: Option<LLVMValueRef>,
    /// Map: block-label string ("%bbN") → basic-block ref.
    blocks: HashMap<String, LLVMBasicBlockRef>,
    /// Cached panic-declaration signatures (keyed by fn name).
    declared: HashMap<String, LLVMValueRef>,
    /// Stage 14.22: Cache for struct types — key is the Debug format of the
    /// EmitType::Struct fields. This ensures that structurally-identical
    /// structs (e.g. two `{ i32, i32 }` fields in a nested struct) resolve
    /// to the SAME LLVM type. Without this, LLVMStructTypeInContext creates
    /// distinct nominal types for each call, causing insertvalue to fail
    /// when the field type doesn't match the aggregate's field type.
    /// Uses RefCell for interior mutability since llvm_type takes &self.
    struct_type_cache: std::cell::RefCell<HashMap<String, LLVMTypeRef>>,
    /// Stage 14.65: Map from function name → (return type, param types).
    /// Populated by `set_fn_sigs` before codegen, used by `interpret_adhoc`
    /// to create forward declarations with the CORRECT signature when a
    /// function is referenced before its body is emitted (e.g., `fn adder()
    /// -> fn(i32) -> i32 { double }` references `double` before it's emitted).
    fn_sigs: HashMap<String, (EmitType, Vec<EmitType>)>,
}

impl Default for LLVMSysEmitter {
    fn default() -> Self {
        Self::new()
    }
}

impl LLVMSysEmitter {
    /// Create a new emitter. Initializes the LLVM context, an empty module
    /// named "landin_module", and an IR builder.
    pub fn new() -> Self {
        unsafe {
            let ctx = LLVMContextCreate();
            let name = CString::new("landin_module").unwrap();
            let module = LLVMModuleCreateWithNameInContext(name.as_ptr(), ctx);
            let builder = LLVMCreateBuilderInContext(ctx);
            Self {
                ctx,
                module,
                builder,
                next_val: 1,
                next_str: 0,
                values: HashMap::new(),
                local_ptrs: HashMap::new(),
                locals: HashMap::new(),
                cur_fn: None,
                blocks: HashMap::new(),
                declared: HashMap::new(),
                struct_type_cache: std::cell::RefCell::new(HashMap::new()),
                fn_sigs: HashMap::new(),
            }
        }
    }

    /// Stage 14.65: Set the function signatures for forward-reference resolution.
    ///
    /// Called by `codegen_crate_to_module` before `codegen_from_mir` so that
    /// `interpret_adhoc` can create forward declarations with the CORRECT
    /// signature when a function is referenced before its body is emitted.
    pub(crate) fn set_fn_sigs(&mut self, sigs: HashMap<String, (EmitType, Vec<EmitType>)>) {
        self.fn_sigs = sigs;
    }

    /// Return the underlying `LLVMModuleRef`.
    ///
    /// The caller is responsible for not disposing the module while the
    /// emitter is still in use.
    pub fn to_module(&self) -> LLVMModuleRef {
        self.module
    }

    // Stage 16.35: Removed `to_context` — dead code (never called).
    // Per §1.0 原則 5 "去除兼容思维": dead code removed.

    /// Emit an object file via `LLVMTargetMachineEmitToFile`.
    ///
    /// Initializes all LLVM target components, looks up the host triple's
    /// target, builds a target machine, and emits `out_path` as an object
    /// file. Returns `Ok(())` on success or `Err(CodegenError)` describing
    /// the LLVM error.
    ///
    /// Stage 17.02: Changed return type from `Result<(), String>` to
    /// `Result<(), CodegenError>` for structured error reporting with span.
    /// Replaced `unwrap()` calls with `cstr_result()?` for error safety.
    ///
    /// Per §1.0 原則 4 "报错 > 静默": LLVM errors are reported, not panicked.
    /// Per §10.1.8: CodegenError follows `{ message, span }` minimal form.
    pub fn to_object_file(&self, out_path: &str) -> crate::codegen::CodegenResult<()> {
        use crate::codegen::error::CodegenError;
        use crate::codegen::llvm::helpers::cstr_result;
        use crate::session::Span;
        unsafe {
            // Stage 14.44: Verify the module before emitting.
            let mut verify_err: *mut std::os::raw::c_char = std::ptr::null_mut();
            let verify_rc = LLVMVerifyModule(
                self.module,
                LLVMVerifierFailureAction::LLVMPrintMessageAction,
                &mut verify_err,
            );
            if !verify_err.is_null() {
                LLVMDisposeMessage(verify_err);
            }
            if verify_rc != 0 {
                return Err(CodegenError::new(
                    "LLVM module verification failed (see messages above)",
                    Span::DUMMY,
                ));
            }

            // 1. Initialise all targets / asm printers.
            llvm_sys::target::LLVM_InitializeAllTargetInfos();
            llvm_sys::target::LLVM_InitializeAllTargets();
            llvm_sys::target::LLVM_InitializeAllTargetMCs();
            llvm_sys::target::LLVM_InitializeAllAsmPrinters();

            // 2. Get the host triple.
            let triple_ptr = LLVMGetDefaultTargetTriple();
            if triple_ptr.is_null() {
                return Err(CodegenError::new(
                    "LLVMGetDefaultTargetTriple returned null",
                    Span::DUMMY,
                ));
            }
            let triple = collect_cstring(triple_ptr);
            LLVMDisposeMessage(triple_ptr);

            // 3. Look up the target.
            let triple_c = cstr_result(&triple)?;
            let mut target: LLVMTargetRef = std::ptr::null_mut();
            let mut err_buf: *mut std::os::raw::c_char = std::ptr::null_mut();
            let rc = LLVMGetTargetFromTriple(triple_c.as_ptr(), &mut target, &mut err_buf);
            if rc != 0 {
                let msg = if err_buf.is_null() {
                    "unknown".to_string()
                } else {
                    let m = collect_cstring(err_buf);
                    LLVMDisposeMessage(err_buf);
                    m
                };
                return Err(CodegenError::new(
                    format!("LLVMGetTargetFromTriple failed: {}", msg),
                    Span::DUMMY,
                ));
            }

            // 4. Build the target machine.
            // Stage 17.02: Use cstr_result instead of unwrap for error safety.
            let cpu_c = cstr_result("generic")?;
            let feat_c = cstr_result("")?;
            let tm = LLVMCreateTargetMachine(
                target,
                triple_c.as_ptr(),
                cpu_c.as_ptr(),
                feat_c.as_ptr(),
                LLVMCodeGenOptLevel::LLVMCodeGenLevelDefault,
                LLVMRelocMode::LLVMRelocDefault,
                LLVMCodeModel::LLVMCodeModelDefault,
            );
            if tm.is_null() {
                return Err(CodegenError::new(
                    "LLVMCreateTargetMachine returned null",
                    Span::DUMMY,
                ));
            }

            // 5. Emit to file.
            let path_c = cstr_result(out_path)?;
            let mut err2: *mut std::os::raw::c_char = std::ptr::null_mut();
            let rc2 = LLVMTargetMachineEmitToFile(
                tm,
                self.module,
                path_c.as_ptr(),
                LLVMCodeGenFileType::LLVMObjectFile,
                &mut err2,
            );
            LLVMDisposeTargetMachine(tm);
            if rc2 != 0 {
                let msg = if err2.is_null() {
                    "unknown".to_string()
                } else {
                    let m = collect_cstring(err2);
                    LLVMDisposeMessage(err2);
                    m
                };
                return Err(CodegenError::new(
                    format!("LLVMTargetMachineEmitToFile failed: {}", msg),
                    Span::DUMMY,
                ));
            }
            Ok(())
        }
    }

    // ---- private helpers ----------------------------------------------

    /// Allocate the next SSA register name, store the LLVM value under
    /// that name, and return the name as an `EmitValue`.
    fn fresh_named(&mut self, val: LLVMValueRef) -> EmitValue {
        let name = format!("v{}", self.next_val);
        self.next_val += 1;
        let key = format!("%{}", name);
        self.set_value_name(val, &name);
        self.values.insert(key.clone(), val);
        key
    }

    /// Same as `fresh_named` but with a caller-supplied name.
    /// If `name` already starts with `%`, use it as-is; otherwise prepend `%`.
    fn named(&mut self, val: LLVMValueRef, name: &str) -> EmitValue {
        let key = if name.starts_with('%') {
            name.to_string()
        } else {
            format!("%{}", name)
        };
        let bare = key.strip_prefix('%').unwrap_or(&key);
        self.set_value_name(val, bare);
        self.values.insert(key.clone(), val);
        key
    }

    /// Set LLVM-side name for a value (debug aid; not strictly required).
    fn set_value_name(&self, val: LLVMValueRef, name: &str) {
        unsafe {
            let c = CString::new(name).unwrap();
            LLVMSetValueName2(val, c.as_ptr(), name.len());
        }
    }

    /// Resolve a `&EmitValue` (LLVM-side SSA name) to its `LLVMValueRef`.
    ///
    /// Falls back to ad-hoc constant construction for non-registered
    /// strings like `"0"`, `"-1"`, `"undef"`, decimal integers, and
    /// `getelementptr ...` text produced by `codegen_operand` for
    /// `&str` fat pointers (stubbed — see `interpret_adhoc`).
    fn lookup(&mut self, name: &EmitValue) -> LLVMValueRef {
        if let Some(v) = self.values.get(name) {
            return *v;
        }
        self.interpret_adhoc(name)
    }

    /// Best-effort interpretation of a literal / GEP-text `EmitValue`.
    /// Used when `lookup()` can't find the name in `values`.
    fn interpret_adhoc(&mut self, name: &EmitValue) -> LLVMValueRef {
        unsafe {
            // undef → i32 undef (placeholder).
            if name == "undef" {
                return LLVMGetUndef(LLVMInt32TypeInContext(self.ctx));
            }
            // null pointer.
            if name == "null" {
                return LLVMConstNull(LLVMPointerTypeInContext(self.ctx, 0));
            }
            // Decimal integer literal (positive or negative).
            if let Ok(n) = name.parse::<i64>() {
                let ty = LLVMInt32TypeInContext(self.ctx);
                return LLVMConstInt(ty, n as u64, 1);
            }
            // "getelementptr inbounds ([N x i8], [N x i8]* @.str.M, i32 0, i32 0)"
            // — this is produced by codegen_operand for &str fat-pointer
            // construction. Stage 13.20: parse the global name (@.str.M) and
            // build a real GEP to get the i8* pointer to the string data.
            //
            // Before Stage 13.20, this returned i32 zero (null pointer),
            // causing `printf("%s", null)` → "(null)" output for ALL string
            // arguments.
            if name.starts_with("getelementptr") {
                // Extract the global name: find "@." and read until the next
                // space or comma.
                if let Some(at_pos) = name.find("@.") {
                    let rest = &name[at_pos + 1..]; // skip "@"
                    let global_name: String = rest
                        .chars()
                        .take_while(|c| c.is_alphanumeric() || *c == '.' || *c == '_')
                        .collect();
                    // Look up the global value.
                    if let Some(&global) = self.values.get(&global_name) {
                        // Build GEP: getelementptr inbounds [N x i8], ptr @global, i32 0, i32 0
                        let i8_ty = LLVMInt8TypeInContext(self.ctx);
                        let i32_ty = LLVMInt32TypeInContext(self.ctx);
                        let zero = LLVMConstInt(i32_ty, 0, 0);
                        let mut indices = [zero, zero];
                        let gep = LLVMConstInBoundsGEP2(i8_ty, global, indices.as_mut_ptr(), 2);
                        return gep;
                    }
                }
                // Fallback: null pointer if we can't parse.
                let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
                return LLVMConstNull(ptr_ty);
            }
            // Stage 14.58: Handle function reference values like "@landin_double".
            // These are produced by codegen_operand for FnDef constants.
            // We look up the function value from the values map or declare it.
            if name.starts_with('@') {
                let func_name: String = name.strip_prefix('@').unwrap().to_string();
                // Check if already in values map
                if let Some(&v) = self.values.get(name) {
                    return v;
                }
                // Try to get the function from the module
                let name_c = CString::new(func_name.clone()).unwrap();
                let func = LLVMGetNamedFunction(self.module, name_c.as_ptr());
                if !func.is_null() {
                    return func;
                }
                // Stage 14.65: Function not yet defined — create a forward
                // declaration with the CORRECT signature from `fn_sigs`.
                //
                // Previously, this returned a null pointer when the function
                // hadn't been emitted yet (e.g., `adder` returns `double`
                // where `double` is defined AFTER `adder` in source order).
                // The null pointer was then stored and returned, causing
                // segfaults when the function was later called.
                //
                // Fix: look up the function's signature in `self.fn_sigs`
                // (populated by `set_fn_sigs` before codegen) and create
                // a forward declaration with the correct return + param types.
                // When the actual function is emitted later,
                // `emit_function_begin` will reuse this declaration (Stage
                // 14.63 forward-decl dedup) because the signature matches.
                //
                // Per §1.0 原则 5 "报错 > 静默": function references are
                // never null — they always point to a real (possibly
                // forward-declared) function value.
                if let Some((ret_ty, param_tys)) = self.fn_sigs.get(&func_name) {
                    if crate::session::debug_codegen_enabled() {
                        eprintln!(
                            "[CODEGEN] get_or_declare: found sig for {} params={}",
                            func_name,
                            param_tys.len()
                        );
                    }
                    let ret_llvm_ty = self.llvm_type(ret_ty);
                    let param_llvm_tys: Vec<LLVMTypeRef> =
                        param_tys.iter().map(|t| self.llvm_type(t)).collect();
                    let fty = LLVMFunctionType(
                        ret_llvm_ty,
                        param_llvm_tys.as_ptr() as *mut LLVMTypeRef,
                        param_llvm_tys.len() as u32,
                        0,
                    );
                    let fwd = LLVMAddFunction(self.module, name_c.as_ptr(), fty);
                    self.declared.insert(func_name, fwd);
                    return fwd;
                }
                // Fallback: signature not in fn_sigs — use generic variadic.
                if crate::session::debug_codegen_enabled() {
                    eprintln!("[CODEGEN] get_or_declare: NOT found in fn_sigs: {} (fn_sigs has {} entries)", func_name, self.fn_sigs.len());
                }
                let ret_ty = LLVMInt32TypeInContext(self.ctx);
                let fty = LLVMFunctionType(ret_ty, std::ptr::null_mut(), 0, 1);
                let fwd = LLVMAddFunction(self.module, name_c.as_ptr(), fty);
                return fwd;
            }
            // Fallback: i32 zero.
            let ty = LLVMInt32TypeInContext(self.ctx);
            LLVMConstInt(ty, 0, 0)
        }
    }

    /// Map an `EmitType` to its LLVM type.
    fn llvm_type(&self, ty: &EmitType) -> LLVMTypeRef {
        unsafe {
            match ty {
                EmitType::I1 => LLVMInt1TypeInContext(self.ctx),
                EmitType::I8 => LLVMInt8TypeInContext(self.ctx),
                EmitType::I16 => LLVMInt16TypeInContext(self.ctx),
                EmitType::I32 => LLVMInt32TypeInContext(self.ctx),
                EmitType::I64 => LLVMInt64TypeInContext(self.ctx),
                EmitType::I128 => LLVMInt128TypeInContext(self.ctx),
                EmitType::F32 => LLVMFloatTypeInContext(self.ctx),
                EmitType::F64 => LLVMDoubleTypeInContext(self.ctx),
                EmitType::Void => LLVMVoidTypeInContext(self.ctx),
                EmitType::Ptr(_) | EmitType::OpaquePtr => LLVMPointerTypeInContext(self.ctx, 0),
                EmitType::Struct(fields) => {
                    // Stage 16.22: Empty struct has size 0 in LLVM → UB with
                    // alloca. Use i8 (size 1) instead, matching the text
                    // emitter fix in emit_type_to_llvm_str.
                    if fields.is_empty() {
                        return LLVMInt8TypeInContext(self.ctx);
                    }
                    // Stage 14.22: Cache struct types by their field layout
                    // to ensure structurally-identical structs resolve to the
                    // SAME LLVM type. Without this, LLVMStructTypeInContext
                    // creates distinct nominal types, causing insertvalue to
                    // fail when field types don't match the aggregate's field
                    // type (e.g. nested structs).
                    let cache_key = format!("{:?}", fields);
                    {
                        let cache = self.struct_type_cache.borrow();
                        if let Some(cached) = cache.get(&cache_key) {
                            return *cached;
                        }
                    }
                    let elems: Vec<LLVMTypeRef> =
                        fields.iter().map(|f| self.llvm_type(f)).collect();
                    let struct_ty = LLVMStructTypeInContext(
                        self.ctx,
                        elems.as_ptr() as *mut LLVMTypeRef,
                        elems.len() as u32,
                        0,
                    );
                    self.struct_type_cache
                        .borrow_mut()
                        .insert(cache_key, struct_ty);
                    struct_ty
                }
                EmitType::Array(elem, n) => {
                    let elem_ty = self.llvm_type(elem);
                    LLVMArrayType2(elem_ty, *n)
                }
            }
        }
    }

    /// Look up or create a function by name in the module.
    /// Used by `emit_call` to resolve the callee.
    pub(crate) fn get_or_declare_function(
        &mut self,
        name: &str,
        ret_ty: &EmitType,
        arg_tys: &[EmitType],
    ) -> LLVMValueRef {
        if let Some(v) = self.declared.get(name) {
            return *v;
        }
        unsafe {
            let name_c = CString::new(name).unwrap();
            // Look up an existing function with this name first.
            let existing = LLVMGetNamedFunction(self.module, name_c.as_ptr());
            if !existing.is_null() {
                self.declared.insert(name.to_string(), existing);
                return existing;
            }
            // Build a function type.
            let ret = self.llvm_type(ret_ty);
            let params: Vec<LLVMTypeRef> = arg_tys.iter().map(|t| self.llvm_type(t)).collect();
            // Stage 13.16: printf and __landin_eprintf are variadic — declare
            // them with isVariadic=1 so the LLVM module declaration matches
            // the variadic call sites in emit_call.
            let is_variadic: i32 = if name == "printf" || name == "__landin_eprintf" {
                1
            } else {
                0
            };
            let fty = LLVMFunctionType(
                ret,
                params.as_ptr() as *mut LLVMTypeRef,
                params.len() as u32,
                is_variadic,
            );
            let f = LLVMAddFunction(self.module, name_c.as_ptr(), fty);
            self.declared.insert(name.to_string(), f);
            f
        }
    }

    // Stage 16.35: Removed `predeclare_function` — dead code.
    // Was a wrapper around `get_or_declare_function`, marked
    // `#[allow(dead_code)]` and never called. Its docstring referenced
    // a non-existent `predeclare_all_functions`.
    // Per §1.0 原則 5 "去除兼容思维": dead code removed.

    /// Get or create the basic block for `label`, mirroring `TextEmitter::emit_block`.
    fn block_for(&mut self, label: &str) -> LLVMBasicBlockRef {
        // Strip leading '%' if present.
        let key = if let Some(stripped) = label.strip_prefix('%') {
            stripped.to_string()
        } else {
            label.to_string()
        };
        let full_key = format!("%{}", key);
        if let Some(bb) = self.blocks.get(&full_key) {
            return *bb;
        }
        unsafe {
            let label_c = CString::new(key.as_str()).unwrap();
            // Append to current function (or entry block if no function yet).
            let parent = self.cur_fn.expect("emit_block called outside function");
            let bb = LLVMAppendBasicBlockInContext(self.ctx, parent, label_c.as_ptr());
            self.blocks.insert(full_key, bb);
            bb
        }
    }
}

impl Drop for LLVMSysEmitter {
    fn drop(&mut self) {
        unsafe {
            if !self.builder.is_null() {
                LLVMDisposeBuilder(self.builder);
                self.builder = std::ptr::null_mut();
            }
            // Note: we deliberately do NOT dispose the module or context here.
            // The caller may still want to extract the module via `to_module()`
            // for further processing (e.g. object-file emission). Disposing
            // the context invalidates the module. The host process is
            // expected to clean these up on exit.
        }
    }
}
