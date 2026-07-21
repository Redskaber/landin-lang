# Stage 1.2 — AST → HIR Lowering

> **Sub-stage**: 1.2 (Month 3, weeks 3-4)
> **Goal**: Implement `lower_crate(AstCrate) -> HirCrate` that converts all
> AST nodes to HIR nodes, assigning fresh `HirId`s and populating
> `Res::Unknown` placeholders for Stage 1.3 name resolution.
> **Acceptance gate**: All 5 Stage Committee members vote APPROVED (strict
> mode, no NEEDS REVISION allowed).
> **Process**: 4-10 rounds of review-refine-iterate per user mandate.

---

## Tasks (15 atomic items across 5 phases)

### Phase A — HIR Crate + LowerCtxt infrastructure

#### A1. `HirCrate` + `HirOwnerNodes` container

Define in `src/hir/kinds.rs`:

```rust
pub struct HirCrate {
    pub owners: FxHashMap<DefId, OwnerNode>,
    pub bodies: FxHashMap<BodyId, Body>,
    pub hir_id_counter: ... // for fresh ID allocation
}
```

For Stage 1.2 we use `std::collections::HashMap` (no `fxhash` dependency
yet; can swap later without API change).

#### A2. `LowerCtxt` — lowering context

Define in `src/hir/lower/mod.rs`:

```rust
pub struct LowerCtxt<'a> {
    interner: &'a Rodeo,
    def_id_counter: DefIdCounter,
    /// Per-owner ItemLocalId counter; reset when entering a new owner.
    local_id_counter: ItemLocalIdCounter,
    /// The current owner (DefId). Set when entering an owner's body.
    current_owner: Option<DefId>,
    /// The HIR crate being built.
    hir: HirCrate,
    /// Errors encountered during lowering (non-fatal: continue).
    errors: Vec<LowerError>,
}

impl<'a> LowerCtxt<'a> {
    pub fn new(interner: &'a Rodeo) -> Self { ... }

    /// Allocate the next DefId. Used when entering a new owner.
    pub fn fresh_def_id(&mut self) -> DefId { ... }

    /// Allocate the next ItemLocalId within the current owner.
    /// Panics if `current_owner` is None.
    pub fn fresh_hir_id(&mut self) -> HirId { ... }

    /// Enter an owner context. Allocates a DefId, sets `current_owner`,
    /// resets the local ID counter. Returns the DefId.
    pub fn enter_owner(&mut self) -> DefId { ... }

    /// Exit the current owner context.
    pub fn exit_owner(&mut self, prev_owner: Option<DefId>) { ... }

    /// Register a body in the HIR crate.
    pub fn store_body(&mut self, body: Body) -> BodyId { ... }

    /// Register an owner node in the HIR crate.
    pub fn store_owner(&mut self, def_id: DefId, node: OwnerNode) { ... }
}
```

#### A3. `LowerError` type

```rust
#[derive(Debug, Clone)]
pub struct LowerError {
    pub message: String,
    pub span: Span,
}
```

Simple error type; non-fatal. Stage 1.3+ will integrate with the
`Diagnostic` system.

### Phase B — Item lowering (11 item kinds)

#### B1. `lower_crate` — entry point

```rust
pub fn lower_crate(ast: &ast::Crate, interner: &Rodeo) -> HirCrate {
    let mut cx = LowerCtxt::new(interner);
    for item in &ast.items {
        let owner = cx.lower_item(item);
        // owner is already stored in cx.hir
    }
    cx.into_hir()
}
```

#### B2. `lower_item` — dispatch on `ItemKind`

Dispatch to `lower_fn`, `lower_const`, `lower_static`, `lower_struct`,
`lower_enum`, `lower_trait`, `lower_impl`, `lower_type_alias`,
`lower_extern_block`, `lower_mod`, `lower_use`.

Each returns the `HirItem` variant and stores the owner node + body (if
any) in the context.

#### B3. `lower_fn` — fn item with body

1. `enter_owner()` → DefId
2. Allocate `HirId` for the fn itself (local_id = 0)
3. Lower `generics` → `HirGenerics`
4. Lower `sig.inputs` → `Vec<HirParam>` (handling `self_kind`)
5. Lower `sig.output` → `HirFnRetTy`
6. If body present: lower body to `Body` and store; `body_id = Some(...)`
7. If body absent (trait method signature): `body_id = None`
8. `exit_owner()`
9. Construct `HirFn { hir_id, ident, generics, sig, body, vis, attrs, span }`
10. Store as `OwnerNode::Item(HirItem::Fn(...))`

#### B4. `lower_const` / `lower_static`

Similar to `lower_fn` but simpler (no inputs, no generics, body is the
initializer expression).

#### B5. `lower_struct` / `lower_enum`

1. `enter_owner()`
2. Lower `generics`
3. Lower fields/variants (each field gets a fresh `HirId`)
4. No body (struct/enum decls don't have bodies)
5. `exit_owner()`

#### B6. `lower_trait` / `lower_impl`

1. `enter_owner()`
2. Lower `generics` + `supertraits` (trait) / `of_trait` + `self_ty` (impl)
3. Lower items: each trait item / impl item is itself an owner with its
   own DefId (nested lowering)
4. `exit_owner()`

#### B7. `lower_type_alias` / `lower_extern_block` / `lower_mod` / `lower_use`

Straightforward; see AST→HIR field mapping in plan §C.

### Phase C — Body lowering (expressions + statements + patterns + types)

#### C1. `lower_body` — fn/const/static body

```rust
fn lower_body(&mut self, ast_body: &ast::Block, params: Vec<HirParam>) -> Body {
    let hir_id = self.fresh_hir_id();
    let value = self.lower_expr(&ast::Expr::Block(ast_body.clone(), ast_body.span));
    Body { hir_id, params, value, span: ast_body.span }
}
```

#### C2. `lower_expr` — dispatch on `Expr` variant (28 variants)

Each variant gets a fresh `HirId` and is converted to the corresponding
`HirExprKind` variant. Recursive calls lower sub-expressions.

Notable cases:
- `Expr::Path` → `HirExprKind::Path(HirPath)` with `res: Res::Unknown`
- `Expr::Lit` → `HirExprKind::Lit(HirLitKind)` (convert `ast::LitKind` → `HirLitKind`)
- `Expr::Block` → `HirExprKind::Block(HirBlock)`
- `Expr::Closure` → `HirExprKind::Closure { is_move, params, body }`
  (params lowered; body is a sub-expression, NOT a separate Body for Stage 1.2)
- `Expr::MacroCall` → `HirExprKind::MacroCall { path, delim }` (body not
  lowered; macro expansion is Stage 4)

#### C3. `lower_stmt` — 4 stmt variants

- `Stmt::Local` → `HirStmt::Local(HirLocal)` (lower pat + ty + init)
- `Stmt::Expr(e, has_semi)` → `HirStmt::Expr(Box<HirExpr>, has_semi)`
- `Stmt::Semi` → `HirStmt::Semi` (rare; dead code)
- `Stmt::Empty(span)` → `HirStmt::Empty(span)`

#### C4. `lower_pat` — 12 pattern variants

Each variant gets a fresh `HirId`. Notable:
- `Pat::Ident(mode, ident, sub)` → `HirPatKind::Ident(mode, ident, sub.map(lower_pat))`
- `Pat::Struct(path, fields, rest)` → lower path + each field
- `Pat::TupleStruct(path, pats)` → lower path + each sub-pat
- `Pat::Lit(expr)` → `HirPatKind::Lit(Box<HirExpr>)`

#### C5. `lower_ty` — 16 type variants

Each variant gets a fresh `HirId` + `inferred: None` (set by Stage 2).
Notable:
- `Ty::Ref(lt, mut, ty)` → `HirTyKind::Ref(lt, mut, lower_ty(ty))`
- `Ty::Path(qself, path, span)` → `HirTyKind::Path(HirQSelf { ty: qself.ty.map(lower_ty), position: qself.position }, lower_path(path))`
- `Ty::ImplTrait(bounds)` → `HirTyKind::ImplTrait(lower_bounds(bounds))`

#### C6. `lower_path` — path with `Res::Unknown`

```rust
fn lower_path(&mut self, ast_path: &ast::Path) -> HirPath {
    HirPath {
        hir_id: self.fresh_hir_id(),
        segments: ast_path.segments.iter().map(|s| HirPathSegment {
            ident: s.ident,
            args: s.args.clone(),
        }).collect(),
        leading: ast_path.leading,
        res: Res::Unknown, // populated by Stage 1.3
        span: ast_path.span,
    }
}
```

### Phase D — Generics + Where + Use tree lowering

#### D1. `lower_generics` / `lower_generic_param` / `lower_where_clause`

Each generic param gets a fresh `HirId`. Lifetime params and type params
are lowered to `HirGenericParam::Lifetime` / `HirGenericParam::Type` with
bounds and defaults preserved.

#### D2. `lower_type_bounds` / `lower_trait_bound` / `lower_lifetime`

`TypeBound::Trait(TraitBound)` → `HirTypeBound::Trait(HirTraitBound)`
`TypeBound::Lifetime(Lifetime)` → `HirTypeBound::Lifetime(Lifetime)` (copied)

#### D3. `lower_use_tree` — recursive

`UseTree::Leaf(path, alias)` → `HirUseTree::Leaf(path, alias)`
`UseTree::Glob(path)` → `HirUseTree::Glob(path)`
`UseTree::Path { prefix, children }` → `HirUseTree::Path { prefix, children.map(lower_use_tree) }`

### Phase E — Tests + integration

#### E1. Integration test: all 245 parse cases lower without panic

```rust
// tests/v0/stage1/plan/hir_lowering_tests.rs
#[test]
fn all_parse_cases_lower() {
    let cases = [
        "fn main() {}",
        "struct Point { x: i32, y: i32 }",
        // ... 245 cases from tests/v0/stage0/plan/parser_tests.rs + tests/v0/stage0/plan/ast_structure_tests.rs
    ];
    for src in cases {
        let (krate, errors) = parse(src);
        assert!(errors.is_empty(), "parse failed for {:?}: {:?}", src, errors);
        let mut interner = Rodeo::new();
        let hir = lower_crate(&krate, &interner);
        // Just assert no panic; structural assertions in E2.
    }
}
```

#### E2. Structural lowering tests (30+ tests)

- 5 tests: item kind round-trip (fn/struct/enum/trait/impl)
- 5 tests: expression round-trip (binary/call/closure/match/if)
- 5 tests: pattern round-trip (ident/tuple/struct/or/lit)
- 5 tests: type round-trip (ref/path/array/tuple/fn-ptr)
- 5 tests: generics + where clause preservation
- 5 tests: HirId uniqueness + DefId allocation order

#### E3. Documentation updates

- `docs/development-log.md`: Stage 1.2 entry
- `docs/stage-1.2-plan.md`: this plan (already written)
- `README.md`: bump to v0.2.1, mention Stage 1.2 complete
- `Cargo.toml`: v0.2.0 → v0.2.1

---

## Acceptance Criteria

1. ✅ All 15 tasks implemented and committed
2. ✅ `cargo build` produces 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥405 tests (375 existing + 30 new lowering)
6. ✅ All 245 existing parse cases lower without panic
7. ✅ HIR structures are populated correctly (structural tests verify
   field-by-field equivalence with AST for representative cases)
8. ✅ `Res::Unknown` is set on all `HirPath` nodes (Stage 1.3 will fill in)
9. ✅ `InferTy` is `None` on all `HirTy` nodes (Stage 2 will fill in)
10. ✅ All 5 Stage Committee members vote APPROVED (strict mode)

---

## Risk Assessment

- **Large surface area**: 28 Expr + 12 Pat + 16 Ty + 11 Item variants = 67
  lowering functions. Risk: missed variant.
  Mitigation: exhaustiveness check via `match` on AST enum (compiler
  warns if a variant is missing).
- **HirId allocation order**: must be deterministic for reproducible
  builds. Risk: non-deterministic iteration over HashMap.
  Mitigation: use Vec for owners/bodies during construction, convert to
  HashMap at the end if needed; or use BTreeMap.
- **Owner nesting**: trait items and impl items are nested owners.
  Risk: forgetting to `enter_owner` / `exit_owner`.
  Mitigation: RAII guard `OwnerGuard` that auto-exits on drop.
- **BodyId vs inline closure body**: Stage 1.1 inlined closure bodies.
  Stage 1.2 keeps this design (closure body is a sub-expression, not a
  separate BodyId). Risk: Stage 1.3 name resolution may need to treat
  closures as separate scopes.
  Mitigation: document this design choice; revisit in Stage 1.4 if needed.

## Time estimate

- Phase A (infrastructure): 1 hour
- Phase B (items): 2 hours
- Phase C (body): 2 hours
- Phase D (generics/use): 1 hour
- Phase E (tests + docs): 2 hours
- Self-review + committee: 1 hour
- **Total**: 9 hours

---

**This plan is the contract for Stage 1.2. Deviations require a new plan
and re-approval.**
