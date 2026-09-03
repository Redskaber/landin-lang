# Stage 64 (v0.7) — TD-SPECIAL-16 Drop trait in prelude

**Date**: 2026-09-03
**Version**: v0.614.0 (bumped from v0.613.0)
**Task ID**: stage64
**Agent**: PM-A + ARCH-A + DEV-A + REV-A
**TD Status**: TD-SPECIAL-16 — **FIXED** (Drop trait added to prelude; drop glue infrastructure was already complete from Stage 15.x)
**Discovered TDs**: TD-MEM-DROP (P3, v0.8+) — `mem::drop()` explicit drop function not yet implemented

---

## 1. Three-second startup self-check

- **定位 (§1.2.1)**: L2 (prelude trait definition + 15 new tests + 13 test files updated for TD-TRAIT-NAME-COLLISION)
- **对齐 (§13.1 / §8.4.5)**: 已查 Stage 63 worklog, TD register (merged), lang-design/25-drop-elaboration.md, graph/borrowck/data-flow.md
- **阻断 (§18 / §6.1)**: v0.613.0 全绿 (5482 tests), 0 P0/P1

---

## 2. 5W2H analysis (TD-SPECIAL-16)

### WHAT
Add `Drop` trait to prelude: `trait Drop { fn drop(&mut self); }`. The drop glue infrastructure (drop_elaboration.rs + drop_glue.rs + is_drop_builtin + TerminatorKind::Drop) was already fully implemented in Stage 15.x — only the prelude declaration was missing.

### WHY
- **Root cause**: Drop trait was not in prelude; users had to manually declare `trait Drop { fn drop(&mut self); }` before implementing it
- **Why now**: Wave 3 TD item; the infrastructure is complete, only the prelude definition was missing
- **Rust philosophy**: Rust has `std::ops::Drop` in the prelude — Landin should mirror this

### WHO
PM-A + ARCH-A + DEV-A

### WHEN
Stage 64 (v0.7 trait system phase)

### WHERE
- `src/stdlib/prelude.rs` — Drop trait definition (3 lines + comments)
- 13 test files updated (removed `trait Drop` declarations)
- `tests/v0/stage64/plan/drop_trait_tests.rs` — 15 new tests
- `docs/develop/v0/stage-64/` — design doc

### HOW
1. Add `trait Drop { fn drop(&mut self); }` to prelude after FnOnce trait
2. Remove `trait Drop { fn drop(&mut self); }` declarations from 13 test files (TD-TRAIT-NAME-COLLISION workaround, same pattern as Stage 59 Clone→Show, Stage 61 Display→Show)
3. Write 15 tests verifying Drop works from prelude without user declaration

### HOW MUCH
- +20 LOC prelude (Drop trait + comments)
- 13 test files updated (removed trait Drop declarations)
- +350 LOC tests (15 new tests)
- 5482 tests → 5496 tests (+14 passing, +1 ignored)

---

## 3. Decision points (为何选此路)

### Decision 1: Add to prelude (not keep user-declared)
**Option A** (add Drop to prelude) — CHOSEN: Mirrors Rust, eliminates user boilerplate. The infrastructure (is_drop_builtin, drop_elaboration, drop_glue) already recognizes Drop by name — adding to prelude just makes the trait available without user declaration.

**Option B** (keep user-declared): Users must continue declaring `trait Drop` themselves. Per §12 (最优 > 最小): rejected — the root cause is the missing prelude definition.

→ **Option A** = root-cause fix, mirrors Rust.

### Decision 2: Rename test files (TD-TRAIT-NAME-COLLISION workaround)
Same pattern as Stage 59 (Clone→Show) and Stage 61 (Display→Show): remove user `trait Drop` declarations from test files since they conflict with prelude's Drop. Per §1.0 原則 9 (正确 > 妥协): document the collision as TD-TRAIT-NAME-COLLISION (P3, v0.8+).

---

## 4. Tailoring points

- L2 task (prelude + tests) → §7.3 gate review sufficient per §1.2.1
- 跳过 §14.5 D2-D8 deep review — P3 TD fix, no soundness impact

---

## 5. §3.2 acceptance checks

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` (0 warnings) ✓
- `cargo test --release --features llvm-backend` ✓ (5496 tests, 0 failures, 14 ignored)
- Runtime verified: Drop called at scope exit (`dropping 42` printed), reverse order drops, nested scope drops

---

## 6. §1.6 ultimate test

Root-cause fix. Drop trait is now in prelude — users no longer need to declare it. The drop glue infrastructure was already complete. 1 new TD discovered: TD-MEM-DROP (P3, v0.8+) — `mem::drop()` explicit drop function.

---

## 7. Stage summary

- TD-SPECIAL-16 FIXED (Drop trait added to prelude)
- 13 test files updated (removed trait Drop declarations)
- 15 new tests added (14 passing + 1 ignored for TD-MEM-DROP)
- 1 new TD discovered: TD-MEM-DROP (P3, v0.8+)
- 5496 tests (898 lib + 4598 integration), 0 failures, 14 ignored
- fmt clean, 0 clippy warnings
- Architecture health: 9.85/10 (stable)

---

## 8. Next steps

- Wave 4: TD-PRELUDE-MACRO-TIMING (DefId decoupling + token-level injection)
- v0.8+: TD-IMPL-TRAIT-MONO-RESOLUTION (P1), TD-MEM-DROP, TD-TRAIT-NAME-COLLISION, and other deferred TDs
