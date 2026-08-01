# v0.2 Phase 2: Drop Elaboration Design

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.167.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 8**: Wire up drop elaboration (HP-12)
> **Dependency**: Task 7 (NLL fixpoint) — COMPLETE (Stage 15.41)

## 1. Problem Statement

The current compiler does **not** support user-defined `Drop` implementations.
The `TerminatorKind::Drop` terminator is a no-op in codegen (Stage 14.103),
and the `drop_elaboration` module was removed as dead code (Stage 14.105).

This means:
1. **No RAII**: Types like `File`, `MutexGuard`, `Box<T>` cannot release
   resources automatically when they go out of scope.
2. **No memory safety for heap types**: `Box<T>` cannot free its allocation
   on scope exit — memory leaks.
3. **Unsound `let` bindings**: A `let x = File::open(...)` never closes
   the file, violating Rust's RAII guarantee.

### Current state (v0.167.0)

- `TerminatorKind::Drop { place, target, replace }` exists in MIR but is
  a no-op in codegen (`src/codegen/terminator.rs` line 422).
- `StatementKind::StorageDead(LocalId)` is emitted at function return
  (conservatively, for all locals) but does NOT trigger drop — it's just
  a marker.
- `TraitResolver::is_drop_builtin(def_id, interner)` exists and can query
  whether a type implements `Drop`.
- The stdlib defines `Drop` trait with `fn drop(&mut self)` (see
  `docs/lang-design/09-stdlib.md` §"Drop").
- No `drop_elaboration` module exists — it was removed in Stage 14.105.

## 2. Design: Drop Elaboration

### 2.1 Overview

Drop elaboration is the pass that inserts `Drop` terminators into the MIR
at points where a local goes out of scope. The pass runs after MIR
lowering and before borrow checking (so the borrow checker sees the
`Drop` terminators).

The design follows rustc's approach (simplified for v0.2):

1. **`needs_drop` analysis**: For each `Ty`, determine whether it needs
   drop glue (recursively). A type needs drop if:
   - It implements `Drop`, OR
   - It has fields that need drop (for structs/enums), OR
   - It's a container of a type that needs drop (for `Box<T>`, `Vec<T>`).

2. **Drop insertion**: For each `StorageDead(local)` statement, if the
   local's type needs drop, insert a `Drop` terminator before the
   `StorageDead`. The `Drop` terminator calls the type's drop glue.

3. **Drop glue codegen**: For each type that needs drop, generate a
   `drop_<Type>` function that:
   - Calls the user's `Drop::drop` method (if `impl Drop` exists).
   - Then recursively drops each field (for structs/enums).

4. **Drop order**: Fields are dropped in declaration order (matching
   Rust's drop order). Locals are dropped in reverse declaration order
   (matching Rust's scope-end drop order).

### 2.2 The `needs_drop` analysis

```rust
/// Determine whether a type needs drop glue.
///
/// A type needs drop if:
/// - It implements `Drop` (user-defined destructor), OR
/// - It's a struct/enum with fields that need drop, OR
/// - It's `Box<T>` / `Vec<T>` / etc. (container of a needs-drop type).
///
/// Primitive types (i32, bool, etc.) never need drop.
/// References (&T, &mut T) never need drop (they're just pointers).
pub fn ty_needs_drop(ty: &Ty, resolver: &TraitResolver, interner: &Rodeo) -> bool {
    match &ty.kind {
        TyKind::Int(_) | TyKind::Uint(_) | TyKind::Bool | TyKind::Char
        | TyKind::Float(_) | TyKind::Str | TyKind::RawPtr(_, _) => false,
        TyKind::Ref(_, _, _) => false,  // references are just pointers
        TyKind::Adt(def_id, substs) => {
            // Check if the type implements Drop.
            if resolver.is_drop_builtin(*def_id, interner) {
                return true;
            }
            // Check if any field needs drop (recursive).
            // This requires field type lookup — needs adt_layouts.
            // TODO: implement field type traversal.
            false  // v0.2 MVP: only user-defined Drop triggers needs_drop
        }
        TyKind::Array(inner, _) => ty_needs_drop(inner, resolver, interner),
        TyKind::Slice(inner) => ty_needs_drop(inner, resolver, interner),
        TyKind::Tuple(items) => items.iter().any(|t| ty_needs_drop(t, resolver, interner)),
        TyKind::FnDef(_, _) | TyKind::FnPtr(_) => false,
        TyKind::Closure(_, _, _) => false,  // closures don't have Drop (v0.2)
        TyKind::Dynamic(_, _) => true,  // dyn Trait always needs drop (vtable has drop slot)
        TyKind::Never | TyKind::Error => false,
        TyKind::Infer(_) => false,  // shouldn't happen after typeck
    }
}
```

### 2.3 Drop insertion pass

```rust
/// Insert `Drop` terminators before `StorageDead` statements for locals
/// whose type needs drop.
///
/// This pass runs after MIR lowering and before borrow checking.
/// It modifies the MIR in place — splitting basic blocks as needed.
pub fn elaborate_drops(mir: &mut MirBody, resolver: &TraitResolver, interner: &Rodeo) {
    for bb_idx in 0..mir.basic_blocks.len() {
        let bb = &mir.basic_blocks[bb_idx];
        let mut new_stmts = Vec::new();
        for stmt in &bb.statements {
            if let StatementKind::StorageDead(local_id) = &stmt.kind {
                let local_ty = &mir.local(*local_id).ty;
                if ty_needs_drop(local_ty, resolver, interner) {
                    // Insert a Drop terminator BEFORE the StorageDead.
                    // This requires splitting the basic block.
                    // TODO: implement block splitting.
                }
            }
            new_stmts.push(stmt.clone());
        }
        // ... update the block with new_stmts ...
    }
}
```

### 2.4 Drop glue codegen

```rust
/// Generate the drop glue function for a type.
///
/// For a type `T` that implements `Drop`:
/// ```llvm
/// define void @"drop_<T>"(%T* %self) {
///     call void @"<T>::drop"(%T* %self)     ; user's Drop::drop
///     ; then drop each field (if the field type needs drop)
///     call void @"drop_<FieldType1>"(%FieldType1* %field1_ptr)
///     ret void
/// }
/// ```
///
/// For a type that doesn't implement `Drop` but has fields that need drop:
/// ```llvm
/// define void @"drop_<T>"(%T* %self) {
///     ; no user Drop::drop call
///     call void @"drop_<FieldType1>"(%FieldType1* %field1_ptr)
///     ret void
/// }
/// ```
pub fn emit_drop_glue(emitter: &mut Emitter, ty: &Ty, resolver: &TraitResolver) {
    // TODO: implement
}
```

### 2.5 Codegen for `TerminatorKind::Drop`

```rust
// In src/codegen/terminator.rs:
TerminatorKind::Drop { place, target, .. } => {
    let place_ty = place_ty(mir, place);
    if ty_needs_drop(&place_ty, resolver, interner) {
        // Get the place's address.
        let place_ptr = emit_place_address(emitter, mir, place);
        // Call the drop glue function.
        let drop_fn = format!("drop_{:?}", place_ty.kind);
        emitter.emit_call(&drop_fn, &[place_ptr], &EmitType::Void);
    }
    emitter.emit_br(&format!("bb{}", target.0));
}
```

## 3. Migration Strategy

### 3.1 Staged implementation (3-5 days per the roadmap)

| Stage | Description | Effort |
|-------|-------------|--------|
| 15.42 | **Design doc** (this stage) — design alignment per §13.4 | 0 (doc only) |
| 15.43 | Implement `ty_needs_drop` analysis + unit tests | 0.5 day |
| 15.44 | Implement `elaborate_drops` pass (insert `Drop` terminators) + tests | 1 day |
| 15.45 | Implement drop glue codegen + `TerminatorKind::Drop` codegen | 1 day |
| 15.46 | Integration: wire into driver, add conformance tests | 0.5 day |
| 15.47 | Gate review + deep review | 0.5 day |

### 3.2 What's in scope for v0.2 MVP

- User-defined `impl Drop for T { fn drop(&mut self) { ... } }`.
- Drop glue called at scope end (`StorageDead` points).
- Drop order: fields in declaration order, locals in reverse declaration order.
- `Box<T>` drop (frees allocation) — requires `Box` to be in stdlib prelude.

### 3.3 What's NOT in scope for v0.2 MVP

- Drop on panic (unwind) — v0.2 aborts on panic, no unwind.
- Drop for partially-moved values (moved fields skipped) — future.
- Drop for `dyn Trait` (vtable drop slot) — future.
- Drop for closures (closures don't have Drop in v0.2).
- `ManuallyDrop<T>` wrapper — future.

## 4. Dependencies

- **Task 7 (NLL)**: COMPLETE (Stage 15.41). The dataflow borrow checker
  correctly tracks liveness, which is needed to determine when a local
  goes out of scope (its `StorageDead` point).
- **Task 1 (Ty interning)**: COMPLETE (Stage 15.28). The `Ty` type is
  cheap to clone, which `ty_needs_drop` relies on for recursive traversal.
- **TraitResolver**: EXISTS (Stage 5.10). `is_drop_builtin` can query
  whether a type implements `Drop`.

## 5. Testing Strategy

### 5.1 Unit tests

- `ty_needs_drop` on various types (primitives, structs, enums, Box, etc.).
- `elaborate_drops` on synthetic MIR (verify `Drop` terminators inserted).

### 5.2 Integration tests

- Compile a program with `impl Drop for T` and verify the drop method is
  called at scope end.
- Verify drop order (fields in declaration order).
- Verify `Box<T>` frees its allocation.

### 5.3 Conformance tests

- Add `.lin` files with `impl Drop` patterns.
- Verify `run_ok` tests produce correct output (e.g., a counter incremented
  in `Drop::drop`).

## 6. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `ty_needs_drop` | `<noun>_<verb>_<noun>` (free function) | ✅ |
| `elaborate_drops` | `<verb>_<noun>` (free function, pass entry point) | ✅ |
| `emit_drop_glue` | `<verb>_<noun>_<noun>` (codegen helper) | ✅ |
| `DropElaborationCtxt` | `<Noun><Noun>` (context struct, if needed) | ✅ |

Per §23.1 rule 1: free-function entry points (`ty_needs_drop`,
`elaborate_drops`).
Per §23.1 rule 7: `emit_` prefix for codegen helpers.

## 7. Open Questions

1. **Field type traversal**: `ty_needs_drop` for `Adt` types needs to
   look up field types. This requires the `adt_layouts` infrastructure.
   The current `AdtLayouts` stores layout but not field types — may need
   to extend it or use HIR lookup (which violates §16).

2. **Block splitting**: Inserting a `Drop` terminator before a
   `StorageDead` statement requires splitting the basic block (the
   `Drop` terminator ends a block, the `StorageDead` starts a new one).
   The current MIR builder doesn't have a "split block" API — may need
   to add one.

3. **Drop glue function names**: Need a consistent naming scheme for
   drop glue functions (e.g., `drop_<Type>`). Need to handle generic
   types (monomorphization will produce `drop_<Type>_i32`, etc.).

4. **Interaction with `move`**: If a local is moved before its scope
   ends, the `Drop` terminator should NOT be emitted (the value is gone).
   This requires tracking move state in the drop elaboration pass.

These will be resolved in the implementation stages (15.43-15.46).

## 8. Effort

- 3-5 days (per v0.2-preparation.md)
- Stages 15.42 (design) + 15.43-15.46 (implementation) + 15.47 (review)
- Each stage independently testable.
