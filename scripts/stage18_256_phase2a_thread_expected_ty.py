#!/usr/bin/env python3
"""Stage 18.256 Phase 2a: Add expected_ty: Option<&Ty> param to
lower_expr_to_operand and lower_expr_to_place. Update all call sites
to pass None.

Per plan-18.255.md §4.2.3 + §13.4 J4 (single coherent concept threaded).

Simple line-by-line approach: process each Rust source file, find lines
containing `func_name(cx, ...)` or `func_name(&mut cx, ...)` patterns,
and append `, None` before the matching close paren.

Skips:
  - Function definitions (`fn func_name(`)
  - Comment lines (starting with //)
  - Lines where the call already has 3 args (heuristic: ends with `None)`)
"""

from __future__ import annotations

import re
import sys
from pathlib import Path


def find_matching_paren(s: str, start: int) -> int:
    """Find the index of the close paren matching the open paren at `start`.
    Respects nested parens/braces/brackets and string/char literals.
    """
    depth = 0
    in_string = False
    string_char = ""
    i = start
    while i < len(s):
        c = s[i]
        if in_string:
            if c == "\\":
                i += 2
                continue
            if c == string_char:
                in_string = False
            i += 1
            continue
        # Skip line comments
        if c == "/" and i + 1 < len(s) and s[i + 1] == "/":
            # Skip to end of line
            while i < len(s) and s[i] != "\n":
                i += 1
            continue
        # Skip block comments
        if c == "/" and i + 1 < len(s) and s[i + 1] == "*":
            i += 2
            while i + 1 < len(s) and not (s[i] == "*" and s[i + 1] == "/"):
                i += 1
            i += 2
            continue
        # String literals
        if c == '"':
            in_string = True
            string_char = '"'
            i += 1
            continue
        # Char literals (handle lifetime false positive: 'a is not a char)
        # Heuristic: only treat ' as char-literal-start if the closing ' is
        # within 4 chars AND the content is a single char or escape.
        # For simplicity, just skip ' tracking — it doesn't affect paren matching.
        if c in "([{":
            depth += 1
        elif c in ")]}":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


def transform_line(line: str, func_name: str) -> tuple[str, bool]:
    """Transform `func_name(cx, ARG)` → `func_name(cx, ARG, None)` in a single line.

    Returns (new_line, was_transformed).
    """
    # Skip comment lines.
    stripped = line.lstrip()
    if stripped.startswith("//"):
        return line, False
    # Skip function definitions.
    if re.match(rf"^\s*(pub\s*\()?(crate|super|pub\(crate\)|pub\(super\))?\s*fn\s+{re.escape(func_name)}\b", line):
        return line, False
    # Find call sites: `func_name(cx,` or `func_name(&mut cx,` or `super::func_name(cx,`
    pattern = re.compile(rf"\b(?:super::)?{re.escape(func_name)}\s*\(\s*(?:&mut\s+cx|&cx|cx)\s*,")
    m = pattern.search(line)
    if not m:
        return line, False
    # Find matching close paren starting from the open paren.
    open_paren_idx = line.index("(", m.start())
    close_paren_idx = find_matching_paren(line, open_paren_idx)
    if close_paren_idx == -1:
        return line, False
    # Check if already transformed (heuristic: ends with `None)`).
    inner = line[open_paren_idx + 1 : close_paren_idx]
    if inner.rstrip().endswith("None"):
        return line, False
    # Insert `, None` before close paren.
    new_line = line[:close_paren_idx] + ", None" + line[close_paren_idx:]
    return new_line, True


def transform_file(path: Path, func_names: list[str]) -> int:
    """Transform all call sites in a file. Returns total replacements."""
    content = path.read_text()
    lines = content.split("\n")
    total = 0
    for i, line in enumerate(lines):
        for fname in func_names:
            new_line, transformed = transform_line(line, fname)
            if transformed:
                lines[i] = new_line
                total += 1
                break  # one transformation per line is enough
    if total > 0:
        path.write_text("\n".join(lines))
    return total


def main() -> int:
    base = Path("/home/z/my-project/landin-stage0/src/mir/lower")
    files = ["body_lower.rs", "call_lower.rs", "control_flow.rs", "expr_operand.rs", "expr_variants.rs", "mod.rs"]
    func_names = ["lower_expr_to_operand", "lower_expr_to_place"]
    total = 0
    for fname in files:
        path = base / fname
        if not path.exists():
            print(f"  skip (missing): {fname}")
            continue
        n = transform_file(path, func_names)
        if n > 0:
            print(f"  {fname}: {n} replacements")
            total += n
        else:
            print(f"  {fname}: no changes")
    print(f"Total: {total} replacements")
    return 0


if __name__ == "__main__":
    sys.exit(main())
