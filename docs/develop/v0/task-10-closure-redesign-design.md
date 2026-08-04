# Task 10: Closure Redesign — Design Document

> **Author**: redskaber
> **Date**: 2026-08-03
> **Status**: Step 1 in progress (Stage 16.13), Steps 2-5 pending
> **Process**: stage-committee-process.md v3.24 §1.0 原則 6 "通用 > 特例" + §13.4 (数据结构选型) + §16 (接口隔离)
> **Reference**: `docs/develop/v0/stage-13/stage-13.3-design-alignment.md` (original design)

## 1. Executive Summary

Task 10 redesigns closure call lowering from the current inline approach
(Stage 13.3a) to **Strategy A** (rustc-style synthesized `call` function
per closure). This aligns with the pre-sanctioned design in
`docs/lang-design/07-codegen.md` §8.1-8.2.

**Current state**: Stage 13.3a (inline approach) works but has limitations:
- Closure body is inlined at each call site → code bloat
- No optimization opportunity (LLVM can't deduplicate)
- Closure body locals pollute the enclosing function's MIR
- Doesn't match Rust's approach (synthesized `call` function)

**Target**: Strategy A — each closure literal generates:
1. An anonymous struct holding captures as fields (already exists)
2. A synthesized `call` function: `extern "Landin" fn call(&self, args...) -> ret`
3. Call site `f(42)` lowers to `TerminatorKind::Call` to the synthesized function

## 2. Problem Statement

### 2.1 Current Architecture (Stage 13.3a — Inline)

```
let f = |x| x + 1;
f(42);
```

MIR (current — inline):
```
// In main's MIR:
local_1 = Aggregate(Closure, [captures...])  // closure struct
// f(42) call:
local_2 = 42  // arg
// Inline closure body:
local_3 = local_2 + 1  // body inlined
// Result: local_3
```

### 2.2 Problems with Inline Approach

1. **Code bloat**: Each call site gets a full copy of the body
2. **No optimization**: LLVM can't deduplicate the inlined copies
3. **MIR pollution**: Body locals pollute the enclosing function
4. **Doesn't match design**: `07-codegen.md` §8.1-8.2 prescribes Strategy A
5. **Doesn't match Rust**: Rust synthesizes a `call` function per closure

### 2.3 Target Architecture (Strategy A)

```
let f = |x| x + 1;
f(42);
```

MIR (target — synthesized `call` function):
```
// In main's MIR:
local_1 = Aggregate(Closure, [captures...])  // closure struct
// f(42) call:
local_2 = 42  // arg
local_3 = Call(closure_call_fn_0, [local_1, local_2])  // real call

// Synthesized function (separate MIR body):
fn closure_call_fn_0(self: Closure_0, x: i32) -> i32 {
    local_1 = Projection(self, Field(0))  // extract captures if needed
    local_2 = x + 1  // body
    return local_2
}
```

## 3. Design

### 3.1 Synthesized Closure Function Representation

Add a side-table to `MirLowerCtxt`:

```rust
/// Stage 16.13 (Task 10 Step 1): Synthesized closure `call` functions.
/// Each entry represents a closure literal that needs a synthesized
/// `call` function. The function is built during MIR lowering and
/// emitted as a separate MIR body by codegen.
///
/// Keyed by the closure's DefId (allocated during lowering).
pub synthesized_closure_functions: HashMap<DefId, SynthesizedClosureFunction>,
```

```rust
/// Stage 16.13: A synthesized `call` function for a closure.
#[derive(Clone, Debug)]
pub struct SynthesizedClosureFunction {
    /// The closure's DefId (unique per closure literal).
    pub def_id: DefId,
    /// The closure's parameters (HIR).
    pub params: Vec<HirParam>,
    /// The closure's body (HIR).
    pub body: Box<HirExpr>,
    /// The capture info: (HirId, field_index, field_type).
    pub captures: Vec<(HirId, u32, Ty)>,
    /// The closure struct type (for the `self` parameter).
    pub closure_struct_ty: Ty,
    /// The function name for codegen (e.g., "closure_call_fn_0").
    pub fn_name: String,
}
```

### 3.2 DefId Allocation

Each closure literal gets a unique DefId. The current code uses the
crate's first owner DefId (incorrect — all closures in a crate share
the same DefId). Stage 16.13 allocates unique DefIds:

```rust
/// Stage 16.13: Allocate a unique DefId for a closure.
/// Uses a reserved range (CLOSURE_DEF_ID_BASE downward) to avoid
/// collision with user-defined items.
fn allocate_closure_def_id(cx: &mut MirLowerCtxt) -> DefId {
    let id = CLOSURE_DEF_ID_BASE - cx.closure_def_id_counter as u32;
    cx.closure_def_id_counter += 1;
    DefId::new(id)
}
```

### 3.3 Migration Plan

The migration from inline to synthesized `call` function is done in
steps to avoid breaking existing tests:

#### Step 1 (Stage 16.13 — this stage): Infrastructure
- Add `synthesized_closure_functions` side-table to `MirLowerCtxt`
- Add `SynthesizedClosureFunction` struct
- Add `allocate_closure_def_id()` helper
- Add `build_synthesized_closure_function()` (builds the struct but
  doesn't emit MIR yet)
- **No behavior change** — inline approach still used for calls

#### Step 2 (future): MIR Body Synthesis
- Build a separate MIR body for each synthesized closure function
- The MIR body has: `self` parameter, closure params, body lowered
- Store the synthesized MIR body in `CompileResult`

#### Step 3 (future): Call Site Migration
- Change `lower_closure_call_inline` to emit `TerminatorKind::Call`
  to the synthesized function
- Remove the inline body lowering

#### Step 4 (future): Codegen
- Emit the synthesized function as an LLVM function
- Call site generates a real LLVM `call` instruction

#### Step 5 (future): Cleanup
- Remove `ClosureBodyInfo` side-table (no longer needed)
- Remove `lower_closure_call_inline` function
- Update tests

## 4. Data Structure Selection (§13.4)

### 4.1 Side-Table vs. Embedded

**Choice**: Side-table (`HashMap<DefId, SynthesizedClosureFunction>`)

**Rationale**:
- Closures are relatively rare (most functions don't have closures)
- HashMap lookup by DefId is O(1)
- Keeps `MirBody` clean (closure metadata is separate from function MIR)
- Matches the pattern of `adt_layouts`, `closure_bodies` (existing side-tables)

### 4.2 DefId Allocation

**Choice**: Reserved range (`CLOSURE_DEF_ID_BASE = u32::MAX - 1000`)

**Rationale**:
- Avoids collision with user-defined items (which use DefIds from HIR)
- Matches the pattern of `BUILTIN_DEF_ID_BASE` (Stage 5.8)
- Simple counter, no need for a DefId allocator

### 4.3 Function Naming

**Choice**: `closure_call_fn_{counter}` (e.g., `closure_call_fn_0`)

**Rationale**:
- Unique per closure literal
- Descriptive (clear it's a synthesized closure function)
- Matches the pattern of `landin_{type}_{method}` for trait methods

## 5. §16 Interface Isolation Compliance

- `MirLowerCtxt` builds the side-table during lowering (allowed — MIR lower reads HIR)
- Codegen reads the side-table from `CompileResult` (allowed — data flows downstream)
- Borrow checker doesn't need to know about synthesized functions (they're just regular functions)
- No new HIR access from codegen (side-table carries all needed info)

## 6. §23 API Naming Compliance

| Item | Pattern | Status |
|------|---------|--------|
| `synthesized_closure_functions` | `<adj>_<noun>_<noun>` | ✅ |
| `SynthesizedClosureFunction` | `<Adj><Noun>` | ✅ |
| `allocate_closure_def_id` | `<verb>_<noun>_<noun>` | ✅ |
| `build_synthesized_closure_function` | `<verb>_<adj>_<noun>` | ✅ |
| `closure_call_fn_{n}` | `<noun>_<verb>_<noun>_{n}` | ✅ |

## 7. Testing Strategy

### Step 1 Tests (this stage)
- `synthesized_closure_functions` side-table is populated
- DefId allocation is unique
- `SynthesizedClosureFunction` struct has correct fields
- No behavior change (inline approach still works)

### Step 2+ Tests (future)
- Synthesized MIR body has correct structure
- Call site emits `TerminatorKind::Call`
- Codegen emits LLVM function
- End-to-end closure call works

## 8. Risks and Mitigations

| Risk | Mitigation |
|------|------------|
| Breaking existing closure tests | Step 1 is infrastructure-only, no behavior change |
| DefId collision | Reserved range, counter-based allocation |
| Performance regression | Synthesized function is built once, called many times |
| Migration complexity | 5-step gradual migration, each step independently testable |

## 9. References

- Original design: `docs/develop/v0/stage-13/stage-13.3-design-alignment.md`
- Codegen design: `docs/lang-design/07-codegen.md` §8.1-8.2
- MIR design: `docs/lang-design/06-mir.md` §5
- Ownership design: `docs/lang-design/04-ownership-borrowing.md` §8
- Stage committee process: `docs/stage-committee-process.md` §13.4, §16, §23
