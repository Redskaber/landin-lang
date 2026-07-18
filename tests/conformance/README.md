# Conformance Suite — Stage 0 Parse Tests

Per blueprint `17-conformance-suite.md`, Stage 0 must ship with 600 parse
conformance tests in `.lin` file format. Each test file has a header
specifying the expected outcome (pass/fail) and optional metadata.

## Directory structure

```
tests/conformance/
├── 00-parse/                    # Stage 0 parse tests (target: 600)
│   ├── 00-literals/             # Integer, float, char, string, byte literals
│   ├── 01-operators/            # All operator forms + Pratt precedence
│   ├── 02-control-flow/         # if/while/for/loop/match/break/continue
│   ├── 03-patterns/             # Wild, ident, lit, struct, tuple, or, range
│   ├── 04-types/                # Primitives, refs, ptrs, arrays, generics
│   ├── 05-attributes/           # #[derive], #![inner], meta forms
│   ├── 06-generics/             # Type params, bounds, where clauses
│   ├── 07-closures/             # ||, |args|, move ||
│   ├── 08-modules/              # mod, use (group/glob/alias), visibility
│   ├── 09-error-recovery/       # Malformed programs that should error
│   └── 10-realistic/            # Full programs (fib, iterators, traits)
└── run_all.py                   # Test runner
```

## Test file format

Each `.lin` file starts with a header comment block:

```landin
//! PASS
//! category: literals
//! description: decimal integer literal
//! source: lexer regression
42
```

Or for expected-failure tests:

```landin
//! FAIL
//! category: error-recovery
//! description: empty hex literal must error
//! error_pattern: hexadecimal literal has no digits
0x
```

The runner (`run_all.py`) parses the header, runs the lexer+parser via the
`landin-stage0` CLI, and verifies the expected outcome.

## Status

This is a **skeleton** — only a few representative tests have been ported.
The remaining ~590 tests will be added incrementally during Stage 1 HIR work,
since the conformance suite is also the natural regression-test bed for HIR
lowering.
