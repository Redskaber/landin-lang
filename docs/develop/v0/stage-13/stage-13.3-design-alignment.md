# Stage 13.3 Design Alignment (§13.4) — Closure call lowering (TD-030 P0 closure)

> **Auditor**: ARCH-A + ALG-C (combined subagent) | **Date**: 2026-07-26 | **Baseline**: v0.22.0
> **Process**: stage-committee-process.md v3.21 §13.4 + §14.4 + §25.8
> **Priority**: P0 (largest single blocker for v0.3 self-hosting) — second user-facing compiler feature
> **Inputs**: `plan-13.1.md` (Stage 13 active plan, MUV-7/MUV-8 for Stage 13.3) +
> `stage-13.1-design-alignment.md` (version-policy reference) +
> `stage-13.2-design-alignment.md` (format reference) +
> r216 architecture audit §3.5 (TD-030 detail) + r217 stages-0-4 re-audit §2.4 + §4 (Stage 4.4 root cause) +
> 4 design docs (`04-ownership-borrowing.md` / `06-mir.md` / `07-codegen.md` / `13-stage1-feature-whitelist.md`) +
> `src/ast/kinds.rs` / `src/hir/kinds.rs` / `src/mir/ty.rs` / `src/mir/place.rs` /
> `src/mir/lower/expr_operand.rs` / `src/mir/lower/closure_capture.rs` /
> `src/mir/lower/mod.rs` / `src/typeck/checker.rs` / `src/codegen/mod.rs` /
> `src/codegen/mir_translation.rs` / `src/codegen/emitter.rs` / `src/traits/builtin.rs` +
> 40 conformance `compile_error` closure test files
> **Scope**: Stage 13.3 MUV-7 (closure call `Terminator::Call` synthesis) + MUV-8 (Fn/FnMut/FnOnce auto-impl strategy decision)

---

## 1. Executive Summary

Stage 13.3 closes TD-030 — the largest single P0 blocker for v0.3 self-hosting.
Closures currently **parse, lower to a `TyKind::Closure` value, and capture locals**
(Stage 4.4 type lowering + Stage 4.7 capture analysis), but **cannot be called**:
the `HirExprKind::Call` arm in `src/mir/lower/expr_operand.rs:527-589` detects closure
callees but emits only a placeholder result local with inferred type — no `Terminator::Call`
is generated, no closure body is dispatched, and codegen never sees a closure call.

**Findings**:

- **Codegen design alignment (smoking gun)**: `docs/lang-design/07-codegen.md` §8.1-8.2
  (lines 490-526) **explicitly prescribes Strategy A (rustc-style direct call function
  synthesis)**: each closure literal generates a unique anonymous struct holding captures
  as fields, plus a synthesized `call` function `extern "Landin" fn call(&self, args...) -> ret`
  that takes the closure struct as `&self` and the closure's declared parameters. The call
  site `f(42)` lowers to `call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)`.
  This is **exactly** rustc's approach and is **pre-sanctioned by the design doc** — Stage
  13.3 must implement what the design already prescribes (a B1 implementation gap, not a
  design gray area).

- **MIR design alignment**: `docs/lang-design/06-mir.md` §5 (line 278) defines
  `AggregateKind::Closure(DefId, Vec<GenericArg>)` — present in implementation at
  `src/mir/place.rs:181`. The implementation also has `TyKind::Closure(DefId, SubstsRef)`
  at `src/mir/ty.rs:51` — **the design doc does NOT explicitly document `TyKind::Closure`**
  (a B4 design-gray-area). The §14 + §15 §25.8 write-back sections cover
  `source_scopes`/dyn Trait lowering/DynTraitMIRSummary but are **silent on closure call
  lowering** — the deferral note lives only in a code comment at
  `src/mir/lower/expr_operand.rs:876` ("Closure call lowering: closure calls still go
  through regular Call" — Stage 4.13 inline-attempt comment, never fully realized).

- **Ownership design alignment**: `docs/lang-design/04-ownership-borrowing.md` §8
  documents disjoint closure captures (RFC 2229, deferred to v0.2+). The doc is silent
  on capture-mode inference (by-ref → `Fn`, by-mut-ref → `FnMut`, by-value → `FnOnce`).
  This is a B4 gray area but **not blocking** for Stage 13.3 (capture mode can default
  to by-ref, matching the current `closure_capture.rs` implementation that always emits
  `Operand::Copy`).

- **Stage 1 feature whitelist**: `docs/lang-design/13-stage1-feature-whitelist.md` §2.5
  (line 128) lists `Closure |x| x + 1` ✅ ALLOWED with remark "Fn/FnMut/FnOnce 自动推导".
  §3.1 (line 226) lists `core::ops::{Fn, FnMut, FnOnce}` as required stdlib dependencies.
  §4.1 lists "Disjoint closure captures (RFC 2229)" as Stage 0 must-support (deferred).
  The whitelist requires closures callable for Stage 1 — Stage 13.3 closes the B1 deviation.

- **Implementation status** (verified by direct read):
  - AST `Closure { is_move, params, body, span }` — ✅ at `src/ast/kinds.rs:509`
  - HIR `Closure { is_move, params, body }` — ✅ at `src/hir/kinds.rs:756`
  - MIR `TyKind::Closure(DefId, SubstsRef)` — ✅ at `src/mir/ty.rs:50-51`
  - MIR `AggregateKind::Closure(DefId, SubstsRef)` — ✅ at `src/mir/place.rs:181`
  - Capture analysis (`collect_captured_locals`) — ✅ at `src/mir/lower/closure_capture.rs:15-156`
  - `HirExprKind::Closure` lowering — ⚠️ partial at `src/mir/lower/expr_operand.rs:879-935`
    (constructs closure struct value, but lowers body inline into enclosing function — body
    result is discarded; body locals pollute the enclosing MIR)
  - `HirExprKind::Call` with closure callee — ❌ **DEFERRED** at `expr_operand.rs:527-589`
    (Stage 4.13 inline approach incomplete; produces placeholder result with inferred type;
    no `Terminator::Call` emitted)
  - Typeck `Terminator::Call` arm — ❌ rejects `TyKind::Closure` callee at `src/typeck/checker.rs:433-441`
    (G7 fix: "if func is neither FnDef nor FnPtr, emit an error")
  - Typeck `AggregateKind::Closure` arm — ❌ falls through to `TyKind::Error` at `checker.rs:847`
  - Codegen `TyKind::Closure` → EmitType — ✅ at `src/codegen/emitter.rs:487-490`
    (emits struct with capture field types)
  - Codegen `Rvalue::Aggregate(AggregateKind::Closure, ...)` — ❌ falls through to `"0"`
    placeholder at `src/codegen/mod.rs:630`
  - Codegen `Terminator::Call` with closure callee — ❌ no closure path; falls through to
    `"0"` placeholder at `src/codegen/mod.rs:940-942`
  - Traits `Fn`/`FnMut`/`FnOnce` — ⚠️ registered as builtin trait names at
    `src/traits/builtin.rs:8` but no auto-impl logic for closures

- **Conformance FAIL test count** (r217-verified methodology): **40** `// EXPECTED: compile_error`
  closure-related tests across the conformance tree (breakdown: 20 in
  `02-borrowck/03-closure-capture/`, 11 in `01-typecheck/03-closures/`, 3 in
  `04-e2e/03-closures/`, 3 in `02-borrowck/02-move-semantics/`, 2 in
  `02-borrowck/01-nll-advanced/`, 1 in `06-stdlib/02-std/`). **NOT 41** as in r216 (r216
  methodology error corrected by r217). These tests currently PASS because the compiler
  errors out; Stage 13.3 must flip them to `// EXPECTED: compile_ok` and remove
  `// ERROR_PATTERN:` lines. **0 `//! FAIL` markers** in closure test dirs (the modern
  conformance marker style is unused here).

- **Stage 4.4 root cause** (r217 §2.4 verified): Stage 4.4 added `AggregateKind::Closure`
  + `TyKind::Closure` (closure TYPE lowering) but **explicitly deferred call dispatch**.
  The deferral note at `src/mir/lower/expr_operand.rs:876` reads "Closure call lowering:
  closure calls still go through regular Call" — but the actual code at `expr_operand.rs:527-589`
  (Stage 4.13) is a half-finished inline approach that **does not** go through regular Call
  (it produces a placeholder). The comment is stale; the implementation is incomplete.

**Recommendation**: **Strategy A — Direct call function synthesis (rustc-style)** +
**Option B — Closure call lowering only, defer Fn/FnMut/FnOnce auto-impl to Stage 13.5+**.

- **File count**: **9 src + 1 new stage13_3 test + 40 conformance .lin + 4 design-doc
  write-back = 54 files**.
- **Risk**: **HIGH** (significant new MIR lowering infrastructure: synthesized `call`
  function MirBody per closure; new codegen for synthesized functions; typeck changes
  to accept `TyKind::Closure` callees). The 5026 conformance + 2179 integration tests
  provide strong regression coverage.
- **Version policy**: v0.22.0 → **v0.23.0** (minor bump — second user-facing compiler
  feature; per `stage-13.1-design-alignment.md` §5.4 line 543 pre-established policy).
- **Estimated effort**: 2-3 weeks per `plan-13.1.md` §2 Stage 13.3; r216's "200-400 LOC,
  ≤5 files" estimate was optimistic — actual scope is **~600-1000 LOC across 9 src files**.

---

## 2. Design Document Alignment (§13.4)

Per §13.4.1 step 1-3, each design doc is read against the planned implementation to identify
alignment, deviation, and gray-area decisions.

### 2.1 `07-codegen.md` §8 — Closure codegen (THE smoking gun)

**Read**: §8 闭包 codegen (lines 490-526), specifically §8.1 (闭包类型) and §8.2 (闭包调用 codegen).

**What the design says** (verified by direct read of `07-codegen.md:490-526`):

```
## 8. 闭包 codegen

### 8.1 闭包类型

每个闭包字面量生成一个唯一的匿名 struct：

    let f = |a: i32| a + outer;

    struct Closure<'a> {
        outer: &'a i32,
    }

    impl<'a> Fn<(i32,)> for Closure<'a> {
        extern "Landin" fn call(&self, a: i32) -> i32 {
            a + *self.outer
        }
    }

闭包类型在 typeck 阶段确定，每个调用点唯一。

### 8.2 闭包调用 codegen

    ; let f = |a| a + outer;
    %closure = alloca %Closure_type
    %outer_gep = getelementptr %Closure_type, %Closure_type* %closure, 0, 0
    store i32* %outer, i32** %outer_gep

    ; f(42)
    %result = call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)
```

**Does the design specify the closure struct layout?** **YES** — §8.1 explicitly: anonymous
struct with one field per captured variable. The example shows `outer: &'a i32` (capture by
reference). Implementation: `src/mir/ty.rs:51` `TyKind::Closure(DefId, SubstsRef)` where
`SubstsRef = Vec<Ty>` holds capture field types — matches design intent. Codegen of the
struct type is at `src/codegen/emitter.rs:487-490` — also matches.

**Does the design specify the call ABI for closures?** **YES** — §8.1 specifies
`extern "Landin" fn call(&self, a: i32) -> i32`. The `&self` is the closure struct (passed
by reference); the remaining params are the closure's declared params. §8.2 specifies the
LLVM IR pattern: `call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)` — direct
call to the synthesized `call` function with the closure struct pointer as first arg +
actual args.

**Does the design specify how closure calls are lowered?** **YES** — §8.2 is unambiguous:
the call site emits a direct `call` to the synthesized `call` function. This is **Strategy A
(rustc-style direct call function synthesis)**, pre-sanctioned by the design doc.

**Does the design mention `Fn`/`FnMut`/`FnOnce` auto-impl?** **YES** — §8.1's example
`impl<'a> Fn<(i32,)> for Closure<'a>` shows the closure struct implementing `Fn` with the
`call` method. The `&self` receiver matches `Fn::call(&self, args)`. `FnMut` would use
`&mut self`; `FnOnce` would use `self` (by value). The design implies auto-impl based on
capture mode (by-ref → Fn; by-mut-ref → FnMut; by-value → FnOnce), but does not explicitly
state the inference rule.

**Alignment verdict**: ✅ **PASS §13.4**. The codegen design fully anticipates Strategy A.
The implementation gap is a B1 deviation (impl < design) traced to Stage 4.4 (closure type
lowering added, call dispatch deferred per `expr_operand.rs:876` code comment). Stage 13.3
closes the gap by implementing what the design already specifies. The §14 + §15 §25.8
write-back sections cover Trait dispatch codegen (Stage 5.40-5.80) and extern "C" ABI
(Stage 8.3) but do NOT mention closure codegen status — Stage 13.3 §25.8 write-back should
add a §15.3 sub-section noting closure codegen is now implemented.

### 2.2 `06-mir.md` — MIR closure representation

**Read**: §5 Rvalue (lines 201-295), specifically `AggregateKind` at lines 274-283; §14
(Stage 6.18 §25.8 write-back, lines 929-995); §15 (Stage 12.4 §25.8 retroactive, lines 996-1035).

**What the design says** (verified by direct read of `06-mir.md:274-283`):

```rust
enum AggregateKind {
    Tuple,
    Array(Ty),
    Adt(DefId, VariantIdx, Vec<GenericArg>),
    Closure(DefId, Vec<GenericArg>),
    // v1.2.2 修正：与 rustc master 一致，Coroutine 不含 Movability 字段
    Coroutine(DefId, Vec<GenericArg>),
    CoroutineClosure(DefId, Vec<GenericArg>),
    // v0.2: 实际实现 Coroutine/CoroutineClosure
}
```

**Does the design document `AggregateKind::Closure`?** **YES** — §5 line 278 explicitly.
The signature `Closure(DefId, Vec<GenericArg>)` matches implementation at
`src/mir/place.rs:181` (`Closure(crate::hir::DefId, SubstsRef)` — `SubstsRef = Vec<Ty>`
holds capture field types).

**Does the design document `TyKind::Closure`?** **NO** — the MIR design doc's type system
section (§2 of `06-mir.md` does not enumerate `TyKind` variants; the only `TyKind` mentions
are passing references in §12 rustc-diff table and §14.1 implementation-status table). The
implementation has `TyKind::Closure(DefId, SubstsRef)` at `src/mir/ty.rs:50-51` — **a B4
design-gray-area write-back is required** to add `TyKind::Closure` to the MIR design doc
(either in §2 type enumeration or in a new sub-section).

**Does the design specify how closure calls are lowered in MIR?** **NO** — §8 MIR construction
algorithm (lines 467-568) describes CFG framework + drop elaboration but is silent on closure
call lowering. §14.2 §25.8 write-back covers dyn Trait lowering algorithm (Stage 5.78-5.80);
**closure call lowering algorithm is undocumented**. The implementation deferral note at
`expr_operand.rs:876` ("closure calls still go through regular Call") is the only place this
is mentioned — and it's a code comment, not a design doc.

**§14 + §15 §25.8 coverage**: Neither section mentions closure call lowering. The root cause
per r217 §2.4 + §4: "Stage 4.4 added closure TYPE lowering but explicitly deferred call
dispatch" — this deferral was never written back to `06-mir.md` §14 or §15. Stage 13.3
§25.8 write-back must add a §15.3 (or §16) sub-section documenting the closure call lowering
algorithm (synthesized `call` function MirBody per closure, registered in a per-crate side-table).

**Alignment verdict**: ⚠️ **PARTIAL §13.4**. `AggregateKind::Closure` is explicitly documented;
`TyKind::Closure` and the closure call lowering algorithm are B4 design-gray-areas. Stage 13.3
§25.8 write-back must:
1. Add `TyKind::Closure(DefId, SubstsRef)` to the MIR type enumeration in §2 or §5.
2. Add a new sub-section (§15.3 or §16) documenting the closure call lowering algorithm:
   synthesized `call` function MirBody per closure; closure struct as first param (`&self`);
   captures dereferenced via `Place::Projection(self, Field(i))`; call site dispatches via
   `Terminator::Call` to the synthesized function's DefId.

### 2.3 `04-ownership-borrowing.md` — Closure capture semantics

**Read**: §8 Disjoint closure captures (lines 508-544); §11.6 implementation status
(lines 628-641); grep across the 702-line file for `capture mode` / `Fn` / `FnMut` / `FnOnce` /
`by-ref` / `by-value`.

**What the design says**:

- §8 documents disjoint closure captures (RFC 2229): "Rust 2018+ 让闭包只捕获访问的字段"
  (line 524). Implementation deferred to v0.2+ (§11.6 line 632: "❌ 未实现 | B1 | v0.2+").
- §8.4: "闭包捕获 `big.a` 的 `&` 借用; 外部代码仍可 `&mut big.b` (不冲突)". This implies
  capture by-ref is the default mode.
- §7.3 (HRTB): `for<'a> Fn(&'a T) -> &'a U` — `Fn` appears only as a trait bound, not as a
  closure-kind inference rule.
- §2.4 Two-phase borrows: applies to method-call receiver borrows — orthogonal to closure
  capture mode.
- §5 Drop check: no closure-specific drop semantics.
- **No explicit mention** of capture mode inference (by-ref → Fn; by-mut-ref → FnMut;
  by-value → FnOnce). The doc is silent on the Fn/FnMut/FnOnce kind taxonomy.

**Does the design specify how closures capture variables?** **PARTIAL** — §8 implies by-ref
is the default (RFC 2229 disjoint captures). The doc does not enumerate the three capture
modes (by-ref, by-mut-ref, by-value/move) or specify when each is chosen. The implementation
at `src/mir/lower/closure_capture.rs:15-156` walks the body and collects all external locals
— it does not distinguish capture modes (currently always emits `Operand::Copy` at
`expr_operand.rs:910`).

**Does the design specify closure kind inference (Fn / FnMut / FnOnce)?** **NO** — the 702-line
doc has no mention of "Fn kind" or "closure kind" or the Fn/FnMut/FnOnce auto-impl rule. The
only `Fn` reference is in §7.3 HRTB as a trait bound. This is a B4 design-gray-area.

**Alignment verdict**: ⚠️ **PARTIAL §13.4**. The ownership doc covers disjoint captures
(deferred to v0.2) but is silent on capture mode inference and Fn/FnMut/FnOnce kind
taxonomy. For Stage 13.3, the practical decision is:
- **Default capture mode = by-ref** (matches current `Operand::Copy` behavior for Copy types;
  for non-Copy types, we need `Operand::Move` or borrow — but this is a Stage 13.5+ concern).
- **Defer Fn/FnMut/FnOnce auto-impl** to Stage 13.5+ (Option B). Stage 13.3 implements
  direct `call` function synthesis — closures are callable but not yet usable as `impl Fn(...)`
  trait bounds. The §25.8 write-back should add a §11.7 sub-section documenting this staging
  decision.

### 2.4 `13-stage1-feature-whitelist.md` §2.2 + §2.5 — Fn/FnMut/FnOnce

**Read**: §2.2 Trait 系统 (lines 53-74); §2.5 表达式 (lines 112-132); §3.1 core 模块 (lines 219-240);
§4.1 Stage 0 必须支持的特性 (lines 304-315).

**What the design says** (verified by direct read):

- §2.5 line 128: `Closure |x| x + 1` ✅ ALLOWED with remark **"Fn/FnMut/FnOnce 自动推导"**.
- §2.5 line 129: `move closure` ❌ v0.2 (deferred).
- §2.2 lists Trait definitions, `impl Trait for Type`, `dyn Trait`, `impl Trait` parameter
  syntax — all ✅ ALLOWED. The list does NOT explicitly enumerate "Fn/FnMut/FnOnce auto-impl
  for closures" — this is implicit in §2.5's remark "Fn/FnMut/FnOnce 自动推导".
- §3.1 line 226: `core::ops::{Add, Sub, ..., Fn, FnMut, FnOnce, Deref, DerefMut, Drop, Try}` —
  these are required stdlib dependencies for Stage 1.
- §4.1 (Stage 0 must-support): lists "Trait resolution 三阶段" and "Disjoint closure captures
  (RFC 2229)" but does NOT explicitly list "closure call lowering" or "Fn/FnMut/FnOnce
  auto-impl" as Stage 0 must-support items.

**Are closures listed as ALLOWED for Stage 1?** **YES** — §2.5 line 128.

**Is Fn/FnMut/FnOnce trait auto-impl specified?** **PARTIAL** — §2.5 remark "Fn/FnMut/FnOnce
自动推导" implies auto-impl, but the mechanism (capture mode → kind) is not specified. §3.1
requires `core::ops::{Fn, FnMut, FnOnce}` as stdlib deps but does not say the compiler
auto-impls them for closure types.

**Alignment verdict**: ⚠️ **PARTIAL §13.4**. The whitelist requires closures callable for
Stage 1 — Stage 13.3 closes the B1 deviation. The Fn/FnMut/FnOnce auto-impl requirement is
implicit but not detailed; Stage 13.3 implements the **call lowering** (closures become
callable) but **defers the trait auto-impl** to Stage 13.5+ (Option B). The §25.8 write-back
should update §2.5's remark from "Fn/FnMut/FnOnce 自动推导" to "call lowering: Stage 13.3
(v0.23.0); Fn/FnMut/FnOnce auto-impl: Stage 13.5+".

---

## 3. Current Implementation Analysis

### 3.1 AST closure representation — `src/ast/kinds.rs`

**Verified by direct read of `src/ast/kinds.rs:509-514`** (the `Closure` variant in `Expr`):

```rust
Closure {
    is_move: bool,
    params: Vec<Param>,
    body: Box<Expr>,
    // span: Span (next field)
},
```

**Fields**: `is_move: bool` (move closure flag, parsed but unused per Stage 4.4 deferral),
`params: Vec<Param>` (closure parameters), `body: Box<Expr>` (closure body), `span: Span`
(inherited from `Expr`).

**Alignment with design**: ✅ matches `02-grammar.md` §3.5 closure production
`"|" param_list? "|" expr`. The `is_move` flag is parsed but not used (move closure deferred
to v0.2 per `13-stage1-feature-whitelist.md` §2.5 line 129). Stage 13.3 does NOT need to
modify AST.

### 3.2 HIR closure representation — `src/hir/kinds.rs`

**Verified by direct read of `src/hir/kinds.rs:756-760`**:

```rust
Closure {
    is_move: bool,
    params: Vec<HirParam>,
    body: Box<HirExpr>,
},
```

**Fields**: same shape as AST (`is_move`, `params`, `body`). No `captures` field — captures
are computed at MIR lowering time via `closure_capture::collect_captured_locals`.

**Alignment with design**: ✅ matches `05-ast.md` §8 HIR lowering expectations. Stage 13.3
does NOT need to modify HIR.

### 3.3 MIR closure representation — `src/mir/ty.rs` + `src/mir/place.rs`

**Verified by direct read of `src/mir/ty.rs:50-51`** (`TyKind::Closure`):

```rust
/// Closure type
Closure(DefId, SubstsRef),
```

Where `SubstsRef = Vec<Ty>` (verified by `expr_operand.rs:572` usage). The `DefId` is the
owning function's def_id (per `expr_operand.rs:914-917` — currently `cx.hir.map(...).owners.first()`,
which is the **crate root** for top-level closures, not a unique per-closure def_id — **this
is a known limitation**: all closures in a crate currently share the same DefId).

**Verified by direct read of `src/mir/place.rs:181`** (`AggregateKind::Closure`):

```rust
/// `Foo(a, b)` — closure
Closure(crate::hir::DefId, SubstsRef),
```

Same shape — `DefId` + `SubstsRef` (capture field types).

**Critical finding**: The `DefId` field in `TyKind::Closure` and `AggregateKind::Closure` is
**not currently unique per closure** — it's set to the owning function's def_id (or crate
root for top-level closures) at `expr_operand.rs:914-917`. For Stage 13.3 Strategy A, we
need **unique per-closure DefIds** so each closure can have its own synthesized `call`
function. This requires:
- Either allocate fresh DefIds at closure-lowering time (new `MirLowerCtxt` method).
- Or use a `(owner_def_id, closure_index)` tuple as the unique key.

This is a non-trivial infrastructure change — DefId allocation is currently HIR-side
(`hir/lower/cx.rs::fresh_def_id`), and MIR lowering would need to either request new DefIds
from HIR (reverse-direction — §16 violation) or carry a per-body closure counter that gets
baked into the DefId.

**Recommended approach**: Per-body closure counter; the synthesized `call` function's DefId
= `(owner_def_id, closure_index)` encoded as `DefId(owner_def_id.0 * 1000000 + closure_index)`
or similar. The encoding is internal to MIR lowering + codegen; no HIR changes needed.

### 3.4 MIR lowering of closures — `src/mir/lower/expr_operand.rs` + `closure_capture.rs`

#### 3.4.1 `HirExprKind::Closure` arm (lines 879-935) — partial

**Verified by direct read of `src/mir/lower/expr_operand.rs:879-935`**:

```rust
HirExprKind::Closure { params, body, .. } => {
    // 1. Register closure params as locals + collect their hir_ids
    let mut param_hir_ids: HashSet<HirId> = HashSet::new();
    for param in params {
        let ty = cx.fresh_infer_ty(param.pat.span);
        cx.new_local(param.pat.hir_id, ty, None);
        // ... param_hir_ids.insert(...)
    }

    // 2. Stage 4.7: Collect captured locals
    let mut captured: Vec<(HirId, LocalId)> = Vec::new();
    let mut seen: HashSet<HirId> = HashSet::new();
    closure_capture::collect_captured_locals(cx, body, &param_hir_ids, &mut captured, &mut seen);

    // 3. Lower closure body — RESULT IS DISCARDED
    let _body_local = lower_expr_to_operand(cx, body);

    // 4. Build capture field types + operands
    let mut capture_tys: Vec<Ty> = Vec::new();
    let mut capture_operands: Vec<Operand> = Vec::new();
    for (_, local_id) in &captured {
        let ty = cx.mir.local(*local_id).ty.clone();
        capture_tys.push(ty);
        capture_operands.push(Operand::Copy(Place::local(*local_id, expr.span)));
    }

    // 5. Create closure value with captures
    let closure_def_id = cx.hir
        .map(|h| h.owners.first().map(|(id, _)| *id).unwrap_or_default())
        .unwrap_or_default();
    let closure_ty = Ty::new(TyKind::Closure(closure_def_id, capture_tys), expr.span);
    let closure_local = cx.mir.new_local(closure_ty, None, expr.span);
    cx.mir.block_mut(cx.current_block).statements.push(Statement {
        kind: StatementKind::Assign(Box::new((
            Place::local(closure_local, expr.span),
            Rvalue::Aggregate(
                AggregateKind::Closure(closure_def_id, vec![]),
                capture_operands,
            ),
        ))),
        span: expr.span,
    });
    closure_local
}
```

**What works**:
- Param registration (step 1) — ✅
- Capture analysis (step 2) — ✅ uses `closure_capture::collect_captured_locals`
- Closure struct value construction (step 4-5) — ✅ produces `AggregateKind::Closure` with
  capture operands

**What's broken**:
- Step 3: `let _body_local = lower_expr_to_operand(cx, body);` — **the closure body is
  lowered INLINE into the enclosing function's MIR**. The body's locals, statements, and
  basic blocks pollute the enclosing MIR. The body's result is discarded (`_body_local`).
  This means a closure body with side effects (e.g., `|x| { println!(...); x + 1 }`) would
  execute the side effects at closure CONSTRUCTION time, not at call time. **This is a
  soundness bug** (currently masked because closures can't be called — the body's effects
  happen at construction time, but the result is unused).
- Step 5: `closure_def_id` is the **owning function's def_id**, not a unique per-closure
  def_id (see §3.3 above).
- Step 5: `AggregateKind::Closure(closure_def_id, vec![])` — the second field (substs) is
  **empty `vec![]`**, not the `capture_tys`. This is inconsistent with `TyKind::Closure(def_id, capture_tys)`
  which DOES carry capture_tys. The codegen of `AggregateKind::Closure` (per §3.6 below)
  falls through to placeholder anyway, so this inconsistency is currently masked.

#### 3.4.2 `HirExprKind::Call` arm with closure callee (lines 527-589) — DEFERRED

**Verified by direct read of `src/mir/lower/expr_operand.rs:527-589`**:

```rust
} else {
    // Stage 4.9: Check if func is a closure type.
    let is_closure = {
        let func_local_decl = cx.mir.local_decls.get(func_local.0 as usize);
        func_local_decl
            .map(|ld| matches!(&ld.ty.kind, TyKind::Closure(_, _)))
            .unwrap_or(false)
    };

    if is_closure {
        // Stage 4.13: Full closure call lowering — inline approach.
        // [lengthy comment explaining the intended inline approach]
        // However, we don't have access to the HIR closure definition
        // from here (we only have the func operand's type). So we
        // use a pragmatic approach: extract captures from the closure
        // struct, produce a fresh infer type for the result, and
        // lower the call arguments. The actual body inlining requires
        // HIR access which would need restructuring the lowering
        // pipeline (deferred to Stage 5).

        // Get the closure type's capture field types
        let closure_ty = &cx.mir.local(func_local).ty;
        let capture_tys: Vec<Ty> = match &closure_ty.kind {
            TyKind::Closure(_, substs) => substs.clone(),
            _ => vec![],
        };

        // Extract each captured field from the closure struct
        for cap_ty in &capture_tys {
            let field_ty = cap_ty.clone();
            let _extracted_local = cx.mir.new_local(field_ty, None, expr.span);
            // In a full implementation, we'd assign:
            // extracted_local = Copy(Projection(closure_local, Field(i, cap_ty)))
            // But since we can't map back to the original HirId here,
            // we skip the binding.
        }

        // Produce a result local with inferred type
        let dest_ty = cx.fresh_infer_ty(expr.span);
        cx.mir.new_local(dest_ty, None, expr.span)
    } else {
        // Real function call.
        let dest_ty = cx.fresh_infer_ty(Span::DUMMY);
        let dest = cx.mir.new_local(dest_ty, None, expr.span);
        let cont = cx.new_block();
        cx.terminate_and_goto(
            Terminator::Call {
                func: Operand::Copy(Place::local(func_local, func.span)),
                args: arg_operands,
                destination: Place::local(dest, expr.span),
                target: Some(cont),
            },
            cont,
        );
        dest
    }
}
```

**What's broken**:
- The closure-call arm creates fresh locals for capture field types (line 577-585) but
  **does not bind them** (comment "we skip the binding" at line 583-584).
- The arm produces a result local with **inferred type** (line 588-589) but **does not
  emit a `Terminator::Call`** — the call args are lowered but never used.
- The intended "inline approach" (described in the comment at lines 542-563) was never
  completed because the lowering context lacks access to the HIR closure definition
  (only the closure's TYPE is visible at the Call site).
- The stale comment at line 876 ("Closure call lowering: closure calls still go through
  regular Call") is incorrect — the code at 527-589 does NOT go through regular Call;
  it produces a placeholder result.

**Root cause**: The inline approach (Strategy B from the task description) requires HIR
access at the Call site, but `MirLowerCtxt` does not carry enough HIR context to re-lower
the closure body inline. The Stage 4.13 comment explicitly says "deferred to Stage 5" —
Stage 13.3 IS that deferral closure.

**Stage 13.3 fix (Strategy A)**: Replace the inline approach with synthesized `call` function.
The `HirExprKind::Closure` arm (§3.4.1) is rewritten to:
1. Allocate a fresh DefId for the closure (per-body counter).
2. Lower the closure body to a **separate MirBody** (the synthesized `call` function), with:
   - First param = `&closure_struct` (a Place::Projection-able local).
   - Subsequent params = the closure's declared params.
   - Body = the closure's expression with captures dereferenced from `self.field_i`.
3. Register the synthesized MirBody in a per-crate `closure_call_bodies: Vec<MirBody>` table.
4. Construct the closure value via `Aggregate(AggregateKind::Closure(def_id, capture_tys), capture_operands)`.

The `HirExprKind::Call` arm with closure callee is rewritten to emit:
```rust
Terminator::Call {
    func: Operand::Constant(Const { ty: FnDef(closure_def_id, substs), val: Uint(closure_def_id.0) }),
    args: [{closure_struct_operand}, ...arg_operands],  // closure struct as first arg
    destination: result_place,
    target: continuation_block,
}
```

This is exactly what the design `07-codegen.md` §8.2 prescribes.

### 3.5 Codegen of closures — `src/codegen/mod.rs` + `src/codegen/mir_translation.rs` + `src/codegen/emitter.rs`

#### 3.5.1 `TyKind::Closure` → EmitType (✅ implemented)

**Verified by direct read of `src/codegen/emitter.rs:485-490`**:

```rust
// Stage 4.7 (L3): Closure type — emit as a struct with captured fields.
// The substs vector carries the capture field types.
TyKind::Closure(_, substs) => {
    let fields: Vec<EmitType> = substs.iter().map(mir_type_to_emit_type).collect();
    EmitType::Struct(fields)
}
```

This matches `07-codegen.md` §8.1 design intent (anonymous struct with capture fields). ✅

#### 3.5.2 `Rvalue::Aggregate(AggregateKind::Closure, operands)` → LLVM IR (❌ placeholder)

**Verified by direct read of `src/codegen/mod.rs:491-630`**:

The `Aggregate` arms handle `Tuple`, `Array`, `Adt` — but **`AggregateKind::Closure` falls
through to `_ => "0".to_string()`** at line 630. The closure struct value construction
produces the literal string `"0"` in LLVM IR — garbage.

**Stage 13.3 fix**: Add an `AggregateKind::Closure(def_id, substs)` arm that constructs the
closure struct via `emit_insertvalue` per capture field (mirroring the `AggregateKind::Adt`
struct-construction path at lines 603-622).

#### 3.5.3 `Terminator::Call` with closure callee (❌ no path)

**Verified by direct read of `src/codegen/mod.rs:844-958`**:

The `Terminator::Call` arm has three paths:
1. Dyn Trait marker (Const Error + Int index) → `codegen_dyn_trait_call` (lines 863-888).
2. `TyKind::FnDef(def_id, _)` callee → look up `fn_name_by_def_id[def_id]`, emit direct call (lines 890-917).
3. `Operand::Constant(ConstVal::Uint/Int(n))` → look up `fn_name_by_def_id[DefId(n)]` (lines 905-914).

If none match, `fn_name = None` and `ret_val = "0".to_string()` (line 941) — placeholder.

**For closure calls** (Strategy A): The synthesized `call` function has a `DefId`. If the
MIR `Terminator::Call`'s `func` operand is `Operand::Constant(Const { ty: FnDef(closure_def_id, _), val: Uint(closure_def_id.0) })`,
the existing path 2 (line 894) would resolve it correctly **IF** `fn_name_by_def_id` contains
the synthesized function's name. Stage 13.3 needs to:
1. Populate `fn_name_by_def_id` with synthesized closure call function names (e.g.,
   `closure_call_<owner>_<index>`).
2. Emit the synthesized closure call function bodies in codegen (new `codegen_closure_call_fns`
   pass that iterates the per-crate `closure_call_bodies` table and emits one LLVM function
   per entry).

The first arg of the call is the closure struct pointer (`%Closure_type* %closure`); subsequent
args are the closure's declared params. This matches `07-codegen.md` §8.2 exactly.

#### 3.5.4 §16 compliance check

The synthesized `call` function MirBody must be **carried as data** from MIR lower to codegen
(per §16 single-direction data flow). The recommended approach:
- MIR lower produces a per-crate `closure_call_bodies: Vec<MirBody>` side-table (mirroring
  the existing `dyn_trait_calls` side-table pattern at `mir/body.rs`).
- Driver passes this table to codegen alongside the main per-fn MirBody table.
- Codegen iterates the table and emits one LLVM function per entry.
- No HIR access from codegen (§16 compliant).

### 3.6 Type checker — `src/typeck/checker.rs` (❌ rejects closure calls)

**Verified by direct read of `src/typeck/checker.rs:433-565`**:

The `Terminator::Call` arm has three checks:
1. Line 433-441 (G7 fix): If `func_ty` is neither `FnDef` nor `FnPtr` (after defaulting),
   emit a type error. **`TyKind::Closure` is NOT in the accepted list** — closure calls
   emit "calling a non-function" type errors.
2. Line 510-540: FnDef call handling — verifies arg count, arg types, return type using
   `fn_sigs` table.
3. Line 543-555: FnPtr call handling — unifies args with sig inputs.

**For closure calls** (Strategy A): The synthesized `call` function has a `DefId`. If the
MIR `Terminator::Call`'s `func` operand is typed `TyKind::FnDef(closure_call_def_id, _)`,
the existing path 2 (line 510-540) would handle it **IF**:
- The `fn_sigs` table contains the synthesized function's signature.
- The arg count check includes the implicit `&self` first arg.

Stage 13.3 needs to:
1. Extend the G7 check (line 433-441) to also accept `TyKind::FnDef` whose DefId resolves
   to a synthesized closure call function. (The func operand's type IS `FnDef` after Strategy
   A lowering — the G7 check already accepts `FnDef`. So this may be a no-op if the lowering
   produces `FnDef` typed func operands.)
2. Populate `fn_sigs` table with synthesized closure call function signatures.
3. The arg-count check at line 519 must expect `closure.params.len() + 1` args (the +1 is
   the implicit `&self` closure struct).

The `Rvalue::Aggregate` arm at line 800-848 has `_ => Ty::new(TyKind::Error, Span::DUMMY)`
(line 847) — `AggregateKind::Closure` falls through to Error type. Stage 13.3 needs to add
an `AggregateKind::Closure(def_id, substs)` arm that returns `Ty::new(TyKind::Closure(*def_id, substs.clone()), ...)`.

### 3.7 Conformance FAIL test analysis

**Methodology note**: Per r217 §2.4, the r216 claim of "41 FAIL tests" was a methodology
error conflating `//! FAIL` markers (Stage 0 limitations, broken tests) with
`// EXPECTED: compile_error` tests (intended-failure tests, working as designed). The r217
correction: **0 `//! FAIL` markers** in the 3 cited closure dirs; **40 `// EXPECTED: compile_error`
closure-related tests** across the conformance tree.

**Verified by Bash**:

```
=== Closure-related compile_error tests across conformance ===
40 total
  20 tests/conformance/02-borrowck/03-closure-capture
  11 tests/conformance/01-typecheck/03-closures
   3 tests/conformance/04-e2e/03-closures
   3 tests/conformance/02-borrowck/02-move-semantics
   2 tests/conformance/02-borrowck/01-nll-advanced
   1 tests/conformance/06-stdlib/02-std
```

**Total in 3 main closure dirs**: 34 (20+11+3) — these are the directly-attributable TD-030
tests. The remaining 6 (in `02-move-semantics`, `01-nll-advanced`, `06-stdlib/02-std`) are
closure-related but in adjacent categories.

**Sample of 5 .lin files** (verified by direct read):

| # | File | Source | Pattern |
|---|------|--------|---------|
| 1 | `01-typecheck/03-closures/006-closure-call.lin` | `fn main() { let x = (|y| y)(42); }` | immediate closure call |
| 2 | `01-typecheck/03-closures/013-closure-in-let.lin` | `fn main() { let g = |x: i32| x + 1; let r = g(5); }` | closure assigned to let, then called |
| 3 | `04-e2e/03-closures/001-closure-basic.lin` | `fn main() { let f = |x| x + 1; let y = f(5); }` | basic closure call (e2e) |
| 4 | `04-e2e/03-closures/010-clos-closure-called-immediately.lin` | `fn main(){let x=(|y:i32|y*2)(5);}` | immediately-invoked closure expression |
| 5 | `01-typecheck/03-closures/clos-0429-0-closure-nested.lin` | `fn main(){let f=|||y|x+y;}` | nested closure |

All 5 follow the same pattern: **closure parses + captures but cannot be called** — exactly
the TD-030 deferral. The `// EXPECTED: compile_error` + `// ERROR_PATTERN: error` markers
indicate these tests currently PASS because the compiler errors out. Stage 13.3 must flip
them to `// EXPECTED: compile_ok` and remove the `// ERROR_PATTERN:` line.

**Test flip mechanics for Stage 13.3**:
1. For each of the 40 (or 34 in main dirs) compile_error closure tests:
   - Change `// EXPECTED: compile_error` → `// EXPECTED: compile_ok`.
   - Remove the `// ERROR_PATTERN: error` line.
   - Update `// SOURCE:` line to append "(Stage 13.3 closure call lowering)".
2. The conformance runner (per `tests/v0/stage10/plan/stage10_0_tests.rs:74`) supports both
   `compile_ok` and `compile_error` — no runner change needed.
3. Stage 9.3 / Stage 10.x / Stage 11.x unit tests that assert these tests are compile_error
   must be updated (similar to Stage 13.2's `control_flow_tests.rs` update). Audit needed:
   `rg "compile_error" tests/v0/stage*/plan/` to find coupled unit tests.

---

## 4. Implementation Strategy (per §15 long-term > short-term)

### 4.1 Strategy comparison

| Strategy | Description | Files | LOC delta | Risk | Long-term value |
|----------|-------------|------:|----------:|------|-----------------|
| **A** | **Direct call function synthesis (rustc-style)**. Each closure literal synthesizes a companion `call` function MirBody. The closure value is a struct of captures. `HirExprKind::Call` with closure callee emits `Terminator::Call` to the synthesized `call` function's DefId, with the closure struct as first arg + actual args. | 9 src files (mir/lower/expr_operand.rs, mir/lower/closure_capture.rs, mir/lower/mod.rs, mir/body.rs, typeck/checker.rs, codegen/mod.rs, codegen/mir_translation.rs, codegen/emitter.rs, driver.rs) + 1 new test file + 40 conformance .lin + 4 design-doc write-back = 54 files | ~600-1000 LOC new | **HIGH** | ✅ **Highest** — matches `07-codegen.md` §8.1-8.2 design prescription exactly; rustc-idiomatic; supports closures-as-values (passed to functions, returned, stored in structs); enables Fn/FnMut/FnOnce auto-impl later (Stage 13.5+) by adding trait impls that delegate to the synthesized `call` function |
| **B** | **Inline closure body at call site**. `HirExprKind::Call` with closure callee inlines the closure body at the call site. No synthesized `call` function; no function pointer. | 5 src files (mir/lower/expr_operand.rs, mir/lower/closure_capture.rs, typeck/checker.rs, codegen/mod.rs, codegen/mir_translation.rs) + 1 test + 40 conformance + 4 design = 50 files | ~300-500 LOC | **MEDIUM** | ⚠️ Limited — doesn't support closures passed as arguments (`fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 { f(x) }` fails because `f` has no callable representation); limits v0.3 self-hosting (Stage 1 source code uses higher-order functions pervasively) |
| **C** | **Function pointer field in closure struct**. Closure struct has a function pointer field pointing to a globally-defined `call` function. `HirExprKind::Call` with closure callee loads function pointer + indirect call. | 7 src files + 1 test + 40 conformance + 4 design = 52 files | ~400-600 LOC | **MEDIUM** | ⚠️ Intermediate — supports closures-as-values but requires indirect call (LLVM optimizer can devirtualize but not guaranteed); still needs synthesized `call` function per closure; function pointer field adds 8 bytes per closure struct; doesn't match `07-codegen.md` §8.2 (which shows direct call, not indirect) |

### 4.2 §15 long-term > short-term analysis

Per `stage-committee-process.md` §15 ("最优 > 最小" — best > smallest), the long-term value
criterion dominates the short-term cost criterion when the two conflict.

**Long-term value**:

- **Strategy A** is **exactly what `07-codegen.md` §8.1-8.2 prescribes** — direct call to
  synthesized `call` function. The design has been on the books since Stage 0; the
  implementation simply never grew the call dispatch. This is the same situation as Stage
  13.2 (where `05-ast.md` §12.4 prescribed the if-let/while-let desugar strategy). The
  design pre-sanctions the approach.
- **Strategy A** supports closures-as-values: closures can be passed to functions
  (`fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 { f(x) }`), returned from functions
  (`fn make_adder(n: i32) -> impl Fn(i32) -> i32 { move |x| x + n }`), stored in structs.
  This is **critical for v0.3 self-hosting** — Stage 1 source code (per
  `13-stage1-feature-whitelist.md` §3.1 line 226) uses `core::ops::{Fn, FnMut, FnOnce}`
  pervasively.
- **Strategy B** (inline) is simpler but **does not support closures-as-values**. A closure
  passed to a function would have no callable representation at the call site (the callee
  only sees a `TyKind::Closure` value, not the body). This breaks `Iterator::map/filter/fold`
  — the most pervasive Stage 1 stdlib pattern.
- **Strategy C** (function pointer) supports closures-as-values but adds an indirection.
  The design `07-codegen.md` §8.2 shows **direct call** (`call i32 @"<closure_type>::call"(...)`),
  not indirect call (`call i32 %fn_ptr(...)`). Strategy C deviates from the design.
- **Strategy A** enables Fn/FnMut/FnOnce auto-impl in Stage 13.5+ by adding trait impls
  that delegate to the synthesized `call` function. Strategy B does not (no callable
  representation; trait impl would need to inline the body, which is impossible for
  type-erased `dyn Fn` receivers).

**Short-term cost**:

- Strategy A: ~600-1000 LOC across 9 src files. Risk concentrated in:
  - Synthesized MirBody construction (new `lower_closure_to_call_fn` in `mir/lower/mod.rs`,
    ~200-300 LOC).
  - Per-crate `closure_call_bodies` side-table plumbing (driver + mir/body.rs + codegen,
    ~100-150 LOC).
  - Codegen of synthesized call functions (new pass in codegen/mod.rs, ~150-200 LOC).
  - Typeck changes to accept closure calls (~50-100 LOC).
  - Conformance test flips (~40 files, mechanical).
- Strategy B: ~300-500 LOC across 5 src files. Risk concentrated in inline-body correctness.
- Strategy C: ~400-600 LOC across 7 src files. Risk concentrated in function pointer
  indirection + lifetime of the function pointer global.

**§15 verdict**: Strategy A has the highest long-term value (design-aligned, supports
closures-as-values, enables Fn/FnMut/FnOnce auto-impl later) at the highest short-term cost
(9 src files, ~600-1000 LOC). Per §15.1 "best > smallest", the long-term value dominates:
v0.3 self-hosting requires closures-as-values (Iterator combinators are pervasive in Stage 1
source); Strategy B would close TD-030 nominally but leave a critical gap (closures passed
to functions still fail) — requiring a Stage 13.3b rework to add the synthesized call
function. **Strategy A is the rustc-idiomatic, design-aligned, one-shot path**.

### 4.3 rustc reference

Per the rustc dev guide
(https://rustc-dev-guide.rust-lang.org/mir/closures.html),
rustc lowers each closure to:
1. A **closure struct** (`{captures...}`) — anonymous ADT.
2. A **call function** (`fn closure_def_id_call(&self, args...) -> ret`) — separate MIR body.
3. `Fn`/`FnMut`/`FnOnce` trait auto-impls based on capture mode (by-ref → Fn; by-mut-ref →
   FnMut; by-value → FnOnce).

The call site `f(args)` desugars to `Fn::call(&f, args)` (or `FnMut::call_mut(&mut f, args)`
/ `FnOnce::call_once(f, args)` depending on kind). The trait method delegates to the
synthesized `call` function.

**Strategy A is rustc-idiomatic**, with one simplification for Stage 13.3: **defer the
Fn/FnMut/FnOnce trait auto-impl** (Option B, see §5) — call sites dispatch directly to the
synthesized `call` function, bypassing the trait layer. This is a Stage 13.3 simplification;
Stage 13.5+ adds the trait layer for `impl Fn(...)` bound support.

### 4.4 §14.4 J1-J6 evaluation (Strategy A + Option B)

| # | Criterion | Verdict | Justification |
|---|-----------|---------|---------------|
| J1 | Architecture alignment | ✅ PASS | Implements `07-codegen.md` §8.1-8.2 prescribed direct-call-function-synthesis. `06-mir.md` §5 already has `AggregateKind::Closure`; `TyKind::Closure` is B4 gray-area write-back. `13-stage1-feature-whitelist.md` §2.5 requires closures callable. |
| J2 | Single responsibility | ✅ PASS | `mir/lower/expr_operand.rs` adds closure-call dispatch arm; `mir/lower/mod.rs` adds `lower_closure_to_call_fn` helper (new responsibility: synthesize closure call MirBody); `mir/body.rs` adds `closure_call_bodies` side-table (data carrier, mirroring existing `dyn_trait_calls`); `codegen/mod.rs` adds closure-call codegen path + closure-call-fn emission pass; `typeck/checker.rs` adds closure-call acceptance. Each file gains one cohesive responsibility — no file gains mixed concerns. |
| J3 | Single-direction flow | ✅ PASS | No new module dependencies. Synthesized MirBody flows: MIR lower → `MirBody.closure_call_bodies` side-table → driver → codegen. No HIR access from codegen (§16 compliant — mirrors existing `dyn_trait_calls` side-table pattern). |
| J4 | Compilation expression complete | ✅ PASS | Closures become first-class callable values: can be passed to functions, returned, stored in structs. Matches `13-stage1-feature-whitelist.md` §3.1 stdlib deps (`core::ops::{Fn, FnMut, FnOnce}`). |
| J5 | Stage division clear | ⚠️ MARGINAL | 9 src files modified (slightly above the ≤5 file §14.4 J5 guideline). Justification: closure call lowering is a cross-cutting feature (MIR + typeck + codegen + driver); r216 estimated ≤5 files but the actual scope (per §3 analysis) requires 9. Per §15, the long-term value (closures-as-values) justifies the file count. |
| J6 | Scientific granularity | ✅ PASS | Total LOC delta ~600-1000. Largest single-file delta: `mir/lower/mod.rs` (+200-300 LOC for `lower_closure_to_call_fn`); `codegen/mod.rs` (+150-200 LOC for closure-call codegen + emission pass); `expr_operand.rs` (+50-100 LOC for closure-call dispatch rewrite). All files stay below 1500 LOC ceiling. |

**Strategy A + Option B §14.4 verdict**: ✅ 5/6 criteria PASS, J5 MARGINAL (file count
justified by §15 long-term value). Strategy is cleared for execution with committee
approval.

---

## 5. Fn/FnMut/FnOnce Auto-impl Strategy

### 5.1 Three options

| Option | Description | Files | LOC delta | Risk | Long-term value |
|--------|-------------|------:|----------:|------|-----------------|
| **A** | Implement closure call lowering + Fn/FnMut/FnOnce auto-impl together (full feature). Closures get auto-impl'd Fn/FnMut/FnOnce based on capture mode. Call sites dispatch via trait method (`Fn::call(&f, args)`). | 11 src files (Strategy A's 9 + `traits/builtin.rs` + `traits/resolver.rs`) + 1 test + 40 conformance + 5 design = 57 files | ~900-1300 LOC | **VERY HIGH** | ✅ Full feature — `impl Fn(...)` bounds work immediately; but requires capture-mode inference (by-ref / by-mut-ref / by-value) which is currently unimplemented and undocumented in `04-ownership-borrowing.md` |
| **B** | **Implement closure call lowering only (direct call, no trait auto-impl)**. Closures are callable via direct dispatch to synthesized `call` function. Defer Fn/FnMut/FnOnce auto-impl to Stage 13.5+. | 9 src files (Strategy A's 9) + 1 test + 40 conformance + 4 design = 54 files | ~600-1000 LOC | **HIGH** | ✅ Closures callable — unblocks v0.3 self-hosting for the most pervasive pattern (closures defined + called locally). `impl Fn(...)` bounds deferred — Stage 1 source can use closures directly but not yet pass them as `impl Fn(...)` trait bounds. |
| **C** | Implement closure call lowering + minimal Fn auto-impl (Fn only, defer FnMut/FnOnce). Closures auto-impl `Fn` (by-ref capture only); FnMut/FnOnce deferred. | 10 src files (Strategy A's 9 + `traits/builtin.rs`) + 1 test + 40 conformance + 4 design = 55 files | ~700-1100 LOC | **HIGH** | ⚠️ Partial — `impl Fn(...)` bounds work for by-ref closures; but `FnMut`/`FnOnce` closures (which need `&mut self` / `self` receivers) still fail. Forces all closures to by-ref capture mode — incorrect for `move` closures (which need FnOnce). |

### 5.2 Recommendation: Option B

**Rationale**:

1. **v0.3 self-hosting needs closures callable, not necessarily `impl Fn(...)`-bound**. The
   most pervasive Stage 1 pattern is `let f = |x| x + 1; let y = f(5);` — direct call. This
   works with Option B. The `impl Fn(...)` bound pattern (`fn apply(f: impl Fn(i32) -> i32, ...)`)
   is less pervasive and can be deferred.

2. **Capture-mode inference is undocumented and unimplemented**. `04-ownership-borrowing.md`
   §8 implies by-ref is the default (RFC 2229 disjoint captures) but does not specify the
   by-ref / by-mut-ref / by-value taxonomy. The implementation at `closure_capture.rs:15-156`
   collects all external locals uniformly — no mode distinction. Implementing Fn/FnMut/FnOnce
   auto-impl (Option A or C) requires:
   - Capture-mode inference algorithm (walk body, classify each capture as by-ref / by-mut-ref / by-value).
   - Closure kind selection (Fn if all by-ref; FnMut if any by-mut-ref; FnOnce if any by-value).
   - Trait auto-impl emission (3 trait impls per closure, with appropriate receiver kind).
   - Trait solver integration (resolve `F: Fn(i32) -> i32` bound to the closure's auto-impl).
   
   This is substantial work (estimated +300-500 LOC over Option B) and requires design-doc
   write-back for capture-mode inference (B4 gray-area). Deferring to Stage 13.5+ allows
   Stage 13.3 to ship the callable-closure feature faster.

3. **Option B matches `07-codegen.md` §8.2 design intent**. The codegen example shows
   `call i32 @"<closure_type>::call"(%Closure_type* %closure, i32 42)` — direct call to
   the synthesized `call` function. The `impl<'a> Fn<(i32,)> for Closure<'a>` in §8.1 is
   illustrative (showing how the `call` function maps to the `Fn` trait method signature),
   not prescriptive (the trait impl is not required for the call to work). Stage 13.3
   implements the direct call; Stage 13.5+ adds the trait impl layer.

4. **Option C is incorrect for `move` closures**. `move` closures capture by value, which
   requires `FnOnce` (self by value). Option C (Fn only) would force all closures to by-ref
   capture, breaking `move` closures. Since `move` closures are deferred to v0.2 per
   `13-stage1-feature-whitelist.md` §2.5 line 129, this is not immediately blocking — but
   Option C creates a false constraint that would need reworking when `move` closures are
   added. Option B (no trait auto-impl) has no such constraint.

5. **Option B is consistent with Stage 13.2's incremental approach**. Stage 13.2 closed
   TD-031 (if-let/while-let) with the minimum design-aligned strategy (Strategy B desugar
   to Match) — no extra features. Stage 13.3 closes TD-030 (closure call lowering) with
   the minimum design-aligned strategy (Strategy A direct call + Option B no trait auto-impl).
   Both stages ship the user-facing feature (closures callable / if-let parseable) without
   over-engineering.

### 5.3 Stage 13.5+ Fn/FnMut/FnOnce auto-impl scope (deferred)

When Stage 13.5+ implements Fn/FnMut/FnOnce auto-impl (Option A), the work will be:
1. Capture-mode inference in `closure_capture.rs` (~100-150 LOC) — classify each capture
   as by-ref / by-mut-ref / by-value.
2. Closure kind selection in `mir/lower/mod.rs` (~50 LOC) — Fn / FnMut / FnOnce.
3. Trait auto-impl emission in `traits/builtin.rs` + `traits/resolver.rs` (~150-200 LOC) —
   synthesize `impl Fn for Closure { fn call(&self, args) -> ret { closure_call_fn(self, args) } }`.
4. Trait solver integration in `typeck/checker.rs` (~50-100 LOC) — resolve `F: Fn(...)` bound
   to the closure's auto-impl.
5. Capture-mode design write-back to `04-ownership-borrowing.md` §11.7 (new sub-section).

Total: ~400-600 LOC, 4-5 src files. Stage 13.5+ can sequence this as MUV-12 or MUV-13
(per `plan-13.1.md` §2 Stage 13.5).

---

## 6. Scope Analysis

### 6.1 File change list (Strategy A + Option B)

**Total files modified: 54** (9 src + 1 new stage13_3 test + 40 conformance .lin + 4 design-doc
write-back).

| # | File | Change | LOC delta | Risk |
|---|------|--------|-----------|------|
| 1 | `src/mir/lower/expr_operand.rs` (1279 LOC) | **Rewrite** `HirExprKind::Closure` arm (lines 879-935): allocate fresh per-closure DefId; lower closure body to separate MirBody via new `lower_closure_to_call_fn` helper; register MirBody in `cx.closure_call_bodies`; construct closure struct value via `Aggregate(AggregateKind::Closure(def_id, capture_tys), capture_operands)` (fix the `vec![]` bug at line 928). **Rewrite** `HirExprKind::Call` closure-callee arm (lines 527-589): emit `Terminator::Call` with `func = Operand::Constant(Const { ty: FnDef(closure_def_id, substs), val: Uint(closure_def_id.0) })`, `args = [closure_struct_operand, ...arg_operands]` (closure struct as first arg + actual args), `destination = result_place`. Remove the placeholder result-with-inferred-type path. | +100 / -60 LOC | HIGH (core dispatch change; typeck + codegen must agree on the calling convention) |
| 2 | `src/mir/lower/closure_capture.rs` (182 LOC) | **Extend** `collect_captured_locals` to also return capture modes (by-ref / by-mut-ref / by-value) per capture — currently returns `Vec<(HirId, LocalId)>`; extend to `Vec<(HirId, LocalId, CaptureMode)>`. For Stage 13.3 Option B, default all captures to `CaptureMode::ByRef` (matching current `Operand::Copy` behavior for Copy types; for non-Copy types, use `Operand::Move` — this is a Stage 13.5+ refinement when capture-mode inference is properly designed). | +30 LOC | MEDIUM (capture mode is a new concept; default-by-ref is safe but may need refinement for non-Copy captures) |
| 3 | `src/mir/lower/mod.rs` (772 LOC) | **Add** `lower_closure_to_call_fn(cx, params, body, captures, closure_def_id) -> MirBody` function: synthesizes a MirBody for the closure's `call` function. First param = `&closure_struct` (a local of type `Ref(_, _, closure_ty)`); subsequent params = the closure's declared params. Body = re-lower the closure body with captures dereferenced from `self.field_i` via `Place::Projection(self_local, Field(i, capture_ty))`. Return type = inferred from body. | +250 LOC | HIGH (new MirBody synthesis; re-lowering body with capture projections is non-trivial — must rewrite `local_map` to map captured HirIds to `Projection(self, Field(i))` places instead of fresh locals) |
| 4 | `src/mir/body.rs` | **Add** `closure_call_bodies: Vec<MirBody>` field to `MirCrate` (or per-`MirBody` if closures are nested — design decision). Side-table pattern mirrors existing `dyn_trait_calls: Vec<DynTraitMethodCall>` on `MirBody`. Add accessor methods. | +30 LOC | LOW (mechanical side-table addition) |
| 5 | `src/typeck/checker.rs` (1156 LOC) | **Add** `AggregateKind::Closure(def_id, substs)` arm in `infer_rvalue` (line 800-848) returning `Ty::new(TyKind::Closure(*def_id, substs.clone()), ...)`. **Update** `Terminator::Call` G7 check (line 433-441) — `TyKind::FnDef` is already accepted; ensure the synthesized closure call fn's signature is in `fn_sigs` table (populate from `closure_call_bodies`). **Update** arg-count check (line 519) to expect `closure.params.len() + 1` args for closure call fns (the +1 is the implicit `&self` closure struct). | +80 LOC | MEDIUM (typeck must distinguish closure call fns from regular fns for arg-count check; use a `FnKind::ClosureCall` marker or check if def_id is in `closure_call_bodies`) |
| 6 | `src/codegen/mod.rs` (1070 LOC) | **Add** `Rvalue::Aggregate(AggregateKind::Closure(def_id, substs), operands)` arm (currently falls through to `"0"` at line 630) — construct closure struct via `emit_insertvalue` per capture field (mirror `AggregateKind::Adt` struct path at lines 603-622). **Update** `Terminator::Call` arm (line 844-958): the existing `TyKind::FnDef` path at line 894 will resolve the closure call fn name from `fn_name_by_def_id` — populate this map with synthesized closure call fn names. **Add** `codegen_closure_call_fns` pass: iterate `mir.closure_call_bodies`, emit one LLVM function per entry (fn name = `closure_call_<owner>_<index>`; first param = `%Closure_type* %self`; subsequent params = closure params; body = codegen of the synthesized MirBody). | +200 LOC | HIGH (new codegen pass for synthesized functions; must integrate with existing `codegen_function` flow) |
| 7 | `src/codegen/mir_translation.rs` (487 LOC) | **Update** `mir_type_to_emit_type_with_layouts` (line 50-135): add `TyKind::Closure(_, substs)` arm — currently falls through to legacy `mir_type_to_emit_type` which DOES handle Closure (at `emitter.rs:487-490`), but the `_with_layouts` variant should also handle it for nested closure types. The legacy path works but is inconsistent — promote to `_with_layouts` for uniformity. | +10 LOC | LOW (mechanical arm addition; existing legacy path already works) |
| 8 | `src/codegen/emitter.rs` (664 LOC) | **Verify** existing `TyKind::Closure` arm at line 487-490 produces correct struct type. No changes needed unless capture types include nested Adts (then need `_with_layouts` recursion — see #7). | 0 LOC | LOW (existing implementation is correct for Stage 13.3 scope) |
| 9 | `src/driver.rs` (926 LOC) | **Update** `compile` (line 284+): after MIR lowering, collect `closure_call_bodies` from each `MirBody` into a per-crate table. Pass the table to codegen alongside the main per-fn MirBody iterator. Codegen emits both the user-written functions AND the synthesized closure call functions. | +50 LOC | MEDIUM (driver is the orchestration point; careful not to break the existing `dyn_trait_calls` flow) |
| 10 | `tests/v0/stage13/plan/stage13_3_tests.rs` (NEW) | **New test file**: verify (a) AST/HIR Closure variant unchanged; (b) `expr_operand.rs` has `Terminator::Call` in closure-call arm (no placeholder); (c) `mir/body.rs` has `closure_call_bodies` field; (d) `codegen/mod.rs` has `AggregateKind::Closure` arm + closure-call-fn emission; (e) `typeck/checker.rs` accepts closure call fns; (f) 40 conformance tests flipped from `compile_error` to `compile_ok`; (g) §16 compliance — no `crate::hir` imports in `codegen/`. | +200 LOC (new) | LOW (mechanical verification tests, similar to `stage13_2_tests.rs`) |
| 11-50 | `tests/conformance/01-typecheck/03-closures/*.lin` (11 files) + `tests/conformance/02-borrowck/03-closure-capture/*.lin` (20 files) + `tests/conformance/04-e2e/03-closures/*.lin` (3 files) + 6 closure-adjacent files in `02-move-semantics`, `01-nll-advanced`, `06-stdlib` | **Flip** `// EXPECTED: compile_error` → `// EXPECTED: compile_ok`; remove `// ERROR_PATTERN: error` line; update `// SOURCE:` to append "(Stage 13.3 closure call lowering)". | ±1 LOC each | LOW (mechanical marker flip; r217-verified 40-file count) |
| 51 | (Audit) `tests/v0/stage9/plan/*.rs`, `tests/v0/stage10/plan/*.rs`, `tests/v0/stage11/plan/*.rs` | **Audit** for unit tests that assert closure tests are `compile_error` (similar to Stage 13.2's `control_flow_tests.rs` coupling). Update any coupled assertions. | ±10-30 LOC | LOW (mechanical; `rg "compile_error" tests/v0/stage*/plan/` to find sites) |
| 52 | `Cargo.toml` | **Bump** version `0.22.0` → `0.23.0` (minor bump for second user-facing feature) + append Stage 13.3 entry to `description` field. | +1 line (version) + 1 phrase | LOW |

**§25.8 design write-back files** (executed post-implementation, per Section 7 below):

| # | File | Change | Risk |
|---|------|--------|------|
| 53 | `docs/lang-design/06-mir.md` | **Add** `TyKind::Closure(DefId, SubstsRef)` to §2 or §5 type enumeration (B4 design-gray-area write-back — implementation has it, design doc doesn't). **Add** §15.3 (or §16) sub-section documenting closure call lowering algorithm: synthesized `call` function MirBody per closure; first param `&closure_struct`; captures dereferenced via `Place::Projection(self, Field(i))`; per-crate `closure_call_bodies` side-table. | LOW |
| 54 | `docs/lang-design/07-codegen.md` | **Update** §15 (Stage 8.6 §25.8 write-back) — add §15.3 sub-section noting closure codegen (§8) is now implemented in Stage 13.3 (v0.23.0). The §8 design is unchanged (already prescribes Strategy A); only the implementation status is updated. | LOW |
| 55 | `docs/lang-design/04-ownership-borrowing.md` | **Add** §11.7 sub-section documenting Stage 13.3 closure call lowering: default capture mode = by-ref; Fn/FnMut/FnOnce auto-impl deferred to Stage 13.5+; capture-mode inference algorithm deferred (B4 gray-area write-back for the staging decision). | LOW |
| 56 | `docs/lang-design/13-stage1-feature-whitelist.md` | **Update** §2.5 line 128 remark from "Fn/FnMut/FnOnce 自动推导" to "call lowering: Stage 13.3 (v0.23.0); Fn/FnMut/FnOnce auto-impl: Stage 13.5+". | LOW |

### 6.2 Risk assessment

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| Synthesized MirBody for closure `call` fn is malformed (wrong param types, missing captures, broken CFG) | HIGH | HIGH (codegen produces broken LLVM IR; or typeck rejects the call fn; or runtime crashes) | Reuse existing `MirLowerCtxt` infrastructure for body lowering; add unit test in `tests/v0/stage13/plan/stage13_3_tests.rs` that lowers a small closure and asserts the synthesized MirBody shape (param count, capture count, basic block count). Reference rustc's `closure_canonical_clause` / `query_closure_def_id` architecture. |
| Per-closure DefId allocation collides with existing HIR DefIds | MEDIUM | HIGH (codegen emits wrong function name; or typeck looks up wrong fn_sig) | Use a per-body closure counter encoded as `DefId(owner_def_id.0 + 1_000_000 + closure_index)` — high offset ensures no collision with HIR-allocated DefIds (which are typically < 10000 per crate). Add assertion in `mir/lower/mod.rs` that allocated closure DefIds are > 1_000_000. |
| Capture projection in synthesized body uses wrong field index (off-by-one) | MEDIUM | HIGH (closure reads wrong capture; silent runtime bug) | Add unit test that lowers `let a = 1; let b = 2; let f = || a + b;` and asserts the synthesized body reads `self.0` for `a` and `self.1` for `b`. |
| Typeck arg-count check fails for closure call (expects N args, gets N+1 because of implicit `&self`) | HIGH | MEDIUM (typeck emits spurious error; closure call tests fail to compile) | Either (a) populate `fn_sigs` with the full signature including `&self` as first param (existing arg-count check at `checker.rs:519` handles this naturally), or (b) add a special case for closure call fns that subtracts 1 from the expected arg count. Approach (a) is cleaner — the synthesized `call` fn's signature IS `(self_ref, params...) -> ret`, and typeck should treat it like any other fn. |
| Codegen of synthesized closure call fns emits wrong LLVM function name (mismatch with `fn_name_by_def_id` lookup) | MEDIUM | HIGH (Terminator::Call resolves to wrong name; linker error or silent wrong-call) | Centralize the name-mangling logic in one function `closure_call_fn_name(owner_def_id, closure_index) -> String` used by both codegen (fn definition) and `fn_name_by_def_id` map population (fn reference). Add unit test that asserts the name matches. |
| Existing conformance tests that currently PASS (with `compile_error` marker) now FAIL because closure compilation succeeds but produces wrong runtime behavior | MEDIUM | MEDIUM (test regression; gate review blocked) | The 40 conformance tests are `compile_error`-expected — flipping to `compile_ok` requires the compiler to ACCEPT the source (no errors). If codegen produces wrong LLVM IR but typeck accepts, the test still fails (linker error or runtime crash). Mitigation: run the 40 tests post-implementation; for any that fail, debug the codegen path. Stage 13.3 gate review blocks on all 40 passing. |
| §16 violation introduced (codegen reads HIR to get closure body) | LOW | HIGH (architectural regression; §16 violation) | Synthesized MirBody is carried as data via `closure_call_bodies` side-table on `MirCrate` (or per-`MirBody`). Codegen iterates the side-table — no HIR access. Add §16 grep check in `stage13_3_tests.rs`: `rg "crate::hir" src/codegen/` returns 0 (excluding comments). |
| Stage 9.3/10.x/11.x unit tests regress (assert closure tests are `compile_error`) | HIGH (certain) | LOW (test logic update; mechanical) | Audit `rg "compile_error" tests/v0/stage*/plan/` for closure-related assertions; update in lockstep with .lin marker flips. |
| Stage 13.1b (MUV-2 TyKind::Dynamic) not yet executed — version still at v0.22.0 (post-13.2) | LOW | LOW (Stage 13.3 proceeds from v0.22.0 baseline) | Stage 13.3 bumps v0.22.0 → v0.23.0 directly. Stage 13.1b (if executed) bumps v0.22.0 → v0.22.1 (patch); Stage 13.3 then bumps v0.22.1 → v0.23.0. Either path is consistent. |

**Overall risk**: **HIGH**. The closure call lowering is the largest single feature in
Stage 13 — larger than Stage 13.2 (if-let/while-let, LOW risk, 21 files) and comparable to
Stage 13.4 (macro_rules!, 4-8 weeks). The 9 src file count exceeds the §14.4 J5 ≤5 file
guideline; the ~600-1000 LOC delta exceeds r216's optimistic "200-400 LOC" estimate. The
5026 conformance + 2179 integration tests provide strong regression coverage; the 40
flipped conformance tests are the primary acceptance gate.

---

## 7. §25.8 Design Write-back Plan

Per `stage-committee-process.md` §25.8, after Stage 13.3 implementation, design docs must
be updated to reflect the implementation-as-fact. This section identifies which design docs
need write-back and what sections.

### 7.1 Write-back matrix

| Design doc | Section | Deviation type | Write-back action |
|------------|---------|----------------|-------------------|
| `06-mir.md` | §2 or §5 (type enumeration) | **B4 design-gray-area** (design does not document `TyKind::Closure`; implementation has it at `src/mir/ty.rs:51` since Stage 4.4) | **Required**: Add `TyKind::Closure(DefId, SubstsRef)` to the MIR type enumeration. Mark with "// Stage 13.3 §25.8 write-back — implementation-as-fact (present since Stage 4.4, undocumented until now)". |
| `06-mir.md` | §15 (Stage 12.4 §25.8 retroactive) — add §15.3 or new §16 | **B4 design-gray-area** (design does not document closure call lowering algorithm; implementation deferral note at `expr_operand.rs:876` is the only mention) | **Required**: Add new sub-section "§15.3 Closure call lowering algorithm (Stage 13.3 implementation)" documenting: (a) synthesized `call` function MirBody per closure; (b) first param = `&closure_struct`; (c) captures dereferenced via `Place::Projection(self, Field(i))`; (d) per-crate `closure_call_bodies` side-table on `MirCrate`; (e) call site dispatches via `Terminator::Call` with `FnDef(closure_def_id, _)` func operand; (f) §16 compliance — side-table data flow, no HIR access from codegen. |
| `06-mir.md` | §5 (line 278 — `AggregateKind::Closure`) | ✅ Already aligned (no deviation) | Optional: add inline note that `AggregateKind::Closure(def_id, substs)` carries capture field types in `substs` (matching `TyKind::Closure`). |
| `07-codegen.md` | §8 (lines 490-526 — 闭包 codegen) | ✅ Already aligned (design prescribes Strategy A; Stage 13.3 implements it) | Optional: add §8.3 "Implementation status: Stage 13.3 (v0.23.0) implements §8.1-8.2 as specified. Fn/FnMut/FnOnce trait auto-impl deferred to Stage 13.5+." |
| `07-codegen.md` | §15 (Stage 8.6 §25.8 write-back) — add §15.3 | **B4 design-gray-area** (§15 covers extern "C" ABI + unwind + thread-local; does not mention closure codegen status) | **Required**: Add §15.3 "Closure codegen implementation status (Stage 13.3, v0.23.0)" — note that §8 design is now implemented; closure struct type emission at `emitter.rs:487-490`; closure call dispatch via `Terminator::Call` to synthesized `call` fn; `closure_call_bodies` side-table plumbing. |
| `04-ownership-borrowing.md` | §11.6 (implementation status) | **B4 design-gray-area** (design lists disjoint closure captures as B1 v0.2+; does not mention basic closure call lowering or capture-mode inference) | **Required**: Add §11.7 "Stage 13.3 closure call lowering — staging decision": (a) closure call lowering implemented Stage 13.3 (v0.23.0); (b) default capture mode = by-ref (matches current `Operand::Copy` behavior); (c) capture-mode inference (by-ref / by-mut-ref / by-value) deferred to Stage 13.5+; (d) Fn/FnMut/FnOnce auto-impl deferred to Stage 13.5+; (e) disjoint closure captures (RFC 2229) still deferred to v0.2+ (unchanged). |
| `13-stage1-feature-whitelist.md` | §2.5 (line 128 — Closure) | **B1 → ✅ partial** (status update) | Update §2.5 line 128 remark from "Fn/FnMut/FnOnce 自动推导" to "call lowering: ✅ Stage 13.3 (v0.23.0); Fn/FnMut/FnOnce auto-impl: Stage 13.5+". |
| `13-stage1-feature-whitelist.md` | §4.1 (Stage 0 must-support) | **B4 design-gray-area** (does not list "closure call lowering" explicitly) | Optional: add "Closure call lowering (Strategy A: synthesized call fn per closure)" to §4.1, marked "✅ Stage 13.3 (v0.23.0)". |

### 7.2 Write-back timing

Per §25.8.3 #5 "可重构不等于立即重构", the design write-back is best performed **immediately
after Stage 13.3 implementation completes and passes gate review**, before Stage 13.4
(macro_rules!) begins. This ensures:
1. The design doc reflects the as-shipped implementation (no temporal gap).
2. Stage 13.4 planning has accurate design baseline (per §13.4 stage-start alignment).
3. The write-back is a single atomic commit, not entangled with Stage 13.4 work.

### 7.3 Write-back responsibility

Per §25.8.2 step 5, ARCH-A drafts the write-back; REV-A verifies accuracy (does the design
text match the actual implementation in `src/mir/lower/expr_operand.rs`,
`src/mir/lower/mod.rs`, `src/codegen/mod.rs`?); PM-A coordinates inclusion in the Stage 13.4
plan's "design doc alignment" section.

---

## 8. Committee Recommendation

### **GO-WITH-CONDITIONS** for Stage 13.3 launch

**Justification**:

Stage 13.3 (TD-030 closure call lowering) is **fully design-aligned** per §13.4:
- Codegen design (`07-codegen.md` §8.1-8.2) explicitly prescribes Strategy A (direct call
  function synthesis) — the implementation gap is a B1 deviation traced to Stage 4.4
  (closure type lowering added, call dispatch deferred per `expr_operand.rs:876` code comment).
- MIR design (`06-mir.md` §5) has `AggregateKind::Closure`; `TyKind::Closure` is a B4
  design-gray-area write-back (present in implementation since Stage 4.4, undocumented).
- Ownership design (`04-ownership-borrowing.md` §8) covers disjoint captures (deferred to
  v0.2+); basic closure call lowering is a B4 gray-area write-back.
- Stage 1 feature whitelist (`13-stage1-feature-whitelist.md` §2.5 line 128) requires
  closures callable for Stage 1 — Stage 13.3 closes the B1 deviation.

**Strategy A (Direct call function synthesis)** is recommended with **HIGH risk**:
- 9 src files modified (exceeds §14.4 J5 ≤5 file guideline — justified by §15 long-term value).
- ~600-1000 LOC of new code (exceeds r216's optimistic "200-400 LOC" estimate — r216 estimate
  was based on incomplete scope analysis; actual scope per §3 includes synthesized MirBody,
  per-crate side-table, codegen emission pass, typeck changes).
- Reuses existing `MirLowerCtxt` infrastructure for body lowering; reuses existing
  `dyn_trait_calls` side-table pattern for `closure_call_bodies`; reuses existing
  `fn_name_by_def_id` codegen pattern for closure call fn dispatch.
- rustc-idiomatic (rustc lowers closures to closure struct + call fn + Fn/FnMut/FnOnce auto-impl;
  Stage 13.3 implements the first two, defers the third).
- Matches `07-codegen.md` §8.1-8.2 design intent (direct call to synthesized `call` fn).

**Fn/FnMut/FnOnce Option B (call lowering only, defer trait auto-impl)** is recommended:
- v0.3 self-hosting needs closures callable (direct call pattern), not necessarily
  `impl Fn(...)`-bound.
- Capture-mode inference is undocumented (`04-ownership-borrowing.md` silent on by-ref /
  by-mut-ref / by-value taxonomy) — deferring to Stage 13.5+ allows design-doc write-back
  to properly specify the inference algorithm.
- Option B matches `07-codegen.md` §8.2 design intent (direct call, not trait-mediated).
- Option B is consistent with Stage 13.2's incremental approach (minimum design-aligned
  strategy, no over-engineering).

**Version policy**: v0.22.0 → **v0.23.0** (minor bump — second user-facing compiler feature;
per `stage-13.1-design-alignment.md` §5.4 line 543 pre-established policy).

**§25.8 write-back required** post-implementation (per Section 7):
- `06-mir.md` §2/§5 — add `TyKind::Closure` (B4 design-as-fact).
- `06-mir.md` §15.3 (new) — document closure call lowering algorithm.
- `07-codegen.md` §15.3 (new) — document closure codegen implementation status.
- `04-ownership-borrowing.md` §11.7 (new) — document Stage 13.3 staging decision.
- `13-stage1-feature-whitelist.md` §2.5 line 128 — update remark.

**Conditions for GO** (must be satisfied before Stage 13.3 MUV-7 execution begins):

1. **Committee approval of file count exception**: 9 src files exceeds §14.4 J5 ≤5 file
   guideline. Per §15 long-term value justification (closures-as-values required for v0.3
   self-hosting), committee must explicitly approve the file count.

2. **Per-closure DefId allocation strategy confirmed**: The implementation must allocate
   unique per-closure DefIds without HIR-side changes (to maintain §16 compliance). The
   recommended approach (per-body closure counter encoded as `DefId(owner.0 + 1_000_000 + idx)`)
   must be reviewed by ARCH-A before execution.

3. **Capture-mode default decision confirmed**: Stage 13.3 Option B defaults all captures
   to by-ref. For non-Copy capture types, this may produce incorrect codegen (copying a
   non-Copy type). ARCH-A must confirm: (a) Stage 13.3 scope limited to Copy-type captures
   (conformance tests use only `i32`, `bool`, `&` types — all Copy); OR (b) emit
   `Operand::Move` for non-Copy captures (soundness-preserving but may break borrowck in
   nested cases). Recommended: (a) — Stage 13.3 scope explicitly limited to Copy captures;
   non-Copy captures deferred to Stage 13.5+ (matches `04-ownership-borrowing.md` §11.6
   disjoint-captures deferral).

4. **Stage 13.3 gate review criteria confirmed** (per `plan-13.1.md` §3):
   - `cargo build` — zero warnings, zero errors.
   - `cargo test --test all_tests` — 5026 conformance + 2179 integration tests, with
     **+40 expected compile_error→compile_ok flips** (the 40 closure-related conformance
     tests).
   - `cargo fmt --check` — zero diff.
   - `cargo clippy --all-targets` — zero warnings.
   - §16 grep: `rg "crate::hir" src/codegen/` returns 0 non-comment matches (no HIR
     access from codegen — `closure_call_bodies` side-table is the data carrier).
   - §16 grep: `rg "crate::codegen" src/mir/` returns 0 non-comment matches (no codegen
     access from MIR — the existing TD-028 fix must not regress).
   - Post-implementation grep: `rg "TD-030|closure call lowering.*deferred" src/` returns
     0 matches (deferral notes removed); `rg "closure_call_bodies" src/` returns ≥4 matches
     (side-table field + accessor + driver pass + codegen iteration).

5. **Coupled unit test audit**: `rg "compile_error" tests/v0/stage*/plan/` must be audited
   for closure-related assertions; coupled unit tests must be updated in lockstep with
   .lin marker flips. (Similar to Stage 13.2's `control_flow_tests.rs:68-97` coupling —
   Stage 13.3 may have analogous couplings in stage9/10/11 unit tests.)

**GO-WITH-CONDITIONS for Stage 13.3 launch**: Strategy A + Option B is the rustc-idiomatic,
design-aligned path to closing TD-030. The 40 conformance `compile_error` tests flip to
`compile_ok` as the second concrete user-facing value delivered by Stage 13 — unblocking
the most pervasive Rust pattern (closures defined + called locally) for v0.3 self-hosting.
The HIGH risk is justified by §15 long-term value (closures-as-values required for Stage 1
source; Strategy B inline approach would close TD-030 nominally but leave a critical gap
requiring Stage 13.3b rework). Conditions 1-5 must be satisfied before MUV-7 execution begins.

---

**Audit completed**: 2026-07-26
**Next action**: Stage Committee vote on this design alignment → if GO-WITH-CONDITIONS,
satisfy conditions 1-5 → Stage 13.3 MUV-7/8 execution (estimated 2-3 weeks per
`plan-13.1.md` §2 Stage 13.3).
