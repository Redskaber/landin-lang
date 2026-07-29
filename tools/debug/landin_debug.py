#!/usr/bin/env python3
"""
Landin Debug Tool — Pipeline inspection and test runner.

Usage:
    python3 tools/debug/landin_debug.py <command> [options]

Commands:
    trace <file>           — Trace compilation pipeline (tokens → AST → HIR → MIR → IR)
    mir <file>             — Dump MIR for a .lin file
    test-runner [dir]      — Run all run_ok tests and report results
    diff <file>            — Compile and run, compare output with EXPECTED_STDOUT
    stages <file>          — Show which stages pass/fail for a .lin file

Options:
    --compiler <path>      — Path to landin-stage0 binary (default: target/debug/landin-stage0)
    --features <feat>      — Cargo features (default: llvm-backend)
    --verbose              — Show full output for each stage

Examples:
    python3 tools/debug/landin_debug.py trace tests/conformance/04-e2e/06-run-ok/e2e-runok-001-hello.lin
    python3 tools/debug/landin_debug.py test-runner tests/conformance/04-e2e/06-run-ok/
    python3 tools/debug/landin_debug.py diff my_test.lin
"""

import subprocess
import sys
import os
import re
import tempfile
import argparse
from pathlib import Path
from dataclasses import dataclass
from typing import List, Optional, Tuple

# =====================================================================
# Configuration
# =====================================================================

DEFAULT_COMPILER = "target/debug/landin-stage0"
DEFAULT_FEATURES = "llvm-backend"

# =====================================================================
# Data classes
# =====================================================================

@dataclass
class TestResult:
    name: str
    passed: bool
    expected: str
    actual: str
    error: Optional[str] = None

@dataclass
class StageResult:
    stage: str
    passed: bool
    output: str
    error: Optional[str] = None

# =====================================================================
# Compiler wrapper
# =====================================================================

class LandinCompiler:
    """Wrapper around the landin-stage0 compiler binary."""

    def __init__(self, compiler_path: str, features: str = DEFAULT_FEATURES):
        self.compiler_path = compiler_path
        self.features = features
        self._built = False

    def ensure_built(self):
        """Build the compiler if not already built."""
        if self._built:
            return
        if os.path.exists(self.compiler_path):
            self._built = True
            return
        print(f"Building compiler with --features {self.features}...")
        result = subprocess.run(
            ["cargo", "build", f"--features={self.features}"],
            capture_output=True, text=True, cwd="."
        )
        if result.returncode != 0:
            print(f"Build failed:\n{result.stderr}", file=sys.stderr)
            sys.exit(1)
        self._built = True

    def run(self, args: List[str], input_file: str = None) -> Tuple[int, str, str]:
        """Run the compiler with given arguments."""
        self.ensure_built()
        cmd = [self.compiler_path] + args
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        return result.returncode, result.stdout, result.stderr

    def emit_tokens(self, file: str) -> Tuple[int, str, str]:
        return self.run(["--emit-tokens", file])

    def emit_ast(self, file: str) -> Tuple[int, str, str]:
        return self.run(["--emit-ast", file])

    def emit_llvm_ir(self, file: str) -> Tuple[int, str, str]:
        return self.run(["--emit-llvm-ir", file])

    def emit_obj(self, file: str, output: str = None) -> Tuple[int, str, str]:
        args = ["--emit-obj"]
        if output:
            args += ["-o", output]
        args.append(file)
        return self.run(args)

    def run_program(self, file: str) -> Tuple[int, str, str]:
        return self.run(["--run", file])

    def compile(self, file: str) -> Tuple[int, str, str]:
        return self.run(["--compile", file])

# =====================================================================
# Commands
# =====================================================================

def cmd_trace(compiler: LandinCompiler, file: str, verbose: bool = False):
    """Trace the compilation pipeline for a .lin file."""
    print(f"=== Pipeline Trace: {file} ===\n")

    stages = [
        ("Lexer (tokens)", compiler.emit_tokens),
        ("Parser (AST)", compiler.emit_ast),
        ("LLVM IR", compiler.emit_llvm_ir),
    ]

    for stage_name, stage_fn in stages:
        print(f"--- {stage_name} ---")
        rc, stdout, stderr = stage_fn(file)
        if rc != 0:
            print(f"  ❌ FAILED (exit {rc})")
            if stderr:
                for line in stderr.strip().split("\n")[:10]:
                    print(f"     {line}")
            print()
            continue
        print(f"  ✅ OK")
        if verbose:
            for line in stdout.strip().split("\n")[:30]:
                print(f"  {line}")
            if len(stdout.strip().split("\n")) > 30:
                print(f"  ... ({len(stdout.strip().split(chr(10)))} lines total)")
        print()

    # Run
    print("--- Runtime (execute) ---")
    rc, stdout, stderr = compiler.run_program(file)
    if rc != 0:
        print(f"  ❌ FAILED (exit {rc})")
        if stderr:
            for line in stderr.strip().split("\n")[:10]:
                print(f"     {line}")
    else:
        print(f"  ✅ OK (exit {rc})")
        if stdout:
            for line in stdout.strip().split("\n")[:10]:
                print(f"  {line}")
    print()

def cmd_mir(compiler: LandinCompiler, file: str, verbose: bool = False):
    """Dump MIR for a .lin file (via LLVM IR, since MIR dump is not yet supported)."""
    print(f"=== MIR Dump: {file} ===\n")
    # MIR dump is not directly supported, but we can show the LLVM IR
    # which reflects the MIR structure.
    rc, stdout, stderr = compiler.emit_llvm_ir(file)
    if rc != 0:
        print(f"❌ Compilation failed:\n{stderr}")
        return

    # Parse the LLVM IR to show function structure
    functions = re.findall(r'define\s+\S+\s+@(\S+)\s*\([^)]*\)\s*\{', stdout)
    print(f"Functions: {len(functions)}")
    for fn in functions:
        print(f"  - {fn}")

    if verbose:
        print(f"\n--- Full LLVM IR ---\n{stdout}")

def cmd_test_runner(compiler: LandinCompiler, test_dir: str, verbose: bool = False):
    """Run all run_ok tests in a directory and report results."""
    test_dir = Path(test_dir)
    if not test_dir.exists():
        print(f"❌ Directory not found: {test_dir}")
        return

    test_files = sorted(test_dir.glob("*.lin"))
    if not test_files:
        print(f"❌ No .lin files found in {test_dir}")
        return

    print(f"=== Test Runner: {test_dir} ===")
    print(f"Found {len(test_files)} test files\n")

    results: List[TestResult] = []
    passed = 0
    failed = 0

    for tf in test_files:
        name = tf.name
        content = tf.read_text()

        # Parse EXPECTED_STDOUT
        expected_match = re.search(r'//\s*EXPECTED_STDOUT:\s*(.+)', content)
        if not expected_match:
            if verbose:
                print(f"  ⏭️  {name} (no EXPECTED_STDOUT, skipping)")
            continue

        expected_raw = expected_match.group(1).strip()
        expected = expected_raw.replace('\\n', '\n')

        # Parse EXPECTED_EXIT_CODE (optional, default: any non-zero is failure)
        exit_code_match = re.search(r'//\s*EXPECTED_EXIT_CODE:\s*(\d+)', content)
        expected_exit = int(exit_code_match.group(1)) if exit_code_match else 0

        rc, stdout, stderr = compiler.run_program(str(tf))

        # Check exit code
        exit_ok = (rc == expected_exit) if exit_code_match else True

        # Check stdout
        stdout_ok = (stdout.strip() == expected.strip())

        if not exit_ok:
            actual = f"[EXIT {rc}, expected {expected_exit}]"
            result = TestResult(name, False, expected, actual, stderr.strip()[:200])
            failed += 1
        elif not stdout_ok:
            result = TestResult(name, False, expected, stdout.strip())
            failed += 1
        else:
            result = TestResult(name, True, expected, stdout.strip())
            passed += 1

        results.append(result)

        status = "✅" if result.passed else "❌"
        print(f"  {status} {name}")
        if not result.passed and verbose:
            print(f"     Expected: {repr(result.expected[:100])}")
            print(f"     Actual:   {repr(result.actual[:100])}")
            if result.error:
                print(f"     Error:    {result.error[:100]}")

    print(f"\n=== Summary ===")
    print(f"  Total: {len(results)}")
    print(f"  Passed: {passed}")
    print(f"  Failed: {failed}")
    if results:
        print(f"  Pass rate: {passed/len(results)*100:.1f}%")

    return failed == 0

def cmd_diff(compiler: LandinCompiler, file: str, verbose: bool = False):
    """Compile and run a .lin file, compare output with EXPECTED_STDOUT."""
    print(f"=== Diff: {file} ===\n")

    content = Path(file).read_text()
    expected_match = re.search(r'//\s*EXPECTED_STDOUT:\s*(.+)', content)

    if not expected_match:
        print("❌ No EXPECTED_STDOUT comment found in file")
        return

    expected = expected_match.group(1).strip().replace('\\n', '\n')

    # Parse EXPECTED_EXIT_CODE
    exit_code_match = re.search(r'//\s*EXPECTED_EXIT_CODE:\s*(\d+)', content)
    expected_exit = int(exit_code_match.group(1)) if exit_code_match else 0

    rc, stdout, stderr = compiler.run_program(file)

    # Check exit code
    if exit_code_match and rc != expected_exit:
        print(f"❌ Exit code mismatch: got {rc}, expected {expected_exit}")
        if stderr:
            print(f"stderr:\n{stderr}")
        return

    # Check stdout
    actual = stdout.strip()
    expected_stripped = expected.strip()

    if actual == expected_stripped:
        print("✅ Output matches expected")
    else:
        print("❌ Output mismatch")
        print(f"\nExpected:\n{expected_stripped}")
        print(f"\nActual:\n{actual}")

        # Show line-by-line diff
        expected_lines = expected_stripped.split("\n")
        actual_lines = actual.split("\n")
        max_len = max(len(expected_lines), len(actual_lines))

        print(f"\nLine-by-line diff:")
        for i in range(max_len):
            exp = expected_lines[i] if i < len(expected_lines) else "<missing>"
            act = actual_lines[i] if i < len(actual_lines) else "<missing>"
            if exp == act:
                print(f"  {i+1:3d}  {exp}")
            else:
                print(f"  {i+1:3d} - {exp}")
                print(f"       + {act}")

def cmd_stages(compiler: LandinCompiler, file: str, verbose: bool = False):
    """Show which compilation stages pass/fail for a .lin file."""
    print(f"=== Stage Analysis: {file} ===\n")

    stages = [
        ("1. Lexer (--emit-tokens)", compiler.emit_tokens),
        ("2. Parser (--emit-ast)", compiler.emit_ast),
        ("3. Full compile (--compile)", compiler.compile),
        ("4. LLVM IR (--emit-llvm-ir)", compiler.emit_llvm_ir),
        ("5. Object file (--emit-obj)", lambda f: compiler.emit_obj(f)),
        ("6. Execute (--run)", compiler.run_program),
    ]

    results: List[StageResult] = []

    for stage_name, stage_fn in stages:
        try:
            rc, stdout, stderr = stage_fn(file)
            passed = (rc == 0)
            output = stdout[:500] if stdout else ""
            error = stderr[:500] if stderr and not passed else None
        except Exception as e:
            passed = False
            output = ""
            error = str(e)

        result = StageResult(stage_name, passed, output, error)
        results.append(result)

        status = "✅" if passed else "❌"
        print(f"  {status} {stage_name}")
        if not passed and error:
            for line in error.strip().split("\n")[:3]:
                print(f"     {line}")

    # Summary
    passed_count = sum(1 for r in results if r.passed)
    total = len(results)
    print(f"\n  {passed_count}/{total} stages passed")

    # Find first failure
    first_fail = next((r for r in results if not r.passed), None)
    if first_fail:
        print(f"  First failure: {first_fail.stage}")
    else:
        print(f"  All stages passed! 🎉")

# =====================================================================
# Main
# =====================================================================

def main():
    parser = argparse.ArgumentParser(
        description="Landin Debug Tool — Pipeline inspection and test runner",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=__doc__
    )

    parser.add_argument("command", choices=["trace", "mir", "test-runner", "diff", "stages"],
                        help="Command to run")
    parser.add_argument("file", nargs="?", help="Input .lin file (or test directory for test-runner)")
    parser.add_argument("--compiler", default=DEFAULT_COMPILER, help="Path to landin-stage0 binary")
    parser.add_argument("--features", default=DEFAULT_FEATURES, help="Cargo features")
    parser.add_argument("--verbose", "-v", action="store_true", help="Show full output")

    args = parser.parse_args()

    if args.command != "test-runner" and not args.file:
        parser.error(f"{args.command} requires a file argument")

    compiler = LandinCompiler(args.compiler, args.features)

    if args.command == "trace":
        cmd_trace(compiler, args.file, args.verbose)
    elif args.command == "mir":
        cmd_mir(compiler, args.file, args.verbose)
    elif args.command == "test-runner":
        test_dir = args.file or "tests/conformance/04-e2e/06-run-ok/"
        success = cmd_test_runner(compiler, test_dir, args.verbose)
        sys.exit(0 if success else 1)
    elif args.command == "diff":
        cmd_diff(compiler, args.file, args.verbose)
    elif args.command == "stages":
        cmd_stages(compiler, args.file, args.verbose)

if __name__ == "__main__":
    main()
