# Stage 18.335 — ZST Param Skip + __landin_eprintf Declare + Drop Glue Declare Removal

> **Author**: Super Z (main) — PM-A + ARCH-A + DEV-A + REV-A + QA-A
> **Date**: 2026-08-27
> **Complexity**: L3 (cross-module: codegen/function.rs + codegen/terminator.rs + codegen/pipeline.rs + codegen/drop_glue.rs + codegen/mir_translation/types.rs + tests)
> **Status**: planned → in-progress

## 1. 5W2H Analysis

### What
The §20 Round 4 audit found 3 P1 NEW bugs + 2 P2 latent bugs in the codegen layer.
All 3 P1 bugs are in the same family: **`EmitType::Void` leaks into IR positions where
LLVM only allows first-class types** (function parameters, allocas).

**3 P1 NEW bugs + 2 P2 latent bugs**:

1. **Bug 1 (P1 NEW) — TD-ZST-PARAM-VOID**: ZST (`()`) param produces `define void @foo(void %arg0)`
   → `llvm-as` rejects with "void type only allowed for function results".
   - File: `src/codegen/function.rs:325` (param type via `mir_type_to_emit_type_with_layouts_and_mono`)
   - File: `src/codegen/terminator.rs:299` (arg type via `detect_operand_type`)
   - Root cause: `src/codegen/mir_translation/types.rs:84` — `TyKind::Tuple(tys) if tys.is_empty() => EmitType::Void`
   - Same bug at call sites: `call void @foo(void 0)` is also invalid.

2. **Bug 2 (P1 NEW) — TD-EPRINTF-UNDECLARED**: `__landin_eprintf` is called from `eprintln!`/`eprint!`
   macros but never declared. Stage 18.334 added `printf` declare but missed `__landin_eprintf`.
   - File: `src/codegen/pipeline.rs:92` (after printf declare, missing eprintf)
   - TextEmitter IR: rejected by `llvm-as` with "use of undefined value '@__landin_eprintf'".
   - LLVMSysEmitter: implicit non-variadic declaration → ABI mismatch (eprintf is variadic).

3. **Bug 3 (P1 NEW) — TD-DROP-GLUE-REDECLARE**: `drop_glue.rs:101` emits redundant `declare` for
   `landin_<type>_drop` function, which conflicts with the later `define` from `codegen_function`.
   - File: `src/codegen/drop_glue.rs:99-102`
   - TextEmitter IR: rejected by `llvm-as` with "invalid redefinition of function".
   - LLVMSysEmitter: silently reuses the declaration (no error, but wasteful).

4. **Bug 4 (P2 latent) — TD-CALL-DEST-VOID-OVERRIDE**: `call_dest_type` override can produce
   `EmitType::Void` if callee returns `()`, and the `if ty == EmitType::Void { continue }` check
   fires BEFORE the override → `emit_alloca(&Void, ...)` is invalid.
   - File: `src/codegen/function.rs:359-374`

5. **Bug 5 (P2 docs) — TD-MISLEADING-ZST-COMMENT**: Comment in `mir_translation/types.rs:34-37`
   claims `alloca {}` is "valid, zero-size" — but per LLVM docs, size-0 allocas produce undef
   pointers (UB to dereference). The `i8` fallback (Stage 16.22) is the correct workaround.

### Why (root cause per §2.2 + §20)
- Stage 14.63 mapped `TyKind::Tuple([])` → `EmitType::Struct(vec![])` (ZST struct, correct).
- But `mir_type_to_emit_type_with_layouts` (without `_and_mono`) maps it to `EmitType::Void`
  (in `types.rs:84`) for legacy reasons.
- The codegen layer never filters out Void params/args before passing them to `emit_function_begin`
  / `emit_call` — LLVM requires first-class types in these positions.
- §20 Round 4 audit discovered this because we added `llvm-as` smoke tests (Stage 18.334) that
  catch the entire class of "TextEmitter IR silently invalid" bugs. Without those tests, the bug
  would have stayed latent in LLVMSysEmitter's "auto-create non-variadic decl" path.

### What NOT to change (per audit)
- **Do NOT replace `i8` with `{}` for ZST** — the audit confirmed this would reintroduce the
  `alloca {}` UB that Stage 16.22 fixed. Per LLVM docs, size-0 allocas produce undef pointers.
  The `i8` fallback (1-byte placeholder) is the correct workaround. Keep it.
- Just fix the misleading comment (Bug 5).

### Who (roles per §1.4)
- ARCH-A: design the "skip Void params/args" pattern (mirror rustc's ZST param elision)
- DEV-A: implement across 4 sites (function.rs + terminator.rs + pipeline.rs + drop_glue.rs)
- REV-A: review for soundness (no UB, no silent degradation)
- QA-A: regression + `llvm-as` smoke test for ZST/eprintln/Drop cases

### When
- Stop when §3.2 all-green AND `llvm-as` accepts IR for all 3 P1 bug repro programs.

### Where
- `src/codegen/function.rs` — filter Void params + move Void check after override
- `src/codegen/terminator.rs` — skip Void args in Call path
- `src/codegen/pipeline.rs` — add `__landin_eprintf` declare
- `src/codegen/drop_glue.rs` — remove redundant emit_declare
- `src/codegen/mir_translation/types.rs` — fix misleading comment
- `tests/v0/stage18/plan/stage18_335_*.rs` — regression tests

### How (implementation strategy)

#### 4.1 Bug 1 fix: skip Void params/args

In `codegen_function` (function.rs:305-331), filter out params whose type is `EmitType::Void`:
```rust
let params: Vec<(EmitType, String)> = (0..param_count)
    .filter_map(|i| {
        let local_idx = i + 1;
        let ty = /* ... compute ty ... */;
        if ty == EmitType::Void { return None; }  // ZST params not passed
        Some((ty, format!("%arg{}", i)))
    })
    .collect();
```

In `codegen_terminator` Call path (terminator.rs), skip Void args when building `arg_pairs`.

Note: MIR local indices are still 1-based — the filtering happens AFTER computing local_idx.
The function body's `local_ptr(local_idx)` lookups for ZST-param locals (which have no alloca)
already gracefully skip via the `if let Some(ptr) = ...` pattern.

#### 4.2 Bug 2 fix: add __landin_eprintf declare

In `pipeline.rs` after the printf declare (line 92):
```rust
emitter.emit_declare("void @__landin_eprintf(ptr, ...)");
```
This populates `variadic_fns` via `signature_is_variadic` in both backends.

#### 4.3 Bug 3 fix: remove drop_glue emit_declare

In `drop_glue.rs:99-102`, remove the `emit_declare` call. LLVM IR allows forward references
to functions defined later without a preceding `declare`. The `define` from `codegen_function`
handles the symbol.

#### 4.4 Bug 4 fix: move Void check after override

In `function.rs:359-374`, move the `if ty == EmitType::Void { continue }` check to AFTER the
`call_dest_type` override. This catches the case where the override produces Void.

#### 4.5 Bug 5 fix: update misleading comment

In `mir_translation/types.rs:25-41`, update the comment to reflect that `alloca {}` produces
undef pointers (UB to dereference), and the `i8` fallback is the correct workaround.

### How Much (acceptance per §3.2)
- `cargo fmt --check` ✅ 0 diff
- `cargo check --features llvm-backend` ✅ 0 errors, 0 warnings
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✅ 0 warnings
- `cargo test --release --features llvm-backend --test-threads=1` ✅ 0 failures
- Multi-threaded stress: ≥4/5 stable (2 threads)
- **NEW**: `llvm-as` accepts TextEmitter IR for:
  - `fn foo(u: ())` (ZST param)
  - `eprintln!("...")` (stderr macro)
  - `impl Drop for X` (drop trait)

## 2. Decision Points (per §2.2 + §12)

### 2.1 Why skip Void params (A) vs. map ZST to Struct(vec![]) (B)?
- **(B) Map to Struct(vec![])**: would require changing `mir_type_to_emit_type_with_layouts`
  (without `_and_mono`) to return `Struct(vec![])` instead of `Void`. But then the `i8` fallback
  would activate for these ZST params, allocating 1 byte per ZST param — wasteful but works.
  However: this changes the semantic meaning of `Void` (which is also used for true void returns).
- **(A) Skip Void params**: matches rustc's behavior — ZST params are elided from the LLVM
  signature entirely. This is the architecturally correct fix.
- **§1.0 原則 6 (通解 > 特解)**: skip Void params is the GENERIC pattern; mapping to Struct
  is a special-case that conflates ZST with non-ZST.
- **§1.0 原則 9 (正确 > 妥协)**: rustc elides ZST params; we should too.

### 2.2 Why remove drop_glue emit_declare (A) vs. fix the redefinition (B)?
- **(B) Fix the redefinition**: would require making drop_glue's `emit_declare` produce
  the EXACT same signature as `codegen_function`'s `define`. But LLVM `llvm-as` rejects
  `declare + define` of the same function even when signatures match (verified by audit).
- **(A) Remove the emit_declare**: LLVM IR allows forward references to functions defined
  later WITHOUT a preceding `declare`. The `define` from `codegen_function` handles the
  symbol. This is the simpler, more correct fix.
- **§1.0 原則 5 (去除兼容思维)**: the `emit_declare` was redundant — removing it eliminates
  the conflict.
- **§2.2 (根因思维)**: the root cause was the redundancy; removing it is the root-cause fix.

### 2.3 Why NOT change `i8` to `{}` for ZST (per audit recommendation)?
- The audit empirically verified that `alloca {}` produces undef pointers (UB to dereference).
- Stage 16.22 added the `i8` fallback specifically to avoid this UB.
- Replacing `i8` with `{}` would reintroduce the bug.
- The audit's prior recommendation (in Stage 18.334 plan) was based on a misleading comment
  that has now been corrected (Bug 5 fix).

## 3. Capability / Design / Responsibility Boundaries

### 3.1 Capability boundary
- `EmitType::Void` is unchanged — still used for true void returns and ZST returns.
- The codegen layer filters Void from params/args (not from returns — those are valid).
- The `i8` fallback for ZST allocas is retained (Stage 16.22 fix preserved).

### 3.2 Design boundary
- ZST params are elided from the LLVM signature (mirror rustc).
- `__landin_eprintf` is pre-declared as variadic in `pipeline.rs` (one place, both backends).
- Drop glue no longer emits redundant `declare` (LLVM forward-reference handles it).

### 3.3 Responsibility boundary
- `codegen_function`: filters Void params, moves Void check after override.
- `codegen_terminator`: skips Void args in Call path.
- `pipeline.rs`: pre-declares all variadic runtime functions (printf + eprintf).
- `drop_glue.rs`: no longer emits `declare` for drop method (define comes from codegen_function).
- `mir_translation/types.rs`: comment corrected (no behavior change).
