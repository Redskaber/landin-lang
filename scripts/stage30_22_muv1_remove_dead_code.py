#!/usr/bin/env python3
"""
Stage 30.22 MUV 1: Remove dead code from writeback.rs.

Removes:
1. Lines 27-305: `writeback_field_types_with_table` dead method + 4 nested helper fns
   (resolve_place_type_with_table, writeback_field_types_in_place_with_table,
    writeback_field_types_in_rvalue_with_table, writeback_field_types_in_operand_with_table)
2. Lines 488-514: `typeck_type_contains_param` dead module-level fn

Per §1.0 原則 5 (去除兼容思维): dead code "kept for reference" is compatibility mindset.
Per §1.0 原則 3 (显式 > 隐式): dead code obscures the active code path.
Per §12 (最优 > 最小): root-cause fix is to remove, not preserve.

Active methods preserved:
- writeback_field_load_locals_with_table (Pass 1, active)
- resolve_place_for_writeback (active)
"""
import sys
from pathlib import Path

target = Path("/home/z/my-project/landin-stage0/src/typeck/writeback.rs")
src = target.read_text()
lines = src.splitlines(keepends=True)

# Convert to 0-indexed
# Remove lines 27-305 (1-indexed) → indices 26-304
# Remove lines 488-514 (1-indexed) → indices 487-513
# But we need to remove the higher-indexed block FIRST so the lower indices stay valid.

# Find the dead module-level fn block end (line 515 = EOF after newline)
# Block 2: lines 488-514 (1-indexed) = indices 487-513
# But line 515 might be just "\n" or end-of-file. Let's check.
# Also line 487 is blank (separator between `}` of impl and the doc comment)

# Verify boundaries by content
assert lines[26].strip().startswith("/// Stage 18.388"), f"Line 27 mismatch: {lines[26]!r}"
assert lines[304].strip() == "}", f"Line 305 mismatch: {lines[304]!r}"
# Line 487 should be blank or end of impl `}`
# Find the doc comment for typeck_type_contains_param
typeck_param_start = None
for i, line in enumerate(lines):
    if "Stage 18.376 (TD-ARCH-NESTED-GENERIC-FIELD-ACCESS): Local helper" in line:
        typeck_param_start = i
        break
assert typeck_param_start is not None, "Could not find typeck_type_contains_param doc comment"
# The doc comment starts a few lines above the actual fn. Find the start of the comment block.
# Look backwards for the first `///` line preceded by a blank/non-doc line
i = typeck_param_start
while i > 0 and (lines[i - 1].strip().startswith("///") or lines[i - 1].strip().startswith("//!")):
    i -= 1
doc_start = i
# Now find the end of the fn (closing `}` at column 0)
fn_end = None
for j in range(typeck_param_start, len(lines)):
    if lines[j].rstrip() == "}":
        fn_end = j
        break
assert fn_end is not None, "Could not find end of typeck_type_contains_param"
# Include the trailing newline if any
if fn_end + 1 < len(lines) and lines[fn_end + 1].strip() == "":
    fn_end_inclusive = fn_end + 1  # also consume trailing blank line
else:
    fn_end_inclusive = fn_end

print(f"Block 1 (writeback_field_types_with_table): lines 27-305 (indices 26-304)")
print(f"Block 2 (typeck_type_contains_param): lines {doc_start+1}-{fn_end+1} (indices {doc_start}-{fn_end})")
print(f"  (will also remove trailing blank line at index {fn_end_inclusive})")

# Remove Block 2 first (higher indices) so Block 1 indices stay valid
new_lines = lines[:doc_start] + lines[fn_end_inclusive + 1:]
# Now remove Block 1 (indices 26-304)
# After Block 2 removal, indices for Block 1 are unchanged (Block 2 was after Block 1)
new_lines = new_lines[:26] + new_lines[305:]

target.write_text("".join(new_lines))
print(f"Wrote {target} ({len(new_lines)} lines, was {len(lines)} lines, removed {len(lines) - len(new_lines)} lines)")
