//! Stage 18.249: Compile-time utility macros (extracted from builtin_macros.rs).
//! Per §13.4 J2: owns stringify/concat/env/file/line/matches/cfg macros.

use super::*;

pub(crate) fn make_stringify_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_concat_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_env_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_file_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_line_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_module_path_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_include_str_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_matches_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_cfg_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
pub(crate) fn make_option_env_macro_rule(interner: &mut Rodeo) -> MacroRule {
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
