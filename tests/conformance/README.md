# Conformance Suite

The Landin compiler conformance suite contains **2,935 `.lin` test files**
(Stage 18.79: deduplicated from 5,348). Each test file has a header specifying
the expected outcome and optional metadata.

## Test Header Format

Each `.lin` file starts with comment lines specifying:

```text
// CATEGORY: <category>
// DESCRIPTION: <short description>
// EXPECTED: <compile_ok | compile_error | run_ok | run_panic>
// ERROR_PATTERN: <substring>     (optional, for compile_error)
// EXPECTED_STDOUT: <output>      (optional, for run_ok)
// EXPECTED_EXIT_CODE: <code>     (optional, for run_ok/run_panic)
// SOURCE: <stage and notes>      (optional)
```

## Directory Structure

```
tests/conformance/
├── 00-parse/           # Lexer + parser tests
├── 01-typecheck/       # Type checking + inference
├── 02-borrowck/        # Borrow checker + NLL
├── 03-codegen/         # LLVM IR + ABI + vtable
├── 04-e2e/             # End-to-end compile + run
├── 05-soundness/       # Regression + edge cases
├── 06-stdlib/          # Standard library facade
├── 07-integration/     # Multi-crate + feature gates
└── run_all.py          # Test runner (Python)
```

## Running Tests

```bash
# Build release binary first
cargo build --release --features llvm-backend

# Run all conformance tests
python3 tests/conformance/run_all.py

# Verbose output
python3 tests/conformance/run_all.py --verbose
```

## EXPECTED Values

| Value | Behavior |
|-------|----------|
| `compile_ok` | `--compile` must succeed (exit 0, no errors) |
| `compile_error` | `--compile` must fail (exit != 0, error in stderr) |
| `run_ok` | `--run` must succeed + stdout matches `EXPECTED_STDOUT` |
| `run_panic` | `--run` must crash (non-zero exit, SIGSEGV/SIGABRT) |

## ERROR_PATTERN

For `compile_error` tests, `ERROR_PATTERN` specifies a substring that must
appear in stderr. If omitted, any compile error is accepted (less precise).

## Test Count History

| Stage | Count | Notes |
|-------|-------|-------|
| Stage 9 | ~600 | Initial parse tests |
| Stage 10-11 | ~2,000 | Typeck + borrowck expansion |
| Stage 14 | ~5,000 | Full pipeline coverage |
| Stage 18.71-18.73 | 5,348 | P0/P1 typeck fixes |
| **Stage 18.79** | **2,935** | **Deduplicated (removed 2,413 pure duplicates)** |
