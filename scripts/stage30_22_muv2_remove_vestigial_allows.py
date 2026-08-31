#!/usr/bin/env python3
"""
Stage 30.22 MUV 2 Phase 6: Remove vestigial #![allow(deprecated)] attributes.

After migrating all deprecated API calls to their replacements, many test files
still have `#![allow(deprecated)]` attributes that are no longer needed.

This script removes the `#![allow(deprecated)]` line from files where there
are NO remaining deprecated API calls.

Per §1.0 原則 5 (去除兼容思维): vestigial attributes are compatibility mindset.
Per §1.0 原則 3 (显式 > 隐式): remove silent suppressions.
"""
import re
from pathlib import Path

BASE = Path("/home/z/my-project/landin-stage0/tests/v0")

# Files to clean (verified to have no remaining deprecated API calls)
TARGET_FILES = [
    # Phase 2 cleanup (deprecated calls already removed)
    BASE / "stage16/plan/stage16_10_vtable_def_id_lookup_tests.rs",
    BASE / "stage16/plan/stage16_12_deep_review_round2_tests.rs",
    BASE / "stage16/plan/stage16_19_design_writeback_tests.rs",
    BASE / "stage16/plan/stage16_07_def_id_keyed_lookup_tests.rs",
    BASE / "stage16/plan/stage16_08_builtin_trait_migration_tests.rs",
    # Vestigial (only comments mention check_mir_body, which is not actually deprecated)
    BASE / "stage15/plan/kill_borrows_dataflow_tests.rs",
    BASE / "stage15/plan/stage15_37_driver_switch_tests.rs",
    BASE / "stage15/plan/stage15_41_legacy_delegation_tests.rs",
    BASE / "stage15/plan/borrowck_comparison_diagnostic_tests.rs",
    BASE / "stage15/plan/option_b_implementation_tests.rs",
    BASE / "stage15/plan/stage15_40_driver_switch_tests.rs",
    BASE / "stage8/plan/lifetime_elision_tests.rs",
    BASE / "stage8/plan/drop_elaboration_tests.rs",
    BASE / "stage8/plan/deep_review_tests.rs",
    BASE / "stage7/plan/systematic_review_v014_tests.rs",
    BASE / "stage7/plan/deep_review_tests.rs",
    BASE / "stage7/plan/design_writeback_verification_tests.rs",
    BASE / "stage7/plan/region_inference_tests.rs",
]

PATTERN = re.compile(r'^#![ ]*\[allow\(deprecated\)\][^\n]*\n', re.MULTILINE)

for f in TARGET_FILES:
    if not f.exists():
        print(f"  ⚠ {f.name}: file not found, skipping")
        continue
    content = f.read_text()
    new_content, n = PATTERN.subn('', content)
    if n > 0:
        f.write_text(new_content)
        print(f"✓ {f.name}: removed {n} #![allow(deprecated)] attribute(s)")
    else:
        print(f"  - {f.name}: no #![allow(deprecated)] found")

print("\nDone.")
