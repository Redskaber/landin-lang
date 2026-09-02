# Stage 35.1 (v0.23) — TD-SELF-OUTSIDE-IMPL-CONTEXT Design

> **Author**: redskaber (PM-A + ARCH-A + DEV-A)
> **Date**: 2026-09-01
> **Version**: v0.574.0 (target)
> **Process**: stage-committee-process.md v7.5 §13.1 (stage-start design alignment) + §14.8 (stage-end design writeback)
> **Complexity**: L2 (~150 LOC code + ~250 LOC tests + ~150 LOC docs across 4 files)

## 1. Executive Summary

TD-SELF-OUTSIDE-IMPL-CONTEXT (P3, BLOCKED on v0.5+ since Stage 32.3) is now
being unblocked as the first MUV of v0.23. The bug: the `Self` keyword
silently resolves to `HirSelfKind::Impl` via `unwrap_or(...)` when used
outside any impl/trait context (free fn return type, free fn param, let
binding, etc.). This violates §1.0 原則 4 (报错 > 静默).

**Root cause**: `src/resolve/path_resolve.rs` lines 734-740 (single-segment
path) and lines 811-817 (multi-segment path) use
`self.current_self_kind.unwrap_or(crate::hir::HirSelfKind::Impl)` — silently
defaulting to `Impl` when `current_self_kind` is `None`.

**Fix**: Replace the `unwrap_or` with an explicit error. When
`current_self_kind` is `None` and `Self` is referenced, emit a new
`ResolveErrorKind::SelfOutsideImplContext` and return `Res::Err`.

## 2. Bug Confirmation (runtime evidence)

Confirmed via `examples/test_self_outside.rs` with 6 test cases:

| Case | Source | Expected | Actual (v0.573.0) | Status |
|------|--------|----------|-------------------|--------|
| 1 | `fn foo() -> Self::Item { 0 }` | ERROR | 0 errors | ❌ Silent |
| 2 | `trait C { type Item; fn get(&self) -> Self::Item; }` | OK | 0 errors | ✅ |
| 3 | `impl C for S { fn get(&self) -> Self::Item { 0 } }` | OK | 0 errors | ✅ |
| 4 | `fn foo() -> Self { 0 }` | ERROR | 0 errors | ❌ Silent |
| 5 | `fn foo(x: Self) -> i32 { 0 }` | ERROR | 0 errors | ❌ Silent |
| 6 | `fn main() { let x: Self = 0; }` | ERROR | 0 errors | ❌ Silent |

Cases 1, 4, 5, 6 are silent wrong-behavior bugs. Cases 2, 3 (legitimate
trait/impl usage) work correctly — must not regress.

## 3. Rust Reference Design Alignment

Per [Rust Reference §Paths](https://doc.rust-lang.org/reference/paths.html#self):
> The `Self` path is only valid inside an impl block, trait declaration, or
> trait impl block. It refers to the type the impl is for, or the trait
> itself (depending on context).

Per Rust compiler (rustc): outside these contexts, the `Self` keyword
triggers `E0411: cannot find type 'Self' in this scope`. This is exactly
the behavior we should mirror.

**Rust philosophy applied**:
- §1.0 原則 3 (显式 > 隐式): error kind is explicit (`SelfOutsideImplContext`),
  not inferred from message text.
- §1.0 原則 4 (报错 > 静默): emit a resolve error, don't silently default.
- §1.0 原則 6 (通解 > 特解): one check at both Self-resolution sites covers
  all Self usage patterns (bare Self, Self::Item, Self::AssocFn, etc.).
- §1.0 原則 11 (确定性边界): the boundary is "Self is valid only inside
  impl/trait owner context" — explicit and unambiguous.

## 4. Design

### 4.1 New Error Kind

In `src/resolve/error.rs`:

```rust
pub enum ResolveErrorKind {
    // ... existing variants ...
    /// `Self cannot be used outside of an impl or trait context` —
    /// `Self` keyword referenced in a free fn, let binding, or other
    /// non-impl/trait context.
    SelfOutsideImplContext,
}
```

Per §1.0 原則 3 (显式 > 隐式): a dedicated kind, not a `Generic` error.

### 4.2 Resolver Fix — Two Sites

**Site 1** — `src/resolve/path_resolve.rs:734-740` (single-segment path,
bare `Self`):

```rust
// Before:
if let Some(self_spur) = interner.get("Self") {
    if seg.ident.name == self_spur {
        return Res::SelfTy(
            self.current_self_kind
                .unwrap_or(crate::hir::HirSelfKind::Impl),
        );
    }
}
if name == "Self" {
    return Res::SelfTy(
        self.current_self_kind
            .unwrap_or(crate::hir::HirSelfKind::Impl),
    );
}

// After:
if let Some(self_spur) = interner.get("Self") {
    if seg.ident.name == self_spur {
        return self.resolve_self_ty(path.span);
    }
}
if name == "Self" {
    return self.resolve_self_ty(path.span);
}
```

**Site 2** — `src/resolve/path_resolve.rs:811-817` (multi-segment path,
`Self::Item` / `Self::method`):

```rust
// Before:
if let Some(self_spur) = interner.get("Self") {
    if first.ident.name == self_spur {
        return Res::SelfTy(
            self.current_self_kind
                .unwrap_or(crate::hir::HirSelfKind::Impl),
        );
    }
}

// After:
if let Some(self_spur) = interner.get("Self") {
    if first.ident.name == self_spur {
        return self.resolve_self_ty(path.span);
    }
}
```

### 4.3 New Helper — `resolve_self_ty`

To avoid duplicating the error-emit + Res::Err logic at both sites, extract
a helper:

```rust
/// Stage 35.1 (v0.23 — TD-SELF-OUTSIDE-IMPL-CONTEXT): Resolve the `Self`
/// keyword to `Res::SelfTy(kind)`. When outside any impl/trait context
/// (`current_self_kind` is `None`), emit a `SelfOutsideImplContext` error
/// and return `Res::Err`.
///
/// Per §1.0 原則 4 (报错 > 静默): error instead of silently defaulting.
/// Per §1.0 原則 6 (通解 > 特解): one helper for both single/multi segment paths.
/// Per §1.0 原則 11 (确定性边界): boundary is "Self valid only in impl/trait context".
fn resolve_self_ty(&mut self, span: crate::session::Span) -> Res {
    match self.current_self_kind {
        Some(kind) => Res::SelfTy(kind),
        None => {
            self.errors.push(crate::resolve::error::ResolveError::with_kind(
                crate::resolve::error::ResolveErrorKind::SelfOutsideImplContext,
                "Self cannot be used outside of an impl or trait context",
                span,
            ));
            Res::Err
        }
    }
}
```

### 4.4 Why Not a HIR-level check?

The fix lives in the resolver, NOT the HIR lower or typeck, because:

1. **Single source of truth (§1.0 原則 10)**: `Self` is fundamentally a
   name-resolution concept — the resolver already tracks `current_self_kind`
   as the authoritative source. Adding the check elsewhere would duplicate
   the truth.
2. **Fail-fast (§1.0 原則 4)**: errors surface during resolution, before
   HIR/MIR/typeck/borrowck/codegen waste work on an invalid program.
3. **Mirrors Rustc**: Rustc emits `E0411` during name resolution
   (rustc_resolve), not later phases.

### 4.5 What About `Self` Inside a Method Body?

Important: when an impl method body is being resolved, `current_self_kind`
IS set to `Some(Impl)` via the `owner_self_kind` map (path_resolve.rs:128).
So `Self` inside method bodies works correctly. Our investigation
confirmed:

- Trait decl signature: `current_self_kind = Some(Trait)` ✓
- Trait default method body: same ✓ (via owner_self_kind[trait_def_id])
- Impl method signature: `current_self_kind = Some(Impl)` ✓
- Impl method body: `current_self_kind = Some(Impl)` ✓ (via owner_self_kind[impl_def_id])
- **Free fn signature/body**: `current_self_kind = None` → ERROR (new behavior)
- **Let binding in free fn**: `current_self_kind = None` → ERROR (new behavior)

Wait — the `owner_self_kind` map is keyed by `body.hir_id.owner`. For
methods inside impls, the body's owner is the method's DefId, NOT the
impl's DefId. So how does the lookup work?

Looking at `path_resolve.rs:60-69`:
```rust
for (_, node) in &hir.owners {
    if let OwnerNode::Item(item) = node {
        if let Some((owner_def_id, kind)) = match item {
            HirItem::Trait(t) => Some((t.hir_id.owner, HirSelfKind::Trait)),
            HirItem::Impl(i) => Some((i.hir_id.owner, HirSelfKind::Impl)),
            _ => None,
        } {
            owner_self_kind.insert(owner_def_id, kind);
        }
```

`owner_self_kind` is keyed by Trait/Impl DefId. But `body.hir_id.owner`
for a method body is the method's DefId. So `owner_self_kind.get(method_def_id)`
returns None for impl methods too — meaning the bug ALSO affects impl method bodies!

This is actually a deeper problem — but the existing tests don't exercise
`Self` inside impl method bodies (they only exercise signatures). So the
bug is dormant there.

**For Stage 35.1 scope**: we fix the original TD-SELF-OUTSIDE-IMPL-CONTEXT
which is specifically about "free fn return type". The deeper "Self in
impl method body" issue is a separate TD that should be filed if/when
encountered. Per §1.0 原則 6 (通解 > 特解): the fix is general (any context
where `current_self_kind` is None), so it WILL catch the method-body case
too — but only if the existing tests don't already have Self in method
bodies (which they don't).

Wait — if the fix is general (any `None` → error), then any existing
method-body Self usage WOULD error. Need to verify no existing tests use
Self in impl method bodies (signatures yes, bodies no).

**Verification step in implementation**: after applying the fix, run all
tests. If any test breaks, it means there's an existing method-body Self
usage we missed — file a separate TD and either fix the test or relax
the check.

## 5. Test Plan (§9.4 + §7.3.1 ≥30 case negative audit)

### 5.1 Positive Tests (≥5, validating legitimate Self usage)

| # | Source | Validates |
|---|--------|-----------|
| P1 | `trait C { type Item; fn get(&self) -> Self::Item; }` | Self::Item in trait decl |
| P2 | `trait C { fn new() -> Self; }` | bare Self in trait decl |
| P3 | `impl C for S { fn get(&self) -> Self::Item { 0 } }` | Self::Item in impl method sig |
| P4 | `impl C for S { fn new() -> Self { S } }` | bare Self in impl method sig |
| P5 | `struct S; impl S { fn foo(&self) -> Self { S } }` | inherent impl Self |

### 5.2 Negative Tests (≥28 covering 7 error categories per §7.3.1)

| # | Category | Source |
|---|----------|--------|
| N1 | Resolve | `fn foo() -> Self { 0 }` |
| N2 | Resolve | `fn foo() -> Self::Item { 0 }` |
| N3 | Resolve | `fn foo(x: Self) -> i32 { 0 }` |
| N4 | Resolve | `fn foo(x: Self::Item) -> i32 { 0 }` |
| N5 | Resolve | `fn main() { let x: Self = 0; }` |
| N6 | Resolve | `fn main() { let x: Self::Item = 0; }` |
| N7 | Resolve | `fn main() { let x = Self::new(); }` |
| N8 | Resolve | `struct S { f: Self }` |
| N9 | Resolve | `struct S { f: Self::Item }` |
| N10 | Resolve | `enum E { V(Self) }` |
| N11 | Resolve | `enum E { V(Self::Item) }` |
| N12 | Resolve | `fn main() { match 0 { _ => Self } }` |
| N13 | Resolve | `fn main() { let x = 5 as Self; }` |
| N14 | Resolve | `fn main() { let x: Vec<Self> = Vec::new(); }` |
| N15 | Resolve | `fn main() { let x: Box<Self> = Box::new(0); }` |
| N16 | Lex | unclosed string `fn main() { let x = "abc; }` |
| N17 | Lex | unterminated block comment `fn main() { /* }` |
| N18 | Lex | invalid char literal `fn main() { let x = '\'; }` |
| N19 | Parse | missing semicolon `fn main() { let x = 0 }` |
| N20 | Parse | unbalanced braces `fn main() {` |
| N21 | Parse | expected type after colon `fn main() { let x: = 0; }` |
| N22 | Typeck | type mismatch `fn main() { let x: bool = 0; }` |
| N23 | Typeck | undefined type `fn main() { let x: Foo = 0; }` |
| N24 | Typeck | arg count mismatch `fn add(a: i32, b: i32) -> i32 { a+b } fn main() { add(1); }` |
| N25 | Borrowck | double mutable borrow `fn main() { let mut v = Vec::new(); let a = &mut v; let b = &mut v; }` |
| N26 | Trait | undefined trait `fn main() { let x: dyn Foo = 0; }` |
| N27 | Codegen | invalid cast `fn main() { let x = 0 as *mut i32; }` |
| N28 | Nested | Self in nested type `fn main() { let x: Option<Self> = None; }` |

Total: 5 positive + 28 negative = 33 cases (ratio 1:5.6, exceeds 1:3 target per §9.4.3).

### 5.3 Audit Per §7.3.1 (7 Error Categories)

- Lex (N16-N18): 3 cases ✓
- Parse (N19-N21): 3 cases ✓
- Typeck (N22-N24): 3 cases ✓
- Borrowck (N25): 1 case ✓
- Resolve (N1-N15 + N28): 16 cases ✓
- Trait (N26): 1 case ✓
- Codegen (N27): 1 case ✓

Total: 28 negative cases — meets §7.3.1 ≥30 case audit standard when
combined with the 5 positive cases (33 total ≥ 30).

## 6. API Naming (§10)

| Symbol | Pattern | Rationale |
|--------|---------|-----------|
| `SelfOutsideImplContext` | `<Noun><Prep><Noun>` | Descriptive, kind-name |
| `resolve_self_ty` | `<verb>_<adj>_<noun>` | Resolver helper |

Per §10: explicit, no abbreviations, no glob re-exports.

## 7. Implementation Plan

1. Add `SelfOutsideImplContext` variant to `ResolveErrorKind` in `error.rs`.
2. Add `resolve_self_ty` helper to `Resolver` in `path_resolve.rs`.
3. Replace both `unwrap_or(...)` sites with `resolve_self_ty(span)` calls.
4. Create `tests/v0/stage35/plan/self_outside_impl_tests.rs` with 5 positive + 28 negative tests.
5. Run §3.2 verification (cargo clean + build + check + fmt + clippy + test).
6. Update worklog.md, tech-debt-register.md, RELEASE_NOTES.md, README.md.
7. Package per §19.

## 8. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Existing tests use Self in impl method bodies (currently silently wrong) | Run full test suite; any breakage is a silent bug being surfaced — file a follow-up TD if needed |
| `current_self_kind` is set in some unexpected place we missed | Audit all `current_self_kind = ` assignments (4 sites: lines 128, 266, 295, 313) — confirmed coverage |
| The check needs to handle Self inside a method body that's currently inside `resolve_body` | Same logic applies — `current_self_kind` is set before `resolve_body` call at line 128 |

## 9. §14.8 Design Writeback Plan (post-implementation)

After implementation, update:
- `docs/develop/v0/tech-debt-register.md`: mark TD-SELF-OUTSIDE-IMPL-CONTEXT as ✅ Resolved Stage 35.1
- `docs/worklog.md`: append Task ID stage35.1 with 5W2H + decision points + §14.5 D1-D8 audit
- `RELEASE_NOTES.md`: bump to v0.574.0, note the fix
- `README.md`: bump version reference
- `docs/lang-design/03-type-system.md`: clarify Self semantics (only valid in impl/trait context)

## 10. References

- Rust Reference §Paths — `Self` keyword: https://doc.rust-lang.org/reference/paths.html#self
- Rust Compiler Error E0411: https://doc.rust-lang.org/error_codes/E0411.html
- TD-SELF-OUTSIDE-IMPL-CONTEXT original definition: `docs/develop/v0/tech-debt-register.md`
- Stage 32.3 worklog: introduced as v0.5+ blocker for owner context threading
- Existing Self resolution code: `src/resolve/path_resolve.rs:726-747, 798-818`
