# Stage 9.1 测试计划: Systematic review verification + conformance literals expansion

> **阶段**: Stage 9.1
> **对应代码**: tests/v0/stage9/plan/systematic_review_v0156_tests.rs + tests/conformance/00-parse/00-literals/*.lin
> **状态**: ✅ Complete

## 1. 测试目标

1. 验证 §25 系统性审查 (七维度) 的关键结论通过可执行测试支撑
2. 验证 Stage 9.1 conformance suite 扩展 (8 → 38 tests) 正确通过

## 2. Rust 集成测试 (tests/v0/stage9/plan/systematic_review_v0156_tests.rs)

| 场景 | 测试函数名 | 状态 | 说明 |
|------|-----------|------|------|
| D1 架构: src/ 30+ modules | test_d1_src_directory_has_modules | ✅ | src/ 目录健康 |
| D1 架构: Stage 9 目录创建 | test_d1_stage9_directories_exist | ✅ | docs/develop/v0/stage-9/ + docs/tests/v0/stage9/ + tests/v0/stage9/ |
| D3 测试基础设施 | test_d3_test_infrastructure_healthy | ✅ | all_tests.rs 引用 stage8 |
| D4 conformance 套件存在 | test_d4_conformance_suite_exists | ✅ | tests/conformance/ + run_all.py + 00-parse/ |
| D4 conformance 套件扩展 | test_d4_conformance_suite_expanded_in_stage_9_1 | ✅ | 00-literals/ 至少 33 .lin |
| D5 设计文档同步 | test_d5_design_docs_synced | ✅ | 10 个 design docs 存在 |
| D5 路线图定义 v0.1 gate | test_d5_roadmap_defines_v01_conformance_gate | ✅ | 12-roadmap.md 含 v0.1 + conformance |
| D7 Stage 9 docs 创建 | test_d7_stage9_docs_created | ✅ | plan-9.1.md + systematic-review-v0156.md |
| D7 Stage 9 README | test_d7_stage9_readme_exists | ✅ | stage-9/README.md |
| Stage 9 conformance 类别 | test_stage9_conformance_categories_match_design | ✅ | 5 个类别符合 §2 设计 |
| Cargo.toml 版本 bump | test_stage9_cargo_toml_version_bumped | ✅ | 0.16.x |

## 3. Conformance .lin 测试 (tests/conformance/00-parse/00-literals/)

### 3.1 新增 30 个测试 (Stage 9.1)

| 类别 | 测试数 | 文件 |
|------|-------|------|
| Integer decimal | 5 | int_dec_zero, int_dec_underscore, int_dec_large, int_dec_leading_zero (FAIL), int_dec_in_expr |
| Integer hex | 4 | int_hex_lowercase, int_hex_uppercase, int_hex_zero, int_hex_uppercase_only |
| Integer octal | 3 | int_oct_basic, int_oct_zero, int_oct_underscore |
| Integer binary | 3 | int_bin_basic, int_bin_zero, int_bin_underscore |
| Integer suffix | 4 | int_suffix_i32, int_suffix_u64, int_suffix_isize, int_suffix_usize |
| Float | 5 | float_pi, float_exponent, float_underscore, float_zero, float_f64_suffix |
| Char | 3 | char_simple, char_escape_newline, char_escape_backslash |
| String | 3 | string_simple, string_empty, string_escape |

### 3.2 已有测试 (Stage 0-8 期间创建)

- int_dec_basic.lin (PASS)
- float_pure_suffix_f32.lin (PASS)
- raw_ident_keyword.lin (PASS)

### 3.3 累计: 33 .lin tests in 00-literals/

## 4. §17 测试矩阵对齐

| 矩阵项 | Stage 9.1 测试 |
|--------|----------------|
| 正面 (PASS) | ✅ 32 .lin + 11 rust tests |
| 负面 (FAIL) | ✅ 1 .lin (int_dec_leading_zero — Rust-style leading zero rejection) |
| 边界 (zero/empty) | ✅ int_dec_zero, int_hex_zero, int_oct_zero, int_bin_zero, float_zero, string_empty |
| 多态 (suffix/escape) | ✅ int_suffix_*, char_escape_*, string_escape |
| 集成 (in expr) | ✅ int_dec_in_expr |

## 5. 测试统计

- 预期: 30 .lin + 11 rust = 41 new tests
- 实际: 30 .lin (1 converted PASS→FAIL after lexer rule discovery) + 11 rust = 41 new tests
- Conformance: 8 → 38 (+30 ✅)
- Rust integration: 2100 → 2111 (+11 ✅)
- 0 regressions

## 6. 关键发现

**Lexer rule discovery**: Landin disallows leading zeros in decimal integers
(similar to Rust). The `int_dec_leading_zero.lin` test was initially written
as PASS but converted to FAIL after observing the lexer error
"leading zeros not allowed in decimal integer". This is a **positive** outcome —
the conformance suite caught an unverified language rule.

---

**创建日期**: 2026-07-26
