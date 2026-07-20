# Stage 3 Development Log

> **Author**: redskaber
> **Date**: 2026-07-19
> **Version**: v0.7.3
> **Status**: Active

## Sub-stages

### Stage 3.1 — MVP (v0.5.1)
- Created `src/codegen/mod.rs` with direct string-based codegen
- Supports: function definition, return, i32 constants, binary ops
- 13 codegen tests

### Stage 3.2 — Variables + Control Flow + Calls (v0.5.2)
- Added alloca/store/load for variable allocation
- SwitchInt → br i1 (bool conditionals for if/while)
- Goto → br label
- Call → call instruction
- Basic block labels (bb0, bb1, ...)

### Stage 3.3 — Comparisons + Borrow/Deref (v0.6.0)
- Comparison ops (Eq/Ne/Lt/Le/Gt/Ge) → icmp + zext
- Rvalue::Ref → alloca pointer
- Projection::Deref → double load
- Call resolution from FnDef type in local's ty

### Stage 3.4 — Emitter Trait Refactor (v0.6.0)
- Introduced Emitter trait (backend-agnostic)
- TextEmitter implements Emitter for .ll text
- Translation layer walks MIR and calls Emitter methods
- Future InkwellEmitter only needs to implement Emitter trait

### Stage 3.5 — Parameter Passing (v0.7.0)
- HIR body.params.len() determines param count
- Each param gets LLVM arg: %arg0, %arg1, ...
- Params stored to alloca slots at function entry
- Call instruction emits typed args

### Stage 3.6 — Match Switch + Float (v0.7.1)
- LLVM switch instruction for match on integers
- Float constant handling (f64)
- Process v3.5: Documentation sync rules (§11)

### Stage 3.7 — Cast + Author (v0.7.2)
- Rvalue::Cast → proper LLVM cast instructions
- All cast types: sext/zext/trunc/sitofp/fptosi/fpext/fptrunc/bitcast
- Author "redskaber" added to all documents
- Process v3.6: Author标注

### Stage 3.8 — Doc Reorganization (v0.7.3)
- Process v3.7: Document organization rules (§12)
- Reorganized docs/ into agent-team/ + develop/v0/stage-N/ + lang-design/
- Created lang-design design documents
- Created agent-team role definitions

### Stage 3.9 — Imported documentation (v0.7.4)
- Process v3.7 final form
- Imported agent-team/ (12 docs) + lang-design/ (22 docs)
- Total docs: 56 (was 25)

### Stage 3.10 – 3.19 — Incremental codegen hardening (v0.8.0 → v0.8.4)
- 3.10: Drop terminator
- 3.11: Assert + panic stubs (overflow / div-by-zero / bounds-check)
- 3.12: External function declarations
- 3.13: PHI node support in Emitter trait
- 3.14: insertvalue / extractvalue for tuples
- 3.15: Array aggregate via insertvalue
- 3.16: Assert message routing (Overflow(op) / DivisionByZero / BoundsCheck)
- 3.17: GEP field + GEP index helpers
- 3.18: Typed load helpers (detect_lvalue_type, detect_operand_type)
- 3.19: Emitter API rename (emit_* / get_* / set_* prefix convention)

### Stage 3.20 — Typed load refactor (v0.8.5)
- codegen_lvalue_load_typed takes explicit EmitType (was hardcoded I32)
- detect_lvalue_type recurses through Projection::Field / Deref / Index
- detect_operand_type handles Constant (Float → F64, Bool → I1, Char → I8)

### Stage 3.21 — Typed aggregate codegen (v0.8.6)
- **Problem**: EmitType::Tuple hardcoded to `{ i32 }`, EmitType::Array to `[10 x i32]`,
  Ptr to opaque `i32*`, emit_call hardcoded all args as i32.
- **Fix**: EmitType now carries full structure (Struct/Array/Ptr variants);
  emit_type_to_llvm_str returns String; emit_gep_field/index take the actual
  base type; emit_insertvalue takes val_ty; emit_call takes (EmitType, &EmitValue)
  pairs; new detect_lvalue_storage_type helper.
- 10 new tests (tuple mixed types, array of i64, typed call args, typed GEP).
- Total: 709 → 719.

### Stage 3.22 — Block-scoped local value cache (v0.8.6)
- **Problem**: TextEmitter::locals cached values across block boundaries,
  causing `if x > 0 { 1 } else { 2 }` to return `2` regardless of `x`.
- **Fix**: emit_block clears self.locals at each block boundary; local_ptrs
  persist. Within-block constant shortcut still works.
- 6 new tests (if-else merge correctness, nested if, match, while, if-with-arith).
- Total: 719 → 725.

### Stage 3.23 — Gate Review Round 1 (v0.8.6)
- 38-case codegen audit (`examples/stage3_gate_audit.rs`)
- 5 groups: single-stmt / multi-stmt / complex / edge cases / robustness
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- 5/5 committee APPROVED — unanimous
- See `gate-review-round1.md` for full report

### Stage 3.24 — Real overflow checks (v0.8.6)
- **Problem**: `Assert` terminator for overflow used `cond = Bool(true)` placeholder
  — overflow checks never fired. `a + b` silently wrapped on overflow (UB in safe Landin).
- **Fix**:
  * Extended `AssertMessage::Overflow` from `Overflow(BinOp)` to
    `Overflow(BinOp, Operand, Operand)` — now carries lhs and rhs operands
    (per design doc `06-mir.md`).
  * Modified `emit_overflow_assert` in MIR lower to pass lhs/rhs.
  * Added `Emitter::emit_checked_binop` trait method.
  * `TextEmitter::emit_checked_binop` emits:
    - Add → `llvm.sadd.with.overflow.{i32,i64}`
    - Sub → `llvm.ssub.with.overflow.{i32,i64}`
    - Mul → `llvm.smul.with.overflow.{i32,i64}`
    - Others → fallback `{T, i1} undef` with i1 = 0 (no overflow)
  * Codegen: `extractvalue` index 1 from `{T, i1}` aggregate, invert with
    `xor i1 flag, -1`, branch: no-overflow → target, overflow → panic block.
  * Panic block calls `__landin_panic_overflow(op, 0, 0)` + `unreachable`.
- 8 new tests covering: add/sub/mul on i32, i64, branch to panic, no-check for
  comparisons/bitwise/floats, overflow in loops, chained arith.
- Total: 725 → 733.

### Stage 3.25 — Real div-by-zero checks (v0.8.6)
- **Problem**: Div/Rem operations had no runtime check for divisor == 0.
  `a / 0` invoked LLVM's `sdiv` instruction — undefined behavior on zero divisor.
- **Fix**:
  * Extended `AssertMessage::DivisionByZero` from `DivisionByZero` to
    `DivisionByZero(Operand)` — now carries the divisor operand.
  * Added `emit_div_by_zero_assert` in MIR lower, emitted for `Div` and `Rem`
    ops (replaces the wrong `Overflow(op)` for these ops).
  * Codegen: `icmp eq <divisor>, 0`; if true → panic block; if false → target.
  * Panic block calls `__landin_panic_div_by_zero()` + `unreachable`.
- 6 new tests covering: div/rem on i32, div on i64, no-check for add, panic
  unreachable, div in loop with overflow check, mixed arith (add+div).
- Total: 733 → 739.

### Stage 3.26 — Gate Review Round 2 (v0.8.6)
- 43-case codegen audit (`examples/stage3_gate_audit_r2.rs`)
- 5 groups: regression (15) + Stage 3.24 (10) + Stage 3.25 (8) + edge cases (5) + adversarial (5)
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- §9.3.3 CONVERGED: R1=38/38 + R2=43/43 = 2 consecutive rounds 0 new issues
- 5/5 committee APPROVED — unanimous
- See `gate-review-round2.md` for full report
- L6 (overflow) + L7 (div-by-zero) CLOSED; L1-L5, L8-L11 remain (optimizations / new features)

### Stage 3.27 — String literal codegen (v0.8.6)
- **Problem**: `ConstVal::Str(sym)` hardcoded to emit `"0"` (null pointer).
  Any program using string literals produced broken IR — bytes were lost.
- **Fix**:
  * Added `Emitter::emit_string_global(bytes)` trait method.
  * `TextEmitter` accumulates string globals in `Vec<String>`, dedupes via
    `HashMap<Vec<u8>, String>`. Same content → same global name.
  * Globals emitted at module end via `output_with_globals()`.
  * Each global: `@.str.N = private unnamed_addr constant [M x i8] c"..."`.
  * Byte content escaped: printable ASCII verbatim; everything else as
    `\NN` hex (tab → `\09`, newline → `\0A`, quote → `\22`, backslash → `\5C`,
    non-ASCII → UTF-8 bytes hex-escaped).
  * `codegen_operand` for `ConstVal::Str`: looks up bytes via interner
    (now threaded through all codegen functions), emits global, returns
    `getelementptr inbounds ([N x i8], [N x i8]* @.str.N, i32 0, i32 0)` (i8*).
  * `TyKind::Str` maps to `EmitType::ptr_to(EmitType::I8)` (was `I32`).
  * Side fix: skip `alloca`/`store` for void-typed locals (was producing
    invalid `alloca void` / `store void` for unit-typed MIR temp slots).
- 13 new tests: global emission, GEP, dedup, distinct, escapes (tab/newline/
  quote/backslash), Unicode UTF-8, empty, cross-function dedup, no-void-alloca.
- Total: 739 → 752.

### Stage 3.28 — Byte string literal codegen (v0.8.6)
- **Problem**: `b"..."` literals lowered as `Slice(u8)` with `ConstVal::Str`,
  but `Slice` wasn't handled by `mir_type_to_emit_type` (fell through to `I32`),
  and `u8` itself also fell through to `I32`. Result: byte strings got the
  same broken treatment as string literals, AND `u8`-typed locals had wrong type.
- **Fix**:
  * `TyKind::Slice(elem)` maps to `EmitType::ptr_to(mir_type_to_emit_type(elem))`
    (was `I32`). `Slice(u8)` → `Ptr(I8)` → `i8*`.
  * `TyKind::Int(I8)` and `TyKind::Uint(U8)` map to `EmitType::I8` (was `I32`).
  * `TyKind::Int(I16)` / `Uint(U16)` explicitly map to `I32` (Stage 3 simplification).
  * Byte strings share the same global format as string literals and dedup
    across both (`"hello"` and `b"hello"` → one global).
- 9 new tests: byte string global, GEP, dedup with str, escape, empty,
  u8/i8 type mapping, byte string with other locals.
- Total: 752 → 761.

### Stage 3.29 — Gate Review Round 3 (v0.8.6)
- 43-case codegen audit (`examples/stage3_gate_audit_r3.rs`)
- 5 groups: regression (15) + Stage 3.27 strings (10) + Stage 3.28 bytestrings (8)
  + edge cases (5) + adversarial (5)
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- §9.3.3 CONVERGED: R1=38/38 + R2=43/43 + R3=43/43 = 3 consecutive rounds 0 new issues
- 5/5 committee APPROVED — unanimous
- See `gate-review-round3.md` for full report
- L4 (string literals) + L12 (u8/i8 type) CLOSED; new L13 (fat ptr), L14 (i16), L15 (str-as-arg) documented

### Stage 3.30 — ADT/struct codegen + §15/§16 process principles (v0.8.6)
- **Process v3.10 + v3.11**: added §15 (最优 > 最小) and §16 (阶段间接口隔离).
- **3 root-cause bugs fixed** (per §15 — all fixed at root, not via hacks):
  1. Tuple struct ctor `Pair(1, 2)` was lowered as `Terminator::Call`
     (fake function call). Root cause: `Res::Def(DefId)` didn't carry
     `DefKind`. Fix: extended to `Res::Def(DefId, DefKind)`; MIR lower
     dispatches on `DefKind::Struct` to emit `Aggregate(Adt, operands)`.
  2. Named struct types in param/return positions were lost —
     `lower_hir_ty_to_mir_ty` fell through `HirTyKind::Path` to `TyKind::Error`.
     Fix: added `HirTyKind::Path` handling → `TyKind::Adt(def_id, substs)`.
  3. Field access `p.x` / `p.1` always returned field 0 — MIR lower
     hardcoded `FieldId(0)`; parser lost tuple field index via `Spur::default()`.
     Fix: parser interns field index as string; MIR lower's new
     `resolve_field_index` parses it (tuple) or looks up by name (named struct).
- **§16 compliance** (per §16 — data sink, no cross-stage internal-API calls):
  * `AggregateKind::Adt` extended with `field_tys: Vec<Ty>` — MIR lower
    computes field types from HIR and sinks them into MIR.
  * Codegen reads field types from MIR (not from HIR via `lower_hir_ty_to_mir_ty`).
  * New codegen-local `hir_ty_to_emit_type` for HirTy → EmitType conversion.
  * `mir_type_to_emit_type_with_hir` marked L-PIPE-1 (reads HIR for
    `TyKind::Adt` local/param storage types — allowed per §16.2.1 but
    deeper fix would sink field types into `TyKind::Adt`).
- **Parser change**: `Parser.interner` changed from `&Rodeo` to `&mut Rodeo`
  so parser can intern tuple field indices. All callers updated.
- **fn_names indexing bug fixed**: was indexing by body index (wrong when
  struct/enum owners created DefId gaps). Now uses `DefId → name` HashMap.
- 13 new tests: named/tuple struct construction, field access, alloca,
  mutation, mixed types, struct as param/return, unit struct, multiple
  structs, struct in if/loop, struct + overflow.
- Total: 761 → 774.

### Stage 3.31 — Gate Review Round 4 (v0.8.6)
- 37-case codegen audit (`examples/stage3_gate_audit_r4.rs`)
- 6 groups: regression (12) + Stage 3.30 ADT (12) + edge cases (5) +
  adversarial (5) + §16 verification (3)
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- §9.3.3 CONVERGED: R1=38/38 + R2=43/43 + R3=43/43 + R4=37/37 = 4 consecutive rounds 0 new issues
- §15 verified: tuple struct ctor bug fixed at root (no fake call).
- §16 verified: no cross-stage internal-API calls in codegen.
- 5/5 committee APPROVED — unanimous
- See `gate-review-round4.md` for full report
- L2 (struct codegen) CLOSED; new L-ENUM (enum variants), L-DEBT-2 (field type resolution), L-PIPE-1 (HIR lookup for Adt storage) documented

### Stage 3.32 — L-DEBT-2 fix: field type resolution through projections (v0.8.6)
- **Problem** (recorded as L-DEBT-2 in R4): `p.1` where field 1 is `i64`
  loaded as `i32`. The GEP index was correct (1), but the load type used
  the unresolved `field_ty` (a `fresh_infer_ty` that defaulted to `i32`).
- **Root cause**: typeck's `infer_projection` returned `field_ty.clone()`
  for `ProjectionElem::Field(_, field_ty)` — but `field_ty` was a
  `fresh_infer_ty` allocated by MIR lower and never resolved to the actual
  struct field type.
- **Fix** (per §15 — root cause, not hack):
  1. typeck `infer_rvalue` handles `AggregateKind::Adt` — unifies each
     operand with the corresponding `field_tys` entry (sunk into MIR per
     §16 in Stage 3.30), returns `TyKind::Adt(def_id, substs)`. Was: fell
     through to `TyKind::Error`.
  2. typeck Phase 3.5 `writeback_field_types` — after Phase 3 (local types
     resolved), walks all statements and for each
     `ProjectionElem::Field(field_id, field_ty)`: resolves the base type →
     if `Adt(def_id, _)`, looks up the field type from HIR and updates
     `field_ty` in place. Per §16: typeck reads HIR (allowed); resolved
     type sunk into MIR so codegen reads from MIR.
  3. MIR lower `resolve_field_index` fallback scan — when the receiver's
     type can't be resolved at lower time (e.g., `let m = Mixed { ... }; m.b`),
     scan all HIR struct owners for one with a matching field name.
- **New API**: `TypeChecker::check_mir_body_with_hir(mir, hir)`. Legacy
  `check_mir_body(mir)` delegates with `None`.
- 6 new tests: field load i64/f64/bool/u8, field in arithmetic, named field.
- Total: 774 → 780.

### Stage 3.33 — Gate Review Round 5 (v0.8.6)
- 30-case codegen audit (`examples/stage3_gate_audit_r5.rs`)
- 4 groups: regression (10) + Stage 3.32 L-DEBT-2 fix (10) + edge cases (5) + adversarial (5)
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- §9.3.3 CONVERGED: R1-R5 = 5 consecutive rounds 0 new issues
- §15.4 verified: L-DEBT-2 root cause fixed (field types resolve correctly).
- 5/5 committee APPROVED — unanimous
- See `gate-review-round5.md` for full report
- L-DEBT-2 CLOSED; new L-MUT-1 (field mutation MIR lower) documented

### Stage 3.34 — L-MUT-1 fix: field mutation MIR lower (v0.8.6)
- **Problem** (recorded as L-MUT-1 in R5): `a.v = 42` didn't mutate the
  struct — it stored to a temp local instead. The mutation was silently
  dropped. Reading `a.v` after the assignment returned the original value.
- **Root cause**: MIR lower's `HirExprKind::Assign` handling only supported
  `Path` LHS (local variable assignment). For `Field`/`Index`/`Deref` LHS
  (projection places), it fell through to "just evaluate rhs" and discarded
  the assignment.
- **Fix** (per §15 — root cause, not hack):
  * Added `lower_expr_to_lvalue` function that converts a HIR expression
    to a MIR `Lvalue` (a place that can be assigned to). Handles:
    - `Path` → `Lvalue::Local`
    - `Field { receiver, ident }` → `Lvalue::Projection(receiver, Field(idx, ty))`
    - `Index { receiver, index }` → `Lvalue::Projection(receiver, Index(idx))`
    - `Unary { op: Deref, expr }` → `Lvalue::Projection(expr, Deref)`
  * Updated `HirExprKind::Assign` to use `lower_expr_to_lvalue` for the LHS,
    then `push_assign` to the resulting place. Handles ALL LHS shapes
    generically — no special-casing per projection type.
- 8 new tests: field mutation works/persists, named field, i32 field,
  multiple mutations, local assignment regression, mutation in loop,
  correct GEP index, overwrite.
- Total: 780 → 788.

### Stage 3.35 — Gate Review Round 6 (v0.8.6)
- 30-case codegen audit (`examples/stage3_gate_audit_r6.rs`)
- 4 groups: regression (10) + Stage 3.34 L-MUT-1 fix (10) + edge cases (5) + adversarial (5)
- §9.3.1 ≥30 cases ✅, §9.3.2 ≥5 edge cases ✅
- §9.3.3 CONVERGED: R1-R6 = 6 consecutive rounds 0 new issues
- §15.4 verified: L-MUT-1 root cause fixed (field mutations work).
- 5/5 committee APPROVED — unanimous
- See `gate-review-round6.md` for full report
- L-MUT-1 CLOSED; new L-DEBT-3 (field type propagation through arithmetic operands) documented

## Test Progression

| Version | Tests | New |
|---------|-------|-----|
| v0.5.1 | 686 | +13 codegen |
| v0.5.2 | 686 | 0 (refactor) |
| v0.6.0 | 694 | +8 (comparisons, borrow, call) |
| v0.7.0 | 699 | +5 (params, calls) |
| v0.7.1 | 706 | +7 (match, float, complex) |
| v0.7.2 | 709 | +3 (cast, bool return) |
| v0.7.3 | 709 | 0 (doc reorg) |
| v0.7.4 | 709 | 0 (doc import) |
| v0.8.4 | 709 | 0 (3.10–3.19 incremental hardening) |
| v0.8.5 | 709 | 0 (3.20 typed-load refactor) |
| v0.8.6 (3.21-3.23) | 725 | +16 (typed aggregates + block-scoped cache + R1) |
| v0.8.6 (3.24-3.26) | 739 | +14 (real overflow + div-by-zero checks + R2) |
| v0.8.6 (3.27-3.29) | 761 | +22 (string literals + byte strings + R3) |
| v0.8.6 (3.30-3.31) | 774 | +13 (ADT/struct codegen + R4 + §15/§16 process) |
| v0.8.6 (3.32-3.33) | 780 | +6 (L-DEBT-2 field type resolution + R5) |
| **v0.8.6 (3.34-3.35)** | **788** | **+8 (L-MUT-1 field mutation + R6)** |

