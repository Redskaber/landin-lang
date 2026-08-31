#!/usr/bin/env python3
"""
Stage 30.22 MUV 2 Phase 2: Remove deprecated-API test functions.

Removes "consistency check" test functions that compare deprecated Spur-based
APIs against their DefId-keyed replacements. Once the deprecated APIs are
removed, these tests have no purpose.

Files modified:
1. stage16_08_builtin_trait_migration_tests.rs — remove test 10
2. stage16_10_vtable_def_id_lookup_tests.rs — remove test 4
3. stage16_12_deep_review_round2_tests.rs — remove tests 1-3
4. stage16_19_design_writeback_tests.rs — remove test 5 (and its #[allow(deprecated)])

Per §1.0 原則 5 (去除兼容思维): no compatibility testing for removed APIs.
Per §1.0 原則 13 (架构限制记录): document the removal in commit message.
"""
import re
from pathlib import Path

BASE = Path("/home/z/my-project/landin-stage0/tests/v0/stage16/plan")

def remove_test_fn(content: str, fn_name: str) -> tuple[str, bool]:
    """Remove a `#[test] fn <fn_name>() { ... }` block (including preceding doc comments and attributes)."""
    # Find the position of `fn <fn_name>`
    pattern = re.compile(
        r'(/// [^\n]*\n)*'  # preceding /// doc comments
        r'(#\[test\]\s*\n)'  # #[test] attribute
        r'(#\[allow\([^)]*\)\]\s*\n)?'  # optional #[allow(...)] attribute
        r'fn ' + re.escape(fn_name) + r'\s*\(\s*\)\s*\{',
        re.MULTILINE,
    )
    match = pattern.search(content)
    if not match:
        return content, False
    # Find matching closing brace at column 0
    start = match.start()
    brace_start = match.end() - 1  # position of opening `{`
    depth = 0
    i = brace_start
    while i < len(content):
        c = content[i]
        if c == '{':
            depth += 1
        elif c == '}':
            depth -= 1
            if depth == 0:
                end = i + 1
                # Also consume trailing blank lines
                while end < len(content) and content[end] in '\n':
                    end += 1
                # Also consume preceding blank lines before the doc comment
                while start > 0 and content[start - 1] == '\n':
                    start -= 1
                # Keep one blank line separator
                if start > 0 and content[start - 1] == '\n':
                    start += 1
                return content[:start] + content[end:], True
        i += 1
    return content, False


# File 1: stage16_08 — remove stage16_08_copy_def_id_and_spur_lookups_agree
f = BASE / "stage16_08_builtin_trait_migration_tests.rs"
content = f.read_text()
new_content, removed = remove_test_fn(content, "stage16_08_copy_def_id_and_spur_lookups_agree")
if removed:
    f.write_text(new_content)
    print(f"✓ {f.name}: removed stage16_08_copy_def_id_and_spur_lookups_agree")
else:
    print(f"✗ {f.name}: FAILED to remove stage16_08_copy_def_id_and_spur_lookups_agree")

# File 2: stage16_10 — remove stage16_10_def_id_and_spur_vtable_lookups_agree
f = BASE / "stage16_10_vtable_def_id_lookup_tests.rs"
content = f.read_text()
new_content, removed = remove_test_fn(content, "stage16_10_def_id_and_spur_vtable_lookups_agree")
if removed:
    f.write_text(new_content)
    print(f"✓ {f.name}: removed stage16_10_def_id_and_spur_vtable_lookups_agree")
else:
    print(f"✗ {f.name}: FAILED to remove stage16_10_def_id_and_spur_vtable_lookups_agree")

# File 3: stage16_12 — remove 3 tests
f = BASE / "stage16_12_deep_review_round2_tests.rs"
content = f.read_text()
for fn_name in [
    "stage16_12_copy_detection_end_to_end_consistency",
    "stage16_12_vtable_lookup_end_to_end_consistency",
    "stage16_12_impl_methods_end_to_end_consistency",
]:
    content, removed = remove_test_fn(content, fn_name)
    if removed:
        print(f"✓ {f.name}: removed {fn_name}")
    else:
        print(f"✗ {f.name}: FAILED to remove {fn_name}")
f.write_text(content)

# File 4: stage16_19 — remove stage16_19_deprecated_methods_still_work
f = BASE / "stage16_19_design_writeback_tests.rs"
content = f.read_text()
new_content, removed = remove_test_fn(content, "stage16_19_deprecated_methods_still_work")
if removed:
    f.write_text(new_content)
    print(f"✓ {f.name}: removed stage16_19_deprecated_methods_still_work")
else:
    print(f"✗ {f.name}: FAILED to remove stage16_19_deprecated_methods_still_work")

print("\nDone. Now check for remaining #![allow(deprecated)] in these files.")
