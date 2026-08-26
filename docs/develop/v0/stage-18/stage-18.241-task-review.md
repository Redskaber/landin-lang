# Stage 18.241 — Task Review: Primitive Type Impl (impl str) Support

> **Date**: 2026-08-23
> **Version**: v0.485.0 → v0.486.0 (planned)
> **Task ID**: stage18.241
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8
> **设计文档**: docs/lang-design/03-type-system.md

## 1. 触发场景

Per Stage 18.240 v0.3 transition plan: Phase 1 Task 1 = Primitive type impl.
Per Stage 18.239 audit: str::len/is_empty/as_bytes blocked by primitive type impl.

## 2. 依赖与基础设施完整能力审查

### 2.1 Current Architecture

The `resolve_inherent_method` function (method_resolution.rs:160) checks:
```rust
let adt_def_id = match &recv_ty.kind {
    TyKind::Adt(def_id, _) => *def_id,
    _ => return None,  // ← str (TyKind::Str) returns None here
};
```

For `str` (TyKind::Str), there's no Adt DefId, so inherent method resolution
fails. The MIR lower then falls through to the hardcoded `method_name_str == "len"`
check.

### 2.2 The Fix

**Approach**: Extend `resolve_inherent_method` to handle primitive types (str, slice).

When `recv_ty` is `TyKind::Str`:
1. Search impl blocks whose `self_ty` is `str` (a Path resolving to `PrimTy::Str`)
2. If found, resolve the method normally (same as Adt)

The impl block in prelude source would be:
```landin
impl str {
    fn len(&self) -> i64 { self.len }   // Wait — str is a fat pointer { ptr, i64 }
    fn is_empty(&self) -> bool { self.len == 0 }
    fn as_bytes(&self) -> &[u8] { self }  // same fat pointer layout
}
```

Actually, `str` is a fat pointer `{ ptr, i64 }` where field 0 = data ptr,
field 1 = length. So `self.len` would access the length field.

But wait — `str` is not a struct with named fields. It's a built-in fat
pointer type. The `self.len` field access would need to work on `&str` (a
fat pointer reference), accessing field 1 (the length).

In Landin's MIR, `&str` is represented as `Ref(Region, Immutable, Str)`.
Field projection on a `&str` would project into the fat pointer's fields
(field 0 = data ptr, field 1 = length).

### 2.3 Challenge: str field access in Landin source

When the user writes `self.len` inside `impl str { ... }`, the receiver
`self` has type `&str` (it's `&self`). The `self.len` field access needs
to:
1. Auto-deref `&str` → `str` (the fat pointer value)
2. Access field 1 of the fat pointer (the length)

But `str` is not an Adt — it's a built-in type. The MIR lower's field
projection might not handle this case.

**Per §17.8 (任务审查)**: This requires checking if the existing field
projection codegen handles `str` fat pointer fields. If not, we need to
extend it — which is a significant code change.

### 2.4 Simpler Approach: Keep str intrinsics, extend method resolution

Instead of writing `impl str { ... }` in prelude source, we can:
1. Extend `resolve_inherent_method` to check for built-in type methods
2. Keep the hardcoded MIR lowering for str methods (they already work)
3. But route the method resolution THROUGH the standard path

This way, `s.nonexistent_method()` on a `&str` would correctly report
"no method found" instead of silently being accepted.

**Per §1.0 原則 6 (通解 > 特解)**: This is still a 特解 (hardcoded method
list for str), but it removes the need for `impl str` syntax — which is
a large language feature to add.

### 2.5 Decision: Keep str intrinsics, extend method resolution for str

Per user directive "依赖与基础设施完整能力审查":
- Adding `impl str` syntax requires parser + HIR + typeck changes (~200 LOC)
- The benefit is removing 3 hardcoded checks (~100 LOC)
- The ROI is low, and the feature is complex (str is not an Adt)

Instead, extend `resolve_inherent_method` to recognize `TyKind::Str` and
check a built-in method registry. This is a smaller change that unblocks
the key issue: `s.nonexistent_method()` on `&str` should fail.

## 3. Implementation Plan

### 3.1 Files to Modify

| File | Change | LOC |
|------|--------|-----|
| `src/mir/lower/method_resolution.rs` | Add str to resolve_inherent_method | +20 |
| `src/stdlib/mod.rs` | Add built-in str method registry | +15 |

### 3.2 Test Plan

| Test | Category | Expected |
|------|----------|----------|
| `str::len()` works | Positive | `"hello".len()` = 5 |
| `str::is_empty()` works | Positive | `"".is_empty()` = true |
| `str::nonexistent()` fails | Negative | Compile error |

## 4. Recommendation

**Proceed with extending method resolution for str** (not full `impl str` syntax).

This is the 通解 for the method resolution issue — it routes all method
calls through the standard resolution path, catching unknown methods.
The actual MIR lowering for str methods stays hardcoded (MVP, §17.6),
but the resolution path is unified.
