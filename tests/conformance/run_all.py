#!/usr/bin/env python3
"""
Conformance test runner for Landin Stage 0.

Walks the conformance test tree, parses each `.lin` file's header to determine
the expected outcome (PASS/FAIL + optional error_pattern), invokes the
landin-stage0 CLI to parse the file, and verifies the result.

Usage:
    python3 tests/conformance/run_all.py [--binary PATH] [--verbose]

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
    expect_pass: bool
    category: str
    description: str
    error_pattern: Optional[str]
    source: Optional[str]


HEADER_RE = re.compile(r"^//!\s*(\w+)(?::\s*(.*))?$")


def parse_header(path: Path) -> ConformanceTest:
    """Parse the //! header block of a .lin file."""
    expect_pass = True
    category = "uncategorized"
    description = ""
    error_pattern = None
    source = None

    with path.open("r", encoding="utf-8") as f:
        for line in f:
            line = line.rstrip("\n")
            if not line.startswith("//!"):
                # Header ends at first non-//! line
                if line.strip() == "":
                    continue
                break
            m = HEADER_RE.match(line)
            if not m:
                continue
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

    return ConformanceTest(
        path=path,
        expect_pass=expect_pass,
        category=category,
        description=description,
        error_pattern=error_pattern,
        source=source,
    )


def run_test(test: ConformanceTest, binary: Path, verbose: bool) -> tuple[bool, str]:
    """Run a single conformance test. Returns (passed, message)."""
    try:
        result = subprocess.run(
            [str(binary), "--emit-ast", str(test.path)],
            capture_output=True,
            text=True,
            timeout=10,
        )
    except subprocess.TimeoutExpired:
        return False, "TIMEOUT (parser hung — likely infinite loop)"
    except FileNotFoundError:
        return False, f"BINARY NOT FOUND: {binary}"

    stdout = result.stdout or ""
    stderr = result.stderr or ""
    combined = stdout + stderr

    # The CLI prints "parse error: ..." for parse errors and exits 0
    # (errors are non-fatal in Stage 0 — we accumulate and report).
    has_parse_error = "parse error:" in combined or "lex error:" in combined

    if test.expect_pass:
        if has_parse_error:
            # Extract the first error message for the report
            errs = [l for l in combined.splitlines() if "error:" in l]
            return False, f"expected PASS but got errors:\n  " + "\n  ".join(errs[:3])
        return True, "OK"
    else:
        # Expected FAIL
        if not has_parse_error:
            return False, "expected FAIL but parser accepted the input"
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

    print(f"Running {len(tests)} conformance tests against {args.binary}")
    print()

    passed = 0
    failed = 0
    failures: list[tuple[ConformanceTest, str]] = []

    for test in tests:
        ok, msg = run_test(test, args.binary, args.verbose)
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
