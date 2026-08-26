//! Builtin macro definitions: print, assert, panic, vec, format, etc.
//!
//! Per `docs/stage-committee-process.md` v6.4 §13.4 J1-J6 (Stage 18.135):
//! Extracted from `macro_expand.rs` to satisfy J6 (科学合理粒度) + J2 (单一职责).
//! This file contains all builtin macro rule constructors (27 functions).
//!
//! ## Sub-responsibility
//! Builtin macros: construct MacroRule definitions for compiler-provided
//! macros like `println!`, `assert!`, `panic!`, `vec!`, `format!`, etc.
//!
//! ## J1-J6 compliance
//! - J1: parser design unchanged (single stage, internal sub-responsibility)
//! - J2: this file has one clear responsibility (builtin macro definitions)
//! - J3: no circular deps (called by macro_expand; no callback)
//! - J4: builtin macros sub-responsibility is complete in this file
//! - J5: stays within parser stage
//! - J6: LOC driven by responsibility, not arbitrary slicing

use super::macro_expand::BUILTIN_MACRO_NAMES;
use crate::ast::{MacroRule, MacroRulesDef};
use crate::lexer::{Token, TokenKind};
use crate::parser::macro_expand::MacroTable;
use lasso::Rodeo;

// Stage 18.249: Builtin macros split into 3 sub-modules
mod compile_time_macros;
mod low_level_macros;
mod print_macros;

use compile_time_macros::{
    make_cfg_macro_rule, make_concat_macro_rule, make_env_macro_rule, make_file_macro_rule,
    make_include_str_macro_rule, make_line_macro_rule, make_matches_macro_rule,
    make_module_path_macro_rule, make_option_env_macro_rule, make_stringify_macro_rule,
};
use low_level_macros::{
    make_asm_macro_rule, make_cfg_attr_macro_rule, make_compile_error_macro_rule,
    make_format_args_macro_rule, make_noop_macro_rule, make_trace_macros_macro_rule,
    make_unreachable_macro_rule,
};
use print_macros::{
    make_assert_macro_rule, make_dbg_macro_rule, make_format_macro_rule, make_panic_macro_rule,
    make_panic_msg_macro_rule, make_print_macro_rule, make_vec_macro_rule, make_write_macro_rule,
};

/// Stage 18.10: Build the table of built-in `macro_rules!` definitions.
///
/// Each built-in macro has a single rule:
/// - **Pattern**: `($($args:tt)*)` — matches any token sequence inside `()`.
/// - **Body**: `name!($($args)*)` — re-emits the same call form (no-op).
///
/// This means `expand_macros` recognizes the macro name but the
/// expanded tokens are identical to the input, so the parser's
/// existing special-case code path still runs. Phase 2 will replace
/// the body with a real expansion to `Call(__landin_println, [...])`.
///
/// **Pre-condition**: `interner` must already contain the built-in
/// macro names (driver pre-interns them via `BUILTIN_MACRO_NAMES`).
/// Names not yet interned are silently skipped.
///
/// Per §10: `build_builtin_macro_table` follows `<verb>_<noun>_<noun>`.
pub fn build_builtin_macro_table(interner: &mut Rodeo) -> MacroTable {
    let mut table = MacroTable::new();
    for name in BUILTIN_MACRO_NAMES {
        if let Some(name_sym) = interner.get(name) {
            let rule = make_builtin_macro_rule(name, name_sym, interner);
            table.insert(
                name_sym,
                MacroRulesDef {
                    name: name_sym,
                    rules: vec![rule],
                    span: crate::session::Span::DUMMY,
                },
            );
        }
    }
    table
}

/// Stage 18.10 + 18.29: Construct a rule for a built-in macro.
///
/// Dispatches to the appropriate rule constructor based on the macro name:
/// - Print macros (println/print/eprintln/eprint) → `make_print_macro_rule`
/// - assert → `make_assert_macro_rule`
/// - panic → `make_panic_macro_rule`
/// - vec → `make_vec_macro_rule`
/// - Other → `make_noop_macro_rule` (pass-through)
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
/// Per §1.0 原則 6 "通用 > 特解": one dispatcher for all built-in macros.
fn make_builtin_macro_rule(
    name: &str,
    name_sym: crate::lexer::Symbol,
    interner: &mut Rodeo,
) -> MacroRule {
    match name {
        "println" | "print" | "eprintln" | "eprint" => {
            make_print_macro_rule(name, name_sym, interner)
        }
        "assert" => make_assert_macro_rule(interner),
        "panic" => make_panic_macro_rule(interner),
        "vec" => make_vec_macro_rule(interner),
        // Stage 18.32: more non-print macros
        "format" => make_format_macro_rule(interner),
        "dbg" => make_dbg_macro_rule(interner),
        "todo" | "unimplemented" => make_panic_msg_macro_rule(name, interner),
        "write" => make_write_macro_rule(interner),
        // Stage 18.34: compile-time utility macros
        "stringify" => make_stringify_macro_rule(interner),
        "concat" => make_concat_macro_rule(interner),
        "env" => make_env_macro_rule(interner),
        // Stage 18.36: source info + file macros
        "file" => make_file_macro_rule(interner),
        "line" => make_line_macro_rule(interner),
        "module_path" => make_module_path_macro_rule(interner),
        "include_str" => make_include_str_macro_rule(interner),
        // Stage 18.39: pattern + config macros
        "matches" => make_matches_macro_rule(interner),
        "cfg" => make_cfg_macro_rule(interner),
        "option_env" => make_option_env_macro_rule(interner),
        // Stage 18.41: low-level + diagnostic macros
        "asm" => make_asm_macro_rule(interner),
        "compile_error" => make_compile_error_macro_rule(interner),
        "cfg_attr" => make_cfg_attr_macro_rule(interner),
        // Stage 18.43: control-flow + debug macros
        "unreachable" => make_unreachable_macro_rule(interner),
        "trace_macros" => make_trace_macros_macro_rule(interner),
        "format_args" => make_format_args_macro_rule(interner),
        _ => make_noop_macro_rule(name_sym, interner),
    }
}
