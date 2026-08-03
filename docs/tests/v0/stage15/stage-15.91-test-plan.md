# Stage 15.91 — Test Plan: Lifetime Elision Rule 3 (Self Param)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.215.0 → v0.216.0
> **Process**: stage-committee-process.md v3.24 §17.5

## 1. Test Scope

Stage 15.91 implements Lifetime Elision Rule 3: if there are multiple
input lifetimes but one is `&self`/`&mut self`, that lifetime is
assigned to all elided output lifetimes. Unified `apply_elision_rule_2`
into `apply_elision_rules` (handles both rules 2 and 3).

## 2. New/Updated Unit Tests

### 2.1 `apply_elision_rule_3_self_lifetime` (NEW)

Tests that with multiple input lifetimes (vids 1, 2) AND a self lifetime
(vid 1), the output lifetime (vid 10) is replaced with self's vid 1.

```rust
let input_vids = vec![RegionVid(1), RegionVid(2)];
let self_vid = Some(RegionVid(1));
let result = apply_elision_rules(&return_ty, &input_vids, self_vid);
// Rule 3: output lifetime → vid 1 (self's lifetime)
```

### 2.2 Updated tests (3 tests)

All 3 existing Stage 15.90 tests updated to call `apply_elision_rules`
(was `apply_elision_rule_2`):
- `apply_elision_rule_2_single_input` — passes `None` for self_vid
- `apply_elision_rule_2_multiple_inputs_no_self` — passes `None` (rule 3 doesn't apply)
- `apply_elision_rule_2_no_inputs` — passes `None`

## 3. Conformance Test Impact

### 3.1 No conformance test changes

All 5216 conformance tests pass unchanged.

## 4. Acceptance Criteria

| Criterion | Target | Actual |
|-----------|--------|--------|
| `cargo build --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo fmt` | clean | ✅ clean |
| `cargo clippy --all-targets --features llvm-backend` | 0 warnings | ✅ 0 warnings |
| `cargo test --features llvm-backend --lib` | 242/242 PASS | ✅ 242/242 PASS (was 241, +1 new) |
| `cargo test --features llvm-backend --test all_tests` | 2144/2144 PASS | ✅ 2144/2144 PASS |
| `python3 tests/conformance/run_all.py` | 5216/5216 PASS | ✅ 5216/5216 PASS |

**Stage 15.91 PASSED**.
