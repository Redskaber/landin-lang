#!/usr/bin/env python3
"""
Stage 101: Add mono_names + type_name_by_def_id params to codegen functions.

Updates:
- codegen_function: signature + pass to codegen_statement/codegen_terminator
- codegen_statement: signature + pass to codegen_rvalue
- codegen_rvalue: signature + pass to codegen_operand
- codegen_terminator: signature + pass to codegen_operand
- codegen_function callers: pass mono_names + type_name_by_def_id

Note: codegen_operand was already updated (manual). This script updates the rest.
"""

import re
from pathlib import Path

FILES = [
    "src/codegen/function.rs",
    "src/codegen/statement.rs",
    "src/codegen/rvalue.rs",
    "src/codegen/terminator.rs",
]

# Pattern: codegen_operand(... fn_name_by_def_id,)
# Need to add mono_names, type_name_by_def_id before closing paren
OPERAND_CALL_PATTERN = re.compile(
    r"codegen_operand\(\s*([^;]+?)fn_name_by_def_id,\s*\)"
)

ROOT = Path("/home/z/my-project/landin-stage0")

for fpath in FILES:
    p = ROOT / fpath
    if not p.exists():
        print(f"SKIP (not found): {fpath}")
        continue
    content = p.read_text()
    orig = content
    # 1. Add params to function signatures
    # codegen_statement signature: ... fn_name_by_def_id: &HashMap<...> String,
    content = re.sub(
        r"(pub\(crate\) fn codegen_statement\(\s*[^)]*?fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,)\s*\)",
        r"\1\n    // Stage 101: mono_names + type_name_by_def_id for FnDef substs mangling.\n    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,\n    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,\n)",
        content,
        count=1,
        flags=re.DOTALL,
    )
    # codegen_rvalue signature
    content = re.sub(
        r"(pub\(crate\) fn codegen_rvalue\(\s*[^)]*?fn_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, String>,)\s*\)",
        r"\1\n    // Stage 101: mono_names + type_name_by_def_id for FnDef substs mangling.\n    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,\n    type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,\n)",
        content,
        count=1,
        flags=re.DOTALL,
    )
    # codegen_terminator signature
    content = re.sub(
        r"(pub\(crate\) fn codegen_terminator\(\s*[^)]*?type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,)\s*\)",
        r"\1\n    // Stage 101: mono_names for FnDef substs mangling in codegen_operand.\n    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,\n)",
        content,
        count=1,
        flags=re.DOTALL,
    )
    # codegen_function signature (already has type_name_by_def_id, add mono_names after)
    content = re.sub(
        r"(pub\(crate\) fn codegen_function\(\s*[^)]*?type_name_by_def_id: &std::collections::HashMap<crate::hir::DefId, crate::lexer::Symbol>,)\s*\)",
        r"\1\n    // Stage 101: mono_names for FnDef substs mangling in codegen_operand.\n    mono_names: &std::collections::HashMap<crate::mir::monomorphize::MonoItem, String>,\n)",
        content,
        count=1,
        flags=re.DOTALL,
    )
    # 2. Update codegen_operand calls: add mono_names, type_name_by_def_id before closing )
    # Pattern: codegen_operand(\n ... fn_name_by_def_id,\n)
    content = re.sub(
        r"codegen_operand\(([^;]*?fn_name_by_def_id),\s*\)",
        r"codegen_operand(\1, mono_names, type_name_by_def_id)",
        content,
        flags=re.DOTALL,
    )
    # 3. Update codegen_rvalue calls: add mono_names, type_name_by_def_id
    content = re.sub(
        r"codegen_rvalue\(([^;]*?fn_name_by_def_id),\s*\)",
        r"codegen_rvalue(\1, mono_names, type_name_by_def_id)",
        content,
        flags=re.DOTALL,
    )
    # 4. Update codegen_statement calls: add mono_names, type_name_by_def_id
    content = re.sub(
        r"codegen_statement\(([^;]*?fn_name_by_def_id),\s*\)",
        r"codegen_statement(\1, mono_names, type_name_by_def_id)",
        content,
        flags=re.DOTALL,
    )
    # 5. Update codegen_terminator calls: add mono_names after type_name_by_def_id
    content = re.sub(
        r"codegen_terminator\(([^;]*?type_name_by_def_id),\s*\)",
        r"codegen_terminator(\1, mono_names)",
        content,
        flags=re.DOTALL,
    )
    if content != orig:
        p.write_text(content)
        print(f"UPDATED: {fpath}")
    else:
        print(f"NO CHANGE: {fpath}")
