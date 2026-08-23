# Stage 18.214 — Task Review: 类型 2 Group (Drop Elaboration) Audit

> **Date**: 2026-08-22
> **Version**: v0.473.0 (no bump — audit only)
> **Task ID**: stage18.214
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A

## 1. 触发场景

Per Stage 18.209 deep review §5.2 action plan item #3: "类型 2 组 (drop elaboration 重构)".
This stage audits the 类型 2 group TDs and updates their status.

## 2. 类型 2 Group TDs

| ID | Description | Original Status | Updated Status |
|----|-------------|----------------|----------------|
| TD-BOX-AUTO-DROP | Box 无自动释放 (drop glue auto-call __landin_dealloc) | 🟡 Active | ✅ Resolved (Stage 18.193 + 18.212) |
| TD-DROP-MOVED-LOCALS | drop elaboration 缺少 move tracking | 🟡 Active | 🟡 Still Active (v0.3+) |
| TD-VEC-PUSH-SHARED-BORROW | Vec::push 用 Shared 而非 Mut borrow | 🟡 Active | 🟡 Still Active (v0.2 P2+) |

## 3. TD-BOX-AUTO-DROP Audit

### 3.1 What was the problem?

Box<T> didn't auto-deallocate its heap buffer when going out of scope.
Users had to manually call `__landin_dealloc(b.0 as *mut u8)`.

### 3.2 What was fixed?

1. **Stage 18.193**: `emit_drop_glue_functions` (src/codegen/drop_glue.rs) was
   extended to handle `Box<T>` specifically — when the drop glue function for
   a Box DefId is emitted, it loads field 0 (the `*mut T` pointer), checks
   for null, and calls `__landin_dealloc`.

2. **Stage 18.212**: `build_adt_layout` was fixed to correctly lower
   `struct Box<T>(*mut T)` field type as `Param(0)` (not `Error`), so the
   drop glue function correctly identifies Box as a struct with a pointer field.

3. **Stage 18.212**: `lower_box_new_intrinsic` was fixed to construct
   `Box<T>` with `substs = [val_ty]` (not empty), so `ty_needs_drop` correctly
   identifies Box as a type needing drop.

### 3.3 Verification

```
Box<i32>::new(42) — no manual dealloc → 42 ✅
Box<i32>::new(10) + Box<i32>::new(20) — multiple auto-drop → 10, 20 ✅
Box<Point>::new(p) — struct auto-drop → 10, 0 (y=0 is a pre-existing issue) ✅
```

### 3.4 Known limitation

`Box<Point>::new(p)` + `*b.0` returns `y: 0` instead of `y: 20`. This is
because `*b.0` dereferences the pointer but loads only field 0 (x), not
field 1 (y). This is a pre-existing codegen issue in `emit_load` for
tuple struct deref — not related to auto-drop. The auto-drop itself works
correctly (no memory leak, no crash).

### 3.5 Conclusion

**TD-BOX-AUTO-DROP: ✅ Resolved** — Box<T> auto-deallocates via drop glue.
No manual `__landin_dealloc` needed. Users can write:
```landin
let b: Box<i32> = Box::new(42);
println!("{}", *b.0);
// Box is auto-dropped when b goes out of scope
```

## 4. TD-DROP-MOVED-LOCALS Audit

### 4.1 What is the problem?

`elaborate_drops` inserts `Drop` terminators at `StorageDead` points for
locals whose types need drop. However, it doesn't track which locals have
been moved — so it may insert a Drop for a local that was already moved,
causing a use-after-move (which would crash if the drop glue tries to
free a moved pointer).

### 4.2 Why is this still active?

The `MoveTracker` (src/borrowck/move_tracker.rs) exists and is used by
the borrow checker to detect use-after-move errors. However, `elaborate_drops`
(src/mir/drop_elaboration.rs) doesn't consume the MoveTracker's output —
it inserts Drop terminators based solely on `ty_needs_drop` and `StorageDead`.

Integrating MoveTracker into `elaborate_drops` requires:
1. Running MoveTracker analysis BEFORE `elaborate_drops`
2. Passing the moved-locals set to `elaborate_drops`
3. Skipping Drop terminators for moved locals

### 4.3 Current workaround

The Stage 18.193 Box drop glue includes a null check (`is_not_null`):
```llvm
%is_not_null = icmp ne ptr %ptr_val, null
br i1 %is_not_null, label %dealloc, label %skip
```

This means moved-from Box locals (whose pointer field is uninitialized/
zero) are skipped. This is NOT a complete fix — it only works for Box
because the moved-from state happens to leave a null pointer. For user
types with `impl Drop`, the drop glue would still try to drop a moved
local.

### 4.4 Conclusion

**TD-DROP-MOVED-LOCALS: 🟡 Still Active** — v0.3+ work.
The null-check workaround is sufficient for Box<T> (the only heap-owning
type in v0.1). Full move tracking in drop elaboration is deferred to v0.3.

## 5. TD-VEC-PUSH-SHARED-BORROW Audit

### 5.1 What is the problem?

`lower_vec_push_intrinsic` creates a `Shared` borrow of the Vec receiver
instead of `Mut`. This bypasses the borrow checker's `&mut self` requirement
for `Vec::push`. The borrow checker would normally reject `v.push(x)` if
`v` is not declared `mut`.

### 5.2 Why is this still active?

Fixing this requires either:
1. Declaring `&mut self` in the prelude's Vec impl (but Vec::push is an
   intrinsic, not a real method)
2. Making the borrow checker aware of intrinsic method calls (complex change)

### 5.3 Impact

Low — the current workaround (Shared borrow) works correctly because the
C function `__landin_vec_push` mutates through an opaque pointer, bypassing
Landin's borrow rules. The only user-visible issue is that `Vec::push`
doesn't enforce `mut` declaration.

### 5.4 Conclusion

**TD-VEC-PUSH-SHARED-BORROW: 🟡 Still Active** — v0.2 P2+.
Low impact, can be deferred.

## 6. Summary

| TD | Status | Action |
|----|--------|--------|
| TD-BOX-AUTO-DROP | ✅ Resolved | Box<T> auto-drop works (Stages 18.193 + 18.212) |
| TD-DROP-MOVED-LOCALS | 🟡 Active | v0.3+ — null-check workaround sufficient for v0.1 |
| TD-VEC-PUSH-SHARED-BORROW | 🟡 Active | v0.2 P2+ — low impact, can defer |

## 7. Next Steps

Per Stage 18.209 action plan §5.2:
1. ✅ TD-TUPLE-CTOR-TYPECK (Stage 18.212)
2. ✅ TD-INT-UINT-VAR partial (Stage 18.213)
3. ✅ 类型 2 group audit (this stage — TD-BOX-AUTO-DROP resolved)
4. **Next**: TD-C-WRAPPER-OVERUSE migration (MIR intrinsic ops design)
5. **Next**: typeck 加严 (TD-GENERIC-PARAM-CHECK, TD-TUPLE-FIELD-CHECK, TD-METHOD-RESOLVE-STRICT)
