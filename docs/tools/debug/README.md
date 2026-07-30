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
6. **`borrowck-trace`** — Show borrow checker errors with context (Stage 14.81+)
7. **`ir-types`** — Show LLVM IR with alloca/load/store types highlighted (Stage 14.82+)
8. **`coverage`** — Show test count by category (Stage 14.81+)
9. **`gaps`** — Show P0/P1 gap status from capability assessment (Stage 14.81+)
10. **`quick-test`** — Compile + run + check expected stdout for a single test (Stage 14.98+)
11. **`stats`** — Show project statistics: version, LOC, test counts, doc counts (Stage 14.98+)
12. **`audit`** — Combined trace + diff + ir-types in one pass (Stage 14.98+)

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

# Stage 14.81+: Show borrow checker errors for a .lin file
python3 tools/debug/landin_debug.py borrowck-trace tests/conformance/02-borrowck/00-nll-basic/bk-0451-18-gap1-double-mut-borrow.lin

# Stage 14.82+: Show LLVM IR with type-bearing instructions highlighted
python3 tools/debug/landin_debug.py ir-types my_test.lin

# Show test count by category
python3 tools/debug/landin_debug.py coverage

# Show P0/P1 gap status
python3 tools/debug/landin_debug.py gaps

# Stage 14.98+: Quick-test a single .lin file with EXPECTED_STDOUT check
python3 tools/debug/landin_debug.py quick-test tests/conformance/04-e2e/06-run-ok/e2e-runok-160-trait-default-body-self-method.lin

# Stage 14.98+: Show project statistics
python3 tools/debug/landin_debug.py stats

# Stage 14.98+: Audit a single .lin file (trace + diff + ir-types)
python3 tools/debug/landin_debug.py audit tests/conformance/04-e2e/06-run-ok/e2e-runok-156-for-loop-range.lin
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

## Stage 14.81 GAP-1 Discovery

The `borrowck-trace` command (added in Stage 14.81) was used to diagnose
GAP-1 (NLL soundness). Running it on
`let mut x = 1; let r1 = &mut x; let r2 = &mut x;` initially showed no
borrowck errors — confirming the silent acceptance. After adding debug
eprintlns to `borrow_set.rs`, the trace revealed that `transfer_borrow_ref`
was never called for `Operand::Copy` (only `Operand::Move`), causing the
first borrow to be killed at the temp's last use (the Copy statement)
instead of the user-visible local's last use. The 1-line fix:
`if let Operand::Move(lv) | Operand::Copy(lv) = op {`.

## Stage 14.82 GAP-7 Discovery

The `ir-types` command (added in Stage 14.82) was used to diagnose GAP-7
(closure struct captures). Running it on `let f = || p.x;` showed the
closure alloca as `{ i32 }` instead of `{ { i32, i32 } }`. Adding debug
eprintlns to `codegen/mod.rs::codegen_function` revealed the closure's
`substs` held `Infer(TyVar)` (stale, captured before typeck ran). The fix:
driver writeback that walks `Aggregate(Closure, operands)` rvalues and
writes back each operand's source local resolved type to the corresponding
subst.

## Stage 14.97-14.98 Bug Discovery

The `quick-test` command (added in Stage 14.98) was used to verify each
new test case during the Bug Y1 + for-loop + Z1-Z4 fixes. It quickly shows
whether a test passes or fails, with clear expected-vs-actual output.

The `stats` command was used to track project growth and verify the release
binary was built before running tests.

The `audit` command combines trace + diff + ir-types in one pass, useful
for comprehensive single-file analysis.

## Adding New Debug Commands

To add a new debug command:

1. Add the command name to the `choices` list in `main()`.
2. Add a `cmd_<name>` function in the Commands section.
3. Add an `elif args.command == "<name>":` branch in `main()`.
4. Update the docstring at the top of the file.
5. Update this README with usage examples.

For commands that need debug output from the compiler itself, use the
`LANDIN_DEBUG_<MODULE>` env var pattern (e.g., `LANDIN_DEBUG_BORROWCK=1`,
`LANDIN_DEBUG_CODEGEN=1`). Add the corresponding `eprintln!` calls gated
by `std::env::var("LANDIN_DEBUG_<MODULE>").is_ok()` in the relevant source
files.

