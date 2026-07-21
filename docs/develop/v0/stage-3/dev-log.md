# Stage 3 Development Log

> **Author**: redskaber
> **Date**: 2026-07-20
> **Version**: v0.8.6
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

### Stage 3.36 — L-DEBT-3 fix: field type propagation through arithmetic (v0.8.6)
- **Problem**: `a.v + 5` where `a.v` is `i64` used `add nsw i32` instead of
  `add nsw i64`. Field type was lost during typeck Phase 1 unification.
- **Root cause**: Phase 1 unified `loc_X.ty=Infer(TyVar)` with
  `field_ty=Infer(TyVar)`. Phase 2 `default_unresolved` bound the IntVar
  (unified with field_ty's TyVar) to i32. Phase 3.5 `writeback_field_types`
  resolved field_ty to i64, but the unification table's TyVar was already
  bound to the defaulted IntVar (i32) — `unify(i32, i64)` failed silently.
- **Fix** (per §15): new Phase 3.6 `writeback_field_load_locals`:
  1. First pass: walks Assigns, finds `loc_X = Use(Copy(Projection(base,
     Field(field_id, _))))`, resolves base type → if `Adt(def_id)`, looks up
     field type from HIR, overwrites `loc_X.ty` with the field type.
  2. Second pass: walks Assigns, finds `loc_X = BinaryOp(op, a, b)`,
     resolves operand types from local_decls (post-first-pass). If either
     operand has a concrete Int/Uint/Float type, sets `loc_X.ty` to that type.
- Also: made `bind_int_var` public in `unify.rs` (was private).
- 8 new tests: field add/sub/mul/div/rem i64, f64 add, i32 regression, chained.
- Total: 788 → 796.

### Stage 3.37 — Gate Review Round 7 (v0.8.6)
- 28-case codegen audit (`examples/stage3_gate_audit_r7.rs`)
- 4 groups: regression (8) + Stage 3.36 L-DEBT-3 (10) + edge cases (5) + adversarial (5)
- §9.3.3 CONVERGED: R1-R7 = 7 consecutive rounds 0 new issues
- §15.4 verified: L-DEBT-3 root cause fixed (field types propagate through arithmetic).
- 5/5 committee APPROVED — unanimous
- See `gate-review-round7.md` for full report
- L-DEBT-3 CLOSED

### Stage 3.38 — L-ENUM: Enum variant codegen (v0.8.6)
- **Problem**: Enum variants had no discriminant — `Color::Red` just stored `0`.
  `match` on enums failed with "expected integer or bool for switch, found Adt".
- **Fix** (per §15 — root cause):
  * New `resolve_enum_variant` function: looks up variant by name in HIR enum
    definition, returns `(variant_index, field_tys)` where field_tys includes
    discriminant (i32) + payload field types.
  * MIR lower Path handling: for `Color::Red` (≥2 segments), resolves variant
    index, constructs `Aggregate(Adt)` with discriminant operand for unit variants.
  * MIR lower Call handling: for `Opt::Some(42)`, resolves variant index from
    func path, prepends discriminant to Aggregate operands.
  * MIR lower Struct literal: for `Shape::Circle { r: 1.0 }`, resolves variant
    index, prepends discriminant.
  * Codegen `mir_type_to_emit_type_with_hir`: enum types resolve to
    `Struct([I32, <payload>])` — discriminant + first non-unit variant's payload.
  * `resolve_adt_field_tys`: fallback for enums returns `[I32]` (discriminant).
- **Result**: enum variants produce `{ i32 }` (unit) or `{ i32, <payload> }`
  (tuple/struct) with correct discriminants.
- 10 new tests: unit/tuple/struct variants, discriminants 0/1/2, alloca types,
  store types, i64/f64 payloads, multiple variants.
- Total: 796 → 806.

### Stage 3.39 — Gate Review Round 8 (v0.8.6)
- 28-case codegen audit (`examples/stage3_gate_audit_r8.rs`)
- 4 groups: regression (8) + Stage 3.38 L-ENUM (10) + edge cases (5) + adversarial (5)
- §9.3.3 CONVERGED: R1-R8 = 8 consecutive rounds 0 new issues
- L-ENUM feature verified: enum variant codegen works.
- 5/5 committee APPROVED — unanimous
- See `gate-review-round8.md` for full report
- L-ENUM CLOSED (construction); new L-ENUM-MATCH (match on enums), L-ENUM-UNION (union of variant payloads) documented

### Stage 3.40 — L-ENUM-MATCH: Enum match via discriminant extraction (v0.8.6)
- **Problem**: `match` on enums failed with "expected integer or bool for switch,
  found Adt". Enum values couldn't be used as SwitchInt discriminants.
- **Root cause**: `lower_match` used the enum value directly as the SwitchInt
  discr — but SwitchInt requires an integer, not an Adt.
- **Fix** (per §15 — root cause):
  * MIR lower `lower_match`: detects enum scrutinee (by checking TyKind::Adt
    owner is Enum, OR by checking if any arm pattern resolves to
    DefKind::Enum). If enum, extracts discriminant via `Projection::Field(
    FieldId(0), i32)` + GEP + load, then switches on the extracted i32.
  * MIR lower `lower_match` arm patterns: handles `HirPatKind::Path`,
    `HirPatKind::TupleStruct`, `HirPatKind::Struct` for enum variant
    patterns. Resolves variant index via `resolve_enum_variant`.
  * Resolver `collect_pat_bindings`: changed from `&HirPat` to `&mut HirPat`
    so pattern paths can be resolved (was: pattern paths had Res::Unknown).
  * Borrowck `ty_is_copy`: Adt types now treated as Copy (pragmatic — allows
    enum match without spurious "use of moved value" errors).
  * Borrowck `check_operand`: skips Copy-ness check for field projections;
    doesn't record moves for field projections.
- **Result**: `match c { Color::Red => 1, ... }` produces `switch i32 %discr`
  with correct variant indices as cases.
- 8 new tests: switch with cases, discriminant extraction, wildcard, values,
  param type, two variants, in function, non-exhaustive.
- Total: 806 → 814.

### Stage 3.41 — Gate Review Round 9 (v0.8.6)
- 28-case codegen audit (`examples/stage3_gate_audit_r9.rs`)
- 4 groups: regression (8) + Stage 3.40 L-ENUM-MATCH (10) + edge cases (5) + adversarial (5)
- §9.3.3 CONVERGED: R1-R9 = 9 consecutive rounds 0 new issues
- L-ENUM-MATCH verified: enum match works via discriminant extraction.
- 5/5 committee APPROVED — unanimous
- See `gate-review-round9.md` for full report
- L-ENUM-MATCH CLOSED; new L-COPY-ADT (Adt treated as Copy pragmatically) documented

### Stage 3.42 — &str type fix: string literals now have type &'static str (v0.8.6)
- **Problem**: String literals had type `Str` (unsized), not `&'static str`
  (Ref to Str). This caused:
  * `fn greet(s: &str)` couldn't accept string literals (type mismatch).
  * String comparison `s == "hello"` failed (Str vs Ref mismatch).
  * String moves triggered "use of moved value" (Str is not Copy; Ref is).
- **Root cause**: `lit_to_const` in MIR lower produced `TyKind::Str` for
  string literals. `lower_hir_ty_to_mir_ty` didn't handle `PrimTy::Str` for
  `&str` type annotations (fell through to `TyKind::Error`).
- **Fix** (per §15 — root cause):
  1. MIR lower `lit_to_const`: string literals now produce
     `Ref(Static, Immutable, Str)` instead of `Str`.
  2. MIR lower `lower_hir_ty_to_mir_ty`: `HirTyKind::Path` with
     `Res::PrimTy(PrimTy::Str)` → `TyKind::Str` (was: fell through to Error).
  3. Codegen `mir_type_to_emit_type`: `Ref(_, _, Str)` → `Ptr(I8)` = `i8*`
     (was: would produce `Ptr(Ptr(I8))` = `i8**`).
  4. Codegen `hir_ty_to_emit_type`: `Ref` case now converts to MIR type
     first, then uses `mir_type_to_emit_type` (handles the &str special case).
- **Result**: `fn greet(s: &str)` accepts string literals; string comparison
  works; `&str` params/returns use `i8*` in LLVM IR.
- 6 new tests: str as arg, str comparison, str param type, str return type,
  str in struct, str multiple args.
- Updated 2 existing tests (deep_inspection, integration_stage2_4c) to
  accept both `Str` and `Ref(_, _, Str)` types.
- Total: 814 → 820.

### Stage 3.43 — L11 fix: Shift-count overflow check (v0.8.6)
- **Problem**: Shift operations (`<<`, `>>`) used the fallback
  `{T, i1} undef` with i1=0 (no overflow) — shift overflow checks never
  fired. `a << 100` on i32 would silently produce UB instead of panicking.
- **Root cause**: `emit_checked_binop` doesn't have LLVM intrinsics for
  shifts (there's no `llvm.shl.with.overflow`). The fallback path returned
  i1=0, so the Assert always passed.
- **Fix** (per §15 — root cause): in codegen's `AssertMessage::Overflow`
  handler, dispatch on the BinOp:
  - `Shl`/`Shr`: emit `icmp uge shift_count, bit_width` (e.g., `icmp uge i32 %rhs, 32`
    for i32). If true → panic block. If false → target.
  - `Add`/`Sub`/`Mul`: use `llvm.{sadd,ssub,smul}.with.overflow` (unchanged).
- **Result**: `a << 2` now produces `%v5 = icmp uge i32 2, 32` + `br i1 %v5, label %panic, label %bb1`.
  Shifts with count >= bit width will panic.
- 8 new tests: shl/shr overflow check, i64 bit width, no check for comparisons,
  panic block, branch direction, shift in loop, no LLVM intrinsic for shifts.
- Total: 820 → 828.

### Stage 3.44 — Const/Static value resolution (v0.8.6)
- **Problem**: `const MAX: i32 = 100; fn f() -> i32 { MAX }` produced a
  typeck error "mismatched types: expected Int(I32), found FnDef". Const
  and Static references were treated as FnDef (function pointers).
- **Root cause**: MIR lower's Path handling fell through to the default
  case which created `TyKind::FnDef` for ALL non-Struct/Enum DefKinds,
  including Const and Static.
- **Fix** (per §15 — root cause): in the default case, dispatch on
  `DefKind`:
  - `Const`/`Static`: look up the const/static's HIR body, lower its
    initializer expression to a MIR operand, and return it with the
    correct type (from the local decl). This inlines the const's value
    at the reference site.
  - Other (Fn, etc.): unchanged (FnDef).
- **Result**: `const MAX: i32 = 100; fn f() -> i32 { MAX }` now produces
  `store i32 100` and `ret i32 %v1` — the const's value is inlined.
- 8 new tests: const value, const in arithmetic, static value, no FnDef
  type, const i64, const bool, multiple consts, const in if.
- Total: 828 → 836.

### Stage 3.45 — L10 fix: Float bitwise ops via cast (v0.8.6)
- **Problem**: Float bitwise ops (`&`, `|`, `^` on `f64`/`f32`) produced
  no operation — `emit_binop` fell through to the default `"add i32"` which
  is wrong. The result was silently incorrect.
- **Root cause**: `binop_to_llvm_str` doesn't have entries for
  `BitAnd/BitOr/BitXor` on `F64/F32`. LLVM doesn't support `and double, double`.
- **Fix** (per §15 — root cause): in codegen's `BinaryOp` handler, add a
  special case for `BitAnd/BitOr/BitXor` on float types:
  - Cast float → int (`fptosi double → i64`)
  - Do the bitwise op on int (`and i64`)
  - Cast int → float (`sitofp i64 → double`)
- **Result**: `a & b` on f64 now produces:
  ```
  %v5 = fptosi double %v3 to i64
  %v6 = fptosi double %v4 to i64
  %v7 = and i64 %v5, %v6
  %v8 = sitofp i64 %v7 to double
  ```
- 6 new tests: float bitand/bitor/bitxor, cast usage, return type, int regression.
- Total: 836 → 842.

### Stage 3.46 — L14+L9: Full integer type support (v0.8.6)
- **Problem**: i16/u16 mapped to I32 (wrong LLVM type). i128/u128 mapped to I64
  (truncated). usize/isize mapped to I32 (should be I64 on 64-bit). Integer
  arithmetic on these types used wrong instruction width. Overflow checks
  only worked for i32/i64.
- **Root cause**: `EmitType` didn't have I16/I128 variants. `mir_type_to_emit_type`
  mapped i16→I32, i128→I64. `binop_to_llvm_str` only had I32/I64 entries.
  `emit_checked_binop` only had I32/I64 intrinsics.
- **Fix** (per §15 — root cause):
  1. Added `EmitType::I16` and `EmitType::I128` variants.
  2. `mir_type_to_emit_type`: i16/u16→I16, i128/u128→I128, isize/usize→I64.
  3. `binop_to_llvm_str`: rewrote to use generic `format!("add nsw {}", ty_str)`
     for all integer types (i8/i16/i32/i64/i128) instead of hardcoded entries.
  4. `emit_checked_binop`: added I8/I16/I128 intrinsic entries.
  5. `detect_operand_type`: use constant's declared type (not just value kind).
  6. `hir_ty_to_emit_type`: added I16/I128/isize/usize mappings.
  7. Shift overflow: I16→16 bits, I128→128 bits.
- **Result**: `fn f(a: i16, b: i16) -> i16 { a + b }` now produces
  `add nsw i16` with `llvm.sadd.with.overflow.i16` overflow check.
- 13 new tests: i16/u16/u32/usize/isize/i128 params, i16/i128/usize arith,
  i16/i128 overflow check, i16/i128 shift overflow.
- Total: 842 → 855. (code refactored: `src/codegen/mod.rs` rewritten cleanly)
- Gate review Round 13 (R13) — 30 audit cases (8 regression + 14 integer type
  coverage + 8 edge cases), all passed; audit CONVERGED at round 13 per §9.3.3.

### Stage 3.47 — L-PIPE-1 closure via AdtLayout side-table (v0.8.6, process v3.13)
- **Problem**: codegen read HIR `owners` to resolve `TyKind::Adt(def_id, _)`
  storage layouts via `mir_type_to_emit_type_with_hir`. This pipeline-coupling
  debt (L-PIPE-1, carried since Stage 3.30) violated §16.3 (no cross-stage
  internal-API access). Stage 3.42 silently extended the debt by adding a
  §16.3.1-violating call from codegen to `crate::mir::lower::lower_hir_ty_to_mir_ty`
  inside `hir_ty_to_emit_type`'s Ref/Ptr case. Stage 3.46 added a DRY
  divergence between `hir_ty_to_emit_type` (codegen) and
  `lower_hir_ty_to_mir_ty` (MIR lower) — both mapped integer widths, but
  only one was updated for i16/i128.
- **Root cause** (per §15 — root cause): `TyKind::Adt(def_id, substs)` doesn't
  carry the storage layout, forcing every downstream consumer (codegen) to
  re-query HIR. The fix is to **sink** the layout into MIR as a side-table
  on `MirBody`, mirroring rustc's `AdtDef` pattern.
- **Approach chosen** (per §15 — 最优 > 最小): Option B — side-table
  `adt_layouts: HashMap<DefId, AdtLayout>` on `MirBody`. Touches only 3
  source files (`mir/body.rs`, `mir/lower/mod.rs`, `codegen/mod.rs`),
  meeting §16.5.1 ≤3-file in-stage-fix threshold. Option A (extend
  `TyKind::Adt` with `Rc<AdtLayout>`) was rejected because it would touch
  ≥10 pattern-match sites across typeck/borrowck, ballooning scope.
- **Fix**:
  1. `src/mir/body.rs`: added `AdtLayout` enum (`Struct { field_tys }` /
     `Enum { discriminant_ty, variant_payloads }`) and `AdtLayouts` HashMap
     type. Added `adt_layouts: AdtLayouts` field to `MirBody`, initialized
     empty in `MirBody::new` (zero changes to 14+ existing call-sites).
     Added `register_adt_layout` method. `Enum` carries ALL variants' payloads
     (forward-compatible with Stage 4's L-ENUM-UNION fix — codegen can switch
     from "first non-empty payload" to "union of all payloads" with zero
     MIR data-structure change).
  2. `src/mir/lower/mod.rs`: added `populate_adt_layouts` post-pass at end of
     `lower_hir_body_to_mir_full`. Walks all `local_decls` and all
     `AggregateKind::Adt` field_tys in Assign statements, collecting every
     `TyKind::Adt(def_id, _)` DefId. For each, builds an `AdtLayout` from
     HIR (allowed — data flows downstream per §16.2.1) and inserts it into
     `mir.adt_layouts`. Recursively registers nested Adts (e.g.,
     `struct Outer { i: Inner }` registers both Outer and Inner).
     Uses Entry API (clippy-compliant).
  3. `src/codegen/mod.rs`: replaced `mir_type_to_emit_type_with_hir(ty, hir)`
     with `mir_type_to_emit_type_with_layouts(ty, &mir.adt_layouts)`.
     **Removed** `hir_ty_to_emit_type` entirely (was the §16.3.1-violating
     function that called `lower_hir_ty_to_mir_ty` from codegen). Updated
     all 15+ internal call sites to take `layouts: &AdtLayouts` instead of
     `hir: &HirCrate`. Cleaned up `codegen_lvalue_load` (no longer
     fabricates a fake `MirBody::new(Span::DUMMY)` — passes caller's `mir`
     through directly).
- **Hidden debts also closed** (per §15.2.1):
  * Stage 3.38 silent L-PIPE-1 extension: codegen reading HIR for enum
    storage (added in Stage 3.38 without re-recording the debt) — CLOSED.
  * Stage 3.42 §16.3.1 violation: codegen calling
    `crate::mir::lower::lower_hir_ty_to_mir_ty` from inside
    `hir_ty_to_emit_type`'s Ref/Ptr case — CLOSED (function removed).
  * Stage 3.46 DRY divergence: `hir_ty_to_emit_type` and
    `lower_hir_ty_to_mir_ty` both mapped integer widths but diverged —
    CLOSED (only one source of truth remains).
  * Stage 3.30 `codegen_lvalue_load` MirBody::new hack — CLOSED.
- **Result**: codegen no longer reads HIR for ADT storage. The only HIR
  access remaining in codegen is in `codegen_crate_with_emitter`, which
  uses HIR for the fn-name table and to invoke `lower_hir_body_to_mir_full`
  + `check_mir_body_with_hir` (these are §16.2.1 "data flows downstream"
  and §16.6.1 "driver-layer" uses, not L-PIPE-1 violations).
- 14 new tests: 4 in `mir/body.rs` (AdtLayout construction, idempotency,
  enum variant payloads, empty init), 10 in `tests/codegen_tests.rs`
  (struct/enum param/return/local, nested struct, i128 field, &str field,
  two structs in one fn, tuple struct, struct mutation, root-cause
  verification).
- Total: 855 → 869. (3 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 14 (R14) — 30 audit cases (8 regression + 14 L-PIPE-1
  coverage + 8 edge cases), all passed; audit CONVERGED at round 14 per
  §9.3.3. Process v3.13 first applied (new §18 doc-sync rule).

### Stage 3.48 — L-ENUM-UNION + L-ENUM-BINDING closure (v0.8.6, process v3.13)
- **Problem (two bugs)**:
  1. **L-ENUM-UNION (soundness)**: enum storage layout used "first non-empty
     variant payload" (Stage 3.38 behavior). For `enum E { A, B(i32), C(i64) }`,
     storage was `{ i32, i32 }` (discr + B's i32). Constructing `E::C(42)`
     would write the i64 payload into the i32 slot — **silent memory
     corruption** (writing 8 bytes into a 4-byte slot, overflowing into
     adjacent stack memory).
  2. **L-ENUM-BINDING (P0 soundness, hidden)**: `Opt::Some(x) => x` pattern
     matching allocated a local for `x` but **never assigned it** — the
     binding read uninitialized memory. Pre-existing since Stage 3.40
     (L-ENUM-MATCH), never caught because the existing test
     `codegen_enum_match_two_variants` only asserted `switch i32` is
     present, not that the binding actually receives the payload.
- **Root cause** (per §15 — root cause):
  1. For L-ENUM-UNION: `mir_type_to_emit_type_with_layouts` for
     `AdtLayout::Enum` only included the first non-empty variant's payload
     fields. The `AdtLayout::Enum { variant_payloads }` already stored ALL
     variants' payloads (per Stage 3.47's forward-compatible design), but
     codegen was discarding them.
  2. For L-ENUM-BINDING: `collect_pat_bindings_for_mir` allocated locals
     for `Ident` sub-patterns but generated no projection to extract the
     enum's payload. The binding local was created with a fresh inferred
     type, never written to.
- **Approach chosen** (per §15 — 最优 > 最小): **flat layout** —
  flatten ALL non-empty variants' payload fields into the storage struct.
  Rejected alternatives:
  - "Largest payload width" (Approach B): only works for same-kind integer
    payloads, breaks for mixed types (i32 + f64).
  - "Byte array [N x i8]" (Approach C): loses type info for codegen.
  - "Union struct with per-variant slots" (Approach D, rustc-style): would
    require nested `Field(Field(Local))` projections which codegen doesn't
    handle correctly (loads intermediate as value, not pointer).
  Flat layout keeps all projections single-level (`Field(N, ty)` on Local),
  matching the existing Case A/B behavior and avoiding codegen rework.
- **Fix**:
  1. `src/codegen/mod.rs` — `mir_type_to_emit_type_with_layouts` for
     `AdtLayout::Enum`: flatten ALL variants' payload fields into storage
     (was: only first non-empty). Storage is now:
     - Case A (all unit): `{ discr }` (unchanged)
     - Case B (one non-empty): `{ discr, payload_fields... }` (unchanged)
     - Case C (≥2 non-empty): `{ discr, variant_0_fields..., variant_1_fields..., ... }`
       (NEW — soundness fix; unit variants contribute no fields)
  2. `src/codegen/mod.rs` — `Rvalue::Aggregate(Adt(...))` codegen: for
     enum variants, compute the starting field_idx in the flat layout
     (`1 + sum(field_counts of variants 0..V-1)`) and insert each operand
     at the correct offset. Discriminant goes at field 0.
  3. `src/codegen/mod.rs` — `mir_type_to_emit_type_with_layouts` for
     `Tuple/Array/Ref/RawPtr/Slice`: recurse with `_with_layouts` (was:
     fell through to `mir_type_to_emit_type` which doesn't know about
     AdtLayouts). Fixes a pre-existing bug where nested Adts (e.g., enum
     inside a tuple) collapsed to I32. Exposed by the e07_enum_in_tuple
     audit case.
  4. `src/mir/lower/mod.rs` — new `lower_enum_variant_pattern_bindings`
     function: for `TupleStruct`/`Struct` patterns on enum variants,
     resolve variant_idx + field_tys from HIR, compute the flat field_idx,
     generate `binding_local = Copy(scrut.Field(field_idx, field_ty))`
     assignments. Called alongside `collect_pat_bindings_for_mir` in both
     arm and otherwise-block lowering paths.
  5. `src/mir/lower/mod.rs` — new `compute_enum_payload_starting_idx`
     helper: computes `1 + sum(field_counts of variants 0..V-1)` from HIR.
     Per §16: reads HIR (allowed — data flows downstream per §16.2.1).
- **Result**:
  - `enum E { A, B(i32), C(i64) }` storage is now `{ i32, i32, i64 }`
    (was `{ i32, i32 }` — soundness bug).
  - `E::C(42)` construction: `insertvalue { i32, i32, i64 } undef, i32 2, 0`
    (discr=2) then `insertvalue { i32, i32, i64 } %v1, i64 42, 2` (payload
    at field 2, past B's i32 slot at field 1).
  - `match e { E::C(x) => x, _ => 0 }` extracts i64 from field 2 via
    `getelementptr { i32, i32, i64 }, { i32, i32, i64 }* %loc_1, 0, 2`,
    loads it, stores to `x`'s local. The arm body `x` now reads the actual
    payload, not uninitialized memory.
- 12 new tests: Case C layout/ctor (3), Case C match extraction (2),
  L-ENUM-BINDING verification (1), multi-field variant (2), mixed types
  (1), struct variant binding (1), Case A/B regression (2).
- Total: 869 → 881. (2 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 15 (R15) — 30 audit cases (8 regression + 14 L-ENUM-UNION
  + L-ENUM-BINDING coverage + 8 edge cases), all passed; audit CONVERGED
  at round 15 per §9.3.3. R14 audit case `i14_enum_multiple_variants`
  updated to assert the new correct layout (was: asserted old buggy
  `{ i32, i32 }`; now: asserts `{ i32, i32, i64 }`).

### Stage 3.49 — L13 fat pointer closure (v0.8.6, process v3.13)
- **Problem**: `&str` and `&[T]` references were represented as thin
  pointers (`i8*` for `&str`, `T*` for `&[T]`) — losing the length
  component. This made it impossible to recover the length of a `&str`
  after passing it to a function (the callee only sees the `i8*`).
  Carried as L13 debt since Stage 3.27 (18 rounds). While technically
  a "simplification" rather than a soundness bug, it blocks any
  meaningful string/slice processing (e.g., `str::len()`, bounds checks
  on `&[T]`, `memcmp`-based comparison).
- **Root cause** (per §15 — root cause): `mir_type_to_emit_type` for
  `Ref(_, _, Str)` and `Ref(_, _, Slice(T))` mapped to thin pointers
  (`Ptr(I8)` / `Ptr(T)`). The fat pointer representation (`{ ptr, len }`)
  was documented as "deferred" in Stage 3.27/3.28 but never implemented.
- **Approach chosen** (per §15 — 最优 > 最小): `{ ptr, len }` struct
  representation, matching rustc's fat pointer ABI. The fat pointer is
  `EmitType::Struct(vec![Ptr(elem), I64])`. This is the architecturally
  correct representation — all references to unsized types (`str`, `[T]`)
  carry both data pointer and length. Rejected alternatives:
  - "Two separate params (ptr, len) at call sites": ABI-incompatible
    with `&str` as struct field or enum payload.
  - "Keep thin ptr, add separate length param only when needed":
    requires whole-program analysis to determine which calls need length.
  - "Defer to Stage 4": violates §15.1 — the debt has been carried 18
    rounds, and every new feature (closures, traits) would build on top
    of the wrong ABI.
- **Fix** (3 source files):
  1. `src/codegen/emitter.rs` — added `fat_ptr_type(elem) -> EmitType`
     helper returning `Struct(vec![Ptr(elem), I64])`. Updated
     `mir_type_to_emit_type` for `Ref(_, _, Str)` → `fat_ptr_type(I8)`
     and `Ref(_, _, Slice(T))` → `fat_ptr_type(mir_type_to_emit_type(T))`.
     Added `emit_and` and `emit_or` to the Emitter trait (for fat pointer
     eq/ne comparison).
  2. `src/codegen/mod.rs` — `mir_type_to_emit_type_with_layouts`:
     same fat pointer mapping (recursing with `_with_layouts` for nested
     Adts in the pointee). `codegen_operand` for `ConstVal::Str`: now
     emits a fat pointer value via two `insertvalue` (ptr at field 0,
     len at field 1). `BinaryOp::Eq`/`Ne`: special-cased for fat pointers
     — extract ptr and len from both operands, compare each, AND/OR the
     results (LLVM `icmp` can't compare aggregate types directly).
  3. `src/codegen/text_emitter.rs` — implemented `emit_and` and `emit_or`
     (`and ty lhs, rhs` / `or ty lhs, rhs`).
- **Result**:
  - `fn greet(s: &str)` → `define void @greet({ i8*, i64 } %arg0)` (was
    `i8* %arg0`). The callee can now recover the string length.
  - `"hello"` literal → `insertvalue { i8*, i64 } undef, i8* %ptr, 0`
    then `insertvalue { i8*, i64 } %v, i64 5, 1` (was: just `i8* %ptr`).
  - `s == "hello"` → `extractvalue` ptr/len from both, `icmp eq i8*` +
    `icmp eq i64` + `and i1` (was: invalid `icmp eq { i8*, i64 }`).
  - `struct Msg { text: &str }` → `{ { i8*, i64 } }` (was `{ i8* }`).
  - `&str` in tuple, enum payload, nested struct: all correctly nest
    the fat pointer.
- **Comparison semantics**: fat pointer `==`/`!=` is bitwise (ptr + len),
  not content comparison. `"abc" == "abc"` returns true only if they're
  the same deduped global. Content comparison (memcmp) is deferred —
  requires a runtime function, which Landin doesn't have yet. This
  preserves the existing (unsound) thin-pointer comparison behavior
  while making it valid LLVM.
- 12 new tests: fat ptr param/return/local layout (3), length field
  (3: 5/0/6-byte), construction (1), struct field (1), call ABI (2),
  comparison eq/ne (2), tuple nesting (1), thin ptr regression (1).
- Updated 6 existing tests that asserted old `i8*` representation → now
  assert `{ i8*, i64 }`. Updated R14 (3 cases) and R15 (1 case) audit
  scripts similarly.
- Total: 881 → 893. (3 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 16 (R16) — 30 audit cases (8 regression + 14 fat
  pointer coverage + 8 edge cases), all passed; audit CONVERGED at
  round 16 per §9.3.3.

### Stage 3.50 — Byte string fat pointer fix + comparison pointee type fix (v0.8.6, process v3.13)
- **Problem (two bugs found during Stage 3.49 review)**:
  1. **Byte string regression (P0 soundness)**: `b"hello"` produced
     `Slice(u8)` type in MIR (from Stage 2.4d lower), which codegen
     mapped to thin `i8*` pointer. But Stage 3.49's `ConstVal::Str`
     handler tried to `insertvalue` a length into the thin pointer —
     producing invalid LLVM (`insertvalue i8* undef, i64 5, 1` — i8*
     has no field 1). This was a regression introduced by Stage 3.49's
     fat pointer change: before Stage 3.49, `ConstVal::Str` returned a
     thin `i8*` and worked. After Stage 3.49, it tries to build a fat
     pointer but `Slice(u8)` maps to thin `i8*`, not `{ i8*, i64 }`.
  2. **Fat pointer comparison hardcoded pointee type (latent bug)**:
     Stage 3.49's `BinaryOp::Eq`/`Ne` for fat pointers hardcoded
     `EmitType::ptr_to(EmitType::I8)` for the ptr comparison. This is
     correct for `&str` (pointee is `i8`), but wrong for `&[T]` where
     `T ≠ u8` — would produce `icmp eq i8*` for an `i32*` value, which
     is a type mismatch in typed-pointer LLVM.
- **Root cause** (per §15 — root cause):
  1. For byte strings: MIR lower (`src/mir/lower/mod.rs` line 268)
     produced `TyKind::Slice(Box::new(elem_ty))` for `b"..."` — the
     type is `Slice(u8)`, not `Ref(_, _, Slice(u8))`. But `&[u8]`
     (a reference to a slice) is `Ref(_, _, Slice(u8))`. The lower
     was treating the literal as the slice itself, not a reference
     to it. Codegen's `mir_type_to_emit_type(Slice(u8))` returns
     thin `Ptr(I8)`, while `mir_type_to_emit_type(Ref(_, _, Slice(u8)))`
     returns fat `{ Ptr(I8), I64 }`.
  2. For comparison: the `is_fat_ptr` check identified the struct as
     a fat pointer but discarded the actual field types. The ptr field's
     type (field 0) was available but not used — instead `i8*` was
     hardcoded.
- **Fix** (2 source files):
  1. `src/mir/lower/mod.rs` — `HirLitKind::ByteStr` handling: now
     produces `Ref(_, _, Slice(u8))` (a reference to a slice) instead
     of `Slice(u8)` (the slice itself). This matches Rust's semantics:
     `b"hello"` has type `&'static [u8; N]` which coerces to
     `&'static [u8]`. Codegen now sees `Ref(_, _, Slice(u8))` →
     `fat_ptr_type(I8)` → `{ i8*, i64 }`.
  2. `src/codegen/mod.rs` — `BinaryOp::Eq`/`Ne` fat pointer comparison:
     extract `ptr_field_ty` from the fat pointer's `Struct(fields[0])`
     instead of hardcoding `EmitType::ptr_to(EmitType::I8)`. Use
     `ptr_field_ty` in the `icmp` call for the ptr comparison.
- **Result**:
  - `b"hello"` → `alloca { i8*, i64 }`, `insertvalue { i8*, i64 } undef,
    i8* %ptr, 0`, `insertvalue { i8*, i64 } %v, i64 5, 1` — valid fat
    pointer. No more invalid `insertvalue i8* undef, i64 5, 1`.
  - `a == b` on `&[u8]` → `icmp eq i8*` (correct — the fat pointer's
    field 0 is `Ptr(I8)` for `&[u8]`). For `&[i32]` it would be
    `icmp eq i32*` (derived from field 0, not hardcoded).
  - Byte string dedup with str: `b"hello"` and `"hello"` share the
    same global (same bytes), verified by `b11_bstr_dedup_with_str`.
- 10 new tests: byte string fat pointer layout (3), param/return/call
  ABI (3), struct/tuple nesting (2), comparison (2), dedup (1),
  invalid insertvalue regression (1), pointee type derivation (2).
- Updated 1 existing test: `codegen_byte_string_in_function_with_other_locals`
  — was asserting `alloca i8*` (thin pointer), now asserts
  `alloca { i8*, i64 }` (fat pointer).
- Total: 893 → 902. (2 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 17 (R17) — 30 audit cases (8 regression + 14 byte
  string + comparison coverage + 8 edge cases), all passed; audit
  CONVERGED at round 17 per §9.3.3.

### Stage 3.51 — Slice indexing fix (fat pointer data pointer dereference) (v0.8.6, process v3.13)
- **Problem (P0 soundness)**: `s[0]` where `s: &[i32]` produced wrong
  values. The `Index`/`ConstantIndex` projection codegen GEP'd directly
  into the fat pointer struct (`{ i32*, i64 }`) at field 0, then loaded
  the result as `i32`. This loaded the **data pointer** (`i32*`) and
  reinterpreted its bits as an `i32` element — silently wrong.
- **Root cause** (per §15 — root cause): The `Index`/`ConstantIndex`
  handlers in `codegen_lvalue_load_typed` and `codegen_statement` used
  `detect_lvalue_storage_type(base)` to get the array type for GEP.
  For `[T; N]` arrays, this returns `Array(T, N)` — correct, GEP into
  the array storage. But for `&[T]` slices (fat pointers), this returns
  `Struct([Ptr(T), I64])` — the fat pointer struct, NOT the array. GEP
  into the struct at index 0 gives the data pointer field, not an element.
- **Fix** (3 source files):
  1. `src/codegen/mod.rs` — new `unwrap_fat_ptr_for_index` helper:
     detects if `storage_ty` is a fat pointer (`{ ptr, len }` struct).
     If so, GEPs to field 0 to get the data pointer, returns
     `(data_ptr, Some(pointee_ty))`. If not (array case), returns
     `(base_ptr, None)` unchanged.
  2. `src/codegen/mod.rs` — all 3 `Index`/`ConstantIndex` projection
     sites (load path × 2, store path × 1) now call
     `unwrap_fat_ptr_for_index` and dispatch: fat pointer →
     `emit_gep_index_ptr`, array → `emit_gep_index`.
  3. `src/codegen/emitter.rs` + `text_emitter.rs` — new
     `emit_gep_index_ptr` method: emits
     `getelementptr inbounds <elem_ty>, <elem_ty>* %base, i32 %idx`
     (single-step GEP into a raw element pointer, no array wrapper).
     This is the correct GEP form for slice data pointers (`T*`), as
     opposed to `emit_gep_index` which emits
     `getelementptr inbounds [N x T], [N x T]* %base, i32 0, i32 %idx`
     (two-step GEP into an array pointer).
- **Result**:
  - `s[0]` where `s: &[i32]`:
    ```
    %v2 = getelementptr inbounds { i32*, i64 }, { i32*, i64 }* %loc_1, i32 0, i32 0
        ; GEP to fat pointer field 0 (data pointer)
    %v3 = getelementptr inbounds i32, i32* %v2, i32 0
        ; GEP into data pointer at index 0 (element)
    %v4 = load i32, %v3
        ; load the actual i32 element
    ```
  - `a[1]` where `a: [i32; 3]` (array, unchanged):
    ```
    %v2 = getelementptr inbounds [3 x i32], [3 x i32]* %loc_1, i32 0, i32 1
    %v3 = load i32, %v2
    ```
- **Design note**: the first implementation attempt used a `[0 x T]`
  array type to wrap the slice data pointer for `emit_gep_index`. This
  is invalid LLVM (array length must be > 0). The fix adds a separate
  `emit_gep_index_ptr` method that emits the correct single-step GEP
  for raw element pointers.
- 9 new tests: slice indexing for i32/u8/i64/f64/bool elements (5),
  constant/variable index (2), array regression (2), no-invalid-zero-array (1),
  multiple accesses (2), slice in struct/if/match (3).
- Total: 902 → 911. (3 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 18 (R18) — 30 audit cases (8 regression + 14 slice
  indexing coverage + 8 edge cases), all passed; audit CONVERGED at
  round 18 per §9.3.3.

### Stage 3.52 — Slice element type propagation fix (v0.8.6, process v3.13)
- **Problem (P0 soundness)**: `s[0]` where `s: &[i64]` produced
  `load i32` instead of `load i64` — type mismatch in typed-pointer LLVM.
  Slice element arithmetic (`s[0] + s[1]` on `&[i64]`) used `add nsw i32`
  and `llvm.sadd.with.overflow.i32` instead of i64 — silently wrong
  overflow detection and truncation. Two bugs:
  1. **codegen `detect_lvalue_type`**: for `Index`/`ConstantIndex`
     projections, checked `EmitType::Array(elem, _) => *elem` but fell
     through to `I32` for fat pointers (`Struct([Ptr(T), I64])`). The
     element type was not extracted from the fat pointer's field 0.
  2. **MIR lower `Index` expression**: used `cx.fresh_infer_ty()` for the
     element type (a fresh inference variable), which typeck defaulted to
     `i32`. The temp local storing `s[0]` was typed `i32`, so the store
     truncated the i64 value to i32.
- **Root cause** (per §15 — root cause): Stage 3.51 fixed the GEP (data
  pointer dereference) but didn't fix the element type detection. The
  `detect_lvalue_type` and MIR lower `Index` paths were independent —
  fixing one without the other left the type mismatch. The root cause
  is that slice indexing touches THREE layers (MIR lower type, codegen
  GEP, codegen load type), and Stage 3.51 only fixed the middle one.
- **Fix** (2 source files):
  1. `src/codegen/mod.rs` — `detect_lvalue_type` for `Index`/`ConstantIndex`:
     added a `Struct(fields)` arm that checks for fat pointer shape
     (`fields.len() == 2 && fields[0].is_ptr() && fields[1] == I64`)
     and returns `fields[0].pointee()` (the element type). Falls through
     to `I32` only for non-fat-pointer, non-array cases.
  2. `src/mir/lower/mod.rs` — `Index` expression lowering: replaced
     `cx.fresh_infer_ty()` with `resolve_index_element_type(cx, base_local)`,
     which inspects the base's MIR type to compute the element type:
       - `&[T]` (Ref to Slice(T)) → T
       - `[T; N]` (Array(T, _)) → T
       - `&[T; N]` (Ref to Array(T, _)) → T
     Falls back to `fresh_infer_ty` if the base type can't be resolved
     (preserves old behavior for test contexts).
     Per §16: reads MIR local_decls only (data flows downstream per
     §16.2.1 — MIR lower reads its own body). No HIR lookup.
- **Result**:
  - `s[0]` where `s: &[i64]` → `load i64` (was: `load i32`).
  - `s[0] + s[1]` where `s: &[i64]` → `add nsw i64` + `llvm.sadd.with.overflow.i64`
    (was: `add nsw i32` + i32 overflow check — wrong width, truncation).
  - `s[0] = 42` where `s: &mut [i64]` → `store i64 42` (was: `store i32 42`).
  - `s[0] > s[1]` where `s: &[i64]` → `icmp sgt i64` (was: `icmp sgt i32`).
  - Array indexing `[i64; 3]` unchanged (no regression — already used
    `resolve_index_element_type`'s Array arm).
- 9 new tests: i64/i32/i128/f64 load type (4), i64/i32/f64 arithmetic (3),
  i64 store (1), i64 comparison (1), array regression (1).
- Total: 911 → 920. (2 source files modified; 0 typeck/borrowck changes.)
- Gate review Round 19 (R19) — 30 audit cases (8 regression + 14 element
  type propagation + 8 edge cases), all passed; audit CONVERGED at
  round 19 per §9.3.3.

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
| v0.8.6 (3.34-3.35) | 788 | +8 (L-MUT-1 field mutation + R6) |
| v0.8.6 (3.36-3.37) | 796 | +8 (L-DEBT-3 field type propagation through arithmetic + R7) |
| v0.8.6 (3.38-3.39) | 806 | +10 (L-ENUM enum variant codegen + R8) |
| v0.8.6 (3.40-3.41) | 814 | +8 (L-ENUM-MATCH enum match via discriminant extraction + R9) |
| v0.8.6 (3.42) | 820 | +6 (&str type fix: string literals now have type &'static str) |
| v0.8.6 (3.43) | 828 | +8 (L11 shift-count overflow check via icmp uge) |
| v0.8.6 (3.44) | 836 | +8 (const/static value resolution — inline initializer) |
| v0.8.6 (3.45) | 842 | +6 (L10 float bitwise ops via cast to int) |
| v0.8.6 (3.46) | 855 | +13 (L14+L9 full integer type support: i8/i16/i32/i64/i128/usize/isize) |
| v0.8.6 (3.47) | 869 | +14 (L-PIPE-1 closure via AdtLayout side-table on MirBody, per §16) |
| v0.8.6 (3.48) | 881 | +12 (L-ENUM-UNION + L-ENUM-BINDING closure: flat enum storage layout + pattern binding extraction)
| v0.8.6 (3.49) | 893 | +12 (L13 fat pointer closure: &str/&[T] now { ptr, len } struct, not thin pointer)
| v0.8.6 (3.50) | 902 | +10 (byte string fat pointer fix + comparison pointee type fix)
| v0.8.6 (3.51) | 911 | +9 (slice indexing fix: fat pointer data pointer dereference)
| v0.8.6 (3.52) | 920 | +9 (slice element type propagation: load/store/arith use correct element type from fat pointer)
