#!/usr/bin/env python3
"""
Stage 30.22 MUV 4: Split driver_validations.rs by validation responsibility.

Per §13.4 J6 (科学合理粒度): granularity driven by responsibility, not LOC.
driver_validations.rs has 5 distinct validation responsibilities:
1. Impl validations (method signatures, assoc types, HRTB bounds)
2. Struct literal validations (fields, in expr, single literal)
3. Trait object validations (object safety, trait object ty)
4. Misc validations (pattern arity, assignment targets, cast types) — kept in original
5. Orchestrator (run_post_typeck_validations) + utilities — kept in original

Creates 3 new files + leaves the rest in driver_validations.rs:
- driver_validations_impl.rs (~493 LOC) — impl validations
- driver_validations_struct.rs (~230 LOC) — struct literal validations
- driver_validations_trait_object.rs (~142 LOC) — trait object validations
- driver_validations.rs (remaining ~750 LOC) — orchestrator + misc

Per §13.4 J2 (单一职责): each new file has one clear responsibility.
"""
import re
from pathlib import Path

SRC = Path("/home/z/my-project/landin-stage0/src/driver/driver_validations.rs")
content = SRC.read_text()
lines = content.splitlines(keepends=True)

# Function start lines (1-indexed)
FUNCTION_STARTS = {
    19: ("owner_return_ty", "misc"),           # keep in original
    48: ("validate_impl_method_signatures", "impl"),
    259: ("validate_impl_assoc_types", "impl"),
    425: ("validate_hrtb_bounds", "impl"),
    541: ("mir_ty_kinds_compatible", "misc"),  # keep in original
    615: ("validate_struct_literal_fields", "struct"),
    684: ("check_struct_literal_in_expr", "struct"),
    791: ("validate_one_struct_literal", "struct"),
    845: ("validate_pattern_arity", "misc"),   # keep in original
    930: ("validate_assignment_targets", "misc"),  # keep in original
    1076: ("validate_cast_types", "misc"),     # keep in original
    1275: ("run_post_typeck_validations", "misc"),  # keep in original (orchestrator)
    1473: ("check_object_safety_for_dyn_trait_usage", "trait_object"),
    1583: ("check_trait_object_ty", "trait_object"),
}

# Find function end lines (line before next function start, or EOF)
func_starts_sorted = sorted(FUNCTION_STARTS.keys())
func_ranges = {}  # fn_name -> (start_1indexed, end_1indexed_inclusive)
for i, start in enumerate(func_starts_sorted):
    fn_name, category = FUNCTION_STARTS[start]
    if i + 1 < len(func_starts_sorted):
        end = func_starts_sorted[i + 1] - 1
    else:
        end = len(lines)
    # Trim trailing blank lines
    while end > start and lines[end - 1].strip() == "":
        end -= 1
    func_ranges[fn_name] = (start, end, category)

# Group functions by category
categories = {"impl": [], "struct": [], "trait_object": [], "misc": []}
for fn_name, (start, end, cat) in func_ranges.items():
    categories[cat].append((fn_name, start, end))

# File headers
HEADERS = {
    "impl": """//! Driver impl validations: method signatures, associated types, HRTB bounds.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 30.22):
//! Extracted from `driver_validations.rs` to satisfy J2 (单一职责) + J6 (科学合理粒度).
//! This file owns all impl-block validations: method signature conformance,
//! associated type definitions, and higher-ranked trait bound checking.

use super::driver_scan::{walk_hir_ty, walk_hir_ty_in_body};
use super::CompileErrors;
use crate::hir::*;
use crate::typeck::TypeError;
use lasso::Rodeo;

""",
    "struct": """//! Driver struct literal validations: field conformance, in-expr checks, single-literal validation.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 30.22):
//! Extracted from `driver_validations.rs` to satisfy J2 (单一职责) + J6 (科学合理粒度).
//! This file owns all struct-literal validations: field type/arity conformance,
//! struct literal in expression context, and single-literal validation.

use super::driver_scan::{walk_hir_ty, walk_hir_ty_in_body};
use super::CompileErrors;
use crate::hir::*;
use crate::typeck::TypeError;
use lasso::Rodeo;

""",
    "trait_object": """//! Driver trait object validations: object safety checks, trait object type validation.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 30.22):
//! Extracted from `driver_validations.rs` to satisfy J2 (单一职责) + J6 (科学合理粒度).
//! This file owns all trait-object validations: dyn trait object safety,
//! and trait object type checking.

use super::driver_scan::{walk_hir_ty, walk_hir_ty_in_body};
use super::CompileErrors;
use crate::hir::*;
use crate::typeck::TypeError;
use lasso::Rodeo;

""",
}

# Build new files
for cat in ["impl", "struct", "trait_object"]:
    fns = categories[cat]
    if not fns:
        continue
    new_path = SRC.parent / f"driver_validations_{cat}.rs"
    parts = [HEADERS[cat]]
    for fn_name, start, end in fns:
        # Include preceding doc comments (/// lines)
        doc_start = start
        while doc_start > 1 and lines[doc_start - 2].strip().startswith("///"):
            doc_start -= 1
        # Extract function with doc comments
        parts.append("".join(lines[doc_start - 1 : end]))
        parts.append("\n\n")
    new_path.write_text("".join(parts))
    total_lines = sum(end - doc_start + 1 for _, (fn_name, start, end, cat) in [(None, (fn, s, e, cat)) for fn, s, e in fns] for cat in [cat])
    print(f"✓ Created {new_path.name} ({sum(end - start + 1 for _, start, end in fns)} LOC of functions)")

# Rebuild driver_validations.rs with only "misc" functions + updated header
misc_fns = categories["misc"]
new_main_parts = ["""//! Driver validation orchestrator + misc validations: pattern arity, assignment targets, cast types.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.134 + 30.22):
//! Extracted from `driver.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//!
//! Stage 30.22: Split out 3 sibling files for distinct validation categories:
//! - `driver_validations_impl.rs` — impl method signatures, assoc types, HRTB bounds
//! - `driver_validations_struct.rs` — struct literal field/in-expr/single validations
//! - `driver_validations_trait_object.rs` — object safety + trait object type checks
//!
//! This file retains the orchestrator (`run_post_typeck_validations`) + misc
//! validations (pattern arity, assignment targets, cast types) + shared utilities.

use super::driver_scan::{walk_hir_ty, walk_hir_ty_in_body};
use super::CompileErrors;
use crate::hir::*;
use crate::typeck::TypeError;
use lasso::Rodeo;

"""]
for fn_name, start, end in misc_fns:
    # Include preceding doc comments
    doc_start = start
    while doc_start > 1 and lines[doc_start - 2].strip().startswith("///"):
        doc_start -= 1
    new_main_parts.append("".join(lines[doc_start - 1 : end]))
    new_main_parts.append("\n\n")
SRC.write_text("".join(new_main_parts))
print(f"✓ Rewrote {SRC.name} ({sum(end - start + 1 for _, start, end in misc_fns)} LOC of misc functions)")
print("\nDone. Next: update driver/mod.rs to declare the 3 new submodules.")
