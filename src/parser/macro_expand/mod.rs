//! Stage 18.03 + 18.04: macro_rules! expansion engine.
//!
//! This module implements the macro_rules! system:
//!
//! # Stage 18.03 — token tree matching + substitution
//! 1. **Pattern matching**: Match input tokens against a macro rule pattern.
//!    Supports `$name:fragment` captures and literal token matching.
//! 2. **Substitution**: Replace `$name` captures in the body with matched values.
//! 3. **Expansion**: Produce expanded tokens that are re-parsed as AST.
//!
//! Supported fragments (Stage 18.03):
//! - `$name:expr` — matches an expression (token tree until `;` or `}`)
//! - `$name:ident` — matches a single identifier
//! - `$name:tt` — matches a single token tree
//!
//! # Stage 18.04 — macro call invocation + driver integration
//! 4. **collect_macro_defs**: Scan a token stream and extract every
//!    `macro_rules! name { ... }` definition into a `MacroTable`.
//! 5. **expand_macro_calls**: Walk a token stream and expand every
//!    `name!(args)` call site whose `name` is in the table.
//! 6. **expand_macros**: Top-level driver entry — collect defs, then
//!    iteratively expand calls until no more expansions occur.
//!
//! Per §10: `expand_macros` is the free-function entry point
//! (`<verb>_<noun>` pattern); `collect_macro_defs` and
//! `expand_macro_calls` follow `<verb>_<noun>_<noun>`.
//! Per §11: the entire module is parser-internal; driver only sees
//! the `expand_macros` free-function entry.
//! Per §1.0 原則 6 "通用 > 特例": one engine handles all macro_rules!.

use crate::ast::{MacroRule, MacroRulesDef};
use crate::lexer::{Token, TokenKind};
use lasso::Rodeo;
use std::collections::HashMap;

/// A captured fragment from pattern matching.
///
/// Stage 18.03: scalar captures — maps `Symbol` (capture name) →
/// single token slice.
///
/// Stage 18.06: extended to also hold repetition captures — when a
/// name is bound inside `$(...)*` / `+` / `?`, the capture stores one
/// `Vec<Token>` per iteration.
#[derive(Debug, Default, Clone)]
pub(crate) enum CaptureValue {
    /// No value (used as HashMap default; replaced on insert).
    #[default]
    Empty,
    /// Scalar capture: a single fragment match (Stage 18.03).
    Single(Vec<Token>),
    /// Repetition capture: one entry per matched iteration (Stage 18.06).
    Repetition(Vec<Vec<Token>>),
}

/// Captured fragments from pattern matching.
///
/// Maps `Symbol` (capture name) → `CaptureValue` (either `Single` for
/// scalar fragments or `Repetition` for `$(...)*`-bound names).
type Captures = HashMap<crate::lexer::Symbol, CaptureValue>;

/// Stage 18.03: Expand a macro call by matching input tokens against rules.
///
/// Tries each rule in order. The first rule whose pattern matches the
/// input tokens is used. The body is then substituted with captured
/// values and returned.
///
/// Returns `Some(expanded_tokens)` if a rule matched, `None` if no rule matched.
///
/// Per §23: `expand_macro` follows `<verb>_<noun>` pattern.
// Stage 18.135: builtin macros extracted to builtin_macros.rs
// Stage 18.247: build_builtin_macro_table import moved to expansion.rs
pub fn expand_macro(
    def: &MacroRulesDef,
    input: &[Token],
    interner: &mut Rodeo,
) -> Option<Vec<Token>> {
    for rule in &def.rules {
        let mut captures = HashMap::new();
        if match_pattern(&rule.pattern, input, &mut captures, interner) {
            // Stage 18.26: Apply hygiene before substitution.
            // Renames non-capture identifiers in the body to unique names
            // (__landin_macro_<orig>_<n>) to prevent collisions with
            // caller scope. Per §1.0 原則 6 "通用 > 特解".
            let mut hygiene = HygieneContext::new();
            let hygienic_body = apply_hygiene(&rule.body, &captures, interner, &mut hygiene);
            return Some(substitute_body(&hygienic_body, &captures));
        }
    }
    None
}

/// Match a pattern against input tokens, capturing `$name:fragment` bindings.
///
/// Returns `true` if the pattern matches, `false` otherwise.
/// On success, `captures` contains all `$name` bindings and the entire
/// input is consumed (i.e. `ii == input.len()` at end).
///
/// Per §10: internal helper, named `<verb>_<noun>`.
fn match_pattern(
    pattern: &[Token],
    input: &[Token],
    captures: &mut Captures,
    interner: &Rodeo,
) -> bool {
    let mut idx = 0usize;
    if !match_pattern_at(pattern, input, &mut idx, captures, interner) {
        return false;
    }
    // Top-level: require all input consumed.
    idx == input.len()
}

/// Stage 18.06: Position-aware variant of `match_pattern`.
///
/// Same as `match_pattern` but takes `idx` by mutable reference so callers
/// (like `match_repetition`) can resume matching from a specific input
/// position. On success, `*idx` is advanced past all matched input.
/// Does NOT require all input to be consumed — the caller is responsible
/// for checking that.
///
/// Per §10: internal helper, named `<verb>_<noun>_<preposition>`.
fn match_pattern_at(
    pattern: &[Token],
    input: &[Token],
    idx: &mut usize,
    captures: &mut Captures,
    interner: &Rodeo,
) -> bool {
    let mut pi = 0; // pattern index
    let mut ii = *idx; // input index

    while pi < pattern.len() {
        let pt = &pattern[pi];

        // Stage 18.06: Check for `$ ( ... ) op` repetition pattern.
        if pt.kind == TokenKind::Dollar
            && pi + 1 < pattern.len()
            && pattern[pi + 1].kind == TokenKind::LParen
        {
            // Find matching `)` in pattern starting from pi+1.
            let (inner_pattern, after_close) = match collect_pattern_inner(pattern, pi + 1) {
                Some(x) => x,
                None => return false,
            };
            // Read $op (Star/Plus/Question), possibly with separator.
            let (op, after_op) = match parse_repetition_op(pattern, after_close) {
                Some(x) => x,
                None => return false,
            };
            // Match repetition.
            let iter_count =
                match_repetition(&inner_pattern, input, &mut ii, op, captures, interner);
            if iter_count.is_none() {
                return false;
            }
            pi = after_op; // past `)` and `$op` (and separator if present)
            continue;
        }

        // Check for `$name:fragment` pattern (dollar sign followed by ident + colon + fragment)
        if pt.kind == TokenKind::Dollar {
            // Expect: $ ident : fragment
            if pi + 3 < pattern.len() {
                if let (TokenKind::Ident(name_sym), TokenKind::Colon) =
                    (&pattern[pi + 1].kind, &pattern[pi + 2].kind)
                {
                    let name = *name_sym;
                    if let TokenKind::Ident(frag_sym) = &pattern[pi + 3].kind {
                        let frag = interner.resolve(frag_sym);
                        // Stage 18.05: extended fragment dispatch — one match
                        // handles all 7 supported fragments (expr/ident/tt
                        // from Stage 18.03 + ty/literal/block/path new).
                        let captured = match frag {
                            "expr" => capture_expr(input, &mut ii),
                            "ident" => capture_ident(input, &mut ii),
                            "tt" => capture_tt(input, &mut ii),
                            "ty" => capture_ty(input, &mut ii),
                            "literal" => capture_literal(input, &mut ii),
                            "block" => capture_block(input, &mut ii),
                            "path" => capture_path(input, &mut ii),
                            "lifetime" => capture_lifetime(input, &mut ii),
                            "stmt" => capture_stmt(input, &mut ii),
                            _ => return false,
                        };
                        if captured.is_empty() {
                            return false;
                        }
                        captures.insert(name, CaptureValue::Single(captured));
                        pi += 4; // Skip $ name : fragment
                        continue;
                    }
                }
            }
        }

        // Literal token matching
        if ii >= input.len() {
            return false;
        }
        if !tokens_match(&pt.kind, &input[ii].kind) {
            return false;
        }
        pi += 1;
        ii += 1;
    }

    // All pattern tokens consumed — advance *idx.
    *idx = ii;
    true
}

/// Capture an expression: tokens until top-level `,`, `;`, or `)`.
fn capture_expr(input: &[Token], idx: &mut usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Comma | TokenKind::Semicolon if depth == 0 => break,
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    tokens
}

/// Capture a single identifier.
fn capture_ident(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx < input.len() {
        if let TokenKind::Ident(_) = &input[*idx].kind {
            let token = input[*idx].clone();
            *idx += 1;
            return vec![token];
        }
    }
    Vec::new()
}

/// Capture a single token tree (one token, or a balanced delimited group).
fn capture_tt(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx >= input.len() {
        return Vec::new();
    }

    match &input[*idx].kind {
        TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
            // Balanced group — delegate to the existing balance-tracking loop.
            let mut tokens = Vec::new();
            let mut depth = 0i32;

            while *idx < input.len() {
                match &input[*idx].kind {
                    TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                        depth += 1;
                        tokens.push(input[*idx].clone());
                    }
                    TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                        depth -= 1;
                        tokens.push(input[*idx].clone());
                        if depth == 0 {
                            *idx += 1;
                            break;
                        }
                    }
                    _ => tokens.push(input[*idx].clone()),
                }
                *idx += 1;
            }
            tokens
        }
        _ => {
            // Single token
            let token = input[*idx].clone();
            *idx += 1;
            vec![token]
        }
    }
}

// =============================================================================
// Stage 18.05: Additional Fragment Specifiers (ty/literal/block/path)
// =============================================================================

/// Stage 18.05: Capture a type: tokens until top-level `,`, `;`, `)`,
/// `}`, or `=>`. Tracks nested `<...>` and `(...)` so generic types
/// like `Vec<HashMap<K, V>>` are captured as a single type.
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_ty(input: &[Token], idx: &mut usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Lt => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Gt => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Comma | TokenKind::Semicolon | TokenKind::FatArrow if depth == 0 => break,
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    tokens
}

/// Stage 18.05: Capture a literal: a single `IntLit` / `FloatLit` /
/// `StrLit` / `CharLit` / `KwTrue` / `KwFalse` token. Returns empty
/// if the current token is not a literal.
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_literal(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx >= input.len() {
        return Vec::new();
    }
    match &input[*idx].kind {
        TokenKind::IntLit(_, _)
        | TokenKind::FloatLit(_, _)
        | TokenKind::StrLit(_)
        | TokenKind::CharLit(_)
        | TokenKind::KwTrue
        | TokenKind::KwFalse => {
            let token = input[*idx].clone();
            *idx += 1;
            vec![token]
        }
        _ => Vec::new(),
    }
}

/// Stage 18.05: Capture a block: a balanced `{ ... }` (delimiters included).
/// Returns empty if the current token is not `{` or if the block is
/// unbalanced.
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_block(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx >= input.len() {
        return Vec::new();
    }
    if !matches!(&input[*idx].kind, TokenKind::LBrace) {
        return Vec::new();
    }

    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RBrace => {
                depth -= 1;
                tokens.push(input[*idx].clone());
                if depth == 0 {
                    *idx += 1;
                    break;
                }
            }
            TokenKind::Eof => return Vec::new(),
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    tokens
}

/// Stage 18.05: Capture a path: `a`, `a::b`, `a::b::c`, etc.
/// First segment must be `Ident` / `RawIdent` / `self` / `Self` /
/// `crate` / `super`. Subsequent segments must be `::` followed by
/// an identifier-like token.
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_path(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx >= input.len() {
        return Vec::new();
    }

    // First segment must be an identifier or path keyword.
    let first_ok = matches!(
        &input[*idx].kind,
        TokenKind::Ident(_)
            | TokenKind::RawIdent(_)
            | TokenKind::KwSelf_
            | TokenKind::KwSelfType
            | TokenKind::KwCrate
            | TokenKind::KwSuper
    );
    if !first_ok {
        return Vec::new();
    }

    let mut tokens = vec![input[*idx].clone()];
    *idx += 1;

    // Optional `:: Ident` repetitions.
    while *idx + 1 < input.len() {
        if matches!(&input[*idx].kind, TokenKind::PathSep)
            && matches!(
                &input[*idx + 1].kind,
                TokenKind::Ident(_)
                    | TokenKind::RawIdent(_)
                    | TokenKind::KwSelf_
                    | TokenKind::KwSelfType
                    | TokenKind::KwCrate
                    | TokenKind::KwSuper
            )
        {
            tokens.push(input[*idx].clone()); // ::
            *idx += 1;
            tokens.push(input[*idx].clone()); // ident
            *idx += 1;
        } else {
            break;
        }
    }

    tokens
}

/// Check if two token kinds match (for literal pattern matching).
fn tokens_match(pat: &TokenKind, input: &TokenKind) -> bool {
    // For identifiers, match by discriminant (any ident matches any ident in pattern).
    // For other tokens, match exactly.
    pat == input
}

// =============================================================================
// Stage 18.24: Additional Fragment Specifiers (lifetime + stmt)
// =============================================================================

/// Stage 18.24: Capture a lifetime: a single `Lifetime(Symbol)` token
/// (e.g., `'a`, `'static`).
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_lifetime(input: &[Token], idx: &mut usize) -> Vec<Token> {
    if *idx < input.len() {
        if let TokenKind::Lifetime(_) = &input[*idx].kind {
            let token = input[*idx].clone();
            *idx += 1;
            return vec![token];
        }
    }
    Vec::new()
}

/// Stage 18.24: Capture a statement: tokens until top-level `;` (inclusive)
/// or `}` (exclusive). Tracks nested delimiters so `;` inside `{}` or `()`
/// doesn't end the capture.
///
/// Per §10: internal helper, named `capture_<fragment>`.
fn capture_stmt(input: &[Token], idx: &mut usize) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut depth = 0i32;

    while *idx < input.len() {
        match &input[*idx].kind {
            TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => {
                depth += 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
                tokens.push(input[*idx].clone());
            }
            TokenKind::Semicolon if depth == 0 => {
                tokens.push(input[*idx].clone());
                *idx += 1;
                break;
            }
            TokenKind::Eof => break,
            _ => tokens.push(input[*idx].clone()),
        }
        *idx += 1;
    }

    tokens
}

/// Substitute `$name` captures in the body, producing expanded tokens.
fn substitute_body(body: &[Token], captures: &Captures) -> Vec<Token> {
    let mut result = Vec::new();
    let mut i = 0;

    while i < body.len() {
        let bt = &body[i];

        // Stage 18.06: Check for `$ ( ... ) op` repetition substitution.
        if bt.kind == TokenKind::Dollar
            && i + 1 < body.len()
            && body[i + 1].kind == TokenKind::LParen
        {
            // Find matching `)` in body starting from i+1.
            let (inner_body, after_close) = match collect_pattern_inner(body, i + 1) {
                Some(x) => x,
                None => {
                    // Unbalanced — emit `$` literally and continue.
                    result.push(bt.clone());
                    i += 1;
                    continue;
                }
            };
            // Read $op (Star/Plus/Question), possibly with separator.
            let (op, after_op) = match parse_repetition_op(body, after_close) {
                Some(x) => x,
                None => {
                    result.push(bt.clone());
                    i += 1;
                    continue;
                }
            };
            // Substitute repetition.
            substitute_repetition(&inner_body, captures, op, &mut result);
            i = after_op; // past `)` and `$op` (and separator if present)
            continue;
        }

        // Check for `$name` substitution
        if bt.kind == TokenKind::Dollar && i + 1 < body.len() {
            if let TokenKind::Ident(name_sym) = &body[i + 1].kind {
                if let Some(CaptureValue::Single(captured)) = captures.get(name_sym) {
                    result.extend(captured.iter().cloned());
                    i += 2; // Skip $ name
                    continue;
                }
            }
        }

        result.push(bt.clone());
        i += 1;
    }

    result
}

// =============================================================================
// Stage 18.06: Repetition — $(...)* / $(...)+ / $(...)?
// =============================================================================

/// Stage 18.06 + 18.13: Kind of repetition operator in macro_rules! patterns,
/// with optional separator.
///
/// Per §10: enum follows `<Noun>Kind` pattern (mirrors `BorrowKind`,
/// `IntTy`, etc.).
#[derive(Debug, Clone, PartialEq)]
enum RepetitionKind {
    /// `$(...)*` — zero or more (with optional separator)
    ZeroOrMore(RepetitionSep),
    /// `$(...)+` — one or more (with optional separator)
    OneOrMore(RepetitionSep),
    /// `$(...)?` — zero or one (with optional separator)
    ZeroOrOne(RepetitionSep),
}

/// Stage 18.13: Optional separator in a macro_rules! repetition.
///
/// `$(...)*`  → `RepetitionSep::None`
/// `$(...),*` → `RepetitionSep::Token(TokenKind::Comma)`
/// `$(...);+` → `RepetitionSep::Token(TokenKind::Semicolon)`
///
/// Per §10: enum follows `<Noun>` pattern.
///
/// Note: does not derive `Eq` because `TokenKind` contains `f64` (FloatLit).
/// `Default` is derived — `None` is the first variant, so it's the default.
#[derive(Debug, Clone, PartialEq, Default)]
pub(crate) enum RepetitionSep {
    /// No separator — `$(...)*` / `+` / `?`
    #[default]
    None,
    /// A single token separator — `$(...),*` / `$(...);+` / etc.
    /// Stores the token kind (without span) for matching.
    Token(TokenKind),
}

/// Stage 18.06 + 18.13: Parse the repetition operator (and optional
/// separator) at/after `tokens[idx]`.
///
/// Syntax:
///   `*`           → ZeroOrMore(None)
///   `+`           → OneOrMore(None)
///   `?`           → ZeroOrOne(None)
///   `, *`         → ZeroOrMore(Comma)
///   `; +`         → OneOrMore(Semicolon)
///   `=> ?`        → ZeroOrOne(FatArrow)  [unusual but valid]
///
/// Returns `Some((RepetitionKind, after_op_index))` if a valid operator
/// (possibly preceded by a separator token) is found, otherwise `None`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn parse_repetition_op(tokens: &[Token], idx: usize) -> Option<(RepetitionKind, usize)> {
    // Stage 18.13: Check for a separator token before the operator.
    // A separator is any single token that is NOT `*`/`+`/`?` (those are
    // the operators themselves) and NOT `)` (end of repetition group).
    let (sep, op_idx) = match tokens.get(idx).map(|t| &t.kind) {
        Some(TokenKind::Star) => (RepetitionSep::None, idx),
        Some(TokenKind::Plus) => (RepetitionSep::None, idx),
        Some(TokenKind::Question) => (RepetitionSep::None, idx),
        Some(kind) if !matches!(kind, TokenKind::RParen | TokenKind::Eof) => {
            // This token is a potential separator.
            // Verify the NEXT token is a valid operator.
            if matches!(
                tokens.get(idx + 1).map(|t| &t.kind),
                Some(TokenKind::Star | TokenKind::Plus | TokenKind::Question)
            ) {
                (RepetitionSep::Token(kind.clone()), idx + 1)
            } else {
                return None;
            }
        }
        _ => return None,
    };

    let after_op = op_idx + 1;
    match tokens.get(op_idx).map(|t| &t.kind) {
        Some(TokenKind::Star) => Some((RepetitionKind::ZeroOrMore(sep), after_op)),
        Some(TokenKind::Plus) => Some((RepetitionKind::OneOrMore(sep), after_op)),
        Some(TokenKind::Question) => Some((RepetitionKind::ZeroOrOne(sep), after_op)),
        _ => None,
    }
}

/// Stage 18.06: Collect tokens between an opening `(` (at `start`) and its
/// matching `)`. Returns `(inner_tokens, index_after_close)`.
///
/// Per §10: internal helper, named `<verb>_<noun>_<adj>`.
fn collect_pattern_inner(tokens: &[Token], start: usize) -> Option<(Vec<Token>, usize)> {
    // `start` must point at `LParen`.
    if !matches!(tokens.get(start).map(|t| &t.kind), Some(TokenKind::LParen)) {
        return None;
    }
    let mut depth = 0i32;
    let mut inside = Vec::new();
    let mut i = start;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::LParen => {
                depth += 1;
                if i != start {
                    inside.push(tokens[i].clone());
                }
            }
            TokenKind::RParen => {
                depth -= 1;
                if depth == 0 {
                    return Some((inside, i + 1));
                }
                inside.push(tokens[i].clone());
            }
            TokenKind::Eof => return None,
            _ => inside.push(tokens[i].clone()),
        }
        i += 1;
    }
    None
}

/// Stage 18.14: Push a capture value from one iteration into the
/// per-iteration `rep_names` map.
///
/// For `Single(tokens)`: pushes `tokens` as one iteration's capture.
/// For `Repetition(inner_iters)`: flattens all inner iterations' tokens
/// into a single token slice and pushes that — this enables nested
/// repetition where outer iteration captures contain inner repetition
/// results.
/// For `Empty`: no-op.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>_<noun>`.
fn push_capture_into_rep_names(
    rep_names: &mut HashMap<crate::lexer::Symbol, Vec<Vec<Token>>>,
    name: crate::lexer::Symbol,
    val: CaptureValue,
) {
    match val {
        CaptureValue::Single(tokens) => {
            rep_names.entry(name).or_default().push(tokens);
        }
        CaptureValue::Repetition(inner_iters) => {
            // Stage 18.14: Flatten inner repetitions into one token slice.
            // This allows nested $( $( $x )* )* to work — each outer
            // iteration's capture for $x is the concatenation of all
            // inner iterations' tokens.
            let mut flat = Vec::new();
            for inner_tokens in inner_iters {
                flat.extend(inner_tokens);
            }
            rep_names.entry(name).or_default().push(flat);
        }
        CaptureValue::Empty => {}
    }
}

/// Stage 18.06: Match a repetition pattern `$( $inner ) $op` against
/// input tokens starting at `*idx`.
///
/// On success: returns `Some(iter_count)` and `captures` is updated
/// with `CaptureValue::Repetition(...)` entries for each `$name` bound
/// inside `inner`. `*idx` is advanced past all matched input.
///
/// On failure: returns `None` and `captures` may be partially modified
/// (callers should discard `captures` on `None`).
///
/// Per §10: internal helper, named `<verb>_<noun>`.
fn match_repetition(
    inner: &[Token],
    input: &[Token],
    idx: &mut usize,
    op: RepetitionKind,
    captures: &mut Captures,
    interner: &Rodeo,
) -> Option<usize> {
    // Stage 18.13: Extract separator from op.
    let sep = match &op {
        RepetitionKind::ZeroOrMore(s)
        | RepetitionKind::OneOrMore(s)
        | RepetitionKind::ZeroOrOne(s) => s,
    };
    // Local captures for this repetition — each iteration appends to the
    // per-name Vec. We collect here, then merge into `captures` on success.
    let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
    let mut iter_count = 0usize;

    loop {
        // Stage 18.13: Before iterations after the first, expect a separator.
        if iter_count > 0 {
            if let RepetitionSep::Token(sep_kind) = sep {
                if *idx < input.len() && tokens_match(sep_kind, &input[*idx].kind) {
                    *idx += 1; // consume separator
                } else {
                    break; // No separator — stop.
                }
            }
        }
        // Try to match `inner` against input starting at *idx.
        let mut iter_captures = Captures::new();
        let mut local_idx = *idx;
        if !match_pattern_at(inner, input, &mut local_idx, &mut iter_captures, interner) {
            break; // No more matches.
        }
        // No progress guard — if matching consumed nothing, stop to avoid
        // infinite loop. (Empty inner pattern matches every position with
        // zero progress; we treat that as one match then stop.)
        if local_idx == *idx {
            // Stage 18.14: Merge this single empty iteration's captures,
            // including nested Repetition captures (flattened).
            for (name, val) in iter_captures {
                push_capture_into_rep_names(&mut rep_names, name, val);
            }
            iter_count += 1;
            break;
        }
        // Stage 18.14: Merge iter_captures into rep_names, including
        // nested Repetition captures (flattened).
        for (name, val) in iter_captures {
            push_capture_into_rep_names(&mut rep_names, name, val);
        }
        *idx = local_idx;
        iter_count += 1;
    }

    // Apply $op constraints.
    match op {
        RepetitionKind::ZeroOrMore(_) => { /* 0+ always OK */ }
        RepetitionKind::OneOrMore(_) => {
            if iter_count == 0 {
                return None;
            }
        }
        RepetitionKind::ZeroOrOne(_) => {
            if iter_count > 1 {
                // Only keep the first iteration; rewind idx.
                // This is tricky — we'd need to undo input consumption.
                // Simple approach: if more than 1 matched, treat as 1
                // by truncating rep_names and "un-advancing" idx.
                // For now, we just truncate to 1 (input consumption stays).
                // This is acceptable for the simplified Stage 18.06 (no separator).
                let first_iters: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = rep_names
                    .into_iter()
                    .map(|(k, mut v)| {
                        v.truncate(1);
                        (k, v)
                    })
                    .collect();
                rep_names = first_iters;
                iter_count = 1;
            }
        }
    }

    // Commit to `captures`.
    for (name, iters) in rep_names {
        captures.insert(name, CaptureValue::Repetition(iters));
    }

    Some(iter_count)
}

/// Stage 18.06: Substitute a repetition in the body. For each iteration
/// index `i`, builds a local capture map with `name → captures[name][i]`
/// and appends `substitute_body(inner, &local)` to `result`.
///
/// If a repetition name in `inner` has no corresponding capture (e.g.
/// user wrote `$( $x )*` but `$x` wasn't matched in pattern), the
/// iteration body is emitted literally.
///
/// Per §10: internal helper, named `<verb>_<noun>`.
fn substitute_repetition(
    inner: &[Token],
    captures: &Captures,
    op: RepetitionKind,
    result: &mut Vec<Token>,
) {
    // Stage 18.13: Extract separator from op.
    let sep = match &op {
        RepetitionKind::ZeroOrMore(s)
        | RepetitionKind::OneOrMore(s)
        | RepetitionKind::ZeroOrOne(s) => s,
    };
    // Determine iteration count: look at all Repetition captures referenced
    // in `inner` and take the max (or 0 if none). In well-formed macros
    // all repetition names share the same count.
    let mut iter_count = 0usize;
    for val in captures.values() {
        if let CaptureValue::Repetition(iters) = val {
            if iters.len() > iter_count {
                iter_count = iters.len();
            }
        }
    }

    for i in 0..iter_count {
        // Build local captures: each Repetition(name) → Single(name[i]).
        let mut local: Captures = HashMap::new();
        for (name, val) in captures.iter() {
            match val {
                CaptureValue::Repetition(iters) => {
                    if let Some(single) = iters.get(i) {
                        local.insert(*name, CaptureValue::Single(single.clone()));
                    }
                }
                CaptureValue::Single(toks) => {
                    // Scalar captures are visible inside repetition body too.
                    local.insert(*name, CaptureValue::Single(toks.clone()));
                }
                CaptureValue::Empty => {}
            }
        }
        let expanded = substitute_body(inner, &local);
        result.extend(expanded);
        // Stage 18.13: Emit separator between iterations (not after last).
        if let RepetitionSep::Token(sep_kind) = sep {
            if i + 1 < iter_count {
                result.push(Token {
                    kind: sep_kind.clone(),
                    span: crate::session::Span::DUMMY,
                });
            }
        }
    }
}

/// Stage 18.04: Maximum number of expansion rounds to prevent infinite
/// recursion when a macro expands to a call to itself.
///
/// 32 is sufficient for realistic macro chains (real code rarely nests
/// more than 5-10 levels deep) while providing a hard safety bound.
const MAX_EXPANSION_ROUNDS: usize = 32;

/// Stage 18.04: Macro definition table collected from a token stream.
///
/// Maps macro name (`Symbol`) → `MacroRulesDef` (with parsed rules).
/// Built by [`collect_macro_defs`] and consumed by
/// [`expand_macro_calls`].
///
/// Per §10: type name follows the `<Noun>` + `Table` suffix convention
/// (mirrors `FnSigTable`, `FieldTyTable`).
pub type MacroTable = HashMap<crate::lexer::Symbol, MacroRulesDef>;

/// Stage 18.08: Error during macro_rules! expansion.
///
/// Captures malformed `macro_rules!` definitions, no-matching-rule
/// macro calls, and recursion-limit violations. Errors do NOT stop
/// expansion — the compiler continues with whatever tokens were
/// produced, so downstream phases can produce their own errors too.
///
/// Per §10: error type follows `<Stage>Error` suffix pattern
/// (mirrors `LexError`, `ParseError`, `ResolveError`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacroErrorKind {
    Generic,
    NoMatchingRule,
    RecursionLimit,
    InvalidDefinition,
    InvalidFragment,
}

#[derive(Debug, Clone)]
pub struct MacroError {
    /// Human-readable error message.
    pub message: String,
    /// Source span where the error occurred (best-effort; may be `DUMMY`
    /// when the error spans a synthetic range).
    pub span: crate::session::Span,
    pub kind: MacroErrorKind,
}

impl MacroError {
    /// Stage 18.08: Construct a new `MacroError`.
    ///
    /// Per §10: constructor follows `new` convention.
    pub fn new(message: impl Into<String>, span: crate::session::Span) -> Self {
        Self {
            message: message.into(),
            span,
            kind: MacroErrorKind::Generic,
        }
    }
}

// =============================================================================
// Stage 18.17: Basic macro hygiene (HygieneContext infrastructure)
// =============================================================================

/// Stage 18.17: Hygiene context for macro expansion.
///
/// Tracks a counter for generating unique identifier names during
/// macro body expansion. Each macro call gets a fresh context.
///
/// **Current status (Stage 18.17)**: Infrastructure only — the context
/// is created but `apply_hygiene` is not yet called from `expand_macro`.
/// This is preparation for future stages that will rename macro body
/// locals to prevent collisions with caller scope.
///
/// **Future**: When `apply_hygiene` is implemented, macro body
/// identifiers (except `$name` captures) will be renamed to
/// `__landin_macro_<original>_<counter>` to isolate them from the
/// caller's scope.
///
/// Per §10: struct follows `<Noun><Noun>` pattern.
/// Stage 18.26: Now activated — `expand_macro` creates a `HygieneContext`
/// and calls `apply_hygiene`.
#[derive(Debug, Default, Clone)]
pub(crate) struct HygieneContext {
    /// Counter for generating unique names. Incremented each time
    /// a macro body identifier is renamed.
    counter: u64,
}

impl HygieneContext {
    /// Stage 18.17: Construct a fresh hygiene context with counter=0.
    ///
    /// Per §10: constructor follows `new` convention.
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// Stage 18.17: Generate a unique hygiene-renamed identifier name.
    ///
    /// Returns `__landin_macro_<original>_<counter>` and increments the
    /// internal counter, so successive calls produce different names.
    ///
    /// Per §10: `<verb>_<noun>_<noun>` pattern.
    pub(crate) fn gen_unique_name(&mut self, original: &str) -> String {
        let name = format!("__landin_macro_{original}_{}", self.counter);
        self.counter += 1;
        name
    }

    /// Stage 18.17: Get the current counter value (for testing).
    ///
    /// Per §10: `<verb>_<noun>` pattern.
    #[cfg(test)]
    pub(crate) fn counter(&self) -> u64 {
        self.counter
    }
}

/// Stage 18.20: Apply macro hygiene to a macro body.
///
/// Renames identifiers in the body that are NOT captures (i.e. not
/// preceded by `$`) to unique names `__landin_macro_<original>_<counter>`.
/// This prevents macro body locals from colliding with caller locals.
///
/// **Skip renaming**:
/// - Identifiers preceded by `$` (these are capture references)
/// - Keywords (`let`, `fn`, `if`, etc. — detected via `TokenKind::is_keyword()`)
/// - Built-in macro names (`println`, `print`, `eprintln`, `eprint`)
/// - Non-identifier tokens (literals, punctuation)
///
/// Per §10: `<verb>_<noun>` pattern.
/// Stage 18.26: Now called from `expand_macro` — `#[allow(dead_code)]` removed.
fn apply_hygiene(
    body: &[Token],
    _captures: &Captures,
    interner: &mut Rodeo,
    hygiene: &mut HygieneContext,
) -> Vec<Token> {
    let mut result = Vec::with_capacity(body.len());
    let mut i = 0;

    while i < body.len() {
        let tok = &body[i];

        // Check for `$ident` capture reference — don't rename.
        if tok.kind == TokenKind::Dollar && i + 1 < body.len() {
            if let TokenKind::Ident(_) = &body[i + 1].kind {
                // Capture reference — emit both tokens unchanged.
                result.push(tok.clone());
                result.push(body[i + 1].clone());
                i += 2;
                continue;
            }
        }

        // Check for identifier that should be renamed.
        if let TokenKind::Ident(sym) = &tok.kind {
            let name = interner.resolve(sym);
            // Skip keywords, built-in macro names, and __landin_ runtime functions.
            let is_keyword = tok.kind.is_keyword();
            let is_builtin = BUILTIN_MACRO_NAMES.contains(&name);
            // Stage 18.27: __landin_ functions are runtime functions that
            // must NOT be renamed — they're the expansion target for
            // built-in print macros.
            let is_runtime = name.starts_with("__landin_");
            // Stage 36.6: Primitive type names (i8..i128, u8..u128, isize,
            // usize, f32, f64, bool, char, str) must NOT be renamed —
            // they're used in cast expressions (e.g., `x as i64`) inside
            // macro bodies. Renaming them would break type resolution.
            // Per §1.0 原則 6 (通解 > 特解): one set for all primitive types.
            let is_primitive_type = matches!(
                name,
                "i8" | "i16"
                    | "i32"
                    | "i64"
                    | "i128"
                    | "isize"
                    | "u8"
                    | "u16"
                    | "u32"
                    | "u64"
                    | "u128"
                    | "usize"
                    | "f32"
                    | "f64"
                    | "bool"
                    | "char"
                    | "str"
            );
            // Stage 40.2 (TD-PANIC-MACRO-BROKEN): Field names used in macro
            // bodies (e.g., `panic!` expands to `__landin_panic_msg($msg.ptr)`
            // where `ptr` is the `&str` struct's field name) must NOT be
            // renamed — they refer to struct fields, not user bindings.
            // Renaming them would produce `__landin_macro_ptr_0` which
            // typeck rejects as "no field on type" (primitive types have
            // no fields).
            //
            // Per §1.0 原則 6 (通解 > 特解): one set for all struct field
            // names used in macro bodies (currently just `ptr` for &str).
            // Per §12 (最优 > 最小): root-cause fix at hygiene layer.
            let is_struct_field = matches!(name, "ptr" | "len" | "cap");
            if !is_keyword && !is_builtin && !is_runtime && !is_primitive_type && !is_struct_field {
                // Rename to unique name.
                let new_name = hygiene.gen_unique_name(name);
                let new_sym = interner.get_or_intern(new_name);
                result.push(Token {
                    kind: TokenKind::Ident(new_sym),
                    span: tok.span,
                });
                i += 1;
                continue;
            }
        }

        // Default: emit token unchanged.
        result.push(tok.clone());
        i += 1;
    }

    result
}

// =============================================================================
// Stage 18.10: Built-in macro_rules! registration (println! 通解化 Phase 1)
// =============================================================================

/// Stage 18.10 + 18.29 + 18.32 + 18.34 + 18.36 + 18.39 + 18.41 + 18.43: Names of
/// the built-in macros that are always available (registered into every
/// `MacroTable` before user macros).
///
/// Stage 18.10: println/print/eprintln/eprint (print macros)
/// Stage 18.29: assert/panic/vec (non-print macros)
/// Stage 18.32: format/dbg/todo/unimplemented/write (more non-print macros)
/// Stage 18.34: stringify/concat/env (compile-time utility macros)
/// Stage 18.36: file/line/module_path/include_str (source info + file macros)
/// Stage 18.39: matches/cfg/option_env (pattern + config macros)
/// Stage 18.41: asm/compile_error/cfg_attr (low-level + diagnostic macros)
/// Stage 18.43: unreachable/trace_macros/format_args (control-flow + debug macros)
///
/// Per §10: const naming follows `UPPER_SNAKE_CASE`.
pub const BUILTIN_MACRO_NAMES: &[&str] = &[
    "println",
    "print",
    "eprintln",
    "eprint", // print macros
    "assert",
    "panic",
    "vec", // non-print macros (Stage 18.29)
    "format",
    "dbg",
    "todo",
    "unimplemented",
    "write", // more macros (Stage 18.32)
    "stringify",
    "concat",
    "env", // compile-time utility macros (Stage 18.34)
    "file",
    "line",
    "module_path",
    "include_str", // source info + file macros (Stage 18.36)
    "matches",
    "cfg",
    "option_env", // pattern + config macros (Stage 18.39)
    "asm",
    "compile_error",
    "cfg_attr", // low-level + diagnostic macros (Stage 18.41)
    "unreachable",
    "trace_macros",
    "format_args", // control-flow + debug macros (Stage 18.43)
];

// Stage 18.247: Collection extracted to collection.rs
mod collection;
pub use collection::{collect_macro_defs, collect_macro_defs_with_errors};

// Stage 18.247: Expansion extracted to expansion.rs
mod expansion;
pub use expansion::{
    expand_macro_calls, expand_macro_calls_with_errors, expand_macros, expand_macros_with_errors,
};
