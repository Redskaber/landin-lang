#!/usr/bin/env python3
"""Stage 18.262 Phase 2e: bulk-add `None` (fn_sigs) argument to all
test call sites of `lower_hir_body_to_mir_full_with_dyn_trait_plan`.

Per §17.6 缺陷纳入: same-class mechanical update — tests pass None
because they don't have a pre-built fn_sig_table (test context).
"""

from pathlib import Path
import re

target_file = Path("/home/z/my-project/landin-stage0/tests/v0/stage5/plan/driver_dyn_trait_plan_integration_tests.rs")
content = target_file.read_text()

# Find each call site of `lower_hir_body_to_mir_full_with_dyn_trait_plan(`
# and add `None,` after the last `None,` argument (which is currently
# the resolver arg — we need to add fn_sigs=None as the 7th arg).
#
# Strategy: find each call, walk forward to find the matching close
# paren, and insert `None,` before it.

def find_matching_paren(s: str, start: int) -> int:
    """Find the close paren matching the open paren at `start`."""
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
        if c == '"':
            in_string = True
            string_char = '"'
            i += 1
            continue
        if c == "(":
            depth += 1
        elif c == ")":
            depth -= 1
            if depth == 0:
                return i
        i += 1
    return -1


# Pattern: `lower_hir_body_to_mir_full_with_dyn_trait_plan(`
pattern = re.compile(r"lower_hir_body_to_mir_full_with_dyn_trait_plan\(")

out = []
i = 0
count = 0
while i < len(content):
    m = pattern.search(content, i)
    if not m:
        out.append(content[i:])
        break
    # Emit content up to the match.
    out.append(content[i:m.start()])
    # Find open paren (immediately after the match).
    open_paren_idx = m.end() - 1  # the `(`
    close_paren_idx = find_matching_paren(content, open_paren_idx)
    if close_paren_idx == -1:
        out.append(content[m.start():])
        break
    # Extract the inner args.
    inner = content[open_paren_idx + 1 : close_paren_idx]
    # Count actual args by counting top-level commas (handles trailing
    # comma correctly — trailing comma means N args has N commas, not N-1).
    depth = 0
    comma_count = 0
    in_string = False
    string_char = ""
    has_non_whitespace = False  # tracks if there's any non-whitespace content
    for c in inner:
        if in_string:
            if c == string_char:
                in_string = False
            continue
        if c == '"':
            in_string = True
            string_char = '"'
            continue
        if c in "([{":
            depth += 1
            has_non_whitespace = True
        elif c in ")]}":
            depth -= 1
        elif c == "," and depth == 0:
            comma_count += 1
        elif not c.isspace():
            has_non_whitespace = True
    # If trailing comma, comma_count = N args (where N is the actual arg count).
    # If no trailing comma, comma_count = N-1 (commas = args - 1).
    # Check if inner ends with comma (after stripping whitespace).
    inner_stripped = inner.rstrip()
    ends_with_comma = inner_stripped.endswith(",")
    actual_args = comma_count if ends_with_comma else comma_count + 1
    # Skip if already has 7+ args (already updated).
    if actual_args >= 7:
        out.append(content[m.start() : close_paren_idx + 1])
        i = close_paren_idx + 1
        continue
    # Insert `        None,` before the close paren.
    # Preserve indentation by checking the indentation of the previous arg.
    # Find the last newline before close_paren_idx.
    last_nl = content.rfind("\n", open_paren_idx, close_paren_idx)
    if last_nl == -1:
        # Single-line call — insert ` None,` before close paren.
        out.append(content[m.start() : close_paren_idx])
        out.append(" None,")
        out.append(")")
    else:
        # Multi-line call — get indentation from the last arg line.
        line_start = last_nl + 1
        # Find leading whitespace.
        indent = ""
        for c in content[line_start:close_paren_idx]:
            if c in " \t":
                indent += c
            else:
                break
        out.append(content[m.start() : close_paren_idx])
        out.append(f"\n{indent}None,")
        out.append(")")
    i = close_paren_idx + 1
    count += 1

target_file.write_text("".join(out))
print(f"Updated {count} call sites in {target_file.name}")
