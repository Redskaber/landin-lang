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

/// Stage 18.10: Construct a print macro rule (println/print/eprintln/eprint).
///
/// Pattern: `$($args:tt)*`
/// Body:    `__landin_<name>($($args)*)`
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_print_macro_rule(
    name: &str,
    name_sym: crate::lexer::Symbol,
    interner: &mut Rodeo,
) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();

    // Pattern: $ ( $ args : tt ) *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_<name> ( $ ( $ args ) * )
    let landin_name = format!("__landin_{name}");
    let landin_name_sym = interner.get(&landin_name).unwrap_or(name_sym);

    let body = vec![
        Token {
            kind: TokenKind::Ident(landin_name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.29: Construct an `assert!` macro rule.
///
/// Pattern: `$cond:expr` — matches a single expression (the condition)
/// Body:    `__landin_assert($cond)` — function call to runtime assert
///
/// The codegen detects `__landin_assert` and generates a conditional
/// panic (if !cond → panic).
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_assert_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let cond_sym = interner.get_or_intern("cond");
    let expr_sym = interner.get_or_intern("expr");
    let assert_sym = interner.get_or_intern("__landin_assert");

    // Pattern: $ cond : expr
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cond_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_assert ( $ cond )
    let body = vec![
        Token {
            kind: TokenKind::Ident(assert_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cond_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.29: Construct a `panic!` macro rule.
///
/// Pattern: `$msg:expr` — matches a single expression (the message)
/// Body:    `__landin_panic_msg($msg)` — function call to runtime panic
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_panic_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let msg_sym = interner.get_or_intern("msg");
    let expr_sym = interner.get_or_intern("expr");
    let panic_msg_sym = interner.get_or_intern("__landin_panic_msg");

    // Pattern: $ msg : expr
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_panic_msg ( $ msg )
    let body = vec![
        Token {
            kind: TokenKind::Ident(panic_msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.29: Construct a `vec!` macro rule.
///
/// Pattern: `$( $x:expr ),*` — comma-separated expressions
/// Body:    `[ $( $x ),* ]` — array literal
///
/// This expands `vec![1, 2, 3]` to `[1, 2, 3]` (array literal).
/// The parser handles array literals natively.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_vec_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let x_sym = interner.get_or_intern("x");
    let expr_sym = interner.get_or_intern("expr");

    // Pattern: $ ( $ x : expr ) , *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: [ $ ( $ x ) , * ]
    let body = vec![
        Token {
            kind: TokenKind::LBracket,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RBracket,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.32: Construct a `format!` macro rule.
///
/// Pattern: `$($args:tt)*` — any token sequence (format string + args)
/// Body:    `__landin_format($($args)*)` — function call to runtime format
///
/// `format!("x={}", x)` → `__landin_format("x={}", x)` → returns a string.
/// For now, this is a pass-through to the runtime function.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_format_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();
    let fmt_sym = interner.get_or_intern("__landin_format");

    // Pattern: $ ( $ args : tt ) *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_format ( $ ( $ args ) * )
    let body = vec![
        Token {
            kind: TokenKind::Ident(fmt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.32: Construct a `dbg!` macro rule.
///
/// Pattern: `$x:expr` — a single expression
/// Body:    `__landin_dbg($x)` — function call to runtime dbg
///
/// `dbg!(x)` → `__landin_dbg(x)` → prints and returns the value.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_dbg_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let x_sym = interner.get_or_intern("x");
    let expr_sym = interner.get_or_intern("expr");
    let dbg_sym = interner.get_or_intern("__landin_dbg");

    // Pattern: $ x : expr
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_dbg ( $ x )
    let body = vec![
        Token {
            kind: TokenKind::Ident(dbg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.32: Construct a `todo!` / `unimplemented!` macro rule.
///
/// Pattern: `$( $msg:expr )?` — optional message
/// Body:    `__landin_panic_msg("not implemented")` or `__landin_panic_msg($msg)`
///
/// `todo!()` → `__landin_panic_msg("not implemented")`
/// `unimplemented!()` → `__landin_panic_msg("not implemented")`
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>_<noun>`.
fn make_panic_msg_macro_rule(_name: &str, interner: &mut Rodeo) -> MacroRule {
    let msg_sym = interner.get_or_intern("msg");
    let expr_sym = interner.get_or_intern("expr");
    let panic_msg_sym = interner.get_or_intern("__landin_panic_msg");
    // Stage 18.32: Both todo! and unimplemented! use the same message.
    let default_msg = "not implemented";
    let default_msg_sym = interner.get_or_intern(default_msg);

    // Pattern: $ ( $ msg : expr ) ?
    // Simplified: just use $msg:expr (required, single expression)
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_panic_msg ( $ msg )
    let body = vec![
        Token {
            kind: TokenKind::Ident(panic_msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    let _ = default_msg_sym; // reserved for future default-message rule
    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.32: Construct a `write!` macro rule.
///
/// Pattern: `$dst:expr, $($args:tt)*` — destination + format args
/// Body:    `__landin_write($dst, $($args)*)` — function call to runtime write
///
/// `write!(dst, "x={}", x)` → `__landin_write(dst, "x={}", x)`
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_write_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let dst_sym = interner.get_or_intern("dst");
    let expr_sym = interner.get_or_intern("expr");
    let args_sym = interner.get_or_intern("args");
    let tt_sym = interner.get_or_intern("tt");
    let write_sym = interner.get_or_intern("__landin_write");

    // Pattern: $ dst : expr , $ ( $ args : tt ) *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(dst_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_write ( $ dst , $ ( $ args ) * )
    let body = vec![
        Token {
            kind: TokenKind::Ident(write_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(dst_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.34: Construct a `stringify!` macro rule.
///
/// Pattern: `$($args:tt)*` — any token sequence
/// Body:    `__landin_stringify($($args)*)` — function call to runtime stringify
///
/// `stringify!(x + 1)` → `__landin_stringify(x + 1)` → returns "x + 1".
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_stringify_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();
    let stringify_sym = interner.get_or_intern("__landin_stringify");

    // Pattern: $ ( $ args : tt ) *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_stringify ( $ ( $ args ) * )
    let body = vec![
        Token {
            kind: TokenKind::Ident(stringify_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.34: Construct a `concat!` macro rule.
///
/// Pattern: `$( $x:expr ),*` — comma-separated expressions
/// Body:    `__landin_concat($($x),*)` — function call to runtime concat
///
/// `concat!("a", "b")` → `__landin_concat("a", "b")` → returns "ab".
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_concat_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let x_sym = interner.get_or_intern("x");
    let expr_sym = interner.get_or_intern("expr");
    let concat_sym = interner.get_or_intern("__landin_concat");

    // Pattern: $ ( $ x : expr ) , *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_concat ( $ ( $ x ) , * )
    let body = vec![
        Token {
            kind: TokenKind::Ident(concat_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(x_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.34: Construct an `env!` macro rule.
///
/// Pattern: `$name:expr` — a single expression (the env var name)
/// Body:    `__landin_env($name)` — function call to runtime env
///
/// `env!("CARGO_PKG_NAME")` → `__landin_env("CARGO_PKG_NAME")`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_env_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let name_sym = interner.get_or_intern("name");
    let expr_sym = interner.get_or_intern("expr");
    let env_sym = interner.get_or_intern("__landin_env");

    // Pattern: $ name : expr
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_env ( $ name )
    let body = vec![
        Token {
            kind: TokenKind::Ident(env_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.36: Construct a `file!` macro rule.
///
/// Pattern: empty (no arguments)
/// Body:    `__landin_file()` — function call returning the current file name.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_file_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let file_sym = interner.get_or_intern("__landin_file");
    let pattern: Vec<Token> = vec![];
    let body = vec![
        Token {
            kind: TokenKind::Ident(file_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.36: Construct a `line!` macro rule.
///
/// Pattern: empty (no arguments)
/// Body:    `__landin_line()` — function call returning the current line number.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_line_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let line_sym = interner.get_or_intern("__landin_line");
    let pattern: Vec<Token> = vec![];
    let body = vec![
        Token {
            kind: TokenKind::Ident(line_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.36: Construct a `module_path!` macro rule.
///
/// Pattern: empty (no arguments)
/// Body:    `__landin_module_path()` — returns the current module path string.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_module_path_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let mp_sym = interner.get_or_intern("__landin_module_path");
    let pattern: Vec<Token> = vec![];
    let body = vec![
        Token {
            kind: TokenKind::Ident(mp_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.36: Construct an `include_str!` macro rule.
///
/// Pattern: `$path:expr` — a single expression (the file path)
/// Body:    `__landin_include_str($path)` — returns the file contents as a string.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_include_str_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let path_sym = interner.get_or_intern("path");
    let expr_sym = interner.get_or_intern("expr");
    let inc_sym = interner.get_or_intern("__landin_include_str");
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(path_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];
    let body = vec![
        Token {
            kind: TokenKind::Ident(inc_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(path_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];
    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.39: Construct a `matches!` macro rule.
///
/// Pattern: `$expr:expr, $($pat:tt)+` — expression + pattern tokens
/// Body:    `__landin_matches($expr, $($pat)+)` — function call to runtime matches
///
/// `matches!(x, Some(_))` → `__landin_matches(x, Some(_))` → returns bool.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_matches_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let expr_sym = interner.get_or_intern("expr");
    let pat_sym = interner.get_or_intern("pat");
    let tt_sym = interner.get_or_intern("tt");
    let matches_sym = interner.get_or_intern("__landin_matches");

    // Pattern: $ expr : expr , $ ( $ pat : tt ) +
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(pat_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Plus,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_matches ( $ expr , $ ( $ pat ) + )
    let body = vec![
        Token {
            kind: TokenKind::Ident(matches_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(pat_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Plus,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.39: Construct a `cfg!` macro rule.
///
/// Pattern: `$cfg:tt` — a single token tree (the cfg expression)
/// Body:    `__landin_cfg($cfg)` — function call returning bool
///
/// `cfg!(target_os = "linux")` → `__landin_cfg(target_os = "linux")`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_cfg_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let cfg_sym = interner.get_or_intern("cfg");
    let tt_sym = interner.get_or_intern("tt");
    let landin_cfg_sym = interner.get_or_intern("__landin_cfg");

    // Pattern: $ cfg : tt
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cfg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_cfg ( $ cfg )
    let body = vec![
        Token {
            kind: TokenKind::Ident(landin_cfg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cfg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.39: Construct an `option_env!` macro rule.
///
/// Pattern: `$name:expr` — a single expression (the env var name)
/// Body:    `__landin_option_env($name)` — returns Option<&str>
///
/// `option_env!("HOME")` → `__landin_option_env("HOME")`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_option_env_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let name_sym = interner.get_or_intern("name");
    let expr_sym = interner.get_or_intern("expr");
    let oe_sym = interner.get_or_intern("__landin_option_env");

    // Pattern: $ name : expr
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_option_env ( $ name )
    let body = vec![
        Token {
            kind: TokenKind::Ident(oe_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.41: Construct an `asm!` macro rule.
///
/// Pattern: `$($args:tt)*` — any token sequence (assembly template + operands)
/// Body:    `__landin_asm($($args)*)` — function call to runtime asm stub
///
/// `asm!("nop")` → `__landin_asm("nop")`.
/// For now, the runtime stub is a no-op (inline assembly not supported).
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_asm_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();
    let asm_sym = interner.get_or_intern("__landin_asm");

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(asm_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.41: Construct a `compile_error!` macro rule.
///
/// Pattern: `$msg:expr` — a single expression (the error message)
/// Body:    `__landin_compile_error($msg)` — function call to runtime error
///
/// `compile_error!("custom error")` → `__landin_compile_error("custom error")`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_compile_error_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let msg_sym = interner.get_or_intern("msg");
    let expr_sym = interner.get_or_intern("expr");
    let ce_sym = interner.get_or_intern("__landin_compile_error");

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(ce_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.41: Construct a `cfg_attr!` macro rule.
///
/// Pattern: `$cfg:expr, $($attr:tt)*` — cfg expression + attribute tokens
/// Body:    `__landin_cfg_attr($cfg, $($attr)*)` — function call
///
/// `cfg_attr!(debug, derive(Debug))` → `__landin_cfg_attr(debug, derive(Debug))`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_cfg_attr_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let cfg_sym = interner.get_or_intern("cfg");
    let expr_sym = interner.get_or_intern("expr");
    let attr_sym = interner.get_or_intern("attr");
    let tt_sym = interner.get_or_intern("tt");
    let ca_sym = interner.get_or_intern("__landin_cfg_attr");

    // Pattern: $ cfg : expr , $ ( $ attr : tt ) *
    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cfg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(attr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    // Body: __landin_cfg_attr ( $ cfg , $ ( $ attr ) * )
    let body = vec![
        Token {
            kind: TokenKind::Ident(ca_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(cfg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Comma,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(attr_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.43: Construct an `unreachable!` macro rule.
///
/// Pattern: `$( $msg:expr )?` — optional message
/// Body:    `__landin_unreachable($msg)` — function call to runtime panic
///
/// `unreachable!()` → `__landin_unreachable("internal error: entered unreachable code")`.
/// Simplified: requires a message argument (like panic!).
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_unreachable_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let msg_sym = interner.get_or_intern("msg");
    let expr_sym = interner.get_or_intern("expr");
    let unreach_sym = interner.get_or_intern("__landin_unreachable");

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(unreach_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(msg_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.43: Construct a `trace_macros!` macro rule.
///
/// Pattern: `$mode:expr` — a single expression (true/false)
/// Body:    `__landin_trace_macros($mode)` — function call (no-op at runtime)
///
/// `trace_macros!(true)` → `__landin_trace_macros(true)`.
/// This is a debug-only macro that controls macro expansion tracing.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_trace_macros_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let mode_sym = interner.get_or_intern("mode");
    let expr_sym = interner.get_or_intern("expr");
    let tm_sym = interner.get_or_intern("__landin_trace_macros");

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(mode_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(expr_sym),
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(tm_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(mode_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.43: Construct a `format_args!` macro rule.
///
/// Pattern: `$($args:tt)*` — any token sequence (format string + args)
/// Body:    `__landin_format_args($($args)*)` — function call to runtime
///
/// `format_args!("x={}", x)` → `__landin_format_args("x={}", x)`.
/// This is the low-level macro that println!/format! are built on.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_format_args_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();
    let fa_sym = interner.get_or_intern("__landin_format_args");

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(fa_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}

/// Stage 18.29: Construct a no-op pass-through rule for unknown built-ins.
///
/// Pattern: `$($args:tt)*`
/// Body:    `name!($($args)*)` — re-emit same call form
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn make_noop_macro_rule(name_sym: crate::lexer::Symbol, interner: &mut Rodeo) -> MacroRule {
    let args_sym = interner.get("args").unwrap_or_default();
    let tt_sym = interner.get("tt").unwrap_or_default();

    let pattern = vec![
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Colon,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(tt_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
    ];

    let body = vec![
        Token {
            kind: TokenKind::Ident(name_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Not,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::LParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(args_sym),
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::RParen,
            span: crate::session::Span::DUMMY,
        },
    ];

    MacroRule {
        pattern,
        body,
        span: crate::session::Span::DUMMY,
    }
}
