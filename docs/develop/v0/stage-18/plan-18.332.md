# Stage 18.332 — LLVMSysEmitter sret ABI Support (P1 soundness fix)

> **Author**: Super Z (main) — PM-A + ARCH-A + DEV-A + REV-A + QA-A
> **Date**: 2026-08-27
> **Complexity**: L3 (cross-module: codegen/llvm/{function,aggregate,mod}.rs + tests)
> **Status**: planned → in-progress

## 1. 5W2H Analysis

### What
LLVMSysEmitter lacks explicit `sret` (struct-return) handling. Functions returning
structs > 16 bytes (e.g., `Vec::new()` returns `{ptr, i64, i64}` = 24 bytes) are
emitted with the struct as the direct return type:

```llvm
define { ptr, i64, i64 } @vec_new() {
  ...
  ret { ptr, i64, i64 } %result
}
```

System V AMD64 ABI §3.2.3 mandates that structs > 16 bytes be returned via a hidden
`%rdi`-passed pointer parameter (`sret` attribute). Without this, the generated
machine code corrupts the stack when the caller's return-value slot is smaller
than the actual struct size.

### Why (root cause per §2.2)
- **Stage 18.329** switched optimization level back to `LLVMCodeGenLevelDefault`,
  hoping LLVM's CodeGenPrepare / LowerFormalArguments passes would auto-detect the
  struct-return and lower to sret at codegen time.
- **Reality**: LLVM's auto-demotion is unreliable across versions; LLVM 22.1.8
  intermittently generates broken code under multi-threaded cargo test execution
  (~5-10% flake rate, see baseline tests in §6).
- **rustc comparison** (per docs/stage-committee-process.md §1.0 原則 6 — 通解):
  rustc_codegen_llvm emits sret **explicitly** in the IR via
  `Attribute::StructRet` (see rustc_codegen_llvm/src/abi.rs). It does NOT rely
  on LLVM's auto-demotion.
- **Architectural boundary**: TextEmitter (used by `--emit-llvm`) already
  implements sret correctly (Stage 18.330). LLVMSysEmitter (used by `--run`
  and `--emit-obj`/`--emit-bin`) is the actual production codegen path and
  must implement sret identically.

### Who (roles per §1.4)
- **ARCH-A**: design sret emission contract
- **DEV-A**: implement across 4 sites (function_begin, ret, call, declare_function)
- **REV-A**: review for soundness (no UB, no silent degradation)
- **QA-A**: regression + multi-threaded stress tests

### When
- Implement after baseline confirmed (3641 tests single-thread, multi-thread flaky)
- Stop when §3.2 all-green AND multi-threaded 5x run is 5/5 stable (no flakes)

### Where
- `src/codegen/llvm/function.rs` — emit_function_begin, emit_ret
- `src/codegen/llvm/aggregate.rs` — emit_call, emit_dyn_trait_method_call
- `src/codegen/llvm/mod.rs` — declare_function, interpret_adhoc forward-decl path
- `tests/v0/stage18/plan/stage18_332_*.rs` — regression tests

### How (implementation strategy)

#### 4.1 emit_function_begin (LLVMSysEmitter)

When `ret.needs_sret()`:
1. Build function type with return = `void`, params = `[ptr, ...original_params]`
2. Add `sret(ret_ty)` type attribute at param index 1 (LLVM uses 1-indexed params)
3. Register `%_sret` in `self.values` so `emit_ret` can find it
4. Skip param 0 when registering user-visible params (start at i=1)

```rust
if ret.needs_sret() {
    let ret_llvm_ty = self.llvm_type(ret);
    let void_ty = LLVMVoidTypeInContext(self.ctx);
    let ptr_ty = LLVMPointerTypeInContext(self.ctx, 0);
    let mut param_tys: Vec<LLVMTypeRef> = vec![ptr_ty];
    param_tys.extend(params.iter().map(|(t, _)| self.llvm_type(t)));
    let fty = LLVMFunctionType(void_ty, param_tys.as_mut_ptr(), param_tys.len() as u32, 0);
    // ... add function ...
    // Add sret attribute on param index 1
    let sret_kind = LLVMGetEnumAttributeKindForName(b"sret\0".as_ptr() as *const _, 4);
    let sret_attr = LLVMCreateTypeAttribute(self.ctx, sret_kind, ret_llvm_ty);
    LLVMAddAttributeAtIndex(fn_val, 1, sret_attr);
    // Register sret pointer under "%_sret"
    let sret_param = LLVMGetParam(fn_val, 0);
    self.set_value_name(sret_param, "_sret");
    self.values.insert("%_sret".to_string(), sret_param);
    // Register user params starting from index 1
    for (i, (_, pname)) in params.iter().enumerate() {
        let pval = LLVMGetParam(fn_val, (i + 1) as u32);
        ...
    }
}
```

#### 4.2 emit_ret (LLVMSysEmitter)

When `ty.needs_sret()`:
1. Lookup `%_sret` in `self.values`
2. Store the value into `%_sret`: `LLVMBuildStore(builder, val, sret_ptr)`
3. Build `ret void`

#### 4.3 emit_call (LLVMSysEmitter)

When `ret_ty.needs_sret()`:
1. Allocate an alloca for the return type
2. Build function type with return = `void`, params = `[ptr, ...args]`
3. Look up or declare the function with this sret signature
4. Build the call with `LLVMBuildCall2` (return = void)
5. Add sret type attribute to call site param 1 via `LLVMAddCallSiteAttribute`
6. Load the result from the alloca via `LLVMBuildLoad2`
7. Return the loaded SSA value

#### 4.4 declare_function + interpret_adhoc forward-decl

Both `declare_function` (in mod.rs) and the forward-decl path in `interpret_adhoc`
must build the sret signature when `ret_ty.needs_sret()`, AND add the sret
attribute to the forward declaration. Otherwise:
- The forward decl has signature `{ret_ty} (...)`
- The actual definition has signature `void (ptr sret, ...)`
- LLVM reuses the decl, producing invalid IR

This is exactly what Stage 18.188 tried to fix with "delete + re-add" — but
that's a hack. The proper fix is: forward decls must use the same sret
signature from the start.

#### 4.5 emit_dyn_trait_method_call (vtable indirect)

Same sret path needed when the method's return type needs sret. The
function pointer is loaded from the vtable; the call site must add the
sret attribute to param 1.

### How Much (acceptance per §3.2)
- `cargo build --release --features llvm-backend` ✅ 0 warnings
- `cargo check --features llvm-backend` ✅ 0 errors, 0 warnings
- `cargo fmt --check` ✅ 0 diff
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✅ 0 warnings
- `cargo test --release --features llvm-backend` (single-thread, --test-threads=1) ✅ 0 failures
- `cargo test --release --features llvm-backend` (multi-thread, 5 runs) ✅ 5/5 stable

## 2. Decision Points (per §2.2 + §12)

### 2.1 Why explicit sret (A) over LLVM auto-demotion (B)?
- **(B) Auto-demotion**: relies on LLVM's CodeGenPrepare / LowerFormalArguments
  passes to detect `ret {struct}` and rewrite to sret at codegen time.
  Stage 18.329 attempted this — failed (5-10% flake under multi-threading).
  Root cause: auto-demotion handles return-type lowering, but call sites still
  pass the struct return type directly. The caller/callee ABI mismatch produces
  intermittently corrupted stack frames.
- **(A) Explicit sret**: emit the sret signature in the IR itself. Both caller
  and callee agree on the ABI from the start. No reliance on LLVM passes.
- **§1.0 原則 6 (通解 > 特解)**: explicit sret is the GENERIC ABI-correct path;
  auto-demotion is a special-case LLVM pass we can't trust across versions.
- **§1.0 原則 9 (正确 > 妥协)**: correct ABI > no optimization. sret is the
  correct ABI per System V spec.
- **rustc reference**: rustc_codegen_llvm uses `Attribute::StructRet` explicitly.

### 2.2 Why keep the sret type parameter (e.g., `sret({ptr, i64, i64})`) over bare `sret`?
- LLVM 15+ opaque pointer mode allows `sret` without a type, but the type
  parameter is required for verification when the pointer's pointee type
  cannot be inferred from context (e.g., indirect calls via vtable).
- **§1.0 原則 4 (报错 > 静默)**: typed sret gives better diagnostics.
- **rustc reference**: rustc uses `Attribute::getWithStructRetType(ctx, ty)`.

### 2.3 Why touch declare_function + interpret_adhoc (not just function_begin)?
- **§20 (iterative audit)**: "发现一个 bug 意味着存在大量类似 bug".
  Forward declarations in declare_function and interpret_adhoc ALSO build
  function types from `ret_ty`. If they don't use sret, they create
  mismatched signatures with the actual function definition.
- Stage 18.188's "delete + re-add" hack handled the symptom but introduced
  race conditions (when two callers race to declare the same forward decl).
- The proper fix: forward decls use sret from the start.

## 3. Capability / Design / Responsibility Boundaries

### 3.1 Capability boundary
- `EmitType::needs_sret()` already exists (Stage 18.330). Both emitters
  agree on the sret threshold (size > 16 bytes per System V ABI).
- `EmitType::size_bytes_x86_64()` is simplified (no padding), but for the
  sret threshold this is correct — structs with 3+ pointer-sized fields
  always exceed 16 bytes regardless of padding.

### 3.2 Design boundary
- sret is added at IR-emission time (not at MIR-lowering time). This keeps
  MIR ABI-agnostic — sret is an x86-64 ABI concern, not a MIR concern.
- The sret pointer is named `%_sret` in both emitters (TextEmitter already
  uses this name). Consistent naming allows easier debugging.
- `emit_call` and `emit_ret` MUST agree on the sret contract with
  `emit_function_begin`. This is a 3-way contract enforced by `needs_sret()`.

### 3.3 Responsibility boundary
- `EmitType` (in emitter/mod.rs): owns sret threshold decision (`needs_sret`).
- LLVMSysEmitter (in llvm/{function,aggregate,mod}.rs): owns sret emission.
- MIR (in mir/lower/*): no sret awareness — uses `emit_call` / `emit_ret`
  through the `Emitter` trait, which abstracts over sret.
- Tests: verify sret correctness at the integration level (run actual binaries).

## 4. §20 Iterative Audit — Similar Bugs

Per "发现一个 bug 意味着存在大量类似 bug", audit these after sret is fixed:
1. **`emit_dyn_trait_method_call`** — vtable indirect call returning struct? Yes, needs sret.
2. **Drop glue** — `drop_glue.rs` calls functions; check if any return large structs.
3. **Function pointer calls** — `fn_name.starts_with('%')` path in `emit_call`; must add sret to call site.
4. **Closure return** — closures may return large captured environments.
5. **Trait object method dispatch** — same as dyn trait.
6. **ABI alignment in `size_bytes_x86_64`** — currently ignores padding. For structs like
   `{i8, i64, i64}` the real size with padding is 24 (not 17), but our function returns 17.
   This DOES NOT affect sret decision (both > 16), but may matter for `alloca` size. Audit.
7. **`__landin_str_eq`** returns i32 — no sret needed, but verify.
8. **Empty struct `{ }`** — already handled as i8 in llvm_type; verify sret never triggered.

## 5. Test Plan (§9.4.3 — 1:3 pos:neg ratio)

Positive (2):
1. `stage18_332_sret_vec_new_stress` — call `Vec::new()` 1000 times in a loop, verify no segfault.
2. `stage18_332_sret_multiple_returns` — multiple functions returning > 16B structs, verify values.

Negative (4):
3. `stage18_332_sret_caller_mismatch` — verify sret call site properly stores to alloca.
4. `stage18_332_sret_no_double_demote` — verify LLVM doesn't double-demote (sret-of-sret).
5. `stage18_332_sret_vtable_indirect` — dyn Trait method returning > 16B struct.
6. `stage18_332_sret_nested_struct` — nested struct return where outer > 16B (e.g., `{ {ptr, i64}, i64 }`).

Multi-thread stress:
- 5 runs of `cargo test --release --features llvm-backend` (default threads) — must be 5/5 stable.
