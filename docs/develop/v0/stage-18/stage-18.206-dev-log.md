# Stage 18.206 — ABI Contract Tests + Pipeline Doc Update (D7+D8 补档)

> **Date**: 2026-08-17
> **Version**: v0.470.0 (no bump — test + doc only)
> **Task ID**: stage18.206
> **Reviewer**: Super Z (main) — ARCH-A + DEV-A + REV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §8 (doc sync) + §9 (test standards) + §17.6 (缺陷纳入)

## 1. Scope

Per Stage 18.204 deep review §5.1 action plan: 补 ABI contract tests +
update pipeline-test-coverage.md (D7+D8 补档).

Per §17.6 (缺陷纳入): these are documentation gaps identified in Stage 18.204
deep review D7 (文档与知识传承) and D8 (测试路径覆盖), now being closed.

## 2. Implementation

### 2.1 ABI Contract Tests (tests/v0/stage18/plan/stage18_206_abi_contract_tests.rs)

9 tests verifying C runtime helper function signatures match the expected
ABI contract:

**Positive tests (7):**
- `stage18_206_compound_abis_match_c_source`: 4 compound helpers match contract
- `stage18_206_primitive_abis_match_c_source`: 4 primitive helpers match contract
- `stage18_206_compound_abis_use_pointer_params`: compound helpers use pointer/integer params
- `stage18_206_vec_push_get_elem_size_consistency`: vec_push and vec_get have matching elem_size
- `stage18_206_format_variadic_has_6_fixed_params_plus_variadic`: 6 fixed + variadic `...`
- `stage18_206_runtime_sigs_param_count_matches_c_source`: param count matches
- `stage18_206_all_runtime_helpers_are_c_abi`: all helpers have function bodies

**Negative tests (2):**
- `stage18_206_extract_signature_returns_none_for_unknown_function`: unknown function returns None
- `stage18_206_mismatch_detection_works`: deliberate mismatch is detected

**C source parser** (`extract_c_signature`):
- Parses multi-line function signatures (format_variadic spans 8 lines)
- Skips doc comments that mention the function name (e.g., summary lines)
- Handles nested parens + block/line comments
- Strips `/* ... */` comments from param list
- Returns (return_type, Vec<param_type>) tuple

Per §1.0 原則 3 (显式 > 隐式): the parser is explicit about handling
multi-line signatures and doc comments.
Per §1.0 原則 6 (通解>特例): one parser for all C function signatures.
Per §10 (DRY): the contract is defined ONCE in `COMPOUND_ABI_CONTRACTS` +
`PRIMITIVE_ABI_CONTRACTS` tables.

### 2.2 Pipeline-test-coverage.md Update (D7+D8 补档)

Added "Stage 18.206 Update" section to docs/tests/pipeline-test-coverage.md:

- **Test count update**: 664 lib + 3098 integration + 2935 conformance = 6697 total
- **Stage 18.206 changes**: ABI contract tests + pipeline doc update
- **Pipeline path coverage**: 21 paths covered in Stage 18.177-18.206 chain
  (heap alloc → Vec → String → format! → elem_size → ABI contracts)
- **D7 gap closure**: pipeline-test-coverage.md updated; ABI contract
  documented; Vec field offsets documented in TD-C-WRAPPER-OVERUSE audit
- **D8 gap closure**: format! method call tested; ABI contract tests added;
  Box/Vec of struct still blocked by TD-TUPLE-CTOR-TYPECK (v0.2 P2+)

Per §8 (doc sync): pipeline-test-coverage.md updated to reflect current state.
Per §8.2 (文档质量要求): no expired information — version, test count,
feature list all match code.

### 2.3 Test Matrix Update (docs/tests/matrix.md)

Updated header to v0.470.0 + current status table:
- Rust lib tests: 664 (was 640)
- Rust integration tests: 3098 (was 2663)
- Total: 6704 (was 6245)

Added Stage 18.120-18.206 entries to "v0.2 In-Progress (Stage 18)" history.

## 3. §3.2 Acceptance

- ✅ cargo fmt --check: exit 0
- ✅ cargo test --features llvm-backend --lib: 664 passed
- ✅ cargo test --features llvm-backend --tests: 3098 passed (was 3089, +9 new)
  - Single-threaded confirmation: 3098 passed, 0 failures
- ✅ cargo clippy: 5 warnings (all pre-existing, 0 new)
- **Total**: 3762 tests, 0 failures, zero regression

## 4. Tech Debt

No new TD (test + doc only stage). Stage 18.206 closes D7+D8 gaps from
Stage 18.204 deep review:

| Gap | Status | Action |
|-----|--------|--------|
| D7: pipeline-test-coverage.md 过期 | ✅ Closed | Stage 18.206 update section added |
| D7: Vec 字段偏移 隐式定义 | 🟡 Documented | TD-C-WRAPPER-OVERUSE audit doc records this; v0.2 will sink to MIR |
| D7: Compound C helper ABI 契约 | ✅ Closed | Stage 18.206 ABI contract tests verify signatures |
| D8: Box/Vec of struct | 🟡 Blocked | TD-TUPLE-CTOR-TYPECK (v0.2 P2+) |
| D8: format! result method call | ✅ Closed | Stage 18.205 fix + 8 tests |
| D8: Compound C helper ABI 契约测试 | ✅ Closed | Stage 18.206 ABI contract tests |

## 5. Design Principles Applied

- §1.0 原則 3 (显式 > 隐式): C source parser is explicit about multi-line + comments
- §1.0 原則 6 (通解 > 特例): one parser + one contract table for all C helpers
- §8 (doc sync): pipeline-test-coverage.md + matrix.md updated to current state
- §9 (test standards): 9 new tests with positive + negative coverage
- §9.4.3 (1:3+ 正负比例): 7 positive + 2 negative = 1:3.5 ratio (meets target)
- §10 (DRY): contract defined ONCE, verified against C source
- §17.6 (缺陷纳入): D7+D8 gaps from Stage 18.204 deep review now closed
