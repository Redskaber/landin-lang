# Stage 18.284 — TD-INTRINSIC-OVERUSE Phase 2-A: Primitive Type Method Resolution via Prelude Impls

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-25
> **Version**: v0.492.0 → v0.493.0 (planned)
> **Process**: stage-committee-process.md v7.3 §13.4 (重构即架构设计) + §13.5 (设计-审查循环) + §18 (依赖审查) + §17.8 (任务审查)
> **Status**: Design — awaiting REV-A review

---

## 1. Problem Statement

`TD-INTRINSIC-OVERUSE` Phase 2 (registered in tech-debt-register.md) tracks
the fact that **stdlib method intrinsics are hardcoded in MIR-lower dispatch**
instead of being declared as regular `impl` blocks in prelude source. This
violates §1.0 原則 6 (通解 > 特解) — each primitive type's methods (str::len,
str::is_empty, str::as_bytes, etc.) require:

1. An early-interception check in `expr_variants.rs` (3+ sites, lines 1377/1413/1472)
2. A whitelist in `checker.rs::check_deferred_method_calls` (`KNOWN_INTRINSIC_METHODS`, line 228)
3. A "known str methods" list in `expr_variants.rs` error reporting (line 1682)
4. No visible signature in prelude → users can't see what methods exist on `str`

Per Stage 18.239 audit: Phase 2 was deferred because all intrinsics need
language features (primitive type impl, fat pointer construction, extern C in
prelude). However, Stage 18.283's §18 audit identified that **primitive type
impl syntax is already partially supported** — `impl str { fn ... }` parses,
HIR-lowers, and name-resolves successfully. The remaining gap is method
resolution at the call site.

### 1.1 Verification (Stage 18.283 claim)

Tested with `compile()` on:
```landin
impl str {
    fn my_len(&self) -> i64 { 42 }
}
fn main() {
    let s: &str = "hello";
    let n = s.my_len();
}
```

Result:
- ✅ Parse: no error
- ✅ HIR lower: no error
- ✅ Name resolution: no error
- ❌ Typeck: "no method `my_len` found for type `str`"

The typeck error originates from `expr_variants.rs:1685-1688` (MIR-lower
reports a TypeError when method resolution fails). The actual method resolution
is `resolve_inherent_method` (`method_resolution.rs:160`), which only handles
`TyKind::Adt` (line 182-184: `let adt_def_id = match &recv_ty.kind { TyKind::Adt(def_id, _) => *def_id, _ => return None, };`).

### 1.2 Scope Decision (Phase 2-A)

Phase 2 was scoped as a single monolithic migration (all intrinsics). Stage
18.283 identified that the **primitive type impl portion** can be partially
unblocked without the other blockers (fat pointer construction, extern C in
prelude). Per §17.8 (任务审查) + §12 (最优 > 最小):

- **Phase 2-A** (this stage): str::len, str::is_empty, str::as_bytes
  - These three methods don't need fat pointer CONSTRUCTION (they only READ
    the fat pointer's existing fields via Field projection, or no-op return)
  - str::len: `Field(1)` projection on `&str` → returns i64 (the length)
  - str::is_empty: `Field(1)` + `== 0` → returns bool
  - str::as_bytes: no-op (returns receiver, same fat pointer layout)

- **Phase 2-B** (future): String::as_str
  - Requires fat pointer construction syntax (lang feature)

- **Phase 2-C** (future): String::from_str, push_str, Vec::push/get, Box::new
  - Requires extern "C" in prelude impl bodies (lang feature)

---

## 2. §18 Dependency Audit Results

### 2.1 Infrastructure capability

| Capability | Location | Status |
|-----------|----------|--------|
| `impl str { fn ... }` parse | parser accepts Path self_ty | ✅ Ready |
| HIR `lower_impl` | `hir/lower/item.rs:435` handles any HirTy self_ty | ✅ Ready |
| Method name lookup in impl blocks | `method_resolution.rs:195-222` iterates impl owners | ✅ Ready |
| `TyKind::Str` recognition | `mir/ty.rs:109` | ✅ Ready |
| Field projection MIR emit | `expr_variants.rs:1389-1395` (existing intrinsic) | ✅ Pattern reusable |

### 2.2 Gap (must implement)

| Gap | File | Line | Fix |
|-----|------|------|-----|
| `resolve_inherent_method` rejects non-Adt | `method_resolution.rs` | 182-184 | Map primitive TyKind → string name |
| No prelude `impl str { ... }` declarations | `stdlib/prelude.rs` | — | Add `impl str` block |
| Hardcoded str early-interception | `expr_variants.rs` | 1377-1483 | Replace with post-resolution dispatch |
| Hardcoded str error list | `expr_variants.rs` | 1682-1688 | Remove (let typeck handle naturally) |
| `KNOWN_INTRINSIC_METHODS` whitelist | `checker.rs` | 228-231 | Remove (prelude provides real signatures) |

### 2.3 Risk assessment

| Risk | Mitigation |
|------|-----------|
| Behavior change: existing str tests may break if return types differ | Use `i64` (not `usize`) to match current intrinsic behavior |
| DefId stability: prelude impl DefIds must be stable across compilations | Use name+impl-block-self-ty based identification (not raw DefId) |
| Performance: extra resolution step per method call | Method dispatch is already O(impl_blocks), one extra name check is negligible |
| Backward compat: code calling `s.len()` directly must still work | Yes — `s.len()` now resolves via standard path, post-dispatch emits same MIR |

---

## 3. Design: DefId-based Interception with Marker Bodies

### 3.1 Architecture Choice

Considered alternatives:

- **Option A**: `intrinsics::` namespace (Rust-style: `fn len(&self) -> i64 { intrinsics::str_len(self) }`)
  - Pro: most "correct" — real method bodies
  - Con: requires new language feature (intrinsics:: namespace + extern "rust-intrinsic" ABI)
  - Verdict: too much for Phase 2-A scope

- **Option B**: Treat `&str` as a 2-field struct (allows `self.0`, `self.1`)
  - Pro: most "Rust-like"
  - Con: significant type system change, affects layout, breaks existing fat pointer code
  - Verdict: out of scope for Phase 2-A

- **Option C (CHOSEN)**: Marker bodies `loop {}` + DefId-based post-resolution interception
  - Pro: no new language feature, minimal surface change, typeck works naturally
  - Pro: prelude provides real signatures → users can see what methods exist
  - Pro: dispatch is centralized (1 place vs current 4+ scattered sites)
  - Con: marker body `loop {}` is unreachable code (acceptable: same pattern as Rust's `extern "rust-intrinsic"` fns)
  - Verdict: ✅ Phase 2-A appropriate scope

### 3.2 Component Design

#### 3.2.1 Primitive type name mapping

New helper function in `method_resolution.rs`:

```rust
/// Map a primitive `TyKind` to its name as it would appear in `impl <name> { ... }`.
/// Returns `None` for types that don't support inherent impls (Ref, RawPtr, etc.).
///
/// Per §1.0 原則 6 (通解>特例): one mapping function for all primitive types.
/// Per §10: `name_of_primitive_ty` follows `<noun>_<prep>_<noun>` pattern.
fn name_of_primitive_ty(ty: &Ty) -> Option<&'static str> {
    match ty.kind {
        TyKind::Str => Some("str"),
        TyKind::Bool => Some("bool"),
        TyKind::Char => Some("char"),
        TyKind::Int(IntTy::I8) => Some("i8"),
        TyKind::Int(IntTy::I16) => Some("i16"),
        TyKind::Int(IntTy::I32) => Some("i32"),
        TyKind::Int(IntTy::I64) => Some("i64"),
        TyKind::Int(IntTy::I128) => Some("i128"),
        TyKind::Int(IntTy::Isize) => Some("isize"),
        TyKind::Uint(UintTy::U8) => Some("u8"),
        TyKind::Uint(UintTy::U16) => Some("u16"),
        TyKind::Uint(UintTy::U32) => Some("u32"),
        TyKind::Uint(UintTy::U64) => Some("u64"),
        TyKind::Uint(UintTy::U128) => Some("u128"),
        TyKind::Uint(UintTy::Usize) => Some("usize"),
        TyKind::Float(FloatTy::F32) => Some("f32"),
        TyKind::Float(FloatTy::F64) => Some("f64"),
        _ => None,
    }
}
```

#### 3.2.2 Extension to `resolve_inherent_method`

Modify `resolve_inherent_method` to use primitive name as fallback when
recv_ty is not Adt:

```rust
pub(super) fn resolve_inherent_method(
    hir: &crate::hir::HirCrate,
    recv_ty: &Ty,
    method_name: &lasso::Spur,
) -> Option<crate::hir::DefId> {
    // ... existing auto-deref logic ...

    // Stage 18.284: Try Adt name lookup first (existing path).
    let type_name: crate::lexer::Symbol = if let TyKind::Adt(def_id, _) = &recv_ty.kind {
        // ... existing ADT name lookup ...
        adt_name?
    } else if let Some(prim_name) = name_of_primitive_ty(recv_ty) {
        // Stage 18.284: Primitive types — use the primitive's name directly.
        // The prelude declares `impl str { ... }`, `impl i32 { ... }` etc.
        // with `self_ty = Path("str")`. The existing impl-matching logic
        // works unchanged: it matches impl_block.self_ty == Path(prim_name).
        //
        // Per §1.0 原則 6 (通解>特例): one resolution path for all inherent impls.
        cx_interner...  // resolve "str" string to Symbol
    } else {
        return None;
    };

    // ... existing impl-matching logic using `type_name` ...
}
```

Note: `resolve_inherent_method` currently takes `hir` and `recv_ty`, but not
the interner. Need to either pass interner as a parameter, or look up the
primitive name as a pre-interned `Symbol`. Cleanest is to intern the primitive
names at compiler startup and look them up by static `&'static str`.

Implementation detail: extend `resolve_inherent_method` signature to take
`interner: &Rodeo` (or use a lookup table). Need to verify all call sites.

#### 3.2.3 Prelude `impl str` block

Add to `stdlib/prelude.rs`:

```landin
impl str {
    // Stage 18.284: Primitive str methods.
    // Bodies are markers (`loop {}`) — intercepted at MIR-lower dispatch
    // via lookup_primitive_intrinsic(). The signatures are real and
    // visible to typeck and users.
    //
    // Per §1.0 原則 6 (通解>特解): one impl block declares all primitive str methods.
    // Per §12 (最优>最小): infrastructure for ALL primitive impls, not just str.
    pub fn len(&self) -> i64 {
        loop {}  // marker — intercepted by lookup_primitive_intrinsic
    }
    pub fn is_empty(&self) -> bool {
        loop {}  // marker — intercepted
    }
    pub fn as_bytes(&self) -> &[u8] {
        loop {}  // marker — intercepted
    }
}
```

#### 3.2.4 DefId-based intrinsic dispatch (new file)

New file `src/mir/lower/primitive_intrinsics.rs`:

```rust
//! Primitive type intrinsic dispatch (Stage 18.284).
//!
//! After method resolution succeeds (returns a DefId), check if the
//! resolved method is a primitive intrinsic (e.g., str::len). If yes,
//! emit the appropriate MIR directly; otherwise, lower the body normally.
//!
//! ## J1-J6 compliance
//! - J1: mir::lower design unchanged (new sub-responsibility)
//! - J2: single responsibility (primitive intrinsic dispatch)
//! - J3: no circular deps (called by call_lower/expr_operand)
//! - J4: complete primitive intrinsic dispatch in this file
//! - J5: stays within mir::lower
//! - J6: LOC driven by responsibility (~150 LOC for table + emit functions)

use crate::hir::*;
use crate::mir::place::*;
use crate::mir::ty::*;

use super::MirLowerCtxt;

/// A primitive intrinsic kind — identifies which MIR emit path to use.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum PrimitiveIntrinsic {
    /// `str::len()` → Field(1) projection of the fat pointer
    StrLen,
    /// `str::is_empty()` → Field(1) + BinOp::Eq with 0
    StrIsEmpty,
    /// `str::as_bytes()` → no-op (return receiver, same fat pointer layout)
    StrAsBytes,
}

/// Check if a resolved method call is a primitive intrinsic.
///
/// Identification is by (impl_block.self_ty name, method_name) — NOT by raw
/// DefId (which is unstable across compilations). This is the same identifier
/// pair the early-interception code used, but centralized here.
///
/// Per §1.0 原則 6 (通解>特例): one lookup for all primitive intrinsics.
pub(crate) fn lookup_primitive_intrinsic(
    hir: &HirCrate,
    method_def_id: DefId,
) -> Option<PrimitiveIntrinsic> {
    // Find the method's owning impl block.
    for (_, owner) in &hir.owners {
        if let OwnerNode::Item(HirItem::Impl(impl_block)) = owner {
            if impl_block.of_trait.is_some() {
                continue;  // skip trait impls
            }
            for impl_item in &impl_block.items {
                if let HirImplItem::Fn(f) = impl_item {
                    if f.hir_id.owner == method_def_id {
                        // Found the method. Check if self_ty is a primitive
                        // path and the method name is a known intrinsic.
                        if let HirTyKind::Path(_, path) = &impl_block.self_ty.kind {
                            if path.segments.len() == 1 {
                                let self_ty_name = path.segments[0].ident.name;
                                let method_name = f.ident.name;
                                return identify_intrinsic(self_ty_name, method_name);
                            }
                        }
                        return None;  // method found but self_ty not a primitive path
                    }
                }
            }
        }
    }
    None
}

/// Map (self_ty_name, method_name) to a PrimitiveIntrinsic.
fn identify_intrinsic(
    self_ty: crate::lexer::Symbol,
    method: crate::lexer::Symbol,
) -> Option<PrimitiveIntrinsic> {
    // Per §1.0 原則 6 (通解>特例): one match table, not scattered checks.
    // Use static strings to avoid interner dependency.
    // (Implementation will use interner.resolve or static lookup.)
    // ...
}
```

#### 3.2.5 Call site integration

In `expr_variants.rs` (or `call_lower.rs`), after `resolve_inherent_method`
returns Some(def_id), check `lookup_primitive_intrinsic`. If Some, emit
intrinsic MIR; else, lower the body normally.

```rust
let method_def_id = resolve_inherent_method(hir, &recv_ty, &method.name)?;

// Stage 18.284: Check if this is a primitive intrinsic.
if let Some(intrinsic) = lookup_primitive_intrinsic(hir, method_def_id) {
    return emit_primitive_intrinsic(cx, intrinsic, recv_local, &arg_operands, expr);
}

// Normal path: lower the call body.
// ... existing code ...
```

#### 3.2.6 Removal of early interception

Delete from `expr_variants.rs`:
- Lines 1377-1407: `if method_name_str == "len" && args.is_empty() { ... is_str ... }` block
- Lines 1413-1465: `if method_name_str == "is_empty" && args.is_empty() { ... }` block
- Lines 1472-1483: `if method_name_str == "as_bytes" && args.is_empty() { ... }` block
- Lines 1677-1688: `is_str`/`known_str_methods`/`is_known_str_method` special case

Delete from `checker.rs`:
- Lines 228-231: `KNOWN_INTRINSIC_METHODS` whitelist (no longer needed — prelude provides real signatures)

---

## 4. §13.4 J1-J6 Compliance

- **J1 (mir::lower design unchanged)**: ✅ `resolve_inherent_method` extended, no design change. New sub-responsibility (`primitive_intrinsics.rs`) follows existing pattern (like `intrinsic_lower.rs`).
- **J2 (single responsibility)**: ✅ `primitive_intrinsics.rs` has one clear responsibility: dispatch primitive intrinsics post-resolution.
- **J3 (no circular deps)**: ✅ Called by `expr_operand` / `call_lower` (one-way). No callback into HIR mutation.
- **J4 (complete in this file)**: ✅ All primitive intrinsic dispatch (str::len/is_empty/as_bytes) lives in `primitive_intrinsics.rs`. Future primitive intrinsics (i32::abs, etc.) also belong here.
- **J5 (stays within mir::lower)**: ✅ New file under `src/mir/lower/`. MIR-lower-only concern.
- **J6 (LOC driven by responsibility)**: ✅ Estimated ~150 LOC for `primitive_intrinsics.rs` + ~30 LOC for `name_of_primitive_ty` helper. No arbitrary slicing.

---

## 5. Test Plan (§9.4.3 — 1:3+ positive/negative ratio)

### 5.1 Positive tests (existing str functionality — must still pass)

| Test | What it verifies |
|------|-----------------|
| `str_len_basic` | `"hello".len()` returns 5 |
| `str_len_empty` | `"".len()` returns 0 |
| `str_is_empty_true` | `"".is_empty()` returns true |
| `str_is_empty_false` | `"hello".is_empty()` returns false |
| `str_as_bytes_basic` | `"hello".as_bytes()` returns [104, 101, 108, 108, 111] |
| `str_as_bytes_empty` | `"".as_bytes()` returns [] |
| `str_len_on_string_as_str` | `String::from_str("hi").as_str().len()` returns 2 (chained) |
| `str_len_via_let_binding` | `let s: &str = "abc"; s.len()` returns 3 |
| `user_impl_str_method` | User-defined `impl str { fn my_len(&self) -> i64 { 42 } }` works (real body) |
| `user_impl_i32_method` | User-defined `impl i32 { fn double(self) -> i32 { self * 2 } }` works |

### 5.2 Negative tests (≥30 cases per §7.3.1)

Cover all 7 error categories:

1. **Wrong arg count**: `s.len(42)` (str::len takes no args)
2. **Wrong arg type**: (N/A for these methods — no args)
3. **Wrong receiver type**: `5.len()` (5 is i32, no len method)
4. **Wrong return usage**: `let n: i32 = s.len()` (i64 ≠ i32)
5. **Method doesn't exist**: `s.nonexistent()` on &str
6. **Method on wrong primitive**: `"hello".abs()` (str has no abs)
7. **User impl with wrong self**: `impl str { fn bad(self) -> i32 { 0 } }` called on `&str` (value self vs ref self)
8. **Mutability violation**: `impl str { fn mutate(&mut self) { } }` called on `&str` (immutable)
9. ... (target: ≥30 total negative cases)

### 5.3 Audit set: ≥30 negative cases

Per §7.3.1, build a dedicated audit set covering all 7 error categories with
≥30 cases. This set will be added to `tests/v0/stage18/plan/stage18_284_*`.

---

## 6. §14.5 D1-D8 Deep Review Checklist (Stage End)

Will be executed at stage end. Checklist:
- D1 (§3.2 全校验流): cargo clean + build --release + check + fmt + clippy -D warnings + test --release 全绿
- D2 (§13.4 J1-J6): all 6 checks pass
- D3 (§7.3.1 audit): ≥30 negative cases, all 7 error categories covered
- D4 (§9.4.3 ratio): 1:3+ positive/negative ratio verified
- D5 (§17.6 tech-debt): TD-INTRINSIC-OVERUSE Phase 2-A → Resolved (Phase 2-B/C still BLOCKED)
- D6 (LOC > 1500): no new file > 1500 LOC
- D7 (§11 cross-stage): no cross-stage internal calls
- D8 (§10 glob re-export): no glob re-exports added

---

## 7. Implementation Plan

### 7.1 Order of changes

1. Add `name_of_primitive_ty` helper to `method_resolution.rs`
2. Extend `resolve_inherent_method` to use primitive name lookup
3. Add `impl str { fn len/is_empty/as_bytes }` to `stdlib/prelude.rs`
4. Create `src/mir/lower/primitive_intrinsics.rs` with dispatch table
5. Add post-resolution dispatch call in `expr_operand.rs` / `call_lower.rs`
6. Remove early interception in `expr_variants.rs` (3 sites)
7. Remove `KNOWN_INTRINSIC_METHODS` whitelist in `checker.rs`
8. Update tests (move/add positive tests for new path)
9. Add ≥30 negative audit cases
10. Update tech-debt-register (Phase 2-A → Resolved, Phase 2-B/C remain BLOCKED)

### 7.2 Files touched

| File | Change | LOC delta |
|------|--------|-----------|
| `src/mir/lower/method_resolution.rs` | + `name_of_primitive_ty`, extend `resolve_inherent_method` | +50 |
| `src/mir/lower/primitive_intrinsics.rs` (NEW) | Dispatch table + emit functions | +180 |
| `src/mir/lower/mod.rs` | Add `mod primitive_intrinsics` | +2 |
| `src/mir/lower/expr_variants.rs` | Remove early interception (3 sites) + error reporting special case | -120 |
| `src/mir/lower/expr_operand.rs` (or `call_lower.rs`) | Add post-resolution dispatch call | +15 |
| `src/stdlib/prelude.rs` | Add `impl str { ... }` block | +20 |
| `src/typeck/checker.rs` | Remove `KNOWN_INTRINSIC_METHODS` | -15 |
| `tests/v0/stage18/plan/stage18_284_*.rs` (NEW) | Positive + negative audit set | +400 |

Net source LOC: ~+130 (excluding tests). Net total with tests: ~+530.

---

## 8. §14.8 Design Writeback Plan

At stage end:
- B1 (Deviation: signature mismatch): if return type changes from `i64` to `usize` during impl, document
- B2 (Deviation: scope cut): if any planned primitive (i32, bool, etc.) is deferred, document
- B3 (Deviation: blocker): if Phase 2-B/C blockers remain, document in tech-debt-register
- B4 (Deviation: refactor): if any unplanned refactor happens, document

---

## 9. Decision Points (思考痕迹)

### 9.1 Why Option C (marker bodies + DefId interception) over Option A (intrinsics:: namespace)?

- 引用 §12 (最优 > 最小): Phase 2-A scope is "remove hardcoded str intrinsics". Adding a new language feature (intrinsics:: namespace) is OUTSIDE this scope — it would expand the change to language design. The 通解 here is "centralize dispatch through prelude impls", not "add a new namespace".
- 引用 §1.0 原則 6 (通解 > 特例): Option C provides infrastructure for ALL future primitive impls (i32::abs, bool::then, char::is_ascii, etc.) — not just str methods. Adding new primitive methods will be a prelude-only change.
- 引用 §17.6 (整体性修复): Phase 2-A removes the scattered str special-casing (4 sites) and replaces with one dispatch table. Future primitive intrinsics follow the same pattern.

### 9.2 Why not just extend `resolve_inherent_method` without prelude impls?

- Without prelude impls, the str method would resolve to `None` (no impl block exists). The hardcoded early interception in `expr_variants.rs` would still be needed.
- This would be a 特解 (extending infrastructure without using it), violating §2.2 原則 9 (正确 > 妥协).
- The prelude impls are what make the new infrastructure actually USED — closing the loop.

### 9.3 Why marker body `loop {}` instead of `panic!()` or `unreachable!()`?

- `loop {}` has type `!` (Never), which unifies with any return type. This means the signature `fn len(&self) -> i64 { loop {} }` type-checks cleanly.
- `panic!()` would require importing panic into prelude scope (currently no panic in prelude).
- `unreachable!()` is a macro, requires macro support in prelude (not yet available).
- The body is NEVER REACHED — `lookup_primitive_intrinsic` intercepts before body lowering.

### 9.4 Why i64 return type for str::len (not usize per lang-design)?

- The current intrinsic returns i64 (line 1384 of expr_variants.rs).
- Changing to usize would require updating ALL existing str::len call sites and tests.
- Per §12 (最优 > 最小): the "最优" here is "no behavior change in Phase 2-A". Switching to usize is a separate concern (lang alignment) — defer to a future stage.
- Document as B1 deviation at stage end.

---

## 10. Next MUV (next stage)

After Stage 18.284 completes (Phase 2-A resolved):
- Option A: Phase 2-B (String::as_str) — needs fat pointer construction syntax (lang feature)
- Option B: Phase 2-C (String::from_str/push_str, Vec::push/get, Box::new) — needs extern "C" in prelude
- Option C: Other TDs (none currently P0/P1 — only BLOCKED ones remain)
- Option D: Add more primitive impls to prelude (i32::abs, bool::then, char::is_ascii) using the new infrastructure (low-risk, validates the architecture)

Recommended: **Option D** — validates the architecture by adding 2-3 more primitive impls. If they all work without further changes, the infrastructure is proven. If they require changes, we discover the gaps now.
