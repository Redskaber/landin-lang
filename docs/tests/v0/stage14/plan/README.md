# Stage 14 — Test Documentation

> **阶段范围**: Stage 14.1 - 14.9 (v0.1 release readiness — architecture cleanup + API standardization + docs sync)
> **测试目录**: `tests/v0/stage14/` (TBD — no new behavioral tests in Stage 14)
> **状态**: 🔄 In Progress (14.1 ✅, 14.2 ✅, 14.3 ✅, 14.4 ✅, 14.5 ✅; 14.6-14.9 in progress)

## 1. Stage 14 测试策略

Stage 14 is a **pure refactoring + documentation stage** — no new language
features, no new behavioral tests. The testing strategy is:

1. **Zero regression**: All existing tests (1951 rust + 5026 conformance)
   must continue to pass after every sub-stage.
2. **§1.2 acceptance checks**: `cargo clean && cargo build --lib --features
   llvm-backend && cargo fmt && cargo clippy --all-targets --features
   llvm-backend -- -D warnings && cargo test --features llvm-backend` must
   be green before each gate review.
3. **Example compilation**: `cargo build --examples --features llvm-backend`
   must succeed (§17.4.2 rule 3 — `usage/` examples must compile with
   current API).

## 2. Sub-stage Test Verification

| Sub-stage | Test verification | Result |
|-----------|------------------|--------|
| 14.1 | (Research only — no code change) | N/A |
| 14.2 | `cargo build --lib --features llvm-backend` (version bump only) | ✅ OK |
| 14.3 | `cargo test --features llvm-backend` (trait_dispatch split — zero behavior change) | ✅ 1951 passed |
| 14.4 | `cargo test --features llvm-backend` (stdlib glob re-export fix — zero behavior change) | ✅ 1951 passed |
| 14.5 | `cargo build --examples --features llvm-backend` (4 examples compile) | ✅ OK |
| 14.6 | (Documentation only — no code change) | N/A |
| 14.7 | (README only — no code change) | N/A |
| 14.8 | (RELEASE_NOTES only — no code change) | N/A |
| 14.9 | Full §1.2 acceptance check + package | ✅ (TBD — final) |

## 3. Test Count Summary

| Test category | Count | Status |
|---------------|-------|--------|
| Rust tests (default) | 1916 | ✅ All pass |
| Rust tests (--features llvm-backend) | 1951 | ✅ All pass |
| Conformance tests | 5026 | ✅ All pass (compile-only; `run_ok` not yet honored — GAP-8) |
| Doc-tests | 2 (ignored) | ✅ OK |
| Examples | 4 (3 no-features + 4 with llvm-backend) | ✅ All compile |
| Benchmarks | 5 | ✅ Pass |

## 4. Known Test Limitations (from Stage 14.1 Assessment)

- **GAP-8**: `tests/conformance/run_all.py` parses `run_ok` headers but does
  not actually invoke `--run` for them; all 5026 conformance tests only
  verify compilation, not execution. Fix deferred to Stage 14.10+.
- **GAP-21**: 229 conformance tests were unsoundly flipped from
  `compile_error` → `compile_ok` in Stage 13.25 (NLL permissiveness
  regression). Fix deferred to Stage 14.10+ (GAP-1).

## 5. Future Stage 14.10+ Test Work

When the deferred P0 blockers are addressed in Stage 14.10+, the following
test work will be needed:

- **Stage 14.10 (NLL soundness)**: Add fixpoint dataflow tests for
  loop-carried borrows, conditional borrows, two-phase interaction
- **Stage 14.11 (Region inference)**: Add tests for the 7 R5 soundness
  holes + `tests/conformance/05-soundness/02-lifetime-edge/*.lin`
- **Stage 14.12 (Drop elaboration)**: Add tests for `Drop::drop` codegen +
  `#[may_dangle]` dropck
- **Stage 14.13 (Lifetime elision)**: Add tests for the 3 elision rules +
  `fn foo(x: &i32, y: &i32) -> &i32` (error: cannot infer)
- **Stage 14.14 (self.x codegen)**: Add tests for `self.x` field access in
  method bodies
- **Stage 14.15 (run_ok runner)**: Rewrite `tests/conformance/run_all.py`
  to actually invoke `--run` for `run_ok` tests; add `expected/` snapshots

---

**创建日期**: 2026-07-28
**Process**: v3.21 (§17.3 + §18)
