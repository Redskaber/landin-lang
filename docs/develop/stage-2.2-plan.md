# Stage 2.2 — Type Inference + Trait Resolution

> **Sub-stage**: 2.2 (Month 5-6)
> **Complexity**: L3 (core algorithm, complex unification logic)
> **Baseline rounds**: 8~15
> **Goal**: Implement a Hindley-Milner-style type inference engine
> that walks MIR bodies, unifies type variables, and populates
> `TyKind::Infer(var)` → concrete `TyKind`. Basic trait resolution
> for method dispatch.

---

## Scope

Stage 2.2 handles:

- Type inference engine (unification table, constraint collection)
- Walking MIR bodies to collect type constraints
- Unifying inference variables to concrete types
- Integer/float type defaulting (unsuffixed literals → i32/f64)
- Basic trait resolution (trait bound checking, method lookup)
- Type error reporting (mismatched types, unresolved types)
- Populating `Ty.inferred` on all `HirTy` nodes

**NOT in scope** (deferred):

- Full trait coherence checking (Stage 2.3+)
- Associated type projection (Stage 2.3+)
- Generic monomorphization (Stage 3)
- Lifetime inference (Stage 2.3 — borrow check)
- Const generic evaluation (Stage 3+)

---

## Tasks (12 atomic items across 4 phases)

### Phase A — Unification Engine

#### A1. `UnificationTable` — the core data structure

```rust
// src/typeck/unify.rs
pub struct UnificationTable {
    /// Inference variables → their current value (None if unbound)
    bindings: HashMap<TyVid, Option<Ty>>,
    /// Int inference variables → their current value
    int_bindings: HashMap<IntVid, Option<IntTy>>,
    /// Float inference variables → their current value
    float_bindings: HashMap<FloatVid, Option<FloatTy>>,
}
```

Methods: `new`, `new_ty_var`, `new_int_var`, `new_float_var`,
`unify(Ty, Ty) -> Result<(), TypeError>`, `resolve(Ty) -> Ty`,
`shallow_resolve(TyVid) -> Option<Ty>`.

#### A2. `TypeError` type

```rust
pub struct TypeError {
    pub message: String,
    pub span: Span,
    pub expected: Option<Ty>,
    pub found: Option<Ty>,
}
```

#### A3. Unification algorithm

`unify(a: Ty, b: Ty) -> Result<(), TypeError>`:

- If both are `Infer(TyVar(v))`: unify the two variables
- If one is `Infer(var)`: bind var to the other type
- If both are concrete with same kind: recursively unify sub-types
- If both are concrete with different kinds: TypeError
- If one is `Error`: ignore (error propagation)

`unify_int(a, b)`: similar for int variables
`unify_float(a, b)`: similar for float variables

### Phase B — Type Inference Pass

#### B1. `TypeChecker` struct

```rust
// src/typeck/mod.rs
pub struct TypeChecker {
    unify: UnificationTable,
    errors: Vec<TypeError>,
}
```

#### B2. `check_body(Body) -> Vec<TypeError>`

Walk each MIR body:

1. Assign types to locals from their declarations
2. Walk each basic block in order
3. For each `Statement::Assign(place, rvalue)`:
   - Infer the type of the rvalue
   - Unify it with the place's type
4. Check terminator (Call args match sig, SwitchInt discr is int/bool)

#### B3. `infer_rvalue(Rvalue) -> Ty`

- `Use(Operand::Constant(c))` → c.ty
- `Use(Operand::Copy/Move(lv))` → type of lv
- `BinaryOp(op, a, b)` → unify a,b; result type depends on op
  (comparison → bool, arithmetic → same as operands)
- `UnaryOp(op, a)` → same type as a (Neg) or bool (Not for bool)
- `Ref(_, _, lv)` → Ref type pointing to lv's type
- `Aggregate(Tuple, ops)` → Tuple type of operand types
- `Cast(_, _, ty)` → ty

#### B4. `infer_lvalue(Lvalue) -> Ty`

- `Local(id)` → local_decls[id].ty
- `Static(_)` → needs resolver (skip for now)
- `Projection(base, elem)` → walk projection

#### B5. Integer/float defaulting

After unification, any remaining `IntVar` defaults to `i32`,
any remaining `FloatVar` defaults to `f64`.

### Phase C — Trait Resolution (Basic)

#### C6. `TraitResolver` — check trait bounds

For each generic param with bounds (`T: Clone`):

- Record the bound as an obligation
- During type inference, when a concrete type is assigned to T,
  check if the type implements the trait
- For Stage 2.2, only check that the bound exists in the trait's
  impl list (no auto-derive, no blanket impls)

#### C7. Method lookup

When resolving `HirExprKind::MethodCall`:

- Look up the method in the receiver type's impl blocks
- For Stage 2.2: simple name-based lookup (no trait methods yet)

### Phase D — Tests

#### D1. Type inference tests (15+)

- 3 tests: basic unification (int with int, bool with bool, mismatch)
- 3 tests: inference variable binding (let x = 42 → x: i32)
- 3 tests: binary op type inference (1 + 2 → i32, a == b → bool)
- 2 tests: integer defaulting (unsuffixed → i32)
- 2 tests: ref type inference (&x → &T)
- 2 tests: tuple type inference

#### D2. Trait resolution tests (5+)

- 2 tests: trait bound satisfaction
- 2 tests: method lookup
- 1 test: trait bound violation → error

#### D3. Integration tests (5+)

- 2 tests: fibonacci type checks correctly
- 2 tests: struct field access type checks
- 1 test: type error is reported (mismatched types)

---

## Acceptance Criteria

1. ✅ All tasks implemented
2. ✅ `cargo build` 0 warnings
3. ✅ `cargo clippy --all-targets -- -D warnings` passes
4. ✅ `cargo fmt --check` passes
5. ✅ `cargo test` passes with ≥510 tests (486 existing + 25 new)
6. ✅ Type inference produces no false positives on valid programs
7. ✅ Type errors are reported for mismatched types
8. ✅ Committee vote ≥ 95% weighted approval (strict mode)
