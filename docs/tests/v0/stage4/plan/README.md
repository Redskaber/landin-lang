# Stage 4 — Test Documentation

> **阶段范围**: Stage 4.1 - 4.13 (modules, closures, macros foundation, benchmarks)
> **测试目录**: `tests/v0/stage4/plan/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage4/plan/
├── README.md                    ← 本文件
├── closure_call_tests.rs        (Stage 4.x — closure call lowering)
├── closure_capture_tests.rs     (Stage 4.x — closure capture verification)
├── closure_full_call_tests.rs   (Stage 4.13 — full closure call lowering)
├── macro_system_tests.rs        (Stage 4.10 — built-in macro system)
├── visibility_tests.rs          (Stage 4.x — module visibility + use resolution)
└── benchmark_adr.md             (Stage 4.11 — benchmark ADR)
```

> Note: Per r217 second-pass audit (Stage 12.7 correction), actual filenames are
> `closure_full_call_tests.rs`, `macro_system_tests.rs`, `visibility_tests.rs`
> (not `module_tests.rs`, `macro_tests.rs` as previously listed — those filenames
> do not exist on disk). The README also previously omitted `closure_full_call_tests.rs`.

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 13 (low — Stage 4 largely relies on conformance + Stage 3 codegen tests) |
| Benchmarks | 5 (compile_bench.rs) |

## 测试覆盖

> Per-module counts verified by r217 second-pass audit (Stage 12.7 correction).
> Total = 13 ✅ (matches); per-module breakdown corrected from r216 first-pass estimates.

| Module | Tests | Focus |
|--------|-------|-------|
| closure_call_tests.rs | 2 | Closure call dispatch (simplified, Stage 4.4) |
| closure_capture_tests.rs | 4 | Variable capture (by-ref, by-value, move) |
| closure_full_call_tests.rs | 2 | Full closure call lowering (Stage 4.13) |
| macro_system_tests.rs | 3 | Built-in macros (println, format, vec) |
| visibility_tests.rs | 2 | mod/use/visibility/pub |
| **Total** | **13** | (verified r217) |

## Conformance coverage

Stage 4 features are exercised via:
- `tests/conformance/03-codegen/05-closures/` (50 tests)
- `tests/conformance/06-stdlib/03-closures/` (50 tests)
- `tests/conformance/07-integration/00-multi-crate/` (mod + use, ~70 tests)

## 关联文档

- `docs/develop/v0/stage-4/dev-log.md` — Stage 4 开发日志
- `docs/develop/v0/stage-4/deep-review-r48.md` — Stage 4.13 深度审查
- `docs/develop/v0/stage-4/gate-review-round1.md` to `gate-review-round10.md` — 10 轮门审查
- `docs/lang-design/13-stage1-feature-whitelist.md` — Stage 1 特性白皮书 (closures, modules, macros spec)
- `docs/tests/v0/stage4/plan/{closure_call,closure_capture,benchmark_adr}.md` — 各模块测试设计文档

## 测试 runner

```bash
cargo test --test all_tests -- closure_call_tests closure_capture_tests closure_full_call_tests macro_system_tests visibility_tests
cargo bench --bench compile_bench
```
