# Stage 18.334 — TextEmitter sret Syntax + sret Load + Variadic Detection

> **Author**: Super Z (main) — PM-A + ARCH-A + DEV-A + REV-A + QA-A
> **Date**: 2026-08-27
> **Complexity**: L3 (cross-module: codegen/text/{function,aggregate,mod}.rs + codegen/llvm/{module,mod,aggregate,helpers}.rs + tests)
> **Status**: planned → in-progress

## 1. 5W2H Analysis

### What
The Stage 18.333 §20 audit found that **TextEmitter's sret path silently produces invalid LLVM IR** (rejected by `llvm-as`/`llc`). Stage 18.332 added sret to TextEmitter but Stage 18.333's `byval` load-then-store fix wasn't mirrored correctly. Plus the deferred P1 (variadic detection hardcoded to name-list).

**2 P1 NEW bugs + 1 P2 NEW bug + 2 known bugs**:

1. **Bug 3 (P1 NEW) — TD-TEXT-SRET-SYNTAX**: TextEmitter emits `ptr sret %_sret` instead of `ptr sret(<ty>) %_sret`. LLVM 17+ opaque pointer mode requires the type argument.
   - File: `src/codegen/text/function.rs:31` + `src/codegen/text/aggregate.rs:31, 191`
   - Generated IR: `define void @landin_transform(ptr sret %_sret, ...)`
   - `llvm-as` rejects: `error: expected '('` at the `sret` token

2. **Bug 4 (P1 NEW) — TD-TEXT-SRET-STORE-PTR-AS-STRUCT**: TextEmitter's `emit_call` returns the sret alloca **pointer** instead of **loading the struct** from the sret slot. Caller's `emit_store(&dest_ty, &ret_val, &ptr)` then tries to store a `ptr` as a `struct` → type mismatch.
   - File: `src/codegen/text/aggregate.rs:108-116` (emit_call) + `:215-220` (emit_dyn_trait_method_call)
   - Generated IR: `store { i64, i64, i64 } %sret_9, ptr %loc_7` (where `%sret_9` is a `ptr`)
   - `llvm-as` rejects: `'%sret_9' defined with type 'ptr' but expected '{ i64, i64, i64 }'`

3. **Bug 5 (P2 NEW) — TD-TEXT-VARIADIC-DECL-SIGNATURE**: `count_args_in_signature("(ptr, ...)")` returns 2 (counts `...` as an arg). Variadic functions declared via `emit_declare("i32 @__landin_println(ptr, ...)")` end up as non-variadic with wrong arg types.
   - File: `src/codegen/llvm/module.rs:33-44` + `helpers.rs:186-201`

4. **Item 1 (P1 known) — TD-VARIADIC-DETECTION**: hardcoded `name == "printf" || name == "__landin_eprintf"` name-list.
   - File: `src/codegen/llvm/mod.rs:710` (declare_function) + `src/codegen/llvm/aggregate.rs:172` (emit_call)

5. **Item 2 (P2 known, deferred to 18.335) — TD-EMPTY-STRUCT-I8**: empty struct modeled as `i8`. Not in scope for this stage.

### Why (root cause per §2.2 + §20)
- Stage 18.332 added sret emission to **both** backends, but the **TextEmitter implementation was incomplete**:
  - Missing type argument in `sret` attribute (Bug 3)
  - Missing load instruction after `call void` (Bug 4)
- The §20 audit discovered this because "finding one bug (Stage 18.332/18.333 sret+byval) means there are many similar bugs" — the audit confirmed TextEmitter's sret path silently diverged from LLVMSysEmitter.
- **No test caught this** because:
  - TextEmitter IR is only used for `--emit-llvm-ir` debug path.
  - `--run` / `--emit-obj` use LLVMSysEmitter (which has the correct load-then-return path).
  - There's no `llvm-as` smoke test in CI to verify TextEmitter IR is valid.

### Who (roles per §1.4)
- ARCH-A: design sret type arg + load-then-return path (mirror LLVMSysEmitter)
- DEV-A: implement across TextEmitter 3 sites + variadic parsing
- REV-A: review for soundness (no UB, no silent degradation)
- QA-A: regression + `llvm-as` smoke test

### When
- Stop when §3.2 all-green AND `llvm-as` accepts TextEmitter IR for the byval+sret combined test.

### Where
- `src/codegen/text/function.rs` — emit_function_begin: sret type arg
- `src/codegen/text/aggregate.rs` — emit_call + emit_dyn_trait_method_call: sret load-then-return
- `src/codegen/llvm/helpers.rs` — parse `...` from signature
- `src/codegen/llvm/module.rs` — emit_declare: pass `is_variadic` to declare_function
- `src/codegen/llvm/mod.rs` — declare_function: accept `is_variadic` parameter
- `src/codegen/llvm/aggregate.rs` — emit_call: same
- `tests/v0/stage18/plan/stage18_334_*.rs` — regression + llvm-as smoke test

### How (implementation strategy)

#### 4.1 Bug 3 fix: `ptr sret(<ty>)` syntax

```rust
// text/function.rs:30-33 (before)
let sret_param_str: Option<String> = if use_sret {
    Some(format!("ptr sret {}", "%_sret"))
} else { None };

// (after)
let sret_param_str: Option<String> = if use_sret {
    let ret_str = emit_type_to_llvm_str(ret);
    Some(format!("ptr sret({}) {}", ret_str, "%_sret"))
} else { None };
```

Same fix at `text/aggregate.rs:31` (emit_call sret slot) and `:191` (emit_dyn_trait_method_call sret slot).

#### 4.2 Bug 4 fix: load struct from sret_slot

```rust
// text/aggregate.rs:108-116 (before)
if use_sret {
    self.line(&format!("  call void {}({})", call_target, all_args.join(", ")));
    sret_name.unwrap()  // BUG: returns ptr, not struct
}

// (after)
if use_sret {
    self.line(&format!("  call void {}({})", call_target, all_args.join(", ")));
    // Load the result from the sret slot (mirrors LLVMSysEmitter).
    let load_r = self.fresh();
    let ret_str = emit_type_to_llvm_str(ret_ty);
    self.line(&format!("  %v{} = load {}, ptr {}", load_r, ret_str, sret_name.as_ref().unwrap()));
    format!("%v{}", load_r)
}
```

Same fix at `text/aggregate.rs:215-220` (emit_dyn_trait_method_call).

#### 4.3 Item 1 + Bug 5 fix: variadic detection via signature parsing

```rust
// helpers.rs: new function
pub(crate) fn signature_is_variadic(signature: &str) -> bool {
    // A signature is variadic if it contains `...` inside the parens.
    let open = match signature.find('(') { Some(i) => i, None => return false };
    let close = match signature[open..].find(')') { Some(i) => open + i, None => return false };
    signature[open..close].contains("...")
}

// helpers.rs: extend count_args_in_signature to ignore `...`
pub(crate) fn count_args_in_signature(sig: &str) -> usize {
    let open = match sig.find('(') { Some(i) => i, None => return 0 };
    let close = match sig[open..].find(')') { Some(i) => open + i, None => return 0 };
    let inside = &sig[open + 1..close];
    if inside.trim().is_empty() { return 0; }
    // Filter out `...` token
    inside.split(',').filter(|s| {
        let s = s.trim();
        !s.is_empty() && s != "..."
    }).count()
}

// llvm/mod.rs::declare_function + llvm/aggregate.rs::emit_call:
// accept `is_variadic: bool` parameter (or detect from name list as fallback)
```

#### 4.4 `llvm-as` smoke test

Add a test that:
1. Compiles a Landin program via `--emit-llvm-ir`
2. Pipes the IR to `llvm-as-22` (or `llvm-as`)
3. Asserts exit 0 (valid IR)

This catches the entire class of "TextEmitter IR silently invalid" bugs.

### How Much (acceptance per §3.2)
- `cargo fmt --check` ✅ 0 diff
- `cargo check --features llvm-backend` ✅ 0 errors, 0 warnings
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✅ 0 warnings
- `cargo test --release --features llvm-backend --test-threads=1` ✅ 0 failures
- Multi-threaded stress: ≥14/15 stable
- **NEW**: `llvm-as` smoke test passes for `--emit-llvm-ir` output of a byval+sret test program.

## 2. Decision Points (per §2.2 + §12)

### 2.1 Why fix TextEmitter at all? (LLVMSysEmitter is the production path)
- Per §1.0 原則 6 (通解 > 特解): TextEmitter is the **reference implementation** that LLVMSysEmitter mirrors. If TextEmitter is wrong, it's a documentation/audit hazard — future developers will copy the wrong pattern.
- Per README.md:163 (current): "Both TextEmitter and LLVMSysEmitter agree on sret emission" — this claim is currently **false**. The fix makes the claim true.
- Per §1.0 原則 4 (报错 > 静默): silent IR invalidity is a class of bug that should be impossible to introduce — adding `llvm-as` smoke test ensures this.

### 2.2 Why signature parsing for variadic (vs. expanding the name-list)?
- Per §1.0 原則 6 (通解 > 特解): signature parsing is the GENERIC path; name-list is a special-case that breaks for any new variadic function.
- Per §1.0 原則 9 (正确 > 妥协): correct variadic detection from the source-of-truth (signature text) is the correct fix; the name-list is a workaround.
- Per §12 (最优 > 最小): root-cause fix at signature parsing, not at call-site.

### 2.3 Why add `llvm-as` smoke test?
- Per §20 (iterative audit): the audit discovered Bug 3 + Bug 4 because there was no automated check for TextEmitter IR validity. A `llvm-as` smoke test would have caught these bugs at Stage 18.332.
- Per §1.0 原則 4 (报错 > 静默): the bugs were silent (no test failure). Adding the smoke test ensures any future TextEmitter IR regression is caught immediately.
- This is the architectural fix that prevents this entire class of bug.

## 3. Capability / Design / Responsibility Boundaries

### 3.1 Capability boundary
- `EmitType::needs_sret()` and `needs_byval()` are unchanged — same threshold (Stage 18.332/18.333).
- `helpers::signature_is_variadic()` + extended `count_args_in_signature()` are new helpers.
- `declare_function` accepts `is_variadic: bool` parameter (replaces name-list inference).

### 3.2 Design boundary
- The sret load-then-return pattern is now identical across both backends (mirrors LLVMSysEmitter's `LLVMBuildLoad2` call).
- The `ptr sret(<ty>)` syntax matches LLVM 22 Language Reference (opaque pointer mode).
- Variadic detection is driven by signature text, not name-list.

### 3.3 Responsibility boundary
- `EmitType`: unchanged.
- TextEmitter: emits `ptr sret(<ty>) %name` + loads struct from sret slot.
- LLVMSysEmitter: unchanged (already correct per Stage 18.332).
- `helpers.rs`: parses `...` from signature.
- Tests: `llvm-as` smoke test catches future regressions.
