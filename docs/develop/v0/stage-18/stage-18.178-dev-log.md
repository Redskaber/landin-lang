# Stage 18.178 — Heap Allocation Infrastructure (TD-HEAP-ALLOC)

> **Date**: 2026-08-17
> **Version**: v0.445.0 → v0.446.0
> **Task ID**: stage18.178
> **Agent**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **Depends on**: Stage 18.177 (task review)
> **Blocks**: Stage 18.179 (Box MVP), Stage 18.180 (Vec MVP), Stage 18.181 (real String)

## 1. Scope

Per Stage 18.177 task review: implement heap allocation infrastructure
(`__landin_alloc` / `__landin_dealloc` runtime stubs + extern fn bug fixes)
to unblock Box<T> (18.179), Vec<T> (18.180), and real String (18.181).

## 2. Implementation

### 2.1 C Wrapper Stubs (src/codegen/runtime.rs)

Added two new runtime stubs:

```c
void* __landin_alloc(long long size) {
    void* ptr = malloc((size_t)size);
    if (ptr == 0) {
        fprintf(stderr, "panic: memory allocation failed (size=%lld)\n", size);
        exit(1);
    }
    return ptr;
}
void __landin_dealloc(void* ptr) {
    if (ptr == 0) return;  /* free(NULL) is a no-op per C standard */
    free(ptr);
}
```

Design decisions:
- `__landin_alloc` takes `i64` size (Landin's `isize`/`usize` width), casts to `size_t` for malloc
- OOM panics with exit(1) per §2 原則 4 (报错>静默)
- `__landin_dealloc` is NULL-safe (matches `free(NULL)` semantics)
- Names follow `__landin_<verb>_<noun>` pattern per api-naming-standard §8.1
- One allocation interface for ALL future heap types (Box/Vec/String/Rc/Arc) per §1.0 原則 6 (通解>特例)

### 2.2 Bug Fixes Discovered During Testing

Stage 18.178's tests are the first to actually CALL extern functions end-to-end.
This exposed 4 latent bugs that previous tests (compile-only) missed:

#### Bug 1: Extern block ABI not propagated to inner fns (src/hir/lower/item.rs)

**Root cause**: `lower_extern_block` called `lower_fn(fn_decl, ...)` for each
foreign fn, but `lower_fn` reads `fn_decl.sig.abi` which defaults to
`Abi::Landin` (the parser doesn't push the extern block's ABI into each
fn_decl). So `extern "C" { fn f(); }` produced a fn with `abi: Landin`.

**Fix**: After `lower_fn`, override `hir_fn.sig.abi = block_abi`.

Per §1.0 原則 3 (显式>隐式): propagate the ABI explicitly.
Per §1.0 原則 6 (通解>特解): one propagation rule for all extern blocks.

#### Bug 2: Extern fns registered with wrong DefKind (src/resolve/module_build.rs)

**Root cause**: The `HirItem::Fn` arm in `collect_item_registration` always
registered fns as `DefKind::Fn`, even when they were extern fns (which should
be `DefKind::ExternFn`). This prevented codegen from distinguishing them.

**Fix**: Check `f.sig.abi` — if not `Abi::Landin`, register as `DefKind::ExternFn`.

Per §1.0 原則 6 (通解>特例): one rule based on ABI, not a name list.

#### Bug 3: Extern fn names mangled with `landin_` prefix (src/driver/driver_codegen_prep.rs)

**Root cause**: `populate_fn_name_by_def_id` applied `landin_<name>` mangling
to ALL `HirItem::Fn` owners, including extern fns. So `__landin_alloc` became
`landin___landin_alloc` — a symbol that doesn't exist in the C runtime.

**Fix**: Check `f.sig.abi` — if not `Abi::Landin`, preserve the name as-is.

Per §1.0 原則 3 (显式>隐式): extern fn names are explicit contracts with
external code — preserve them literally.

#### Bug 4: `__landin_*` resolver fallback DefId collision (src/resolve/path_resolve.rs)

**Root cause**: The resolver intercepted ANY `__landin_*` name and gave it a
synthetic DefId. Known names (println/print/eprintln/eprint) got
`DefId(u32::MAX - i)`, unknown names got `DefId(u32::MAX)` as fallback. But
`u32::MAX - 0 == u32::MAX` — so the fallback collided with `__landin_println`'s
DefId. This caused `__landin_alloc` / `__landin_dealloc` calls to be silently
misresolved to `__landin_println`, producing garbage IR (the call was
compiled as `printf("\n")`).

**Fix**:
1. Try real module tree resolution FIRST — if the user declared
   `extern "C" { fn __landin_alloc(...); }`, use the real DefId.
2. Use `u32::MAX - 1 - i` (offset by 1) for synthetic DefIds to avoid
   collision with `u32::MAX`.
3. Unknown `__landin_*` names without extern declaration now fall through to
   normal resolution, producing a clean "cannot find value" error.

Per §1.0 原則 4 (报错>静默): unknown names must error, not silently misresolve.
Per §2 原則 9 (正确>妥协): fix the root cause (DefId collision), not the
symptom (special-case more names).

### 2.3 Bug Fixes Discovered via Runtime Tests (segfault)

#### Bug 5: DCE removes `let p = call_result` when only used via `*p = val` (src/mir/optimization.rs)

**Root cause**: `collect_read_locals` only collected locals from the rvalue
of an `Assign`. For `*p = 42` (which is `Assign(Projection(Local(p), Deref), Use(42))`),
the rvalue is `42` (no locals). So `p` was NOT marked as used → DCE removed
`let p = call_result` → `p` was uninitialized at runtime → segfault.

**Fix**: `collect_read_locals` now also collects locals from the LHS place
when it's a `Projection(_, Deref)` (the base local is read to get the pointer).
Also handles `Projection(_, Index(i))` (the index `i` is read).

Per §1.0 原則 4 (报错>静默): DCE must not silently remove used assignments.
Per §1.0 原則 6 (通解>特例): one rule for all projection kinds — Deref reads
base, Index reads index, Field reads nothing.

#### Bug 6: Deref codegen treats `*raw_ptr` as no-op (src/codegen/mir_translation/places.rs)

**Root cause**: `codegen_place_load_typed` for `ProjectionElem::Deref` checked
if the base's type is `TyKind::Ref(_, _, _)` to decide whether to load through
the pointer. But it didn't include `TyKind::RawPtr(_, _)`. So `*p` where
`p: *mut u8` returned the pointer value (i64 bits) instead of loading the
byte at the address.

**Fix**: Check for both `Ref` and `RawPtr` in the `base_is_ptr` predicate.

Per §1.0 原則 6 (通解>特例): one check for both pointer types.
Per §2 原則 9 (正确>妥协): fix the root cause (include RawPtr), not the
symptom (special-case raw pointer loads).

### 2.4 Tests (tests/v0/stage18/plan/stage18_178_heap_alloc_tests.rs)

10 tests covering:
- Positive (5): alloc+dealloc smoke, store/load cycle, i32 store/load,
  multiple distinct allocations, dealloc(NULL) no-op
- Negative (5): undeclared alloc fails, wrong arg count fails, wrong arg
  type fails, store through *const fails, OOM panics not returns NULL

All 10 pass. Negative ratio: 50% (exceeds §9.4.3 25% target).

## 3. §3.2 Acceptance

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors (22 pre-existing test warnings)
- ✅ cargo test --features llvm-backend --lib: 658 passed (was 656, +2 new runtime.rs tests)
- ✅ cargo test --features llvm-backend --tests: 2977 passed (was 2967, +10 new heap alloc tests)
- **Total**: 3635 tests, 0 failures

## 4. Tech Debt Status

| ID | Status |
|----|--------|
| TD-HEAP-ALLOC | ✅ Resolved (Stage 18.178) — codegen can now call malloc/free via `__landin_alloc`/`__landin_dealloc` |
| TD-STRING-AS-STR-ALIAS | 🟡 Active — Stage 18.181 will fix (depends on 18.179 Box + 18.180 Vec) |
| TD-VEC-MVP | 🟡 Active — Stage 18.180 will fix (depends on 18.179 Box) |
| TD-NO-FORMAT-MACRO | 🟡 Active — Stage 18.182 will fix (depends on 18.181 real String) |

## 5. Next Steps

Stage 18.179: Box<T> MVP
- Prelude inject `struct Box<T>(*mut T)`
- MIR lower intrinsic for `Box::new(x)` → malloc + store
- Drop glue extension: Box types auto-call `__landin_dealloc`
- Deref trait integration (or MVP: direct field access `(*b).0`)
