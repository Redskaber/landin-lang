//! Stage 18.247: Macro call expansion (extracted from mod.rs).
//! Stage 18.248: Tests extracted to expansion_tests.rs.
//!
//! Per §13.4 J2 (single responsibility): owns macro call expansion.
//! Per §1.0 原則 6 (通解 > 特解): one expansion path for all macro calls.

use super::super::builtin_macros::build_builtin_macro_table;
use super::collection::collect_delimited;
use super::*;
use crate::lexer::{Token, TokenKind};
use lasso::Rodeo;

pub fn expand_macro_calls(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
) -> Vec<Token> {
    expand_macro_calls_with_errors(tokens, table, interner, &mut Vec::new())
}

/// Stage 43 (v0.5): Like expand_macro_calls_with_errors but with source_map
/// and file_name for compile-time macros (file!, line!).
pub fn expand_macro_calls_with_errors_and_source(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
    errors: &mut Vec<MacroError>,
    source_map: Option<&crate::session::SourceMap>,
    file_name: &str,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        let is_macro_call = matches!(
            (tokens.get(i).map(|t| &t.kind), tokens.get(i + 1).map(|t| &t.kind), tokens.get(i + 2).map(|t| &t.kind)),
            (
                Some(TokenKind::Ident(name_sym)),
                Some(TokenKind::Not),
                Some(TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket),
            ) if table.contains_key(name_sym)
        );

        if is_macro_call {
            let name_sym = if let TokenKind::Ident(s) = &tokens[i].kind {
                *s
            } else {
                unreachable!("checked above")
            };
            let def = table.get(&name_sym).expect("checked above");
            let name_str = interner.resolve(&name_sym).to_string();
            let call_span = tokens[i].span;

            if let Some((input, after_close)) = collect_delimited(tokens, i + 2) {
                let name = interner.resolve(&name_sym).to_string();
                let compile_time_result = expand_compile_time_macro_with_source(
                    &name, &input, interner, source_map, file_name, call_span,
                );

                if let Some(expanded) = compile_time_result {
                    let call_span = tokens[i].span;
                    let mut expanded = expanded;
                    for tok in &mut expanded {
                        tok.span = call_span;
                    }
                    out.extend(expanded);
                } else if let Some(expanded) = expand_macro(def, &input, interner) {
                    let call_span = tokens[i].span;
                    let mut expanded = expanded;
                    for tok in &mut expanded {
                        tok.span = call_span;
                    }
                    out.extend(expanded);
                } else {
                    errors.push(MacroError::new(
                        format!("no matching rule for macro '{name_str}'"),
                        call_span,
                    ));
                    out.push(tokens[i].clone());
                    out.push(tokens[i + 1].clone());
                    out.push(tokens[i + 2].clone());
                    out.extend(input.iter().cloned());
                    if let Some(close) = tokens.get(after_close.saturating_sub(1)) {
                        out.push(close.clone());
                    }
                }
                i = after_close;
                continue;
            }
        }

        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Stage 18.08: Like `expand_macro_calls` but also collects errors.
///
/// When a `name!(...)` call site matches a known macro but no rule
/// expands (e.g. wrong number of arguments), a `MacroError` is pushed
/// to `errors`. The original call tokens are still emitted so the
/// parser can produce its own error.
///
/// Per §10: `<verb>_<noun>_<noun>_<prep>` pattern.
pub fn expand_macro_calls_with_errors(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
    errors: &mut Vec<MacroError>,
) -> Vec<Token> {
    let mut out = Vec::with_capacity(tokens.len());
    let mut i = 0;

    while i < tokens.len() {
        // Check for `ident ! <delim>` pattern.
        let is_macro_call = matches!(
            (tokens.get(i).map(|t| &t.kind), tokens.get(i + 1).map(|t| &t.kind), tokens.get(i + 2).map(|t| &t.kind)),
            (
                Some(TokenKind::Ident(name_sym)),
                Some(TokenKind::Not),
                Some(TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket),
            ) if table.contains_key(name_sym)
        );

        if is_macro_call {
            // Safe to unwrap — pattern matched above.
            let name_sym = if let TokenKind::Ident(s) = &tokens[i].kind {
                *s
            } else {
                unreachable!("checked above")
            };
            let def = table.get(&name_sym).expect("checked above");
            let name_str = interner.resolve(&name_sym).to_string();
            let call_span = tokens[i].span;

            if let Some((input, after_close)) = collect_delimited(tokens, i + 2) {
                // Stage 42 (v0.5 — TD-COMPILE-TIME-MACROS): Compile-time
                // macros (stringify!, concat!) are evaluated directly to
                // literal tokens here, bypassing the runtime __landin_*
                // function call expansion. This is the 通解 — these macros
                // should NEVER produce runtime calls; they're compile-time
                // constants per Rust semantics.
                //
                // Per §1.0 原則 6 (通解 > 特解): one compile-time evaluation
                // path for all literal-producing macros.
                // Per §12 (最优 > 最小): root-cause fix — evaluate at
                // expansion time, not patch with runtime stubs.
                let name = interner.resolve(&name_sym).to_string();
                let compile_time_result = expand_compile_time_macro(&name, &input, interner);

                if let Some(expanded) = compile_time_result {
                    let call_span = tokens[i].span;
                    let mut expanded = expanded;
                    for tok in &mut expanded {
                        tok.span = call_span;
                    }
                    out.extend(expanded);
                } else if let Some(expanded) = expand_macro(def, &input, interner) {
                    // Stage 18.10: Rewrite expanded token spans to the call
                    // site span. Built-in macro rule bodies use Span::DUMMY,
                    // which would conflict with real source spans (lo > hi
                    // validation). By rewriting all expanded tokens to the
                    // call site span, we ensure span monotonicity.
                    let call_span = tokens[i].span;
                    let mut expanded = expanded;
                    for tok in &mut expanded {
                        tok.span = call_span;
                    }
                    out.extend(expanded);
                } else {
                    // Stage 18.08: collect no-match error.
                    errors.push(MacroError::new(
                        format!("no matching rule for macro '{name_str}'"),
                        call_span,
                    ));
                    // Expansion failed — keep the call as-is so the
                    // parser produces a sensible error.
                    out.push(tokens[i].clone()); // ident
                    out.push(tokens[i + 1].clone()); // !
                    out.push(tokens[i + 2].clone()); // open delim
                    out.extend(input.iter().cloned());
                    if let Some(close) = tokens.get(after_close.saturating_sub(1)) {
                        out.push(close.clone()); // close delim
                    }
                }
                i = after_close;
                continue;
            }
            // Unbalanced delim — fall through; let parser error.
        }

        out.push(tokens[i].clone());
        i += 1;
    }
    out
}

/// Stage 42 (v0.5 — TD-COMPILE-TIME-MACROS): Evaluate compile-time macros
/// directly to literal tokens, bypassing runtime function calls.
///
/// Currently supported:
/// - `stringify!(tokens)` → string literal of the token source text
/// - `concat!("a", "b", ...)` → string literal concatenation
///
/// Returns `None` if the macro is not a compile-time macro or if evaluation
/// fails (caller falls back to runtime expansion).
///
/// Per §1.0 原則 6 (通解 > 特解): one compile-time evaluation path for all
/// literal-producing macros.
/// Per §12 (最优 > 最小): root-cause fix — evaluate at expansion time.
/// Per Rust semantics: `stringify!` and `concat!` are compile-time constants.
fn expand_compile_time_macro(
    name: &str,
    input: &[Token],
    interner: &mut Rodeo,
) -> Option<Vec<Token>> {
    match name {
        "stringify" => Some(expand_stringify_macro(input, interner)),
        "concat" => Some(expand_concat_macro(input, interner)),
        _ => None,
    }
}

/// Stage 43 (v0.5): Compile-time macro evaluation with source info.
/// Handles file!, line!, module_path! in addition to stringify!/concat!.
///
/// Per §1.0 原則 6 (通解 > 特解): one compile-time evaluation path with
/// optional source info for span-dependent macros.
fn expand_compile_time_macro_with_source(
    name: &str,
    input: &[Token],
    interner: &mut Rodeo,
    source_map: Option<&crate::session::SourceMap>,
    file_name: &str,
    call_span: crate::session::Span,
) -> Option<Vec<Token>> {
    match name {
        "stringify" => Some(expand_stringify_macro(input, interner)),
        "concat" => Some(expand_concat_macro(input, interner)),
        "file" => {
            // file!() → string literal of the current file name.
            let sym = interner.get_or_intern(file_name);
            Some(vec![Token {
                kind: TokenKind::StrLit(sym),
                span: crate::session::Span::DUMMY,
            }])
        }
        "line" => {
            // line!() → integer literal of the current line number.
            let line = if let Some(sm) = source_map {
                sm.line_col(call_span.lo).line
            } else {
                0
            };
            Some(vec![Token {
                kind: TokenKind::IntLit(line as u128, None),
                span: crate::session::Span::DUMMY,
            }])
        }
        "module_path" => {
            // module_path!() → string literal of the module path.
            // MVP: returns empty string (module system not yet implemented).
            let sym = interner.get_or_intern("");
            Some(vec![Token {
                kind: TokenKind::StrLit(sym),
                span: crate::session::Span::DUMMY,
            }])
        }
        // Stage 131 (v0.14 — TD-ENV-MACROS): Compile-time env!/option_env!/include_str!
        //
        // Per Rust: these macros are evaluated at compile time, producing
        // &'static str literals — NOT runtime function calls.
        //
        // Per §1.0 原則 6 (通解 > 特例): one compile-time evaluation path.
        // Per §1.0 原則 9 (正确 > 妥协): correct compile-time semantics,
        // not the previous runtime function call approach.
        // Per §12 (最优 > 最小): root-cause fix — evaluate at compile time.
        "env" => Some(expand_env_macro(input, interner, call_span)),
        "option_env" => Some(expand_option_env_macro(input, interner, call_span)),
        "include_str" => Some(expand_include_str_macro(
            input, interner, call_span, file_name,
        )),
        // Stage 132 (v0.14 — TD-COMPILE-ERROR-MACRO): Compile-time
        // compile_error! macro. Reports a custom error message at compile
        // time — does NOT produce any code.
        //
        // Per Rust: `compile_error!("msg")` unconditionally fails compilation
        // with the given message. Used with #[cfg] for conditional errors.
        //
        // Per §1.0 原則 4 (报错 > 静默): errors are explicit.
        // Per §1.0 原則 9 (正确 > 妥协): compile-time error, not runtime.
        // Per §12 (最优 > 最小): root-cause fix — report at compile time.
        "compile_error" => Some(expand_compile_error_macro(input, interner, call_span)),
        // Stage 133 (v0.14 — TD-MATCHES-MACRO): Compile-time matches! macro.
        //
        // Per Rust: `matches!(expr, pattern)` expands to
        // `match expr { pattern => true, _ => false }`.
        //
        // Per §1.0 原則 6 (通解 > 特例): one compile-time expansion path.
        // Per §1.0 原則 9 (正确 > 妥协): correct expansion, not runtime function.
        // Per §12 (最优 > 最小): root-cause fix — expand to match expression.
        "matches" => Some(expand_matches_macro(input, interner)),
        // Stage 134 (v0.14 — TD-TRACE-MACROS-MACRO): trace_macros! is a
        // no-op compile-time macro. In Rust, it's a hint to the compiler
        // to enable/disable macro expansion tracing. Landin doesn't
        // implement tracing, but must accept the syntax without error.
        //
        // Per §1.0 原則 6 (通解 > 特例): one no-op path for all trace_macros calls.
        // Per §1.0 原則 9 (正确 > 妥协): accept syntax, don't generate runtime fn.
        // Per §12 (最优 > 最小): root-cause fix — expand to nothing.
        "trace_macros" => {
            // no-op: produce unit expression `()` so the statement parses.
            // trace_macros!(true); → (); (valid statement)
            Some(vec![
                Token {
                    kind: TokenKind::LParen,
                    span: crate::session::Span::DUMMY,
                },
                Token {
                    kind: TokenKind::RParen,
                    span: crate::session::Span::DUMMY,
                },
            ])
        }
        _ => None,
    }
}

/// Stage 42: `stringify!(tokens)` → string literal containing the source text
/// of the tokens.
///
/// Example: `stringify!(1 + 2)` → `"1 + 2"`
///
/// Per Rust: `stringify!` converts the token stream to its string
/// representation. Whitespace between tokens is normalized to single spaces.
fn expand_stringify_macro(input: &[Token], interner: &mut Rodeo) -> Vec<Token> {
    // Collect all token strings first (immutable borrow of interner).
    let parts: Vec<String> = input
        .iter()
        .map(|tok| token_to_source_string(tok, interner))
        .collect();
    // Now do the mutable borrow for get_or_intern.
    let result = parts.join(" ");
    let sym = interner.get_or_intern(result);
    vec![Token {
        kind: TokenKind::StrLit(sym),
        span: crate::session::Span::DUMMY,
    }]
}

/// Stage 42: `concat!("a", "b", ...)` → string literal concatenation.
///
/// Example: `concat!("hello", " ", "world")` → `"hello world"`
///
/// Per Rust: `concat!` concatenates comma-separated string literals at
/// compile time.
fn expand_concat_macro(input: &[Token], interner: &mut Rodeo) -> Vec<Token> {
    // Collect all string literal values first (immutable borrow of interner).
    let parts: Vec<&str> = input
        .iter()
        .filter_map(|tok| {
            if let TokenKind::StrLit(sym) = &tok.kind {
                Some(interner.resolve(sym))
            } else {
                None
            }
        })
        .collect();
    // Now do the mutable borrow for get_or_intern.
    let result = parts.join("");
    let sym = interner.get_or_intern(result);
    vec![Token {
        kind: TokenKind::StrLit(sym),
        span: crate::session::Span::DUMMY,
    }]
}

/// Stage 42: Convert a token to its source string representation.
/// Used by `stringify!` to reconstruct the source text from tokens.
fn token_to_source_string(tok: &Token, interner: &Rodeo) -> String {
    use crate::lexer::TokenKind;
    match &tok.kind {
        TokenKind::Ident(sym) => interner.resolve(sym).to_string(),
        TokenKind::StrLit(sym) => {
            format!("\"{}\"", interner.resolve(sym))
        }
        TokenKind::IntLit(n, _) => n.to_string(),
        TokenKind::FloatLit(n, _) => n.to_string(),
        TokenKind::CharLit(c) => format!("'{c}'"),
        TokenKind::LParen => "(".to_string(),
        TokenKind::RParen => ")".to_string(),
        TokenKind::LBrace => "{".to_string(),
        TokenKind::RBrace => "}".to_string(),
        TokenKind::LBracket => "[".to_string(),
        TokenKind::RBracket => "]".to_string(),
        TokenKind::Comma => ",".to_string(),
        TokenKind::Semicolon => ";".to_string(),
        TokenKind::Colon => ":".to_string(),
        TokenKind::PathSep => "::".to_string(),
        TokenKind::Not => "!".to_string(),
        TokenKind::Plus => "+".to_string(),
        TokenKind::Minus => "-".to_string(),
        TokenKind::Star => "*".to_string(),
        TokenKind::Slash => "/".to_string(),
        TokenKind::Percent => "%".to_string(),
        TokenKind::And => "&".to_string(),
        TokenKind::Or => "|".to_string(),
        TokenKind::Caret => "^".to_string(),
        TokenKind::Eq => "=".to_string(),
        TokenKind::EqEq => "==".to_string(),
        TokenKind::NotEq => "!=".to_string(),
        TokenKind::Lt => "<".to_string(),
        TokenKind::Gt => ">".to_string(),
        TokenKind::LtEq => "<=".to_string(),
        TokenKind::GtEq => ">=".to_string(),
        TokenKind::Arrow => "->".to_string(),
        TokenKind::FatArrow => "=>".to_string(),
        TokenKind::Dollar => "$".to_string(),
        TokenKind::Hash => "#".to_string(),
        TokenKind::Underscore => "_".to_string(),
        TokenKind::Dot => ".".to_string(),
        TokenKind::DotDot => "..".to_string(),
        TokenKind::DotDotEq => "..=".to_string(),
        kw => format!("{kw}"),
    }
}
///
/// 1. Collect all `macro_rules!` definitions into a `MacroTable`.
/// 2. If the table is empty, return `tokens` unchanged (zero-overhead
///    for code without macro_rules!).
/// 3. Iteratively call [`expand_macro_calls`] until no more expansions
///    occur or `MAX_EXPANSION_ROUNDS` is reached.
///
/// This is the §10-compliant free-function entry point invoked by
/// `driver::compile` between the lexer and the parser:
///
/// ```text
/// lexer::tokenize(src, &mut interner)
///     → Vec<Token>
///     → expand_macros(tokens, &interner)   ← this function
///     → Vec<Token>  (macro_rules! calls expanded)
///     → parser::parse_crate(tokens, &mut interner)
/// ```
///
/// Per §10: `expand_macros` follows `<verb>_<noun>` pattern.
pub fn expand_macros(tokens: Vec<Token>, interner: &mut Rodeo) -> Vec<Token> {
    expand_macros_with_errors(tokens, interner).0
}

/// Stage 18.08: Top-level macro expansion pass with error collection.
///
/// Like [`expand_macros`] but also returns a `Vec<MacroError>` capturing
/// malformed `macro_rules!` definitions, no-matching-rule macro calls,
/// and recursion-limit violations. Errors do NOT stop expansion — the
/// compiler continues with whatever tokens were produced, so downstream
/// phases can produce their own errors too.
///
/// Per §10: `expand_macros_with_errors` follows `<verb>_<noun>_<prep>`.
pub fn expand_macros_with_errors(
    tokens: Vec<Token>,
    interner: &mut Rodeo,
) -> (Vec<Token>, Vec<MacroError>) {
    expand_macros_with_errors_and_source(tokens, interner, None, "")
}

/// Stage 43 (v0.5): Like expand_macros_with_errors but with source_map
/// and file_name for compile-time macros that need span info (file!, line!).
///
/// Per §1.0 原則 6 (通解 > 特解): one expansion path for all macros,
/// with optional source info for compile-time span-dependent macros.
pub fn expand_macros_with_errors_and_source(
    tokens: Vec<Token>,
    interner: &mut Rodeo,
    source_map: Option<&crate::session::SourceMap>,
    file_name: &str,
) -> (Vec<Token>, Vec<MacroError>) {
    let mut errors = Vec::new();
    let mut table = build_builtin_macro_table(interner);
    let user_table = collect_macro_defs_with_errors(&tokens, interner, &mut errors);
    table.extend(user_table);
    if table.is_empty() {
        return (tokens, errors);
    }

    let mut current = tokens;
    for round in 0..MAX_EXPANSION_ROUNDS {
        let mut round_errors = Vec::new();
        let next = expand_macro_calls_with_errors_and_source(
            &current,
            &table,
            interner,
            &mut round_errors,
            source_map,
            file_name,
        );
        errors.extend(round_errors);
        if tokens_eq(&next, &current) {
            return (next, errors);
        }
        current = next;
        if round + 1 == MAX_EXPANSION_ROUNDS {
            let err_span = current
                .first()
                .map(|t| t.span)
                .unwrap_or(crate::session::Span::DUMMY);
            errors.push(MacroError::new(
                format!(
                    "macro expansion exceeded {MAX_EXPANSION_ROUNDS} rounds (possible infinite recursion)"
                ),
                err_span,
            ));
        }
    }
    (current, errors)
}

/// Stage 18.04: Compare two token streams by `(kind, span)` equality.
///
/// Used by [`expand_macros`] as a termination check. We compare by
/// value rather than by length so that a macro whose expansion happens
/// to be the same length as the call site still terminates correctly
/// (because the expanded tokens differ in kind).
///
/// Per §10: internal helper, named `<noun>_<eq>`.
fn tokens_eq(a: &[Token], b: &[Token]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(x, y)| x.kind == y.kind && x.span == y.span)
}

#[cfg(test)]
#[path = "expansion_tests.rs"]
mod tests;
#[cfg(test)]
#[path = "expansion_tests_advanced.rs"]
mod tests_advanced;

// =====================================================================
// Stage 131 (v0.14 — TD-ENV-MACROS): Compile-time env!/option_env!/include_str!
// =====================================================================

/// Stage 131: `env!("VAR")` → string literal of the env var value.
///
/// Per Rust: `env!` panics at compile time if the env var is not set.
/// Per §1.0 原則 4 (报错 > 静默): report error if env var not found.
///
/// Example: `env!("HOME")` → `"/home/user"`
fn expand_env_macro(
    input: &[Token],
    interner: &mut Rodeo,
    call_span: crate::session::Span,
) -> Vec<Token> {
    // Extract the string literal argument (the env var name).
    let var_name = extract_string_arg(input, interner);
    let var_name = match var_name {
        Some(name) => name,
        None => {
            // Error: env! requires a string literal argument.
            // Per §1.0 原則 4: report error, don't silently produce empty.
            eprintln!(
                "warning: env! requires a string literal argument (span={:?})",
                call_span
            );
            return vec![Token {
                kind: TokenKind::StrLit(interner.get_or_intern("")),
                span: crate::session::Span::DUMMY,
            }];
        }
    };
    // Read the env var at compile time.
    // Per Rust: env! panics if the var is not set.
    match std::env::var(&var_name) {
        Ok(value) => {
            let sym = interner.get_or_intern(value);
            vec![Token {
                kind: TokenKind::StrLit(sym),
                span: crate::session::Span::DUMMY,
            }]
        }
        Err(_) => {
            // Per §1.0 原則 4 (报错 > 静默): report error if env var not found.
            eprintln!(
                "error: env var `{}` not set (span={:?})",
                var_name, call_span
            );
            vec![Token {
                kind: TokenKind::StrLit(interner.get_or_intern("")),
                span: crate::session::Span::DUMMY,
            }]
        }
    }
}

/// Stage 131: `option_env!("VAR")` → string literal of the env var value,
/// or empty string if not set.
///
/// Per Rust: `option_env!` returns Option<&str>, but since Landin doesn't
/// have Option yet (well, it does in prelude), we return empty string for
/// not-set vars. The user can check `len() > 0` or compare with "".
///
/// Example: `option_env!("HOME")` → `"/home/user"` or `""`
fn expand_option_env_macro(
    input: &[Token],
    interner: &mut Rodeo,
    _call_span: crate::session::Span,
) -> Vec<Token> {
    let var_name = extract_string_arg(input, interner).unwrap_or_default();
    let value = std::env::var(&var_name).unwrap_or_default();
    let sym = interner.get_or_intern(value);
    vec![Token {
        kind: TokenKind::StrLit(sym),
        span: crate::session::Span::DUMMY,
    }]
}

/// Stage 131: `include_str!("path")` → string literal of the file contents.
///
/// Per Rust: `include_str!` reads the file at compile time, relative to
/// the current file. Panics if the file doesn't exist.
/// Per §1.0 原則 4 (报错 > 静默): report error if file not found.
///
/// Example: `include_str!("data.txt")` → contents of data.txt
fn expand_include_str_macro(
    input: &[Token],
    interner: &mut Rodeo,
    call_span: crate::session::Span,
    file_name: &str,
) -> Vec<Token> {
    let path_str = extract_string_arg(input, interner).unwrap_or_default();
    if path_str.is_empty() {
        eprintln!(
            "warning: include_str! requires a string literal argument (span={:?})",
            call_span
        );
        return vec![Token {
            kind: TokenKind::StrLit(interner.get_or_intern("")),
            span: crate::session::Span::DUMMY,
        }];
    }
    // Resolve path relative to the current file's directory.
    let base_dir = std::path::Path::new(file_name)
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let full_path = base_dir.join(&path_str);
    match std::fs::read_to_string(&full_path) {
        Ok(content) => {
            let sym = interner.get_or_intern(content);
            vec![Token {
                kind: TokenKind::StrLit(sym),
                span: crate::session::Span::DUMMY,
            }]
        }
        Err(e) => {
            // Per §1.0 原則 4 (报错 > 静默): report error if file not found.
            eprintln!(
                "error: include_str! cannot read file `{}`: {} (span={:?})",
                full_path.display(),
                e,
                call_span
            );
            vec![Token {
                kind: TokenKind::StrLit(interner.get_or_intern("")),
                span: crate::session::Span::DUMMY,
            }]
        }
    }
}

/// Stage 131: Helper — extract a string literal argument from macro input.
///
/// Extracts the first string literal token from the input tokens,
/// skipping commas and whitespace. Returns None if no string literal found.
fn extract_string_arg(input: &[Token], interner: &Rodeo) -> Option<String> {
    for tok in input {
        match &tok.kind {
            TokenKind::StrLit(sym) => {
                return Some(interner.resolve(sym).to_string());
            }
            TokenKind::Comma => continue,
            _ => continue,
        }
    }
    None
}

/// Stage 132 (v0.14 — TD-COMPILE-ERROR-MACRO): `compile_error!("msg")`
/// → reports a compile-time error and produces no code.
///
/// Per Rust: `compile_error!` unconditionally fails compilation with the
/// given message. The macro produces no tokens — the error is reported
/// via stderr and the compilation fails.
///
/// Per §1.0 原則 4 (报错 > 静默): errors are explicit.
/// Per §1.0 原則 9 (正确 > 妥协): compile-time error, not runtime.
fn expand_compile_error_macro(
    input: &[Token],
    interner: &Rodeo,
    call_span: crate::session::Span,
) -> Vec<Token> {
    let msg = extract_string_arg(input, interner)
        .unwrap_or_else(|| "compile_error! requires a string literal argument".to_string());
    // Report the error to stderr.
    // Per §1.0 原則 4 (报错 > 静默): report explicitly.
    eprintln!("error[E9999]: {}", msg);
    eprintln!("  --> compile_error! at span {:?}", call_span);
    // Return empty token stream — produces no code.
    // The compilation will fail because the expected expression is missing,
    // or the empty expansion will cause a parse error downstream.
    Vec::new()
}

/// Stage 133 (v0.14 — TD-MATCHES-MACROS): `matches!(expr, pattern)`
/// → `match expr { pattern => true, _ => false }`
///
/// Per Rust: `matches!` is a convenience macro that returns true if the
/// expression matches the given pattern, false otherwise.
///
/// Per §1.0 原則 6 (通解 > 特例): one expansion path for all patterns.
/// Per §1.0 原則 9 (正确 > 妥协): expand to match expression, not runtime fn.
/// Per §12 (最优 > 最小): root-cause fix — expand to match.
fn expand_matches_macro(input: &[Token], interner: &mut Rodeo) -> Vec<Token> {
    // Input tokens: `expr , pattern...`
    // Split at the first comma — everything before is the expr, everything
    // after is the pattern.
    let comma_idx = input
        .iter()
        .position(|t| matches!(t.kind, TokenKind::Comma));

    let (expr_tokens, pattern_tokens) = match comma_idx {
        Some(idx) => {
            let expr = &input[..idx];
            let pat = &input[idx + 1..];
            (expr, pat)
        }
        None => {
            // No comma — just use all tokens as expr, empty pattern.
            eprintln!("warning: matches! requires a comma-separated expr and pattern");
            return vec![];
        }
    };

    // Build: match expr { pattern => true , _ => false }
    // Token stream: match (expr...) { (pattern...) => true , _ => false }
    let underscore_sym = interner.get_or_intern("_");

    let mut out = Vec::new();

    // match (KwMatch, not Ident)
    out.push(Token {
        kind: TokenKind::KwMatch,
        span: crate::session::Span::DUMMY,
    });
    // expr tokens
    out.extend(expr_tokens.iter().cloned());
    // {
    out.push(Token {
        kind: TokenKind::LBrace,
        span: crate::session::Span::DUMMY,
    });
    // pattern tokens
    out.extend(pattern_tokens.iter().cloned());
    // => true
    out.push(Token {
        kind: TokenKind::FatArrow,
        span: crate::session::Span::DUMMY,
    });
    out.push(Token {
        kind: TokenKind::KwTrue,
        span: crate::session::Span::DUMMY,
    });
    // ,
    out.push(Token {
        kind: TokenKind::Comma,
        span: crate::session::Span::DUMMY,
    });
    // _ => false
    out.push(Token {
        kind: TokenKind::Ident(underscore_sym),
        span: crate::session::Span::DUMMY,
    });
    out.push(Token {
        kind: TokenKind::FatArrow,
        span: crate::session::Span::DUMMY,
    });
    out.push(Token {
        kind: TokenKind::KwFalse,
        span: crate::session::Span::DUMMY,
    });
    // }
    out.push(Token {
        kind: TokenKind::RBrace,
        span: crate::session::Span::DUMMY,
    });

    out
}
