# Gate Review — Stage 13.17: Self Binding Fix + Inherent Method Call Codegen

> **Reviewer**: REV-A (automated)
> **Date**: 2026-07-27
> **Process**: stage-committee-process.md v3.21 §9.3
> **Baseline**: v0.25.0 / 2333 rust tests + 5026 conformance
> **Target**: v0.25.1 (patch bump — bug fixes + partial method call support)
> **Status**: ✅ PASS (7/7 GO)

## 1. Stage Summary

Stage 13.17 fixes two P0 bugs discovered during systematic audit:

**Bug A**: `self` not resolved in impl method bodies — parser used `Spur::default()` instead of interning "self". Fixed by using `self.interner.get_or_intern("self")`.

**Bug B**: Inherent method calls (`p.get()`) dropped from codegen — MIR lower emitted `Const{ty: Error}` placeholder. Fixed by adding `resolve_inherent_method` + `resolve_inherent_method_from_hir_expr` to resolve the method DefId via HIR impl lookup, then emitting a real `Terminator::Call` with `Const{ty: FnDef(def_id), val: Uint(def_id)}`.

**Known limitation**: Methods that access `self.x` (field access on self) still crash because the self parameter's MIR type is `Infer` (not `Adt`), causing codegen to emit invalid GEP. This is a deeper typeck writeback issue deferred to Stage 13.18.

## 2. Committee Vote

**Tally: 7/7 GO → PASS**

## 3. Behavioral Verification

- ✅ `p.get()` where `get(self) -> i32 { 42 }` (no self access) → `get=42`
- ✅ `self` resolves in method bodies (no more "cannot find value in this scope")
- ⚠️ `self.x` field access crashes (Stage 13.18 — typeck writeback needed)

## 4. Acceptance Criteria

- [x] Parser uses `get_or_intern("self")` not `Spur::default()`
- [x] MIR lower resolves inherent methods via HIR impl lookup
- [x] Method calls emit real `Terminator::Call` (not Error placeholder)
- [x] `p.get()` with no self access works end-to-end
- [x] CI/CD all green
- [ ] `self.x` field access works (deferred to Stage 13.18)

## 5. Final Verdict

**Stage 13.17 GATE: ✅ PASS** (with documented limitation)
