# Stage 1.4 — Scope-Based Name Resolution

> **Sub-stage**: 1.4 (Month 4, weeks 3-4)
> **Goal**: Resolve local variable bindings, closure parameters,
> match arm pattern bindings, and for-loop bindings within function
> bodies. Replace `Res::Err` (for unresolved locals) with
> `Res::Local(HirId)` pointing at the binding's HirId.
> **Acceptance gate**: All 5 Stage Committee members vote APPROVED (strict).
> **Process**: 4-12 rounds including destructive-gap checks.

---

## Scope

Stage 1.4 handles **scope-based** resolution within bodies:

- `let` binding registration + reference resolution → `Res::Local(HirId)`
- Closure parameter resolution (`|x| x + 1`)
- Match arm pattern binding (`Some(x) => x`)
- For-loop binding (`for x in iter { x }`)
- Scope nesting (block / fn / closure / loop / match arm)
- Shadowing (inner `let x` shadows outer `let x`)
- Forward reference detection (can't use a `let` before it's declared)
- `self` value resolution (→ `Res::Local` of the self param)
- `super`/`crate`/`self::` path prefixes in value position

**NOT in scope** (deferred to later stages):

- Lifetime resolution (`'a` in scope)
- Label resolution (`'lbl:` for loop/break)
- Unused variable warnings (P1, lint pass)
- Macro hygiene (Stage 4)
- Cross-module glob import expansion (Stage 5)

---

## Tasks (12 atomic items across 4 phases)

### Phase A — Scope data structure

#### A1. `Scope` chain

```rust
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// Bindings in this scope: name → HirId of the binding.
    bindings: HashMap<Spur, HirId>,
    /// Parent scope (None for the root/fn scope).
    parent: Option<Box<Scope>>,
    /// Scope kind (for diagnostics + forward-ref detection).
    kind: ScopeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    Fn,        // function body
    Block,     // { ... } block
    Closure,   // |args| body
    MatchArm,  // match arm pattern scope
    Loop,      // loop / while / for body
}
```

#### A2. `ScopeStack` — managed scope chain

```rust
pub struct ScopeStack {
    current: Scope,
}

impl ScopeStack {
    fn push(&mut self, kind: ScopeKind) { ... }
    fn pop(&mut self) { ... }
    fn insert(&mut self, name: Spur, hir_id: HirId) { ... }
    fn lookup(&self, name: Spur) -> Option<HirId> { ... }
}
```

### Phase B — Pattern binding extraction

#### B1. `collect_pat_bindings`

Walk a `HirPat` and collect all identifier bindings into the current
scope. Handles:

- `Pat::Ident(_, ident, _)` → bind `ident`
- `Pat::Struct(fields)` → bind each field's sub-pattern
- `Pat::TupleStruct(pats)` → bind each sub-pattern
- `Pat::Tuple(pats)` → bind each sub-pattern
- `Pat::Slice(pats, rest)` → bind each sub-pattern
- `Pat::Or(pats)` → all alternatives must bind the same names
- `Pat::Ref(pat, _)` → bind inner
- `Pat::Lit/Path/Range/Rest/Wild` → no bindings

### Phase C — Body walking with scope tracking

#### C1. `resolve_body_with_scopes`

Replace the current `resolve_body` with a version that:

1. Creates a Fn scope
2. Registers all fn params as bindings
3. Walks the body expression with scope tracking

#### C2. `resolve_expr_with_scopes` — updated expression walker

Key changes from Stage 1.3's `resolve_expr`:

- `HirExprKind::Block`: push Block scope, walk stmts, pop
- `HirStmt::Local`: first resolve the init expr (so forward refs fail),
  then collect pat bindings into current scope
- `HirExprKind::Closure`: push Closure scope, register params, walk body, pop
- `HirExprKind::Match`: for each arm, push MatchArm scope, collect pat
  bindings, resolve guard + body, pop
- `HirExprKind::For`: push Loop scope, collect pat bindings, walk body, pop
- `HirExprKind::Path` (single-segment, no leading): first check scope
  (locals), then check module tree (items). Local wins over item
  (shadowing).

#### C3. `resolve_path_with_scope`

Updated path resolution:

1. Single-segment, no leading: check local scope first → `Res::Local(HirId)`.
   If not found, fall back to module tree (Stage 1.3 behavior).
2. `self` in value position → resolve to the self param's HirId.
3. Multi-segment: unchanged (module-level resolution).

### Phase D — Tests

#### D1. Scope resolution tests (20+)

- 5 tests: basic `let` binding resolution
- 3 tests: shadowing (inner shadows outer)
- 3 tests: closure parameter resolution
- 3 tests: match arm pattern binding
- 2 tests: for-loop binding
- 2 tests: forward reference detection (should be Err)
- 2 tests: scope nesting (block scope hides outer)

#### D2. Regression: all 430 existing tests still pass

---

## Acceptance Criteria

1. ✅ All tasks implemented
2. ✅ `cargo build` 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥450 tests (430 existing + 20 new)
6. ✅ `Res::Local(HirId)` is populated for all local variable references
7. ✅ Shadowing works (inner binding shadows outer)
8. ✅ Forward references are `Res::Err`
9. ✅ All 5 Stage Committee members vote APPROVED (strict)
