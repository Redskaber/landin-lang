# Stage 15.91 — Lifetime Elision Rule 3 (Self Param)

> **Author**: redskaber
> **Date**: 2026-08-02
> **Version**: v0.215.0 → v0.216.0
> **Process**: stage-committee-process.md v3.24 §29 + §9.3 + §25

## 1. Executive Summary

Stage 15.91 implements **Lifetime Elision Rule 3** (RFC 141): if there
are multiple input lifetimes but one is `&self`/`&mut self`, that
lifetime is assigned to all elided output lifetimes.

This is the second stage of **Task 12 (Lifetime elision)**.

**What changed**:
- Unified `apply_elision_rule_2` into a new `apply_elision_rules`
  function that handles both rule 2 and rule 3.
- Added `self_region_vid` tracking: when a `&self`/`&mut self` param
  is encountered, its region vid is collected and passed to
  `apply_elision_rules` for rule 3.
- Updated the elision collection loop to resolve self param types and
  collect their region vids (previously skipped).

**Rust Elision Rules (RFC 141)**:
1. Each elided input lifetime gets its own fresh lifetime. ✅ (Stage 15.49)
2. If there's exactly one input lifetime, it's assigned to all elided
   output lifetimes. ✅ (Stage 15.90)
3. If there are multiple input lifetimes but one is `&self`/`&mut self`,
   that lifetime is assigned to all elided output lifetimes. ✅ **(Stage 15.91)**

**Test impact**:
- 1 new unit test (`apply_elision_rule_3_self_lifetime`)
- Updated 3 existing tests to use `apply_elision_rules` (was `apply_elision_rule_2`)
- 0 conformance test changes
- **Total: 7602 tests passing** (242 lib [was 241, +1 new] + 2144
  integration + 5216 conformance), 0 failures, 0 warnings.

Per §1.0 原則 3 "显式 > 隐式": elision rules are explicitly applied.
Per §23 (API Naming): `apply_elision_rules` follows `<verb>_<noun>_<noun>`
pattern.

## 2. Why This Matters

Rule 3 is critical for method-heavy code. Consider:

```rust
impl Foo {
    fn get(&self, idx: usize) -> &Bar { ... }
}
```

With multiple input lifetimes (`&self` and `idx` is `usize` — no lifetime),
rule 3 says the output lifetime is `&self`'s lifetime. Without rule 3,
the output would get a fresh, unconstrained lifetime — the borrow checker
couldn't verify the returned reference doesn't outlive `self`.

Stage 15.91 fixes this: the self param's region vid is tracked and used
for the output when rule 3 applies.

## 3. The Implementation

### 3.1 Unified `apply_elision_rules` function

Replaced `apply_elision_rule_2` with `apply_elision_rules`:

```rust
fn apply_elision_rules(
    return_ty: &Ty,
    input_vids: &[RegionVid],
    self_vid: Option<RegionVid>,
) -> Ty {
    let target_vid = if input_vids.len() == 1 {
        // Rule 2: exactly one input lifetime → use it.
        Some(input_vids[0])
    } else if input_vids.len() > 1 {
        // Rule 3: multiple input lifetimes, but if one is &self/&mut self,
        // use the self lifetime.
        self_vid
    } else {
        None
    };
    // ... replace regions
}
```

### 3.2 Self region vid tracking

In the param lowering loop, when a `self_kind.is_some()` param is
encountered:
1. Resolve the self type via `resolve_self_param_type`
2. Collect its region vids via `collect_region_vids`
3. Store the first vid in `self_region_vid`
4. Also add it to `param_region_vids_collected`

### 3.3 Updated test suite

- `apply_elision_rule_2_single_input` → calls `apply_elision_rules` with `None` for self
- `apply_elision_rule_2_multiple_inputs_no_self` → calls with `None` (no rule 3)
- `apply_elision_rule_3_self_lifetime` → **NEW** — calls with `Some(vid)` for self, verifies rule 3 applies
- `apply_elision_rule_2_no_inputs` → calls with `None`

## 4. API Naming Compliance (§23)

**Renamed function**:

| Old | New | §23 Compliance |
|-----|-----|-----------------|
| `apply_elision_rule_2` | `apply_elision_rules` | ✅ `<verb>_<noun>_<noun>` (more general name) |

**New parameter**:

| Parameter | Type | §23 Compliance |
|-----------|------|-----------------|
| `self_vid` | `Option<RegionVid>` | ✅ `<noun>_<noun>` |

## 5. §16 Interface Isolation

All changes are within `mir::lower::mod`. The `apply_elision_rules`
function reads only `Ty`/`Region`/`RegionVid` data. The `self_region_vid`
is collected from `resolve_self_param_type` (which already exists and
reads HIR per §16.6 exception).

## 6. §25 Deep Review (8 Dimensions)

| Dimension | Status | Notes |
|-----------|--------|-------|
| D1 Architecture | ✅ | Unified function handles both rules 2 and 3 |
| D2 Tech Debt | ✅ | Task 12 rule 3 complete; rules 1-3 all implemented |
| D3 Test Coverage | ✅ | 1 new test + 3 updated tests cover all paths |
| D4 Next-Phase Readiness | ✅ | Foundation for explicit lifetime tracking + region inference |
| D5 Design Rationality | ✅ | Follows RFC 141 elision rules |
| D6 Performance | ✅ | One extra collect_region_vids call for self; negligible |
| D7 Documentation | ✅ | This doc + test plan + matrix updated |
| D8 Test Path Coverage | ✅ | Rule 2 (single input), rule 3 (self), no-rule (multiple/no input) all tested |

**Committee Vote**: GO — Stage 15.91 complete.

## 7. Verification

- `cargo clean && cargo build --features llvm-backend` — ✅ clean, 0 warnings
- `cargo fmt` — ✅ clean
- `cargo clippy --all-targets --features llvm-backend` — ✅ 0 warnings
- `cargo test --features llvm-backend --lib` — ✅ 242/242 PASS (was 241, +1 new)
- `cargo test --features llvm-backend --test all_tests` — ✅ 2144/2144 PASS
- `python3 tests/conformance/run_all.py` — ✅ 5216/5216 PASS
- **Total: 7602 tests passing, 0 failures, 0 warnings.**

## 8. Next Steps for Task 12

Stage 15.91 completes elision rules 1-3. Remaining work for Task 12:

1. **Explicit lifetime tracking**: Currently, explicit lifetimes (`'a`)
   each get a fresh vid — references with the same lifetime name should
   share a vid. Requires HIR lifetime name → vid mapping.
2. **Region inference activation**: The region inference infrastructure
   (Stages 7.1-7.5, 15.48-15.52) needs to use the now-correct region
   vids to actually check lifetime constraints and report errors.

## 9. Version Policy

v0.215.0 → v0.216.0 (minor bump — Phase 3 Task 12 Lifetime elision rule 3
+ 1 new unit test).
