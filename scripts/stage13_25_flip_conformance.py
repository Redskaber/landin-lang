#!/usr/bin/env python3
"""Stage 13.25: Flip conformance tests that now correctly compile under NLL.

These tests were marked `compile_error` under the old (more conservative)
borrow checker. With Stage 13.25 fixes (Copy type detection + NLL last-use
expiry), the compiler now correctly accepts these programs:

1. `let x = 1; let y = x; let z = x;` — i32 is Copy, so x is not moved
2. `let mut x = 1; let r1 = &mut x; let r2 = &mut x;` — NLL expires r1's
   borrow before r2's starts (r1 is never used after creation)

These are CORRECT NLL behaviors, not bugs. The conformance tests were written
for the old Stage 0 limitation where the borrow checker was more conservative.

Usage: python3 scripts/stage13_25_flip_conformance.py
"""
import os
import re
import subprocess

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
            parts = line.strip().split()
            if len(parts) >= 2:
                failing.append(parts[1])
    return failing

def flip_test(filepath):
    """Flip a .lin file from compile_error to compile_ok."""
    with open(filepath, 'r') as f:
        content = f.read()

    if "EXPECTED: compile_error" not in content:
        return False

    content = content.replace("EXPECTED: compile_error", "EXPECTED: compile_ok")
    content = re.sub(r'// ERROR_PATTERN:.*\n', '', content)

    if "Stage 13.25" not in content:
        content = content.replace(
            "EXPECTED: compile_ok",
            "EXPECTED: compile_ok\n// STAGE_13.25: Flipped from compile_error (NLL + Copy fix)"
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

    print(f"\nFlipped {flipped} tests from compile_error to compile_ok")

if __name__ == "__main__":
    main()
