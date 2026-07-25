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
    # New format: expected = "compile_ok" | "compile_error" | "run_ok" | None
    expected: Optional[str]
    category: str
    description: str
    error_pattern: Optional[str]
    source: Optional[str]


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
                    # Map to legacy: compile_ok → PASS, compile_error → FAIL
                    if val == "compile_ok":
                        expect_pass = True
                    elif val == "compile_error":
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
    )


def run_test(test: ConformanceTest, binary: Path, verbose: bool, mode: str = "auto") -> tuple[bool, str]:
    """Run a single conformance test. Returns (passed, message).

    mode="parse": uses --emit-ast (legacy, only validates parse stage)
    mode="compile": uses --compile (validates full pipeline)
    mode="auto": auto-detect based on test path — 00-parse uses parse mode,
                 everything else (01-typecheck, 02-borrowck, etc.) uses compile mode
    """
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
