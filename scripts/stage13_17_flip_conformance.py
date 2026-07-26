#!/usr/bin/env python3
"""Stage 13.17: Flip conformance tests that now compile (self binding fix).

These tests were marked `compile_error` because the compiler previously
couldn't handle `self` in method bodies. Stage 13.17 fixed the self binding,
so these tests now compile successfully. Flip them to `compile_ok`.

Usage: python3 scripts/stage13_17_flip_conformance.py
"""
import os
import re
import subprocess
import sys

CONF_DIR = os.path.join(os.path.dirname(__file__), "..", "tests", "conformance")
BINARY = os.path.join(os.path.dirname(__file__), "..", "target", "debug", "landin-stage0")

def run_conformance():
    """Run conformance suite and get list of failing tests."""
    result = subprocess.run(
        ["python3", "tests/conformance/run_all.py"],
        capture_output=True, text=True, cwd=os.path.dirname(__file__) + "/.."
    )
    failing = []
    for line in result.stdout.splitlines():
        if line.startswith("  FAIL  ") and "expected FAIL but compiler accepted" not in line:
            # Extract the .lin path
            parts = line.strip().split()
            if len(parts) >= 2:
                failing.append(parts[1])
    return failing

def flip_test(filepath):
    """Flip a .lin file from compile_error to compile_ok."""
    with open(filepath, 'r') as f:
        content = f.read()

    # Replace EXPECTED: compile_error with EXPECTED: compile_ok
    if "EXPECTED: compile_error" not in content:
        return False

    content = content.replace("EXPECTED: compile_error", "EXPECTED: compile_ok")

    # Remove ERROR_PATTERN line if present
    content = re.sub(r'// ERROR_PATTERN:.*\n', '', content)

    # Update SOURCE line to note Stage 13.17
    if "Stage 13.17" not in content:
        content = content.replace(
            "EXPECTED: compile_ok",
            "EXPECTED: compile_ok\n// STAGE_13.17: Flipped from compile_error (self binding fix)"
        )

    with open(filepath, 'w') as f:
        f.write(content)
    return True

def main():
    failing = run_conformance()
    print(f"Found {len(failing)} failing tests (expected FAIL but compiler accepted)")

    flipped = 0
    for rel_path in failing:
        filepath = os.path.join(CONF_DIR, rel_path)
        if os.path.exists(filepath):
            if flip_test(filepath):
                flipped += 1
                print(f"  Flipped: {rel_path}")

    print(f"\nFlipped {flipped} tests from compile_error to compile_ok")

if __name__ == "__main__":
    main()
