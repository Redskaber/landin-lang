# Stage 18.333 — byval ABI Support for Large Struct/Array Parameters

> **Author**: Super Z (main) — PM-A + ARCH-A + DEV-A + REV-A + QA-A
> **Date**: 2026-08-27
> **Complexity**: L3 (cross-module: codegen/llvm/{function,aggregate,mod,helpers}.rs + codegen/text/{function,aggregate}.rs + codegen/emitter/mod.rs + tests)
> **Status**: planned → in-progress

## 1. 5W2H Analysis

### What
LLVMSysEmitter and TextEmitter lack explicit `byval` handling for large struct/array
parameters (> 16 bytes). When a function takes a struct/array parameter > 16B, System V
AMD64 ABI §3.2.3 requires the parameter to be passed via a hidden pointer with the
`byval` attribute (mirrors `sret` for returns).

Currently Landin emits:
```llvm
define i64 @consume({ i64, i64, i64 } %v) { ... }   ; BUG: should be byval
```

Correct IR (after fix):
```llvm
define i64 @consume(ptr byval({ i64, i64, i64 }) %v) { ... }
```

### Why (root cause per §2.2 + §20)
- Stage 18.332 fixed `sret` (struct **return** > 16B) — same root cause but on the
  parameter side. Per §20 "finding one bug means there are many similar bugs",
  auditing the codegen layer uncovered 3 same-class bugs:
  1. **P0**: byval for large struct params (this stage)
  2. **P0**: byval for large array params (same code path, same fix)
  3. **P1**: variadic function detection is hardcoded to `printf | __landin_eprintf`
     name-list (deferred to Stage 18.334)
- rustc_codegen_llvm emits `Attribute::ByVal` explicitly via IR-level attributes;
  we mirror this via `LLVMCreateTypeAttribute(ctx, byval_kind, ty)` +
  `LLVMAddAttributeAtIndex` / `LLVMAddCallSiteAttribute`.

### Who (roles per §1.4)
- ARCH-A: design byval emission contract (mirrors sret pattern from Stage 18.332)
- DEV-A: implement across 6 sites (function_begin/declare/interpret_adhoc/call/dyncall/text)
- REV-A: review for soundness (no UB, no silent degradation, correct param index)
- QA-A: regression + multi-threaded stress tests

### When
- Stop when §3.2 all-green AND multi-threaded stress (8-thread, 15+ runs) is stable.
- Per §20, after this fix: re-audit codegen for any other ABI gaps.

### Where
- `src/codegen/emitter/mod.rs` — add `needs_byval()` next to `needs_sret()`
- `src/codegen/llvm/helpers.rs` — add `create_byval_attribute`
- `src/codegen/llvm/function.rs` — emit_function_begin: per-param byval
- `src/codegen/llvm/mod.rs` — declare_function + interpret_adhoc: per-param byval
- `src/codegen/llvm/aggregate.rs` — emit_call + emit_dyn_trait_method_call: byval args
- `src/codegen/text/function.rs` — emit_function_begin: per-param byval (text)
- `src/codegen/text/aggregate.rs` — emit_call + emit_dyn_trait_method_call: byval args (text)
- `tests/v0/stage18/plan/stage18_333_byval_abi_tests.rs` — regression tests

### How (implementation strategy)

#### 4.1 EmitType::needs_byval() — single source of truth

```rust
/// Stage 18.333: Returns true if this type needs byval when passed as a
/// function parameter (struct/array > 16 bytes per System V ABI).
/// Same threshold as needs_sret — both are driven by ABI size > 16.
pub fn needs_byval(&self) -> bool {
    self.size_bytes_x86_64() > 16
}
```

Per §1.0 原則 6 (通解 > 特解): one threshold function for both sret and byval.
Per §1.0 原則 4 (显式 > 隐式): the threshold is explicit at the IR level.

#### 4.2 create_byval_attribute helper (mirrors create_sret_attribute)

```rust
pub(crate) fn create_byval_attribute(ctx, ty) -> LLVMAttributeRef {
    let byval_kind = LLVMGetEnumAttributeKindForName(b"byval", 5);
    LLVMCreateTypeAttribute(ctx, byval_kind, ty)
}
```

#### 4.3 emit_function_begin (LLVMSysEmitter) — per-param byval

The param index shifts when sret is active:
- `use_sret=false`: LLVM param indices are 1..N (1-indexed), user params at indices 1..N
- `use_sret=true`: LLVM param 1 is sret slot, user params at indices 2..N+1

For each user param `i` (0-indexed) where `params[i].0.needs_byval()`:
1. Replace the param LLVM type with `ptr` (opaque pointer)
2. Add `byval(<orig_ty>)` attribute at `LLVMAddAttributeAtIndex(fn_val, user_param_llvm_idx, attr)`
   where `user_param_llvm_idx = i + 1 + (1 if use_sret else 0)`
3. Register the param under its name (param value is now `ptr`)

#### 4.4 emit_call (LLVMSysEmitter) — call site byval

For each user arg `i` where `args[i].0.needs_byval()`:
1. Allocate slot via `entry_block_alloca(orig_ty, name)`
2. Store the arg value to the slot
3. Replace the arg value with the slot pointer
4. Replace the LLVM param type with `ptr`
5. Add `byval(<orig_ty>)` attribute at `LLVMAddCallSiteAttribute(call_val, arg_llvm_idx, attr)`
   where `arg_llvm_idx = i + 1 + (1 if use_sret else 0)`

#### 4.5 declare_function + interpret_adhoc — forward decl byval

Forward declarations must use the same byval signature as the actual function
definition, otherwise LLVM reuses the wrong-typed decl (the Stage 18.188 hack).

Same pattern: replace param types with `ptr`, add byval attribute at correct index.

#### 4.6 TextEmitter — mirror changes

TextEmitter emits the textual IR `ptr byval(<ty>) %name` for each byval param.

### How Much (acceptance per §3.2)
- `cargo fmt --check` ✅ 0 diff
- `cargo check --features llvm-backend` ✅ 0 errors, 0 warnings
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✅ 0 warnings
- `cargo test --release --features llvm-backend --test-threads=1` ✅ 0 failures
- Multi-threaded stress (8 threads, 15+ runs): ≥14/15 stable (1 flake allowed for system resource limits)

## 2. Decision Points (per §2.2 + §12)

### 2.1 Why explicit byval (A) over LLVM auto-lowering (B)?
- **(B) Auto-lowering**: relies on LLVM's CodeGenPrepare to detect `define T @fn(T %x)`
  for large struct T and rewrite to `define T @fn(ptr byval(T) %x)`. Stage 18.329
  found this unreliable for sret; same applies to byval.
- **(A) Explicit byval**: emit the correct signature at IR level. Both caller and
  callee agree on the ABI from the start.
- **§1.0 原則 6 (通解 > 特解)**: explicit byval is the GENERIC ABI-correct path.
- **§1.0 原則 9 (正确 > 妥协)**: correct ABI > no optimization.
- **rustc reference**: rustc_codegen_llvm uses `Attribute::ByVal` explicitly.

### 2.2 Why same `needs_byval()` threshold as `needs_sret()`?
- Both are driven by System V ABI §3.2.3: "If the size of an object is larger than
  eight-bytes (or contains any 16-byte aligned fields), it is passed via pointer
  (sret for return, byval for parameter)."
- For Landin's simplified `size_bytes_x86_64()` (which ignores alignment), the
  threshold `> 16` covers both cases — same function.
- **§1.0 原則 6 (通解 > 特解)**: one threshold function for both sret and byval.

### 2.3 Why mirror the byval fix in TextEmitter too?
- TextEmitter is the `--emit-llvm-ir` path — used for debugging, but also as
  the reference implementation that LLVMSysEmitter mirrors.
- If TextEmitter doesn't emit byval, the dumped IR is wrong (mismatches what
  LLVMSysEmitter would produce). This causes confusion when debugging.
- **§1.0 原則 6 (通解 > 特解)**: one byval emission pattern across both backends.

## 3. Capability / Design / Responsibility Boundaries

### 3.1 Capability boundary
- `EmitType::needs_byval()` and `EmitType::needs_sret()` share the same threshold
  (`size_bytes_x86_64() > 16`). The distinction is **semantic** (return vs param),
  not threshold-based.
- The `entry_block_alloca` helper (Stage 18.332) is reused for byval slots.

### 3.2 Design boundary
- byval is added at IR-emission time (not at MIR-lowering time).
- The sret pointer is named `%_sret`; byval slots are named `%byval_argN`
  (where N is the 0-indexed user arg position).
- `emit_call` and `emit_function_begin` MUST agree on the byval contract.

### 3.3 Responsibility boundary
- `EmitType`: owns byval threshold decision (`needs_byval`).
- LLVMSysEmitter + TextEmitter: own byval emission.
- MIR: no byval awareness — uses `emit_call` / `emit_function_begin` through the
  `Emitter` trait, which abstracts over byval.
- Tests: verify byval correctness at integration level (run actual binaries).

## 4. §20 Iterative Audit — After byval fix

Per §20, after this fix, audit these again:
1. **Variadic function detection** (P1) — hardcoded `name == "printf"` list.
   Plan: Stage 18.334 — parse `...` from `emit_declare` signature.
2. **Empty struct modeling** (P2) — `i8` fallback should be LLVM `{}`.
   Plan: Stage 18.335 — replace `i8` with empty struct type.
3. **ABI alignment** (audited, no bug) — keep monitoring.
4. **Inreg** (audited, no bug) — keep monitoring.
