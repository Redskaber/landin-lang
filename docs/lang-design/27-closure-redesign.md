# v0.2 Phase 2: Closure Redesign Design (HP-3)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.178.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 10**: Synthesized `call` function per closure (HP-3)
> **Dependency**: Task 1 (Ty interning) — COMPLETE (Stage 15.28)

## 1. Problem Statement

The current compiler uses an **inline approach** for closure calls
(Stage 13.3a, TD-030). When a closure is called (`f(args)`), the
compiler inlines the closure body directly at the call site. This works
for simple cases but has limitations:

1. **No separate `call` function**: The closure body is duplicated at
   every call site, increasing code size.
2. **No `Fn`/`FnMut`/`FnOnce` trait support**: The inline approach
   doesn't produce a callable function pointer, so trait-based dispatch
   on closures is impossible.
3. **No recursion**: A closure cannot call itself (the body is inlined,
   not a separate function).
4. **No closure-as-value**: A closure cannot be stored in a variable
   and called later from a different context (the inlining happens at
   the call site, not at the binding).

### Current state (v0.178.0)

- `HirExprKind::Closure` lowers to a closure struct (capturing environment)
  + an inline call mechanism.
- `closure_capture.rs` handles extracting captures from the closure struct.
- `expr_operand.rs` has `inline_closure_call` which inlines the body at
  the call site.
- The inline approach was a pragmatic MVP (Stage 13.3a) — the full
  Strategy A (synthesized `call` function) was deferred to v0.2.

## 2. Design: Strategy A — Synthesized `call` Function

### 2.1 Overview

For each closure literal `|params| body`, the compiler synthesizes a
**separate `call` function**:

```rust
// Source:
let f = |x: i32| x + 1;
f(42)

// Synthesized:
fn __closure_call_0(captures: &Captures, x: i32) -> i32 {
    x + 1
}
// Call site:
__closure_call_0(&captures, 42)
```

The closure value is a fat pointer `{ fn_ptr, captures_ptr }`:
- `fn_ptr`: pointer to the synthesized `__closure_call_N` function.
- `captures_ptr`: pointer to the captures struct (stack-allocated).

When the closure is called (`f(args)`), the codegen:
1. Loads `fn_ptr` from the closure value.
2. Loads `captures_ptr` from the closure value.
3. Calls `fn_ptr(captures_ptr, args)`.

### 2.2 What's already implemented

- **Closure capture extraction** (`closure_capture.rs`): extracts captured
  variables from the environment and stores them in a closure struct.
- **Closure struct type** (`TyKind::Closure(def_id, substs)`): represents
  the closure type in MIR.
- **Inline closure call** (`expr_operand.rs::inline_closure_call`): inlines
  the body at the call site (the MVP approach — will be replaced).
- **Closure field access**: codegen can access closure struct fields
  (captures) via projection.

### 2.3 What needs to be implemented

| Step | Description | Effort |
|------|-------------|--------|
| 1 | **Synthesize `call` function per closure** — during MIR lowering, create a new `MirBody` for each closure's `call` function. The body takes `captures: &Captures` as the first parameter, followed by the closure's params. | 1 week |
| 2 | **Closure value as fat pointer** — the closure value is `{ fn_ptr, captures_ptr }`. Change the closure type representation from a struct to a fat pointer. | 3 days |
| 3 | **Closure call codegen** — when calling a closure value, load `fn_ptr` + `captures_ptr` and call `fn_ptr(captures_ptr, args)`. | 2 days |
| 4 | **`Fn`/`FnMut`/`FnOnce` trait impls** — auto-implement these traits for closures (enables passing closures to generic functions). | 3 days |
| 5 | **Testing** — conformance tests with closures in all positions. | 2 days |

Total: ~2-3 weeks (per v0.2-preparation.md).

### 2.4 Synthesized `call` function

When the MIR lower encounters `HirExprKind::Closure { params, body, captures, .. }`:

1. Create a new `MirBody` for the `call` function:
   - Parameter 0: `captures: &CapturesStruct` (reference to the captures).
   - Parameters 1..N: the closure's parameters.
   - Body: the closure's body, with captures accessed via `(*captures).field_i`.
   - Return type: the closure's return type.

2. Register the `call` function in `fn_name_by_def_id` as
   `__closure_call_<N>` (where N is a unique counter).

3. The closure value is a fat pointer `{ fn_ptr, captures_ptr }`:
   - `fn_ptr` = `@__closure_call_<N>`.
   - `captures_ptr` = pointer to the stack-allocated captures struct.

### 2.5 Closure call codegen

When the codegen encounters `TerminatorKind::Call` where the callee is
a closure value:

1. Load `fn_ptr` from the closure value (field 0 of the fat pointer).
2. Load `captures_ptr` from the closure value (field 1).
3. Call `fn_ptr(captures_ptr, args...)`.

This is similar to dyn Trait vtable calls, but simpler (single slot).

### 2.6 Migration strategy

The inline approach (Stage 13.3a) will be **retained as a fallback**
during migration. A feature flag or runtime check can switch between
inline and synthesized approaches. Once the synthesized approach is
verified, the inline code path will be removed.

## 3. Dependencies

- **Task 1 (Ty interning)**: COMPLETE (Stage 15.28). The `Ty` type is
  cheap to clone, which is needed for the closure type representation.
- **Closure capture infrastructure**: EXISTS (Stage 6.2, `closure_capture.rs`).
- **Inline closure call**: EXISTS (Stage 13.3a, `expr_operand.rs`).

## 4. What's NOT in scope for v0.2 MVP

- `move` closures (explicit `move |x| ...` syntax) — future.
- `async` closures — future (requires async/await support).
- Closure coercion to `fn` pointers (non-capturing closures) — future.
- Generic closures (closures as generic parameters) — requires monomorphization.

## 5. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `synthesize_closure_call` | `<verb>_<noun>_<noun>` (free function) | ✅ |
| `__closure_call_<N>` | `__<noun>_<noun>_<id>` (synthesized function name) | ✅ |
| `ClosureValue` | `<Noun><Noun>` (fat pointer struct, if needed) | ✅ |

## 6. Open Questions

1. **Capture struct layout**: How are captures ordered in the struct?
   Currently they're in declaration order — should this change?

2. **Capture by value vs by reference**: Should captures be moved into
   the closure struct, or borrowed? Rust moves by default (`move` keyword)
   and borrows otherwise. v0.2 MVP: borrow (simpler).

3. **Interaction with `elaborate_drops`**: The captures struct needs to
   be dropped when the closure goes out of scope. Does `elaborate_drops`
   handle this?

4. **Interaction with region allocation**: The captures reference's
   lifetime needs to be tracked. Does the region inference handle this?

These will be resolved in the implementation stages.

## 7. Effort

- 2-3 weeks (per v0.2-preparation.md)
- Stages 15.53 (design) + 15.54-15.57 (implementation) + 15.58 (review)
- Each stage independently testable.
