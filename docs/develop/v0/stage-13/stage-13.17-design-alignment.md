# Stage 13.17 — §13.4 Design Alignment: Self Binding Fix + Inherent Method Call Codegen

> **Author**: redskaber
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 (§13.4 + §14.4 + §25.8)
> **Baseline**: v0.25.0 / 2333 rust tests + 5026 conformance (Stage 13.16 ✅ format args)
> **Version policy**: v0.25.0 → v0.25.1 (patch bump — bug fixes + method call codegen)
> **Status**: 🔄 Active — fixes two P0 bugs discovered during systematic audit

---

## 1. Background & Problem Statement

### 1.1 Systematic Audit Findings

A systematic audit of the v0.25.0 compiler tested diverse Landin programs and found two P0 bugs that block v0.1 release:

**Bug A: `self` not resolved in impl method bodies**
```rust
struct P { x: i32 }
impl P { fn get(&self) -> i32 { self.x } }
// ERROR: [resolve] cannot find value in this scope (at `self`)
```

**Root cause**: `src/parser/generics.rs:63,74` used `Spur::default()` (empty spur) for the `self` parameter's binding name, instead of interning the string `"self"`. When `self.x` was resolved in the method body, the path segment for `self` had a different spur (the interner's key for `"self"`), so the scope lookup failed.

**Bug B: Inherent method calls (`p.get()`) dropped from codegen**
```rust
let p = P { x: 42 };
let r = p.get();  // r is always 0; landin_get is never called
```

**Root cause**: `src/mir/lower/expr_operand.rs:1635-1659` (the `HirExprKind::MethodCall` legacy placeholder path) emits `Terminator::Call { func: Const{ty: Error, val: Int(0)}, ... }` — a placeholder that produces an Error-typed call. Codegen sees `ty: Error` and (since it's not a dyn Trait marker) drops the call entirely.

### 1.2 Impact Assessment

**Bug A impact**: ALL struct/trait methods that access `self` fail to compile. This breaks 229 conformance tests that use `impl ... fn ... &self` — except they're masked because most conformance tests have empty method bodies or don't access self.

**Bug B impact**: ALL inherent method calls (`p.get()`, `p.set(5)`, etc.) produce wrong results (always 0 or unit). This makes struct methods useless — the primary OOP abstraction in Landin.

Together, these bugs mean **struct methods don't work at all**, which is a P0 v0.1 release blocker.

### 1.3 §15 Long-Term vs Short-Term Analysis

| Option | Long-term value | Short-term cost | Decision |
|--------|----------------|----------------|----------|
| A: Fix only Bug A (self binding) | MEDIUM — unblocks resolution; method calls still broken | LOW — 1 file, 5 LOC | ❌ INSUFFICIENT (Bug B remains) |
| **B: Fix Bug A + Bug B (full method call support)** | **HIGH** — struct methods work end-to-end; closes P0 v0.1 blocker | **MEDIUM** — 2 files, ~80 LOC; requires HIR query for impl lookup | ✅ **ADOPTED** |
| C: Defer to v0.2 macro_rules! | LOW — leaves v0.1 without method calls | ZERO | ❌ REJECTED (P0 v0.1 blocker) |
| D: Full trait method resolution (rustc-style) | HIGHEST — proper trait dispatch | HIGH — ~500+ LOC; requires trait coherence + vtable | ❌ DEFERRED (Stage 13.18+) |

**Conclusion**: Strategy B (fix Bug A + Bug B) is the right call:
- Closes the P0 v0.1 blocker (struct methods work)
- Minimal architectural change (additive to existing MethodCall arm)
- Forward-compatible with v0.2 trait resolution
- Per §15: long-term > short-term; per user feedback: fewer special cases (removes the Error placeholder)

---

## 2. §13.4 Design Alignment Verification

### 2.1 Design Doc Survey

| Design doc | Relevant section | Alignment verdict |
|------------|------------------|-------------------|
| `02-grammar.md` | `self` parameter syntax; method call syntax | ✅ ALIGNED |
| `05-ast.md` | `Param.is_self`, `SelfKind` | ✅ ALIGNED |
| `07-codegen.md` §8.1 | Codegen translates MIR functions | ✅ ALIGNED |
| `09-stdlib.md` | Methods on types | ✅ ALIGNED |

### 2.2 Design-Deviation Classification

- **B1-B4**: NONE — no new types, no new fields, no new design surface. Stage 13.17 fixes bugs in existing code.

### 2.3 §14.4 Six Refactoring Criteria (J1-J6)

| Criterion | Verdict | Rationale |
|-----------|---------|-----------|
| J1 Architectural alignment | ✅ PASS | Fixes bugs; removes Error placeholder special case |
| J2 Single responsibility | ✅ PASS | Method call resolution is one job |
| J3 Unidirectional data flow | ✅ PASS | HIR → MIR lower → codegen (forward) |
| J4 Compile-time expressiveness | ✅ PASS | No new types |
| J5 Stage partition (≤5 src files) | ✅ PASS | 2 src files: parser/generics.rs, mir/lower/expr_operand.rs |
| J6 Scientific granularity | ✅ PASS | Two targeted bug fixes |

---

## 3. Implementation Blueprint (Strategy B)

### 3.1 Bug A Fix: Self Binding (parser/generics.rs)

Replace `Spur::default()` with `self.interner.get_or_intern("self")` for the binding name, and `self.interner.get_or_intern("Self")` for the type name. This ensures the resolver's scope lookup matches `self.x` references in method bodies.

### 3.2 Bug B Fix: Inherent Method Call Codegen (mir/lower/expr_operand.rs)

In the `HirExprKind::MethodCall` legacy placeholder path (line 1635), replace the Error placeholder with real method resolution:

1. Get the receiver's type: `cx.mir.local(recv_local).ty`
2. If it's `TyKind::Adt(adt_def_id, _)`, query HIR for an impl block on that type
3. Search the impl's items for `HirImplItem::Fn(f)` where `f.ident.name == method.name`
4. If found, emit `Terminator::Call { func: Const{ty: FnDef(def_id, []), val: Uint(def_id)}, args: [self, ...args], ... }`
5. If not found, fall back to the Error placeholder (graceful degradation)

### 3.3 API Naming Compliance

No new public API. The fix uses existing `TyKind::FnDef`, `Const`, `Terminator::Call` — all already in the MIR surface.

---

## 4. Verification Plan

### 4.1 Behavioral Smoke Tests

```bash
# Test 1: self by value, returns literal
echo 'struct P { x: i32 } impl P { fn get(self) -> i32 { 42 } } fn main() -> i32 { let p = P { x: 1 }; println!("get={}", p.get()); 0 }' > /tmp/t.lin
./target/debug/landin-stage0 --run /tmp/t.lin
# Expected: get=42

# Test 2: self.x field access
echo 'struct P { x: i32 } impl P { fn get(&self) -> i32 { self.x } } fn main() -> i32 { let p = P { x: 42 }; println!("get={}", p.get()); 0 }' > /tmp/t.lin
./target/debug/landin-stage0 --run /tmp/t.lin
# Expected: get=42

# Test 3: self by value with field access
echo 'struct P { x: i32, y: i32 } impl P { fn sum(self) -> i32 { self.x + self.y } } fn main() -> i32 { let p = P { x: 10, y: 20 }; println!("sum={}", p.sum()); 0 }' > /tmp/t.lin
./target/debug/landin-stage0 --run /tmp/t.lin
# Expected: sum=30
```

### 4.2 New Stage 13.17 Verification Tests (6 tests)

1. `test_parser_interns_self_name` — parser uses `get_or_intern("self")` not `Spur::default()`
2. `test_mir_lower_method_call_resolves_inherent` — MethodCall arm resolves inherent methods (not Error placeholder)
3. `test_stage_13_17_design_alignment_exists`
4. `test_stage_13_17_gate_review_exists`
5. `test_v01_gate_still_holds`
6. `test_no_method_call_error_placeholder` — MethodCall arm doesn't use `Const{ty: Error}` for inherent methods

---

## 5. Version Policy

**v0.25.0 → v0.25.1** (patch bump) — bug fixes (self binding + method call codegen); no new language feature.

---

## 6. Risk Assessment

| Risk | Probability | Impact | Mitigation |
|------|------------|--------|------------|
| self.x field access codegen broken (GEP on wrong type) | MEDIUM | MEDIUM | Test with struct field access; may need follow-up |
| Method overload resolution (multiple impls) | LOW (v0.1 has no overloading) | LOW | First-match wins; document as v0.1 limitation |
| Trait method calls (not inherent) still broken | HIGH (out of scope) | MEDIUM | Document as known limitation; Stage 13.18+ |

---

## 7. Stage Committee Recommendation

**GO** — proceed with implementation.

---

## 8. References

- `stage-committee-process.md` v3.21 §13.4, §14.4, §15, §16, §25.8
- `src/parser/generics.rs:51-85` (self param parsing — Bug A fix target)
- `src/mir/lower/expr_operand.rs:1635-1659` (MethodCall legacy placeholder — Bug B fix target)
- `src/hir/kinds.rs:342-352` (HirImpl struct for impl lookup)
- `src/driver.rs:483` (fn_name generation: `landin_<Type>_<method>`)
