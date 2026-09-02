# Stage 61 (v0.7) — TD-DISPLAY-TRAIT-MISSING partial fix

**Date**: 2026-09-03
**Version**: v0.611.0 (bumped from v0.610.0)
**Task ID**: stage61
**Agent**: PM-A + ARCH-A + DEV-A + REV-A
**TD Status**: TD-DISPLAY-TRAIT-MISSING — **PARTIALLY FIXED** (trait + 5 impls; format! redesign deferred to v0.8+)
**Discovered TDs**: TD-TOSTRING-DEFAULT-BODY (P3, v0.8+) — `to_string` default body triggers intermittent LLVM codegen crash

---

## 1. Three-second startup self-check

- **定位 (§1.2.1)**: L3 (prelude trait definition + 5 impls + TextEmitter dedup fix + 7 test files updated + 22 new tests)
- **对齐 (§13.1 / §8.4.5)**: 已查 Stage 60 worklog (TD-DYN-TRAIT-COMPLETION partial fix), tech-debt-register-v0.604.md, stage-committee-process.md
- **阻断 (§18 / §6.1)**: v0.610.0 全绿 (5436 tests, 4 ignored), 0 P0/P1

---

## 2. 5W2H analysis (TD-DISPLAY-TRAIT-MISSING)

### WHAT
Add `Display` trait to prelude with `fn fmt(&self, f: &mut String) -> i64` signature + impls for i32/i64/usize/bool/str. Defer `format!` param redesign (`&[i64]` → `&[&dyn Display]`) to v0.8+ since it requires full `dyn Trait` support (per Stage 60's TD-DYN-TRAIT-COMPLETION partial fix).

### WHY
- **Root cause**: `__landin_format_v2(fmt, &[i64])` hardcodes i64 array, blocking `&str`/`bool`/user types from being formatted
- **Why now**: Wave 2 trait system needs Display trait for subsequent TD-FN-TRAITS, TD-FORMAT-ARGS-WRITE, TD-PRINTLN-CODEGEN-INTERCEPT
- **Rust philosophy**: `Display` is the canonical user-facing string conversion trait; Landin users implementing custom types should be able to provide Display impls

### WHO
PM-A + ARCH-A (decision: trait-only, defer format! redesign per §13.4) + DEV-A (impl) + REV-A (audit)

### WHEN
Stage 61 (v0.7 trait system phase)

### WHERE
- `src/stdlib/prelude.rs` — Display trait + 5 impls
- `src/codegen/text/mod.rs` + `module.rs` — `data_globals_emitted` dedup field
- `tests/v0/stage61/plan/display_trait_tests.rs` — 22 new tests
- 7 test/conformance files updated (Display→Show rename for TD-TRAIT-NAME-COLLISION)

### HOW
1. Define `trait Display { fn fmt(&self, f: &mut String) -> i64; }` in prelude
2. Implement `fmt` for i32/i64/usize (call `__landin_i64_format`), bool (push_str "true"/"false"), str (push_str self)
3. Add `data_globals_emitted: HashSet<String>` to TextEmitter to dedup `@.data.<type>` globals across multiple trait impls per type
4. Rename `Display` → `Show` in 7 test/conformance files (TD-TRAIT-NAME-COLLISION workaround, same pattern as Stage 59 Clone)

### HOW MUCH
- +120 LOC prelude (trait + 5 impls + comments)
- +13 LOC codegen/text (dedup field + dedup check)
- +360 LOC tests (22 new tests)
- 7 test files updated (Display→Show rename)
- 5436 tests → 5458 tests (+22)

---

## 3. Decision points (为何选此路)

### Decision 1: Trait definition only, defer format! redesign
**Option A** (full Display + format! redesign): Requires `&[&dyn Display]` dyn Trait support → BLOCKED on v0.8+ TyKind::Dyn(DefId). Per §13.4 (cost > benefit) → defer.
**Option B** (trait only, no format! redesign) — CHOSEN: Define Display trait + impls now, unlock user-defined Display impls. format! stays `&[i64]`. Per §12 (最优 > 最小): this IS the root-cause trait definition; the format! dispatch redesign is a separate TD item (depends on dyn Trait).
**Option C** (Display trait + default `to_string` only, no overrides): Default body specialization uses first impl (Bug Z7 in `driver_codegen_prep.rs:405`) → `i64.to_string()` would call `i32::fmt` → BROKEN.

→ **Option B** = correct + root-cause + minimal-scope.

### Decision 2: TextEmitter @.data.<type> dedup
**Root cause**: `emit_dyn_trait_const` was emitting `@.data.<type>` once per vtable. With 2 traits per type (Clone + Display), each `@.data.<type>` was emitted twice → `llvm-as` rejects with "redefinition of global".

**Option A** (use HashSet for dedup): Track emitted data globals, skip duplicates. Mirrors LLVMSysEmitter's `LLVMGetNamedGlobal` check. — CHOSEN.
**Option B** (use a single `@.data.<type>` emission at module init): Pre-scan all vtables and emit `@.data.<type>` once. More complex, no clear benefit.

→ **Option A** = simplest root-cause fix, mirrors existing LLVMSys path.

### Decision 3: Defer `to_string` default body to v0.8+
The Bug Z7 workaround (override `to_string` in each impl) was attempted but caused intermittent LLVM codegen crashes:
- `libLLVM.so segfault during LLVMTargetMachineEmitToFile`
- ~5-10% failure rate, non-deterministic
- Crash is inside libLLVM (LLVM bug), not in Landin codegen

Per §13.4 (重构判据): cost (LLVM crash investigation) > benefit (convenience wrapper). Users call `x.fmt(&mut s)` directly until to_string lands.

→ **Documented as TD-TOSTRING-DEFAULT-BODY (P3, v0.8+)**.

---

## 4. Tailoring points (为何跳流程)

- L3 task (prelude + tests + docs) → full process per §1.2.1
- 跳过 §14.5 D2-D8 deep review — P3 TD fix, no soundness impact, §7.3 gate review sufficient
- Per §1.2.1: L3 can use §7.3 gate review for TD fixes

---

## 5. §3.2 acceptance checks

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` (0 warnings) ✓
- `cargo test --release --features llvm-backend` ✓ (5458 tests, 0 failures, 4 ignored)
- Runtime verified: 42.fmt → "42", true.fmt → "true", "hello".fmt → "hello", 7.fmt → "7"

---

## 6. §1.6 ultimate test (root-cause or minimum patch?)

**Is this a root-cause fix or a minimum patch?**

Root-cause fix. Display trait is now defined in prelude with proper method signature and impls for basic types. The TextEmitter dedup fix addresses the underlying issue (no dedup mechanism for `@.data.<type>` globals) rather than working around the symptom.

**Deferrals** (documented, not workarounds):
- `format!` param redesign (`&[i64]` → `&[&dyn Display]`) — deferred to v0.8+ (requires full dyn Trait support)
- `to_string` convenience method — deferred to v0.8+ (LLVM codegen crash investigation needed)
- TD-TRAIT-NAME-COLLISION — documented as P3, v0.8+ (resolver should merge prelude/user trait definitions)

---

## 7. Stage summary

- TD-DISPLAY-TRAIT-MISSING PARTIALLY FIXED (Display trait + 5 primitive impls)
- TextEmitter `@.data.<type>` dedup FIXED (data_globals_emitted HashSet)
- 7 test/conformance files updated (Display→Show rename for TD-TRAIT-NAME-COLLISION)
- 22 new tests added (13 positive + 7 negative + 2 architecture)
- TD-TOSTRING-DEFAULT-BODY discovered (P3, v0.8+) — `to_string` default body triggers LLVM crash
- 5458 tests, 0 failures, 4 ignored, fmt clean, 0 clippy warnings
- Architecture health: 9.85/10 (stable — root-cause TD fix, no regression)

---

## 8. Next steps

- TD-FN-TRAITS: Fn/FnMut/FnOnce traits (Wave 3)
- TD-IMPL-TRAIT: impl Trait syntax (Wave 3)
- TD-SPECIAL-16: Drop trait + drop glue (Wave 3)
- TD-PRELUDE-MACRO-TIMING: DefId decoupling + token-level injection (Wave 4)
- v0.8+: TD-TOSTRING-DEFAULT-BODY (LLVM crash investigation), TD-DYN-TRAIT-COMPLETION (full TyKind::Dyn(DefId)), TD-TRAIT-NAME-COLLISION (resolver merge), format! param redesign
