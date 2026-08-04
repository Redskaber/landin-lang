#!/usr/bin/env python3
"""
Stage 16.03: Automated `impl Copy` migration script.

Scans .lin conformance test files for struct definitions without
`impl Copy for <Name> {}` and adds the impl block after the struct.

Usage:
    python3 tools/migration/add_impl_copy.py tests/conformance/

This is the v0.3 migration tool for enabling sound Copy detection
(Stage 15.99/16.02). The script:
1. Finds all `struct Name { ... }` or `struct Name;` definitions
2. Checks if `impl Copy for Name {}` already exists
3. If not, adds `impl Copy for Name {}` after the struct definition
4. Skips structs that have `impl Drop` (Copy + Drop is a conflict)

Per §1.0 原則 9 "正确 > 妥协": sound Copy detection requires this migration.
Per §23: tool stored in tools/<sub_dirname>/ per project convention.
"""

import os
import re
import sys


def find_structs(content):
    """Find all struct definitions and return (name, line_index, has_semicolon)."""
    structs = []
    lines = content.split('\n')
    for i, line in enumerate(lines):
        # Match: struct Name { ... } or struct Name;
        m = re.match(r'^\s*struct\s+(\w+)\s*(\{|\;)', line)
        if m:
            name = m.group(1)
            brace_or_semi = m.group(2)
            has_semicolon = brace_or_semi == ';'
            structs.append((name, i, has_semicolon))
    return structs


def has_impl_copy(content, struct_name):
    """Check if `impl Copy for <struct_name>` already exists."""
    pattern = rf'impl\s+Copy\s+for\s+{re.escape(struct_name)}\s*\{{'
    return bool(re.search(pattern, content))


def has_impl_drop(content, struct_name):
    """Check if `impl Drop for <struct_name>` exists (Copy+Drop conflict)."""
    pattern = rf'impl\s+Drop\s+for\s+{re.escape(struct_name)}\s*\{{'
    return bool(re.search(pattern, content))


def find_struct_end_line(lines, start_idx, has_semicolon):
    """Find the line index after the struct definition ends."""
    if has_semicolon:
        return start_idx + 1
    # Find matching closing brace
    depth = 0
    for i in range(start_idx, len(lines)):
        depth += lines[i].count('{') - lines[i].count('}')
        if depth == 0:
            return i + 1
    return start_idx + 1


def add_impl_copy_to_file(filepath):
    """Add `impl Copy for Name {}` to all structs in a file that don't have it."""
    with open(filepath, 'r', encoding='utf-8') as f:
        content = f.read()

    structs = find_structs(content)
    if not structs:
        return 0

    lines = content.split('\n')
    additions = []  # (line_index, text_to_insert)

    for name, line_idx, has_semi in structs:
        if has_impl_copy(content, name):
            continue
        if has_impl_drop(content, name):
            continue  # Skip Drop types (Copy+Drop conflict)
        end_line = find_struct_end_line(lines, line_idx, has_semi)
        additions.append((end_line, f'impl Copy for {name} {{}}'))

    if not additions:
        return 0

    # Insert from bottom to top to preserve line indices
    additions.sort(key=lambda x: x[0], reverse=True)
    for idx, text in additions:
        lines.insert(idx, text)

    with open(filepath, 'w', encoding='utf-8') as f:
        f.write('\n'.join(lines))

    return len(additions)


def main():
    if len(sys.argv) < 2:
        print("Usage: python3 add_impl_copy.py <directory>")
        sys.exit(1)

    directory = sys.argv[1]
    total_added = 0
    files_modified = 0

    for root, dirs, files in os.walk(directory):
        for fname in sorted(files):
            if not fname.endswith('.lin'):
                continue
            filepath = os.path.join(root, fname)
            added = add_impl_copy_to_file(filepath)
            if added > 0:
                files_modified += 1
                total_added += added
                print(f"  {filepath}: +{added} impl Copy")

    print(f"\nTotal: {total_added} impl Copy blocks added to {files_modified} files")


if __name__ == '__main__':
    main()
