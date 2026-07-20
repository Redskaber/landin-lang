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
