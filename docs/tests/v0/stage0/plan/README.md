# Stage 0 — Test Documentation

> **阶段范围**: Stage 0.1 - 0.4 (lexer + parser + AST)
> **测试目录**: `tests/v0/stage0/plan/` + `tests/conformance/00-parse/`
> **状态**: ✅ Complete

## 测试目录结构

```
tests/v0/stage0/plan/
├── README.md                    ← 本文件
├── lexer_tests.rs               (Stage 0.1 — lexer unit tests)
├── parser_tests.rs              (Stage 0.2 — parser unit tests)
└── ast_structure_tests.rs       (Stage 0.3 — AST structure verification)

tests/conformance/00-parse/      ← Stage 9 expanded to 600 tests (100% ✅)
├── 00-lexing/                   (100 tests — token classification)
├── 01-literals/                 (50 tests — numeric/string/char literals)
├── 02-operators/                (50 tests — operators + Pratt precedence)
├── 03-control-flow/             (50 tests — if/while/loop/match)
├── 04-functions/                (50 tests — fn definitions)
├── 05-types/                    (50 tests — type expressions)
├── 06-patterns/                 (50 tests — match patterns)
├── 07-attributes/               (50 tests — attributes)
├── 08-generics/                 (50 tests — generic declarations)
├── 09-modules/                  (50 tests — mod/use declarations)
└── 10-realistic-programs/       (50 tests — full programs)
```

## 测试统计

| Type | Count |
|------|-------|
| Rust integration tests | 344 (lexer 109 + parser 85 + ast_structure 149 + 1 misc) |
| Conformance tests | 600 (00-parse, 100% of 600 target) ✅ |

## Conformance per subcategory (00-parse)

| Subcategory | Count | Status |
|-------------|-------|--------|
| 00-lexing | 100 | ✅ |
| 01-literals | 50 | ✅ |
| 02-operators | 50 | ✅ |
| 03-control-flow | 50 | ✅ |
| 04-functions | 50 | ✅ |
| 05-types | 50 | ✅ |
| 06-patterns | 50 | ✅ |
| 07-attributes | 50 | ✅ |
| 08-generics | 50 | ✅ |
| 09-modules | 50 | ✅ |
| 10-realistic-programs | 50 | ✅ |
| **Total** | **600** | **100% ✅** |

## 关联文档

- `docs/develop/v0/stage-0/dev-log.md` — Stage 0 开发日志
- `docs/develop/v0/stage-0/status.md` — Stage 0 完成状态
- `docs/lang-design/02-grammar.md` — 文法定义
- `docs/lang-design/05-ast.md` — AST 设计
- `docs/tests/v0/stage0/plan/{lexer,parser,ast_structure}.md` — 各模块测试设计文档

## 测试 runner

```bash
# Rust 集成测试
cargo test --test all_tests -- lexer_tests parser_tests ast_structure_tests

# Conformance 00-parse category
python3 tests/conformance/run_all.py --category 00-parse
```
