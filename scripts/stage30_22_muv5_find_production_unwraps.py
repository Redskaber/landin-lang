#!/usr/bin/env python3
"""
Stage 30.22 MUV 5: Find production unwrap() calls (not in test modules).

Scans all .rs files in src/ for `.unwrap()` calls that are NOT inside
`#[cfg(test)] mod tests` or `#[test]` functions.

Per §1.0 原則 3 (显式 > 隐式): production unwrap() should be expect() with
invariant documentation.
"""
import re
from pathlib import Path

SRC = Path("/home/z/my-project/landin-stage0/src")

def is_in_test(lines: list, line_idx: int) -> bool:
    """Check if a line is inside a test module or test function by scanning backwards."""
    # Scan backwards for #[test], #[cfg(test)], or mod tests
    depth = 0
    for i in range(line_idx, -1, -1):
        line = lines[i]
        stripped = line.strip()
        # Check for test attributes
        if re.match(r'^#\[(test|cfg\(test\)|ignore)\]', stripped):
            return True
        # Check for `mod tests` declaration
        if re.match(r'^mod\s+tests\s*\{', stripped):
            return True
        # Check for test function pattern (heuristic: fn name starts with test_ or stage)
        if re.match(r'^fn\s+(test_|stage\d+)', stripped):
            return True
    return False

production_unwraps = []
test_unwraps = 0

for f in SRC.rglob("*.rs"):
    lines = f.read_text().splitlines(keepends=True)
    for i, line in enumerate(lines):
        if ".unwrap()" in line:
            if is_in_test(lines, i):
                test_unwraps += 1
            else:
                production_unwraps.append((f, i + 1, line.rstrip()))

print(f"Production unwrap() calls: {len(production_unwraps)}")
print(f"Test unwrap() calls: {test_unwraps} (not converting)")
print()
print("Production unwrap() locations:")
for f, line, content in production_unwraps:
    rel = f.relative_to(SRC.parent)
    print(f"  {rel}:{line}: {content.strip()}")
