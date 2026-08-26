//! Stage 18.249: Print/assert/panic/write builtin macros (extracted from builtin_macros.rs).
//! Per §13.4 J2 (single responsibility): owns print-family macro rules.

use super::*;

pub(crate) fn make_print_macro_rule(
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
pub(crate) fn make_assert_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_panic_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_vec_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_format_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_dbg_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_panic_msg_macro_rule(_name: &str, interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_write_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
