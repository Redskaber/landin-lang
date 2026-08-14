#!/usr/bin/env python3
"""Stage 18.54: Update conformance tests from compile_error → compile_ok.

These tests were marked as compile_error due to Stage 0 limitation
(generic type param resolution). Stage 18.54 fixed this, so they now
compile successfully and should be marked compile_ok.
"""
import re
import sys
from pathlib import Path

def update_test(path: Path) -> bool:
    """Update a conformance test from compile_error to compile_ok.
    Returns True if file was modified."""
    content = path.read_text()
    original = content

    # Update EXPECTED
    content = re.sub(
        r'// EXPECTED: compile_error',
        '// EXPECTED: compile_ok',
        content
    )

    # Remove ERROR_PATTERN line (no longer needed for compile_ok)
    content = re.sub(
        r'// ERROR_PATTERN:.*\n',
        '',
        content
    )

    # Update SOURCE to note Stage 18.54 fix
    content = re.sub(
        r'// SOURCE: (Stage[^\n]*?)\n',
        r'// SOURCE: \1; Stage 18.54 fixed generic type param resolution\n',
        content,
        count=1
    )

    # Update DESCRIPTION if it mentions limitation
    content = re.sub(
        r'// DESCRIPTION: ([^\n]*?) \(Stage 0 limitation[^\n]*\)',
        r'// DESCRIPTION: \1 (Stage 18.54: now compiles)',
        content
    )

    if content != original:
        path.write_text(content)
        return True
    return False

def main():
    failing_file = Path('/tmp/failing_tests.txt')
    if not failing_file.exists():
        print("No failing tests file found")
        return 1

    tests_dir = Path('tests/conformance')
    updated = 0
    skipped = 0

    for line in failing_file.read_text().splitlines():
        line = line.strip()
        if not line:
            continue
        test_path = tests_dir / line
        if not test_path.exists():
            print(f"SKIP (not found): {line}")
            skipped += 1
            continue
        if update_test(test_path):
            print(f"UPDATED: {line}")
            updated += 1
        else:
            print(f"NO CHANGE: {line}")
            skipped += 1

    print(f"\nSummary: {updated} updated, {skipped} skipped")
    return 0

if __name__ == '__main__':
    sys.exit(main())
