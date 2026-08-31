#!/usr/bin/env python3
"""
Stage 30.22 MUV 4 cleanup: Remove orphaned doc comments left by the split.

After extracting functions to new files, some doc comments were left behind
in driver_validations.rs (and possibly the new files) because they were
"orphaned" — not immediately before a function definition.

This script finds and removes orphaned `///` doc comment blocks that are
NOT followed by a valid Rust item (fn, struct, impl, use, etc.).
"""
import re
from pathlib import Path

FILES = [
    Path("/home/z/my-project/landin-stage0/src/driver/driver_validations.rs"),
    Path("/home/z/my-project/landin-stage0/src/driver/driver_validations_impl.rs"),
    Path("/home/z/my-project/landin-stage0/src/driver/driver_validations_struct.rs"),
    Path("/home/z/my-project/landin-stage0/src/driver/driver_validations_trait_object.rs"),
]

# Regex: a `///` doc comment block followed by EOF or a blank line (not an item)
# We consider a doc comment block "orphaned" if it's followed by:
# - EOF
# - a blank line then another `///` block (next orphan) or non-item line
# An "item" starts with: pub, fn, struct, enum, impl, use, mod, const, static, type, trait

ITEM_PATTERN = re.compile(r'^\s*(pub(?:\(super\))?\s+)?(fn|struct|enum|impl|use|mod|const|static|type|trait)\b')

for f in FILES:
    if not f.exists():
        continue
    lines = f.read_text().splitlines(keepends=True)
    new_lines = []
    i = 0
    removed = 0
    while i < len(lines):
        line = lines[i]
        # Check if this line starts a `///` doc comment block
        if line.strip().startswith("///"):
            # Collect the entire doc comment block
            block_start = i
            while i < len(lines) and lines[i].strip().startswith("///"):
                new_lines.append(lines[i])
                i += 1
            # Now check what comes after the doc block
            # Skip blank lines
            j = i
            while j < len(lines) and lines[j].strip() == "":
                j += 1
            # Check if next non-blank line is an item
            if j >= len(lines) or ITEM_PATTERN.match(lines[j]):
                # Valid: doc comment is followed by an item (or EOF after blanks)
                # Keep the blank lines we skipped
                while i < j:
                    new_lines.append(lines[i])
                    i += 1
            else:
                # Orphaned: doc comment is NOT followed by an item
                # Remove the doc block from new_lines
                while new_lines and new_lines[-1].strip().startswith("///"):
                    new_lines.pop()
                    removed += 1
                # Also skip the blank lines after the orphan
                while i < len(lines) and lines[i].strip() == "":
                    i += 1
        else:
            new_lines.append(line)
            i += 1
    if removed > 0:
        f.write_text("".join(new_lines))
        print(f"✓ {f.name}: removed {removed} orphaned doc comment lines")
    else:
        print(f"  - {f.name}: no orphaned doc comments")

print("\nDone.")
