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
├── module_tests.rs              (Stage 4.x — module + use resolution)
├── macro_tests.rs               (Stage 4.x — built-in macros)
└── benchmark_adr.md             (Stage 4.11 — benchmark ADR)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 13 (low — Stage 4 largely relies on conformance + Stage 3 codegen tests) |
| Benchmarks | 5 (compile_bench.rs) |

## 测试覆盖

| Module | Tests | Focus |
|--------|-------|-------|
| closure_call_tests.rs | 4 | Closure call dispatch, Fn/FnMut/FnOnce |
| closure_capture_tests.rs | 3 | Variable capture (by-ref, by-value, move) |
| module_tests.rs | 4 | mod/use/visibility/pub |
| macro_tests.rs | 2 | Built-in macros (println, format, vec) |

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
cargo test --test all_tests -- closure_call_tests closure_capture_tests module_tests macro_tests
cargo bench --bench compile_bench
```
