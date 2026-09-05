#!/usr/bin/env python3
"""Stage 118: Fix test assertions for Debug impl re-add with debug_fmt.
Changes:
1. fn fmt(&self) -> String → fn debug_fmt(&self) -> String (in test impl Debug blocks)
2. .fmt() → .debug_fmt() (in test method calls)
3. For tests that check "no Debug impl" (expect errors): change to "Debug impl present" (expect no errors)
   Only in functions that contain .debug_fmt() calls
"""
import re, glob, sys

files = glob.glob('tests/v0/stage1*/plan/*.rs')
fixed = 0

for f in files:
    with open(f, 'r') as fh:
        content = fh.read()
    original = content

    # 1. Replace fn fmt(&self) -> String with fn debug_fmt(&self) -> String
    # Only in impl Debug blocks (not Display)
    content = re.sub(
        r'(impl Debug for \w+ \{[^}]*?)fn fmt\(&self\) -> String',
        r'\1fn debug_fmt(&self) -> String',
        content,
        flags=re.DOTALL,
    )

    # 2. Replace .fmt() with .debug_fmt() ONLY in test functions that test Debug
    # (identified by being in the same test function as debug_fmt or Debug)
    # We do this by finding test functions that mention Debug or debug_fmt
    # and replacing .fmt() → .debug_fmt() within them

    # Actually, simpler: replace all .fmt() that are on primitive types
    # (42i32).fmt() → (42i32).debug_fmt()
    # (42i64).fmt() → (42i64).debug_fmt()
    # (true).fmt() → (true).debug_fmt()
    # (42usize).fmt() → (42usize).debug_fmt()
    content = re.sub(
        r'\((\d+i(?:32|64)|true|\d+usize)\)\.fmt\(\)',
        r'(\1).debug_fmt()',
        content,
    )

    # 3. For test functions that call .debug_fmt():
    # Change assert!(result.has_errors() to assert!(!result.has_errors()
    # Find test functions containing .debug_fmt() and flip their assertion
    lines = content.split('\n')
    in_debug_test = False
    new_lines = []
    for i, line in enumerate(lines):
        # Detect start of a test function containing debug_fmt
        if '#[test]' in line:
            # Look ahead to see if this function contains .debug_fmt()
            func_text = '\n'.join(lines[i:i+30])
            in_debug_test = '.debug_fmt()' in func_text
        elif in_debug_test and line.strip() == '}':
            in_debug_test = False

        # Flip assertion in debug tests
        if in_debug_test and 'result.has_errors()' in line and '!' not in line.split('result')[0].split('assert!(')[-1]:
            line = line.replace('result.has_errors()', '!result.has_errors()')

        new_lines.append(line)
    content = '\n'.join(new_lines)

    if content != original:
        with open(f, 'w') as fh:
            fh.write(content)
        fixed += 1
        print(f"Fixed: {f}")

print(f"\nTotal: {fixed} files fixed")
