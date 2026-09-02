//! Stage 18.249: Low-level + diagnostic macros (extracted from builtin_macros.rs).
//! Per §13.4 J2: owns asm/compile_error/cfg_attr/unreachable/format_args macros.

use super::*;

pub(crate) fn make_asm_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_compile_error_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_cfg_attr_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
/// Body:    `__landin_unreachable($msg.ptr)` — function call to runtime panic
///
/// `unreachable!()` → `__landin_unreachable("internal error: entered unreachable code")`.
/// Simplified: requires a message argument (like panic!).
///
/// Stage 40.3 (v0.28 — TD-UNREACHABLE-MACRO-BROKEN): previously the body was
/// `__landin_unreachable($msg)` which passed a `&str` (fat pointer {ptr, len})
/// to a C function expecting `const char*`. Same bug as TD-PANIC-MACRO-STR-PTR
/// (Stage 40.2). Now we extract the `.ptr` field from the `&str` to pass the
/// raw `const char*` pointer expected by the C runtime.
///
/// Per §20 (iterative audit): discovered by following the same class of bug
/// (macro body not extracting .ptr for &str → C function type mismatch).
/// Per §1.0 原則 6 (通解 > 特解): same fix pattern as panic! macro.
/// Per §12 (最优 > 最小): root-cause fix at macro expansion layer.
pub(crate) fn make_unreachable_macro_rule(interner: &mut Rodeo) -> MacroRule {
    let msg_sym = interner.get_or_intern("msg");
    let expr_sym = interner.get_or_intern("expr");
    let unreach_sym = interner.get_or_intern("__landin_unreachable");
    let ptr_sym = interner.get_or_intern("ptr");

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

    // Body: __landin_unreachable ( $ msg . ptr )
    // Stage 40.3: extract .ptr field from &str to pass raw const char* to C.
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
            kind: TokenKind::Dot,
            span: crate::session::Span::DUMMY,
        },
        Token {
            kind: TokenKind::Ident(ptr_sym),
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
pub(crate) fn make_trace_macros_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_format_args_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_noop_macro_rule(
    name_sym: crate::lexer::Symbol,
    interner: &mut Rodeo,
) -> MacroRule {
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
