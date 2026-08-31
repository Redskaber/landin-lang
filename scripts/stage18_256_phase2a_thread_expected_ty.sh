#!/bin/bash
# Stage 18.256 Phase 2a: Add expected_ty: Option<&Ty> param to
# lower_expr_to_operand and lower_expr_to_place. Update all call sites
# to pass None (preserving existing behavior). Per plan-18.255.md §4.2.3.
#
# Per §1.0 原則 6 (通解 > 特解): one consistent pattern for all call sites.
# Per §13.4 J4: expected_ty is a single coherent concept threaded through.
set -e

cd /home/z/my-project/landin-stage0/src/mir/lower

# Step 1: Update function signatures (manually verified).
# Step 2: Bulk-update all internal call sites in src/mir/lower/.
#
# Pattern A: `lower_expr_to_operand(cx, X)` → `lower_expr_to_operand(cx, X, None)`
# Pattern B: `lower_expr_to_place(cx, X)`   → `lower_expr_to_place(cx, X, None)`
# Pattern C: `super::lower_expr_to_operand(cx, X)` → `super::lower_expr_to_operand(cx, X, None)`
#
# Avoid touching:
#   - Comments (lines starting with // or containing // before the call)
#   - The function definition lines themselves

for f in body_lower.rs call_lower.rs control_flow.rs expr_operand.rs expr_variants.rs mod.rs; do
  # Use perl for non-greedy matching. Match `lower_expr_to_X(cx, EXPR)` where
  # EXPR is a balanced single argument (no nested commas at top level).
  # We restrict to lines that don't start with // and don't have `fn lower_expr_to_X(` (definition).
  perl -i -pe '
        next if /^\s*\/\//;          # skip comment lines
        next if /^\s*\#\[/;          # skip attribute lines
        # Update lower_expr_to_operand call sites (not the definition)
        s/\b(lower_expr_to_operand)\s*\(\s*cx\s*,\s*([^)]+?)\s*\)/${1}(cx, ${2}, None)/g
            unless /^\s*(pub\s*\()?(crate|super|pub\(crate\)|pub\(super\))?\s*fn\s+lower_expr_to_operand\b/;
        # Update lower_expr_to_place call sites (not the definition)
        s/\b(lower_expr_to_place)\s*\(\s*cx\s*,\s*([^)]+?)\s*\)/${1}(cx, ${2}, None)/g
            unless /^\s*(pub\s*\()?(crate|super|pub\(crate\)|pub\(super\))?\s*fn\s+lower_expr_to_place\b/;
    ' "$f"
  echo "  updated: $f"
done

echo "Done."
