# Landin Debug Tool

> **Location**: `tools/debug/landin_debug.py`
> **Language**: Python 3
> **Purpose**: Pipeline inspection, test running, and debugging assistance

## Overview

The `landin_debug.py` tool provides several commands for debugging Landin
compiler issues:

1. **`trace`** — Trace the full compilation pipeline (Lexer → Parser → IR → Execute)
2. **`mir`** — Dump MIR structure (via LLVM IR, since MIR dump is not yet supported)
3. **`test-runner`** — Run all `run_ok` tests in a directory and report results
4. **`diff`** — Compile and run a single test, compare output with `EXPECTED_STDOUT`
5. **`stages`** — Show which compilation stages pass/fail for a `.lin` file

## Usage

```bash
# Trace the compilation pipeline for a test file
python3 tools/debug/landin_debug.py trace tests/conformance/04-e2e/06-run-ok/e2e-runok-001-hello.lin

# Run all run_ok tests and report pass/fail
python3 tools/debug/landin_debug.py test-runner tests/conformance/04-e2e/06-run-ok/

# Compare a single test's output with expected
python3 tools/debug/landin_debug.py diff my_test.lin

# Show which stages pass/fail
python3 tools/debug/landin_debug.py stages my_test.lin

# Show MIR structure (function list)
python3 tools/debug/landin_debug.py mir my_test.lin --verbose

# Verbose output (show full IR, full test details)
python3 tools/debug/landin_debug.py trace my_test.lin --verbose
```

## Test File Format

Test files use comment-based annotations:

```landin
// CATEGORY: e2e
// DESCRIPTION: run_ok description
// EXPECTED: run_ok
// EXPECTED_STDOUT: expected output here
// EXPECTED_EXIT_CODE: 0  (optional, default: 0)

fn main() -> i32 {
    println!("hello");
    0
}
```

- `EXPECTED_STDOUT`: The expected stdout output. Use `\n` for newlines.
- `EXPECTED_EXIT_CODE`: The expected exit code (optional). If not specified,
  non-zero exit codes are treated as failures.

## How It Works

The tool wraps the `landin-stage0` binary and calls it with different flags:
- `--emit-tokens` for lexer output
- `--emit-ast` for parser output
- `--emit-llvm-ir` for LLVM IR
- `--emit-obj` for object file generation
- `--run` for execution
- `--compile` for full compilation (no output)

The test-runner parses `EXPECTED_STDOUT` and `EXPECTED_EXIT_CODE` comments,
runs each test with `--run`, and compares the output.

## Finding Bugs

The test-runner is particularly useful for finding regressions. Run it after
any code change:

```bash
python3 tools/debug/landin_debug.py test-runner tests/conformance/04-e2e/06-run-ok/ -v
```

Any test that fails will show the expected vs actual output, making it easy
to identify which compiler change caused the regression.

## Stage 14.71 Bug Discovery

This tool was used in Stage 14.71 to discover a regression in `match` wildcard
pattern matching. The test-runner found that `e2e-runok-011-match.lin` was
failing — `classify(5)` returned 1 instead of 10. The `diff` command showed
the exact mismatch, and the `stages` command confirmed all stages passed but
runtime output was wrong. This led to the discovery that the Stage 14.67
otherwise-block rewrite had a bug where `cx.current_block` was reset to
`fallthrough_block` after `lower_expr_to_operand`, orphaning overflow-check
blocks.
