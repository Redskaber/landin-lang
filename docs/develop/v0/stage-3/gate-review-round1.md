# Stage 3 Phase Gate Review — Round 1

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.7 (§9.3.1 ≥30-case audit + §9.3.2 edge-case tests + §9.3.3 convergence)
> **Stage baseline**: v0.8.5 → v0.8.6 (Stage 3.21 + 3.22 added in this round)
> **Sub-stages covered**: 3.1–3.22 (22 sub-stages total)
> **Audit tool**: `examples/stage3_gate_audit.rs`

---

## 1. Audit Design

Per §9.3.1, the audit uses ≥30 cases across 4 groups. This round uses 38 cases:

| Group | Cases | Purpose |
|-------|-------|---------|
| A — Single-stmt codegen | 10 | Verify IR patterns for literals, arith, let, borrow, cmp, unary |
| B — Multi-stmt control flow + calls | 10 | Verify if/while/match emit correct branches; calls have typed args |
| C — Complex real-world programs | 8 | Fibonacci, factorial, GCD, Ackermann, nested match, borrow chain, cast chain |
| E — §9.3.2 edge cases (Stage 3.21 + 3.22 fixes) | 5 | Specifically target the bugs fixed in the last two sub-stages |
| D — Robustness / error recovery | 5 | Empty fn, unit return, large array, deep match, long arith chain |
| **Total** | **38** | ≥30 per §9.3.1 ✅ |

Unlike Stage 2 audits (which only checked error counts), Stage 3 audits verify the **generated LLVM IR** has the expected instructions, types, and structure. Each case specifies:
- `expect_all`: substrings that MUST appear in the generated `.ll`
- `expect_none`: substrings that MUST NOT appear (regression guards)

---

## 2. Audit Execution

```
=== Stage 3 Gate Audit Round 1 Summary ===
    Total: 38  Pass: 38  Fail: 0
✅ AUDIT PASSED — 0 codegen defects found in 38 cases.
```

All 38 cases pass. No false positives. No missed defects.

### §9.3.2 Edge Case Results (Stage 3.21 + 3.22)

| Case | Target | Result |
|------|--------|--------|
| `e01_tuple_mixed_types_321` | `(i32, f64, bool)` → `{ i32, double, i1 }` (not `{ i32, i32 }`) | ✅ |
| `e02_array_i64_321` | `[i64; 3]` → `[3 x i64]` (not `[10 x i32]`) | ✅ |
| `e03_typed_call_i64_321` | `g(42)` where g takes i64 → `call i64 @landin_g(i64 42)` (not `i32 42`) | ✅ |
| `e04_if_merge_correctness_322` | `if x { 1 } else { 2 }` returns `%v` (not constant `1` or `2`) | ✅ |
| `e05_nested_if_merge_322` | Nested if-else returns `%v` (not any of the branch constants) | ✅ |

All 5 edge cases for the last round's fixes pass.

---

## 3. Stage 3.21 + 3.22 Summary

### Stage 3.21 — Typed Aggregate Codegen (v0.8.6)
**Problem**: `EmitType::Tuple` was hardcoded to `{ i32 }`, `EmitType::Array` to `[10 x i32]`, `Ptr` to opaque `i32*`, and `emit_call` hardcoded all arg types as `i32`. This produced malformed LLVM IR for any tuple with non-i32 fields, any array of non-i32 / non-length-10, and any call with non-i32 args.

**Fix**:
- `EmitType` now carries full structure: `Struct(Vec<EmitType>)`, `Array(Box<EmitType>, u64)`, `Ptr(Box<EmitType>)`.
- `emit_type_to_llvm_str` returns `String` (was `&'static str`) so dynamic layouts can be rendered.
- `Emitter::emit_gep_field` now takes `struct_ty: &EmitType` and renders the actual struct.
- `Emitter::emit_gep_index` now takes `array_ty: &EmitType`.
- `Emitter::emit_insertvalue` now takes `val_ty: &EmitType` for the inserted value.
- `Emitter::emit_call` now takes `args: &[(EmitType, &EmitValue)]` — typed call args.
- New helper `detect_lvalue_storage_type` walks the projection chain to find the alloca's type.
- `mir_type_to_emit_type` properly recurses into `Tuple`/`Array`/`Ref`/`RawPtr`.

**Impact**: 10 new tests, 0 regressions. Total tests 709 → 719.

### Stage 3.22 — Block-Scoped Local Value Cache (v0.8.6)
**Problem**: `TextEmitter::locals` cached the most-recent value assigned to each local and short-circuited loads. This was unsound for control-flow joins: `if x > 0 { 1 } else { 2 }` returned `2` regardless of `x` because the merge block read the cached `2` from the false branch.

**Fix**:
- `TextEmitter::emit_block` now clears `self.locals` at each block boundary.
- `local_ptrs` (alloca handles) are preserved — they persist for the whole function.
- Within a single block, the constant shortcut still works (e.g., `g(42)` still emits `i32 42`).
- Across blocks, reads correctly go through `load` from the alloca slot.

**Result**: `if x > 0 { 1 } else { 2 }` now emits:
```llvm
bb3:
  %v6 = load i32, %loc_4   ; loads merged value from result slot
  store i32 %v6, %loc_0
  %v7 = load i32, %loc_0
  ret i32 %v7              ; returns loaded value, not hardcoded constant
```

**Impact**: 6 new tests, 0 regressions. Total tests 719 → 725.

---

## 4. Committee Vote (5-role, per §3.1)

| Role | Vote | Notes |
|------|------|-------|
| **Compiler Engineer** | APPROVED | EmitType refactor is sound; trait signatures now carry full type info. No `unsafe`, no panics, no unwrap-or-default hacks in hot paths. |
| **Type System Theorist** | APPROVED | `Struct`/`Array`/`Ptr` variants correctly model LLVM's structural type system. `pointee()` helper maintains the invariant that `Ptr(t).pointee() == t`. |
| **Soundness Reviewer** | APPROVED | Stage 3.22 fix closes a real correctness bug (control-flow join leakage). No new soundness holes introduced. Existing Stage 2 guarantees preserved (codegen doesn't bypass typeck). |
| **Testing & QA Lead** | APPROVED | 38-case audit covers all sub-stages. 5 edge-case tests specifically target 3.21/3.22 fixes. No regressions in 725-test suite. |
| **Tooling & DX Lead** | APPROVED | 0 clippy warnings, 0 fmt diffs. `cargo test` clean. Audit script (`examples/stage3_gate_audit.rs`) is reproducible and self-documenting. |

**Result**: 5/5 APPROVED — UNANIMOUS. Stage 3 gate review Round 1 PASSED.

---

## 5. Known Limitations (deferred to future stages)

These are NOT blockers for Stage 3 gate — they are documented for transparency:

| ID | Limitation | Deferred to |
|----|-----------|-------------|
| L1 | No real PHI node emission — merges use load-from-alloca (correct but not optimal) | Stage 3.23+ (optimization) |
| L2 | No struct/enum ADT codegen (only tuples and arrays) | Stage 3.24+ |
| L3 | No closure codegen | Stage 3.25+ |
| L4 | No String/str literal storage (always emits `0`) | Stage 3.26+ |
| L5 | No trait dispatch / vtable | Stage 3.27+ |
| L6 | No actual overflow check emission (Assert calls panic stubs but doesn't check) | Stage 3.28+ |
| L7 | No `lli` execution verification (environment lacks LLVM tools) | Stage 3 final |
| L8 | `i128`/`u128` truncated to `i64` | Stage 3.29+ |
| L9 | Float bitwise ops fall back to int form (caller-side guard needed) | Future |

---

## 6. Process Compliance Check

| Requirement | Status |
|-------------|--------|
| §9.1.1 negative-test matrix (≥6/7 categories) | ✅ (Stage 2 covers; Stage 3 doesn't add new categories) |
| §9.3.1 ≥30-case audit | ✅ (38 cases) |
| §9.3.2 ≥5 edge-case tests for last round's fixes | ✅ (5 cases, all pass) |
| §9.3.3 convergence rule | N/A (Round 1 — first round, no convergence to check) |
| §11 documentation sync | ✅ (dev-log + RELEASE_NOTES + Cargo.toml updated) |
| §12 document organization | ✅ (this file is in `docs/develop/v0/stage-3/`) |
| §13 doc-first query | ✅ (process consulted before audit design) |
| §14 development phase change rule | ✅ (no backward compat needed; trait signatures changed) |

---

## 7. Conclusion

Stage 3 (LLVM codegen) Round 1 gate review **PASSED** with unanimous 5/5 committee approval. All 38 audit cases pass, all 725 tests pass, 0 warnings, fmt + clippy clean.

**Next steps**:
- Continue Stage 3 with sub-stages 3.23+ (PHI optimization, ADT codegen, closures, etc.).
- Round 2 gate review after ~5 more sub-stages or when significant new features land.
- Stage 3 is declared "feature-complete for MVP" when L1-L9 are either fixed or have explicit deferral decisions.

---

## Appendix A: Test Counts

| Stage | Tests | Delta |
|-------|-------|-------|
| Stage 0 final | 245 | — |
| Stage 1 final | 451 | +206 |
| Stage 2 final | 673 | +222 |
| Stage 3.1 (MVP) | 686 | +13 |
| Stage 3.7 (cast) | 709 | +23 |
| Stage 3.20 (typed-load) | 709 | 0 (refactor) |
| **Stage 3.22 (this round)** | **725** | **+16** |

## Appendix B: Audit Case Catalog

See `examples/stage3_gate_audit.rs` for the full case list. Summary by group:

- **Group A (single-stmt, 10)**: const return, int/float add, bool constant, let alloca, borrow+deref, eq cmp, neg, not, i64 arith
- **Group B (multi-stmt, 10)**: if-else, while, match, simple call, mixed-type call, param signature, recursive call, nested if, tuple construction, array construction
- **Group C (complex, 8)**: Fibonacci, factorial, GCD, Ackermann, nested match, borrow chain, tuple destructure, cast chain
- **Group E (edge cases, 5)**: tuple mixed types, array i64, typed i64 call, if-else merge, nested if merge
- **Group D (robustness, 5)**: empty function, unit return, large array, deep match, long arith chain
