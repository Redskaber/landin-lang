# v0.2 Phase 2: Region Allocation Design (HP-5)

> **Author**: redskaber
> **Date**: 2026-08-01
> **Version**: v0.173.0
> **Process**: stage-committee-process.md v3.23 §13.4 (设计对齐) + §29
> **v0.2 Phase 2 Task 9**: Proper region allocation (HP-5)
> **Dependency**: Task 7 (NLL fixpoint) — COMPLETE (Stage 15.41)

## 1. Problem Statement

The current compiler has **1472 LOC of region inference infrastructure**
(`src/borrowck/region_inference.rs`) that was built in Stages 7.1-7.5
(TD-015). However, this infrastructure is **effectively a no-op** because
all MIR regions are `Region::Erased` (mapped to `'static` vid 0). The
`run_region_inference` method in `borrowck/mod.rs` runs the inference but
produces empty point sets — it validates the infrastructure without
producing any actual region checking.

### Current state (v0.173.0)

- `RegionInferenceContext` exists with:
  - `new()` — creates an empty context.
  - `region_to_vid()` — maps `Region` to `RegionVid` (all map to 0 = `'static`).
  - `collect_implied_bounds()` — collects `T: 'a` constraints from `&'a T` locals.
  - `infer_regions()` — runs fixpoint iteration (produces empty sets since all regions are `'static`).
- `run_region_inference()` is called in `check_mir_body_with_dataflow` (after the dataflow borrow check).
- All MIR regions are `Region::Erased` or `Region::Static` — no real lifetime annotations.
- The region inference is a **no-op** — it doesn't produce false positives (good) but also doesn't check anything (bad).

### What's missing

1. **Lifetime annotations in MIR**: The HIR-to-MIR lowering currently erases all lifetimes. The MIR needs to carry real `Region::Var(vid)` for each reference's lifetime.
2. **Constraint collection from MIR**: The current `collect_implied_bounds` only looks at local declarations. It needs to also collect constraints from:
   - Assignments: `r = &x` creates a constraint `lifetime(r) >= lifetime(x)`.
   - Function calls: `f(&x, &y)` creates constraints between the argument lifetimes and the parameter lifetimes.
   - Return values: `f() -> &T` creates a constraint between the return lifetime and the function's lifetime parameters.
3. **Error reporting**: When region inference finds a constraint violation (e.g., a reference outlives its referent), it should produce a `BorrowError` with a meaningful message.
4. **Integration with NLL**: The region inference should work alongside the dataflow borrow checker, not replace it. The dataflow checker handles borrow lifetimes (when borrows expire); the region checker handles local lifetimes (when locals go out of scope).

## 2. Design: Region Allocation

### 2.1 Overview

Region allocation is the process of assigning concrete region variables
(`RegionVid`) to each lifetime in the MIR, then running the region
inference algorithm to verify that all lifetime constraints are satisfied.

The design follows rustc's approach (simplified for v0.2):

1. **Lifetime elision**: Apply Rust's lifetime elision rules to infer
   lifetimes where they're not explicitly written. (Most Landin code
   doesn't write explicit lifetimes — elision handles the common cases.)

2. **MIR region assignment**: During HIR-to-MIR lowering, assign a
   fresh `Region::Var(vid)` to each reference type. Track the mapping
   from source-level lifetimes to MIR region variables.

3. **Constraint collection**: Walk the MIR and collect outlives
   constraints:
   - `&'a T` local → `T: 'a` (implied bounds, already implemented).
   - `r = &x` → `lifetime(r) >= lifetime(x)`.
   - `f(&x)` → `lifetime(arg) >= lifetime(param)`.
   - `return &x` → `lifetime(return) >= lifetime(x)`.

4. **Region inference**: Run the existing `infer_regions()` fixpoint
   algorithm to compute the region graph. Check for violations:
   - A reference's lifetime must not outlive its referent.
   - A universal region (`'static`) must not be constrained to outlive
     a local region.

5. **Error reporting**: Convert region inference errors to `BorrowError`s
   with span information and meaningful messages.

### 2.2 What's already implemented

The following are already in `src/borrowck/region_inference.rs` (1472 LOC):

- `RegionInferenceContext` with:
  - `region_to_vid()` — maps `Region` to `RegionVid`.
  - `collect_implied_bounds()` — collects `T: 'a` constraints.
  - `infer_regions()` — fixpoint iteration with SCC compression.
  - Universe tracking (Stage 7.4).
  - Type tests (Stage 7.3).
  - Outlives constraints (Stage 7.2).

The infrastructure is sound — it just needs real region variables and
constraints to work with.

### 2.3 What needs to be implemented

| Step | Description | Effort |
|------|-------------|--------|
| 1 | **Lifetime elision rules** — infer lifetimes for `&T` in function signatures (input lifetimes → output lifetimes). | 2 days |
| 2 | **MIR region assignment** — during lowering, assign `Region::Var(vid)` to each reference type. | 1 day |
| 3 | **Constraint collection from MIR** — walk statements/terminators and collect outlives constraints. | 2 days |
| 4 | **Error reporting** — convert `RegionInferenceError` to `BorrowError` with span + message. | 0.5 day |
| 5 | **Integration** — wire into driver pipeline (after dataflow borrow check). | 0.5 day |
| 6 | **Testing** — conformance tests with lifetime patterns. | 1 day |

Total: ~1 week (per v0.2-preparation.md).

### 2.4 Lifetime elision rules

Rust's elision rules (simplified for v0.2 MVP):

1. **Each elided input lifetime gets its own fresh lifetime.**
   - `fn foo(x: &T, y: &U)` → `fn foo<'a, 'b>(x: &'a T, y: &'b U)`.
2. **If there's exactly one input lifetime, all elided output lifetimes get that lifetime.**
   - `fn foo(x: &T) -> &U` → `fn foo<'a>(x: &'a T) -> &'a U`.
3. **If there are multiple input lifetimes and one is `&self`/`&mut self`, all elided output lifetimes get `self`'s lifetime.**
   - `impl T { fn foo(&self, x: &U) -> &V }` → `fn foo<'a, 'b>(&'a self, x: &'b U) -> &'a V`.
4. **Otherwise, elided output lifetimes are an error (or `'static` for v0.2 MVP).**

### 2.5 MIR region assignment

During HIR-to-MIR lowering (`src/mir/lower/`), when we encounter a
`TyKind::Ref(region, mutability, inner_ty)`:

- If `region` is `Region::Erased` (elided), assign a fresh `Region::Var(vid)`.
- If `region` is `Region::Static`, keep it as `Region::Static`.
- If `region` is `Region::Var(vid)` (explicit lifetime), keep it.

The `MirLowerCtxt` needs a `region_counter: u32` to allocate fresh vids.

### 2.6 Constraint collection from MIR

Walk all basic blocks and collect constraints:

```rust
fn collect_constraints_from_mir(&mut self, mir: &MirBody) {
    for bb in &mir.basic_blocks {
        for stmt in &bb.statements {
            if let StatementKind::Assign((place, rvalue)) = &stmt.kind {
                // r = &x → lifetime(r) >= lifetime(x)
                if let Rvalue::Ref(region, _, borrowed_place) = rvalue {
                    let ref_vid = self.region_to_vid(*region);
                    let borrowed_ty = place_ty(mir, borrowed_place);
                    self.add_outlives(ref_vid, borrowed_ty);
                }
                // r = call(args) → lifetime(r) >= lifetime(return)
                // (handled by terminator constraints)
            }
        }
        // Terminator constraints:
        // f(&x, &y) → lifetime(arg) >= lifetime(param)
        if let TerminatorKind::Call { func, args, .. } = &bb.terminator.kind {
            // For each argument, add constraint: arg_lifetime >= param_lifetime
        }
    }
}
```

### 2.7 Error reporting

Convert `RegionInferenceError` to `BorrowError`:

```rust
match result {
    Ok(()) => {} // No errors
    Err(errors) => {
        for err in errors {
            self.errors.push(BorrowError::new(
                &format!("lifetime error: {}", err.message),
                err.span,
                BorrowErrorKind::LifetimeError,
            ));
        }
    }
}
```

## 3. Migration Strategy

### 3.1 Staged implementation (1 week)

| Stage | Description | Effort |
|-------|-------------|--------|
| 15.48 | **Design doc** (this stage) — design alignment per §13.4 | 0 (doc only) |
| 15.49 | Implement lifetime elision rules + MIR region assignment | 2 days |
| 15.50 | Implement constraint collection from MIR statements/terminators | 2 days |
| 15.51 | Implement error reporting + integration into driver pipeline | 1 day |
| 15.52 | Add conformance tests with lifetime patterns + gate review | 1 day |

### 3.2 What's in scope for v0.2 MVP

- Lifetime elision rules 1-3 (input → output lifetime inference).
- MIR region assignment for reference types.
- Constraint collection from assignments and function calls.
- Error reporting with span information.
- Integration alongside the dataflow borrow checker.

### 3.3 What's NOT in scope for v0.2 MVP

- Explicit lifetime annotations (`'a`, `'static` in source code) — future.
- HRTB (higher-ranked trait bounds) — Task 18, deferred.
- Region variables in generic types — requires monomorphization.
- Universal quantification in function signatures — future.

## 4. Dependencies

- **Task 7 (NLL)**: COMPLETE (Stage 15.41). The dataflow borrow checker
  correctly tracks borrow lifetimes, which is the foundation for region
  allocation.
- **Region inference infrastructure**: EXISTS (Stages 7.1-7.5, 1472 LOC).
  The `RegionInferenceContext`, constraint collection, and fixpoint
  iteration are all implemented — they just need real region variables
  and constraints.

## 5. Testing Strategy

### 5.1 Unit tests

- Lifetime elision rules (each rule, edge cases).
- MIR region assignment (verify fresh vids are assigned).
- Constraint collection (verify correct constraints from MIR).

### 5.2 Integration tests

- Compile programs with `&T` references and verify no false positives.
- Compile programs with lifetime violations and verify errors are reported.
- Verify the existing 5216 conformance tests still pass (zero regression).

## 6. API Naming Compliance (§23)

| Symbol | Pattern | Status |
|--------|---------|--------|
| `elide_lifetimes` | `<verb>_<noun>` (free function) | ✅ |
| `assign_mir_regions` | `<verb>_<noun>_<noun>` (free function) | ✅ |
| `collect_mir_constraints` | `<verb>_<noun>_<noun>` (free function) | ✅ |

Per §23.1 rule 1: free-function entry points.
Per §16: region inference reads MIR data — no HIR lookup (uses sunk data).

## 7. Open Questions

1. **Interaction with `Region::Erased`**: The current MIR has `Region::Erased`
   everywhere. Should we replace all `Erased` with fresh `Var(vid)` during
   lowering, or keep `Erased` for non-reference types?

2. **Function signature lifetimes**: The current `Sig` type doesn't carry
   lifetime information. We may need to extend it to track which parameters
   have lifetimes and how they relate to the return type.

3. **Interaction with `elaborate_drops`**: Drop terminators create new
   control-flow edges. Do these edges need region constraints?

4. **Performance**: The region inference fixpoint iteration is O(N²) in the
   number of regions. For large functions, this could be slow. May need
   work-queue optimization (like rustc's `WorkQueue`).

These will be resolved in the implementation stages (15.49-15.52).

## 8. Effort

- 1 week (per v0.2-preparation.md)
- Stages 15.48 (design) + 15.49-15.51 (implementation) + 15.52 (testing + review)
- Each stage independently testable.
