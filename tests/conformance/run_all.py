#!/usr/bin/env python3
"""
Conformance test runner for Landin Stage 0.

Walks the conformance test tree, parses each `.lin` file's header to determine
the expected outcome (PASS/FAIL or EXPECTED: compile_ok/compile_error/run_ok),
invokes the landin-stage0 CLI, and verifies the result.

Supports two header formats:
  1. Legacy `//!` format (Stage 9): `//! PASS`, `//! FAIL`, `//! error_pattern: ...`
  2. Spec `//` format (§3 of 17-conformance-suite.md):
     `// CATEGORY: parse`, `// EXPECTED: compile_ok`, `// ERROR_PATTERN: ...`

Usage:
    python3 tests/conformance/run_all.py [--binary PATH] [--verbose] [--mode parse|compile]

Exit code:
    0 = all tests passed
    1 = at least one test failed
"""

import argparse
import os
import re
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Optional


@dataclass
class ConformanceTest:
    path: Path
    # Legacy format: expect_pass=True means PASS, False means FAIL
    expect_pass: bool
    # New format: expected = "compile_ok" | "compile_error" | "run_ok" | "run_panic" | None
    expected: Optional[str]
    category: str
    description: str
    error_pattern: Optional[str]
    source: Optional[str]
    # Stage 14.11 (GAP-8): run_ok / run_panic verification fields.
    # EXPECTED_STDOUT: exact stdout string the program should produce.
    # EXPECTED_EXIT_CODE: expected exit code (default 0 for run_ok).
    expected_stdout: Optional[str]
    expected_exit_code: Optional[int]


# Legacy //! format: `//! PASS`, `//! FAIL`, `//! error_pattern: ...`
LEGACY_HEADER_RE = re.compile(r"^//!\s*(\w+)(?::\s*(.*))?$")

# New // format: `// KEY: VALUE` (e.g., `// EXPECTED: compile_ok`)
SPEC_HEADER_RE = re.compile(r"^//\s*(\w+)(?::\s*(.*))?$")


def parse_header(path: Path) -> ConformanceTest:
    """Parse the header block of a .lin file. Supports both //! and // formats."""
    expect_pass = True
    expected = None
    category = "uncategorized"
    description = ""
    error_pattern = None
    source = None
    expected_stdout = None
    expected_exit_code = None

    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if line.strip() == "":
                continue

            # Try legacy //! format first
            m = LEGACY_HEADER_RE.match(line)
            if m:
                key = m.group(1).lower()
                val = (m.group(2) or "").strip()
                if key == "pass":
                    expect_pass = True
                elif key == "fail":
                    expect_pass = False
                elif key == "category":
                    category = val
                elif key == "description":
                    description = val
                elif key == "error_pattern":
                    error_pattern = val
                elif key == "source":
                    source = val
                continue

            # Try spec // format
            m = SPEC_HEADER_RE.match(line)
            if m:
                key = m.group(1).upper()
                val = (m.group(2) or "").strip()
                if key == "EXPECTED":
                    expected = val
                    # Map to legacy: compile_ok/run_ok → PASS, compile_error → FAIL
                    if val == "compile_ok" or val == "run_ok":
                        expect_pass = True
                    elif val == "compile_error" or val == "run_panic":
                        expect_pass = False
                elif key == "CATEGORY":
                    category = val
                elif key == "DESCRIPTION":
                    description = val
                elif key == "ERROR_PATTERN":
                    error_pattern = val
                elif key == "ERROR_CODE":
                    # Store error code as part of error_pattern if not already set
                    if not error_pattern:
                        error_pattern = val
                elif key == "SOURCE":
                    source = val
                elif key == "EXPECTED_STDOUT":
                    # Stage 14.11 (GAP-8): Expected stdout for run_ok tests.
                    # Use \n as literal escape sequence in headers (converted here).
                    expected_stdout = val.replace("\\n", "\n")
                elif key == "EXPECTED_EXIT_CODE":
                    # Stage 14.11 (GAP-8): Expected exit code for run_ok tests.
                    try:
                        expected_exit_code = int(val)
                    except ValueError:
                        pass  # Invalid exit code — leave as None
                continue

            # Non-comment line — header ends
            if not line.startswith("//") and not line.startswith("//!"):
                break

    return ConformanceTest(
        path=path,
        expect_pass=expect_pass,
        expected=expected,
        category=category,
        description=description,
        error_pattern=error_pattern,
        source=source,
        expected_stdout=expected_stdout,
        expected_exit_code=expected_exit_code,
    )


def _run_test_run_ok(test: ConformanceTest, binary: Path, verbose: bool) -> tuple[bool, str]:
    """Stage 14.11 (GAP-8): Execute a run_ok test via --run.

    A run_ok test passes when:
    1. The program compiles and links successfully (no compiler errors)
    2. The program runs and exits with the expected exit code (default 0)
    3. If EXPECTED_STDOUT is set, stdout must match exactly

    A run_ok test fails when:
    - The compiler produces errors (compile-time failure)
    - The program crashes (SIGSEGV=139, SIGABRT=134)
    - The exit code doesn't match EXPECTED_EXIT_CODE
    - stdout doesn't match EXPECTED_STDOUT (if set)
    """
    cmd = [str(binary), "--run", str(test.path)]
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (program hung — likely infinite loop)"
    except FileNotFoundError:
        return False, f"BINARY NOT FOUND: {binary}"

    stdout = result.stdout or ""
    stderr = result.stderr or ""
    combined = stdout + stderr
    exit_code = result.returncode

    # Check for compiler errors (these appear before the program runs)
    has_compile_error = "parse error:" in combined or "lex error:" in combined or "error:" in stderr.lower()

    if has_compile_error:
        errs = [l for l in stderr.splitlines() if "error:" in l.lower()]
        return False, f"compile error during --run:\n  " + "\n  ".join(errs[:3])

    # Check for crashes (SIGSEGV=139, SIGABRT=134, or other signal-based exits)
    if exit_code >= 128:
        sig = exit_code - 128
        return False, f"program crashed with signal {sig} (exit code {exit_code})"

    # Verify exit code (default 0 for run_ok)
    expected_exit = test.expected_exit_code if test.expected_exit_code is not None else 0
    if exit_code != expected_exit:
        return False, f"exit code mismatch: expected {expected_exit}, got {exit_code}"

    # Verify stdout (if EXPECTED_STDOUT is set)
    # Stage 14.11: Be lenient about trailing newlines — println! adds "\n"
    # but test authors shouldn't need to include it in EXPECTED_STDOUT.
    # We strip trailing whitespace from both sides before comparing.
    if test.expected_stdout is not None:
        actual_stripped = stdout.rstrip()
        expected_stripped = test.expected_stdout.rstrip()
        if actual_stripped != expected_stripped:
            return False, (
                f"stdout mismatch:\n  expected: {expected_stripped!r}\n  got:      {actual_stripped!r}"
            )

    return True, f"OK (exit={exit_code})"


def _run_test_run_panic(test: ConformanceTest, binary: Path, verbose: bool) -> tuple[bool, str]:
    """Stage 14.11 (GAP-8): Execute a run_panic test via --run.

    A run_panic test passes when the program crashes or panics:
    - Exit code >= 128 (signal-based termination: SIGSEGV=139, SIGABRT=134)
    - OR non-zero exit code with a panic message in stderr

    A run_panic test fails when:
    - The program compiles and runs successfully (exit 0)
    - The compiler produces errors (this is a compile failure, not a panic)
    """
    cmd = [str(binary), "--run", str(test.path)]
    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=30,
        )
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (program hung — expected a crash, not a hang)"
    except FileNotFoundError:
        return False, f"BINARY NOT FOUND: {binary}"

    stdout = result.stdout or ""
    stderr = result.stderr or ""
    combined = stdout + stderr
    exit_code = result.returncode

    # Check for compiler errors (these are NOT panics — they're compile failures)
    has_compile_error = "parse error:" in combined or "lex error:" in combined or "error:" in stderr.lower()
    if has_compile_error:
        errs = [l for l in stderr.splitlines() if "error:" in l.lower()]
        return False, f"compile error (expected a panic, not a compile error):\n  " + "\n  ".join(errs[:3])

    # Check for crash/panic: exit code >= 128 (signal) or non-zero (panic exit)
    if exit_code == 0:
        return False, "program exited normally (expected a panic/crash)"

    # If PANIC_PATTERN is set (via error_pattern), verify it appears in stderr
    if test.error_pattern and test.error_pattern not in stderr:
        return False, (
            f"expected panic pattern `{test.error_pattern}` not found in stderr:\n  "
            + "\n  ".join(stderr.splitlines()[:5])
        )

    return True, f"OK (panic/crash, exit={exit_code})"


def run_test(test: ConformanceTest, binary: Path, verbose: bool, mode: str = "auto") -> tuple[bool, str]:
    """Run a single conformance test. Returns (passed, message).

    Stage 14.11 (GAP-8): Now dispatches on the `expected` field:
      - compile_ok    → --compile (must succeed, no errors)
      - compile_error → --compile (must fail with error_pattern)
      - run_ok        → --run (must succeed + verify stdout/exit code)
      - run_panic     → --run (must crash: SIGSEGV/SIGABRT or non-zero exit)

    The legacy `mode` parameter is respected for compile_ok/compile_error
    tests (auto-detect parse vs compile). For run_ok/run_panic, `--run` is
    always used regardless of `mode`.
    """
    # Stage 14.11 (GAP-8): Dispatch on expected type for run_ok / run_panic.
    if test.expected == "run_ok":
        return _run_test_run_ok(test, binary, verbose)
    elif test.expected == "run_panic":
        return _run_test_run_panic(test, binary, verbose)

    # Legacy dispatch for compile_ok / compile_error / None
    if mode == "auto":
        # Auto-detect: tests under 00-parse/ use parse mode, everything else uses compile
        if "00-parse" in str(test.path):
            mode = "parse"
        else:
            mode = "compile"

    if mode == "compile":
        cmd = [str(binary), "--compile", str(test.path)]
    else:
        cmd = [str(binary), "--emit-ast", str(test.path)]

    try:
        result = subprocess.run(
            cmd,
            capture_output=True,
            text=True,
            timeout=10,
        )
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (compiler hung — likely infinite loop)"
    except FileNotFoundError:
        return False, f"BINARY NOT FOUND: {binary}"

    stdout = result.stdout or ""
    stderr = result.stderr or ""
    combined = stdout + stderr

    # Check for errors in output
    has_error = "parse error:" in combined or "lex error:" in combined or "error:" in combined.lower()

    if test.expect_pass:
        if has_error:
            errs = [l for l in combined.splitlines() if "error:" in l.lower()]
            return False, f"expected PASS but got errors:\n  " + "\n  ".join(errs[:3])
        return True, "OK"
    else:
        # Expected FAIL
        if not has_error:
            return False, "expected FAIL but compiler accepted the input"
        if test.error_pattern:
            if test.error_pattern not in combined:
                return False, (
                    f"expected error pattern `{test.error_pattern}` not found in:\n  "
                    + "\n  ".join(combined.splitlines()[:5])
                )
        return True, "OK (expected error)"


def discover_tests(root: Path) -> list[ConformanceTest]:
    """Discover all .lin files under root."""
    tests = []
    for path in sorted(root.rglob("*.lin")):
        tests.append(parse_header(path))
    return tests


def main() -> int:
    parser = argparse.ArgumentParser(description="Landin Stage 0 conformance runner")
    parser.add_argument(
        "--binary",
        type=Path,
        default=Path("target/release/landin-stage0"),
        help="Path to the landin-stage0 binary",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).parent,
        help="Root directory of the conformance suite",
    )
    parser.add_argument(
        "--mode",
        choices=["auto", "parse", "compile"],
        default="auto",
        help="Test mode: auto (default, auto-detect per test), parse (--emit-ast), or compile (--compile)",
    )
    parser.add_argument("--verbose", "-v", action="store_true", help="Verbose output")
    args = parser.parse_args()

    if not args.binary.exists():
        # Try debug build
        debug = Path("target/debug/landin-stage0")
        if debug.exists():
            args.binary = debug
        else:
            print(f"ERROR: binary not found at {args.binary} or {debug}", file=sys.stderr)
            print("Run `cargo build --release` first.", file=sys.stderr)
            return 2

    tests = discover_tests(args.root)
    if not tests:
        print("No conformance tests found.")
        return 0

    print(f"Running {len(tests)} conformance tests against {args.binary} (mode={args.mode})")
    print()

    passed = 0
    failed = 0
    failures: list[tuple[ConformanceTest, str]] = []

    for test in tests:
        ok, msg = run_test(test, args.binary, args.verbose, args.mode)
        if ok:
            passed += 1
            if args.verbose:
                print(f"  PASS  {test.path.relative_to(args.root)}")
        else:
            failed += 1
            failures.append((test, msg))
            print(f"  FAIL  {test.path.relative_to(args.root)}")
            print(f"        {msg}")
            print()

    print()
    print(f"Results: {passed} passed, {failed} failed, {len(tests)} total")

    if failed == 0:
        print("ALL TESTS PASSED")
        return 0
    else:
        print(f"{failed} TESTS FAILED")
        return 1


if __name__ == "__main__":
    sys.exit(main())
