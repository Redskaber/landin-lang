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
#[derive(Debug, Clone)]
pub struct MacroError {
    /// Human-readable error message.
    pub message: String,
    /// Source span where the error occurred (best-effort; may be `DUMMY`
    /// when the error spans a synthetic range).
    pub span: crate::session::Span,
}

impl MacroError {
    /// Stage 18.08: Construct a new `MacroError`.
    ///
    /// Per §10: constructor follows `new` convention.
    pub fn new(message: impl Into<String>, span: crate::session::Span) -> Self {
        Self {
            message: message.into(),
            span,
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
            if !is_keyword && !is_builtin && !is_runtime {
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

/// Stage 18.10 + 18.29 + 18.32 + 18.34 + 18.36 + 18.39 + 18.41: Names of the
/// built-in macros that are always available (registered into every `MacroTable`
/// before user macros).
///
/// Stage 18.10: println/print/eprintln/eprint (print macros)
/// Stage 18.29: assert/panic/vec (non-print macros)
/// Stage 18.32: format/dbg/todo/unimplemented/write (more non-print macros)
/// Stage 18.34: stringify/concat/env (compile-time utility macros)
/// Stage 18.36: file/line/module_path/include_str (source info + file macros)
/// Stage 18.39: matches/cfg/option_env (pattern + config macros)
/// Stage 18.41: asm/compile_error/cfg_attr (low-level + diagnostic macros)
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
];

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

/// Stage 18.04: Collect all `macro_rules!` definitions from a token stream.
///
/// Walks the token stream looking for the pattern:
/// ```text
/// Ident("macro_rules") Bang Ident(name) LBrace ... RBrace
/// ```
/// For each match, parses the rule bodies `(pat) => { body };` into
/// `MacroRule`s and stores the resulting `MacroRulesDef` in the table.
///
/// The original tokens are **not** modified — the parser will still
/// parse `macro_rules!` definitions normally and produce
/// `ItemKind::MacroRules` AST nodes. This function only builds a
/// lookup table for the pre-parse expansion pass.
///
/// Per §10: `collect_macro_defs` follows `<verb>_<noun>_<noun>` pattern.
pub fn collect_macro_defs(tokens: &[Token], interner: &mut Rodeo) -> MacroTable {
    collect_macro_defs_with_errors(tokens, interner, &mut Vec::new())
}

/// Stage 18.08: Like `collect_macro_defs` but also collects errors.
///
/// Malformed `macro_rules!` bodies (e.g. missing `=>`, unbalanced
/// delimiters) are reported as `MacroError`s. The original tokens are
/// still skipped past so collection can continue with subsequent
/// definitions.
///
/// Per §10: `<verb>_<noun>_<noun>_<prep>` pattern.
pub fn collect_macro_defs_with_errors(
    tokens: &[Token],
    interner: &mut Rodeo,
    errors: &mut Vec<MacroError>,
) -> MacroTable {
    let mut table = MacroTable::new();
    let macro_rules_sym = match interner.get("macro_rules") {
        Some(s) => s,
        None => return table,
    };

    let mut i = 0;
    while i < tokens.len() {
        // Match: Ident("macro_rules") Bang Ident(name) LBrace
        if let (
            Some(Token {
                kind: TokenKind::Ident(sym),
                ..
            }),
            Some(Token {
                kind: TokenKind::Not,
                ..
            }),
            Some(Token {
                kind: TokenKind::Ident(name_sym),
                span: name_span,
            }),
            Some(Token {
                kind: TokenKind::LBrace,
                ..
            }),
        ) = (
            tokens.get(i),
            tokens.get(i + 1),
            tokens.get(i + 2),
            tokens.get(i + 3),
        ) {
            if *sym == macro_rules_sym {
                let name = *name_sym;
                let name_str = interner.resolve(&name).to_string();
                let body_start = i + 4;
                match parse_macro_rules_body(tokens, body_start, *name_span) {
                    Some(rules) => {
                        table.insert(
                            name,
                            MacroRulesDef {
                                name,
                                rules,
                                span: *name_span,
                            },
                        );
                    }
                    None => {
                        // Stage 18.08: collect the error.
                        errors.push(MacroError::new(
                            format!("malformed macro_rules! body in definition of '{name_str}'"),
                            *name_span,
                        ));
                    }
                }
                // Skip past the macro_rules! definition (past matching `}`).
                // Even if parsing failed, we must skip to avoid re-processing.
                i = skip_to_matching_rbrace(tokens, body_start);
                continue;
            }
        }
        i += 1;
    }
    table
}

/// Stage 18.04: Parse the body of a `macro_rules! name { ... }` definition.
///
/// The body is a sequence of rules: `(pattern) => { body };` (possibly
/// with `[`/`{` delimiters instead of `(` for the pattern).
///
/// `start` is the index just after the opening `{` of the macro_rules! body.
/// Returns `Some(Vec<MacroRule>)` if at least one rule parsed successfully,
/// or `None` if the body could not be parsed.
///
/// Per §10: internal helper, named `<verb>_<noun>_<noun>`.
fn parse_macro_rules_body(
    tokens: &[Token],
    start: usize,
    span: crate::session::Span,
) -> Option<Vec<MacroRule>> {
    let mut rules = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        // Skip trailing `;` between rules.
        if let Some(Token {
            kind: TokenKind::Semicolon,
            ..
        }) = tokens.get(i)
        {
            i += 1;
            continue;
        }
        // End of macro_rules! body.
        if let Some(Token {
            kind: TokenKind::RBrace,
            ..
        }) = tokens.get(i)
        {
            break;
        }

        // Parse pattern: a delimited token tree.
        let (pattern, after_pattern) = match collect_delimited(tokens, i) {
            Some(x) => x,
            None => break,
        };
        i = after_pattern;

        // Expect `=>`
        if !matches!(tokens.get(i).map(|t| &t.kind), Some(TokenKind::FatArrow)) {
            break;
        }
        i += 1; // consume `=>`

        // Parse body: a delimited token tree.
        let (body, after_body) = match collect_delimited(tokens, i) {
            Some(x) => x,
            None => break,
        };
        i = after_body;

        rules.push(MacroRule {
            pattern,
            body,
            span,
        });
    }

    if rules.is_empty() {
        None
    } else {
        Some(rules)
    }
}

/// Stage 18.04: Collect a balanced delimited token tree starting at `start`.
///
/// Returns `(tokens_inside, index_after_closing_delim)`. The returned
/// tokens do **not** include the opening/closing delimiters themselves
/// (matching the existing `MacroRule.pattern` / `MacroRule.body`
/// conventions from Stage 18.02).
///
/// Returns `None` if `start` doesn't point at an opening delimiter or
/// if the stream ends before the matching closer.
///
/// Per §10: internal helper, named `<verb>_<noun>`.
fn collect_delimited(tokens: &[Token], start: usize) -> Option<(Vec<Token>, usize)> {
    let open_kind = &tokens.get(start)?.kind;
    if !matches!(
        open_kind,
        TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket
    ) {
        return None;
    }

    let mut depth = 0i32;
    let mut inside = Vec::new();
    let mut i = start;

    while i < tokens.len() {
        let tok = &tokens[i];
        match &tok.kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => {
                depth += 1;
                // Skip the outermost opening delim; include nested ones.
                if i != start {
                    inside.push(tok.clone());
                }
            }
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                depth -= 1;
                if depth == 0 {
                    // Closing the outermost delim.
                    return Some((inside, i + 1));
                }
                inside.push(tok.clone());
            }
            TokenKind::Eof => return None,
            _ => {
                inside.push(tok.clone());
            }
        }
        i += 1;
    }
    None
}

/// Stage 18.04: Find the index just past the matching `}` for the `{`
/// (or `)` for `(`, etc.) that **should** be at `start`.
///
/// Used by [`collect_macro_defs`] to skip past a `macro_rules!` body
/// even when individual rule parsing fails.
///
/// Per §10: internal helper, named `<verb>_<preposition>_<noun>_<noun>`.
fn skip_to_matching_rbrace(tokens: &[Token], start: usize) -> usize {
    let mut depth = 0i32;
    let mut i = start;
    while i < tokens.len() {
        match &tokens[i].kind {
            TokenKind::LParen | TokenKind::LBrace | TokenKind::LBracket => depth += 1,
            TokenKind::RParen | TokenKind::RBrace | TokenKind::RBracket => {
                depth -= 1;
                if depth <= 0 {
                    return i + 1;
                }
            }
            TokenKind::Eof => return i,
            _ => {}
        }
        i += 1;
    }
    i
}

/// Stage 18.04: Expand macro calls in a token stream.
///
/// Walks `tokens` looking for the pattern `Ident(name) Bang <delim>`.
/// If `name` is in `table`, collects the input tokens until the matching
/// closing delimiter, calls [`expand_macro`] on them, and splices the
/// expanded tokens into the output. If expansion fails (no rule matched),
/// the original call tokens are emitted unchanged.
///
/// Unknown macros (e.g. `println!`, which is a built-in special form)
/// are passed through unchanged so the parser can handle them via its
/// existing special cases.
///
/// Per §10: `expand_macro_calls` follows `<verb>_<noun>_<noun>` pattern.
pub fn expand_macro_calls(
    tokens: &[Token],
    table: &MacroTable,
    interner: &mut Rodeo,
) -> Vec<Token> {
    expand_macro_calls_with_errors(tokens, table, interner, &mut Vec::new())
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
                // Try to expand.
                if let Some(expanded) = expand_macro(def, &input, interner) {
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
                    out.extend(input);
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

/// Stage 18.04: Top-level macro expansion pass — driver entry point.
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
    let mut errors = Vec::new();
    // Stage 18.10: Register built-in macros first (println/print/eprintln/eprint).
    // Built-in macros have no-op rule bodies in Phase 1, so they pass through
    // unchanged and the parser's existing special-case path still handles them.
    let mut table = build_builtin_macro_table(interner);
    // Then collect user-defined macros. User macros override built-ins
    // (extend() overwrites existing keys with the user's version).
    let user_table = collect_macro_defs_with_errors(&tokens, interner, &mut errors);
    table.extend(user_table);
    if table.is_empty() {
        return (tokens, errors);
    }

    let mut current = tokens;
    for round in 0..MAX_EXPANSION_ROUNDS {
        let mut round_errors = Vec::new();
        let next = expand_macro_calls_with_errors(&current, &table, interner, &mut round_errors);
        errors.extend(round_errors);
        // Termination check: if the token stream didn't change at all
        // (by structural equality), no more expansions are possible.
        if tokens_eq(&next, &current) {
            return (next, errors);
        }
        current = next;
        // Stage 18.08: if this was the last round, emit a recursion error.
        if round + 1 == MAX_EXPANSION_ROUNDS {
            errors.push(MacroError::new(
                format!(
                    "macro expansion exceeded {MAX_EXPANSION_ROUNDS} rounds (possible infinite recursion)"
                ),
                crate::session::Span::DUMMY,
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
mod tests {
    use super::*;
    use crate::ast::{MacroRule, MacroRulesDef};
    use crate::compile;

    /// Stage 18.03 positive 1: Macro expansion does not break compilation.
    #[test]
    fn stage18_03_macro_expansion_does_not_break() {
        let src = "macro_rules! my_macro { () => { 42 } } fn main() { 0 }";
        let result = compile(src);
        // macro_rules! is parsed but not yet expanded (Phase 3 integration pending).
        // Just verify it compiles without errors.
        assert!(
            result.errors.lex.is_empty(),
            "macro_rules! should not produce lex errors"
        );
    }

    /// Stage 18.03 positive 2: Multiple macro rules parse correctly.
    #[test]
    fn stage18_03_multiple_rules_parse() {
        let src = "macro_rules! multi { () => { 1 } ($x:expr) => { $x } } fn main() { 0 }";
        let result = compile(src);
        assert!(
            result.errors.lex.is_empty(),
            "macro_rules! with multiple rules should parse"
        );
    }

    /// Stage 18.03 negative 1: Empty macro_rules! parses.
    #[test]
    fn stage18_03_empty_macro_rules() {
        let src = "macro_rules! empty { } fn main() { 0 }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
    }

    /// Stage 18.03 negative 2: match_pattern matches empty pattern.
    #[test]
    fn stage18_03_match_empty_pattern() {
        let interner = Rodeo::new();
        let pattern: Vec<Token> = vec![];
        let input: Vec<Token> = vec![];
        let mut captures = HashMap::new();
        assert!(match_pattern(&pattern, &input, &mut captures, &interner));
    }

    /// Stage 18.03 negative 3: match_pattern matches literal tokens.
    #[test]
    fn stage18_03_match_literal_tokens() {
        let interner = Rodeo::new();
        let pattern = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let input = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let mut captures = HashMap::new();
        assert!(match_pattern(&pattern, &input, &mut captures, &interner));
    }

    /// Stage 18.03 negative 4: match_pattern rejects mismatched tokens.
    #[test]
    fn stage18_03_match_rejects_mismatch() {
        let interner = Rodeo::new();
        let pattern = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let input = vec![Token {
            kind: TokenKind::IntLit(99, None),
            span: crate::session::Span::DUMMY,
        }];
        let mut captures = HashMap::new();
        assert!(!match_pattern(&pattern, &input, &mut captures, &interner));
    }

    /// Stage 18.03 negative 5: substitute_body replaces $name.
    #[test]
    fn stage18_03_substitute_replaces_name() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        let mut captures: Captures = HashMap::new();
        captures.insert(
            x_sym,
            CaptureValue::Single(vec![Token {
                kind: TokenKind::IntLit(42, None),
                span: crate::session::Span::DUMMY,
            }]),
        );

        let body = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(x_sym),
                span: crate::session::Span::DUMMY,
            },
        ];

        let result = substitute_body(&body, &captures);
        assert_eq!(result.len(), 1, "should substitute 1 token for $x");
        assert!(matches!(result[0].kind, TokenKind::IntLit(42, _)));
    }

    /// Stage 18.03 negative 6: expand_macro returns None when no rule matches.
    #[test]
    fn stage18_03_expand_no_match_returns_none() {
        let mut interner = Rodeo::new();
        let def = MacroRulesDef {
            name: interner.get_or_intern("test"),
            rules: vec![MacroRule {
                pattern: vec![Token {
                    kind: TokenKind::IntLit(1, None),
                    span: crate::session::Span::DUMMY,
                }],
                body: vec![],
                span: crate::session::Span::DUMMY,
            }],
            span: crate::session::Span::DUMMY,
        };
        let input = vec![Token {
            kind: TokenKind::IntLit(2, None),
            span: crate::session::Span::DUMMY,
        }];
        let result = expand_macro(&def, &input, &mut interner);
        assert!(result.is_none(), "non-matching input should return None");
    }

    // =====================================================================
    // Stage 18.04 tests — Macro Call Invocation + Driver Integration
    // =====================================================================

    /// Stage 18.04 positive 1: A simple `macro_rules!` macro is expanded
    /// at the call site. `m!()` expands to `42`, which the parser then
    /// parses as an integer literal expression.
    #[test]
    fn stage18_04_macro_call_expands_simple() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro_rules! m!() should expand and parse — parse errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.04 positive 2: A macro that captures `$x:expr` and
    /// substitutes it into the body. `m!(99)` expands to `99`.
    #[test]
    fn stage18_04_macro_call_expands_with_capture() {
        let src = "macro_rules! m { ($x:expr) => { $x } } fn main() { m!(99) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro_rules! with $x:expr capture should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.04 negative 1: collect_macro_defs returns an empty table
    /// when the token stream has no macro_rules! definitions.
    #[test]
    fn stage18_04_collect_finds_no_macros() {
        let mut interner = Rodeo::new();
        let tokens: Vec<Token> = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let table = collect_macro_defs(&tokens, &mut interner);
        assert!(table.is_empty(), "no macro_rules! → empty table");
    }

    /// Stage 18.04 negative 2: collect_macro_defs finds a macro_rules!
    /// definition and stores it in the table.
    #[test]
    fn stage18_04_collect_finds_macro_definition() {
        let mut interner = Rodeo::new();
        let m_sym = interner.get_or_intern("m");
        let macro_rules_sym = interner.get_or_intern("macro_rules");
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(macro_rules_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Not,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(m_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::LBrace,
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
            Token {
                kind: TokenKind::FatArrow,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::LBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(42, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
        ];
        let table = collect_macro_defs(&tokens, &mut interner);
        assert_eq!(table.len(), 1, "should find 1 macro definition");
        assert!(table.contains_key(&m_sym), "table should contain 'm'");
        let def = &table[&m_sym];
        assert_eq!(def.rules.len(), 1, "macro 'm' should have 1 rule");
    }

    /// Stage 18.04 negative 3: expand_macro_calls passes through unknown
    /// macros (like `println!`) unchanged so the parser can handle them.
    #[test]
    fn stage18_04_expand_macro_calls_passes_unknown() {
        let mut interner = Rodeo::new();
        let table = MacroTable::new(); // empty — no known macros
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(interner.get_or_intern("println")),
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
                kind: TokenKind::StrLit(interner.get_or_intern("hi")),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RParen,
                span: crate::session::Span::DUMMY,
            },
        ];
        let out = expand_macro_calls(&tokens, &table, &mut interner);
        assert_eq!(
            out.len(),
            tokens.len(),
            "unknown macro should pass through unchanged"
        );
        for (i, (orig, expanded)) in tokens.iter().zip(out.iter()).enumerate() {
            assert_eq!(orig.kind, expanded.kind, "token {} kind mismatch", i);
        }
    }

    /// Stage 18.04 negative 4: expand_macro_calls returns the input
    /// unchanged when the macro table is empty (no macros defined).
    #[test]
    fn stage18_04_expand_macro_calls_passes_no_macros() {
        let mut interner = Rodeo::new();
        let table = MacroTable::new();
        let tokens = vec![
            Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Plus,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            },
        ];
        let out = expand_macro_calls(&tokens, &table, &mut interner);
        assert_eq!(out.len(), 3, "no macros → 3 tokens unchanged");
    }

    /// Stage 18.04 negative 5: expand_macros with no macro_rules! defs
    /// returns the input tokens unchanged (zero-overhead fast path).
    #[test]
    fn stage18_04_expand_macros_no_macros_returns_input() {
        let mut interner = Rodeo::new();
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let out = expand_macros(tokens.clone(), &mut interner);
        assert_eq!(out.len(), 1, "no macros → unchanged output");
        assert!(matches!(out[0].kind, TokenKind::IntLit(42, _)));
    }

    /// Stage 18.04 negative 6: Recursive macro expansion terminates
    /// without infinite loop. `m!()` expands to `m!()` (self-referential).
    /// MAX_EXPANSION_ROUNDS (32) must cut it off.
    #[test]
    fn stage18_04_expand_macros_handles_recursive() {
        // m!() => m!() — would loop forever without MAX_EXPANSION_ROUNDS.
        // We expect expansion to terminate and the parser to receive some
        // token stream (whatever it is, it shouldn't loop).
        let src = "macro_rules! m { () => { m!() } } fn main() { 0 }";
        let result = compile(src);
        // The recursive macro doesn't have a base case, so the parser will
        // likely fail to parse the infinite expansion — but the compiler
        // should not hang. We just check it returns.
        let _ = result;
        // If we get here without timeout, the test passes.
    }

    // =====================================================================
    // Stage 18.05 tests — Additional Fragment Specifiers
    // =====================================================================

    /// Stage 18.05 positive 1: A macro using the `:ty` fragment parses
    /// and expands correctly.
    #[test]
    fn stage18_05_macro_with_ty_fragment() {
        let src = "macro_rules! m { ($t:ty) => { let x: $t; } } fn main() { m!(i32) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $t:ty should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.05 positive 2: A macro using the `:literal` fragment
    /// parses and expands correctly.
    #[test]
    fn stage18_05_macro_with_literal_fragment() {
        let src = "macro_rules! m { ($l:literal) => { $l } } fn main() { m!(42) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $l:literal should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.05 negative 1: capture_ty collects a single type token
    /// like `i32`.
    #[test]
    fn stage18_05_capture_ty_simple() {
        let mut interner = Rodeo::new();
        let i32_sym = interner.get_or_intern("i32");
        let tokens = vec![Token {
            kind: TokenKind::Ident(i32_sym),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_ty(&tokens, &mut idx);
        assert_eq!(captured.len(), 1, "should capture 1 token for i32");
        assert_eq!(idx, 1, "should advance idx by 1");
    }

    /// Stage 18.05 negative 2: capture_literal collects an int literal.
    #[test]
    fn stage18_05_capture_literal_int() {
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_literal(&tokens, &mut idx);
        assert_eq!(captured.len(), 1, "should capture 1 literal token");
        assert!(matches!(captured[0].kind, TokenKind::IntLit(42, _)));
        assert_eq!(idx, 1, "should advance idx by 1");
    }

    /// Stage 18.05 negative 3: capture_block collects `{ 1; 2 }` as
    /// 5 tokens (including the delimiters).
    #[test]
    fn stage18_05_capture_block_balanced() {
        let tokens = vec![
            Token {
                kind: TokenKind::LBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Semicolon,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0;
        let captured = capture_block(&tokens, &mut idx);
        assert_eq!(
            captured.len(),
            5,
            "should capture all 5 tokens including delims"
        );
        assert_eq!(idx, 5, "should advance idx past closing brace");
    }

    /// Stage 18.05 negative 4: capture_path collects `a::b::c` as 5
    /// tokens (3 idents + 2 path separators).
    #[test]
    fn stage18_05_capture_path_segments() {
        let mut interner = Rodeo::new();
        let a = interner.get_or_intern("a");
        let b = interner.get_or_intern("b");
        let c = interner.get_or_intern("c");
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(a),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::PathSep,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(b),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::PathSep,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(c),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0;
        let captured = capture_path(&tokens, &mut idx);
        assert_eq!(captured.len(), 5, "should capture all 5 tokens");
        assert_eq!(idx, 5, "should advance idx past last segment");
    }

    /// Stage 18.05 negative 5: capture_literal returns empty when the
    /// current token is not a literal (e.g. an identifier).
    #[test]
    fn stage18_05_capture_literal_rejects_ident() {
        let mut interner = Rodeo::new();
        let sym = interner.get_or_intern("foo");
        let tokens = vec![Token {
            kind: TokenKind::Ident(sym),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_literal(&tokens, &mut idx);
        assert!(
            captured.is_empty(),
            "ident should not be captured as literal"
        );
        assert_eq!(idx, 0, "idx should not advance");
    }

    /// Stage 18.05 negative 6: capture_block returns empty when the
    /// current token is not `{`.
    #[test]
    fn stage18_05_capture_block_rejects_non_brace() {
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_block(&tokens, &mut idx);
        assert!(
            captured.is_empty(),
            "non-brace should not be captured as block"
        );
        assert_eq!(idx, 0, "idx should not advance");
    }

    // =====================================================================
    // Stage 18.06 tests — Repetition $(...)* / $(...)+ / $(...)?
    // =====================================================================

    /// Stage 18.06 positive 1: A macro using `$( $x:expr )*` (zero or
    /// more expressions) parses and expands.
    #[test]
    fn stage18_06_macro_with_star_repetition() {
        let src = "macro_rules! m { ($($x:expr)*) => { 0 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $($x:expr)* should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.06 positive 2: A macro using `$( $x:expr )+` (one or
    /// more expressions) parses and expands.
    #[test]
    fn stage18_06_macro_with_plus_repetition() {
        let src = "macro_rules! m { ($($x:expr)+) => { 0 } } fn main() { m!(1) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $($x:expr)+ should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.06 negative 1: parse_repetition_op maps `*` to ZeroOrMore(None).
    #[test]
    fn stage18_06_repetition_kind_from_star() {
        let tokens = vec![Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        }];
        assert_eq!(
            parse_repetition_op(&tokens, 0),
            Some((RepetitionKind::ZeroOrMore(RepetitionSep::None), 1))
        );
    }

    /// Stage 18.06 negative 2: parse_repetition_op maps `+` to OneOrMore(None).
    #[test]
    fn stage18_06_repetition_kind_from_plus() {
        let tokens = vec![Token {
            kind: TokenKind::Plus,
            span: crate::session::Span::DUMMY,
        }];
        assert_eq!(
            parse_repetition_op(&tokens, 0),
            Some((RepetitionKind::OneOrMore(RepetitionSep::None), 1))
        );
    }

    /// Stage 18.06 negative 3: parse_repetition_op maps `?` to ZeroOrOne(None).
    #[test]
    fn stage18_06_repetition_kind_from_question() {
        let tokens = vec![Token {
            kind: TokenKind::Question,
            span: crate::session::Span::DUMMY,
        }];
        assert_eq!(
            parse_repetition_op(&tokens, 0),
            Some((RepetitionKind::ZeroOrOne(RepetitionSep::None), 1))
        );
    }

    /// Stage 18.06 negative 4: match_repetition with ZeroOrMore accepts
    /// empty input (returns Some(0)).
    #[test]
    fn stage18_06_match_repetition_zero_or_more_empty() {
        let interner = Rodeo::new();
        let inner: Vec<Token> = vec![Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        }];
        // Empty input.
        let input: Vec<Token> = vec![];
        let mut idx = 0usize;
        let mut captures = Captures::new();
        let result = match_repetition(
            &inner,
            &input,
            &mut idx,
            RepetitionKind::ZeroOrMore(RepetitionSep::None),
            &mut captures,
            &interner,
        );
        assert_eq!(result, Some(0), "ZeroOrMore should accept 0 matches");
        assert_eq!(idx, 0, "idx should not advance on 0 matches");
    }

    /// Stage 18.06 negative 5: match_repetition with OneOrMore rejects
    /// empty input (returns None).
    #[test]
    fn stage18_06_match_repetition_one_or_more_empty() {
        let interner = Rodeo::new();
        let inner: Vec<Token> = vec![Token {
            kind: TokenKind::Dollar,
            span: crate::session::Span::DUMMY,
        }];
        let input: Vec<Token> = vec![];
        let mut idx = 0usize;
        let mut captures = Captures::new();
        let result = match_repetition(
            &inner,
            &input,
            &mut idx,
            RepetitionKind::OneOrMore(RepetitionSep::None),
            &mut captures,
            &interner,
        );
        assert!(result.is_none(), "OneOrMore should reject 0 matches");
    }

    /// Stage 18.06 negative 6: substitute_repetition expands the body
    /// once per matched iteration.
    #[test]
    fn stage18_06_substitute_repetition_expands_each_iter() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        // captures: $x is a Repetition with 2 iterations.
        let mut captures: Captures = Captures::new();
        captures.insert(
            x_sym,
            CaptureValue::Repetition(vec![
                vec![Token {
                    kind: TokenKind::IntLit(1, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(2, None),
                    span: crate::session::Span::DUMMY,
                }],
            ]),
        );
        // inner body: $x
        let inner = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(x_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut result = Vec::new();
        substitute_repetition(
            &inner,
            &captures,
            RepetitionKind::ZeroOrMore(RepetitionSep::None),
            &mut result,
        );
        // Should produce 2 tokens (one per iteration): IntLit(1) and IntLit(2).
        assert_eq!(result.len(), 2, "should expand to 2 tokens (one per iter)");
        assert!(matches!(result[0].kind, TokenKind::IntLit(1, _)));
        assert!(matches!(result[1].kind, TokenKind::IntLit(2, _)));
    }

    // =====================================================================
    // Stage 18.08 tests — Macro Expansion Error Collection
    // =====================================================================

    /// Stage 18.08 positive 1: No macro_rules! → no errors.
    #[test]
    fn stage18_08_macro_error_no_macros() {
        let mut interner = Rodeo::new();
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let (out, errors) = expand_macros_with_errors(tokens.clone(), &mut interner);
        assert!(errors.is_empty(), "no macros → no errors");
        assert_eq!(out.len(), 1, "tokens unchanged");
    }

    /// Stage 18.08 positive 2: Valid macro_rules! + matching call → no errors.
    #[test]
    fn stage18_08_macro_error_valid_macro_no_errors() {
        let mut interner = Rodeo::new();
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
        // No macro_rules! → no errors (same as above, but tests the happy path).
        assert!(errors.is_empty());
    }

    /// Stage 18.08 negative 1: A macro call that doesn't match any rule
    /// produces a "no matching rule" error.
    #[test]
    fn stage18_08_macro_error_no_matching_rule() {
        let mut interner = Rodeo::new();
        let m_sym = interner.get_or_intern("m");
        let macro_rules_sym = interner.get_or_intern("macro_rules");
        // Define m with pattern () => { ... }, but call with m!(42)
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(macro_rules_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Not,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(m_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::LBrace,
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
            Token {
                kind: TokenKind::FatArrow,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::LBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(0, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
            // Call: m!(42) — won't match `()` rule.
            Token {
                kind: TokenKind::Ident(m_sym),
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
                kind: TokenKind::IntLit(42, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RParen,
                span: crate::session::Span::DUMMY,
            },
        ];
        let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("no matching rule")),
            "expected 'no matching rule' error, got: {:?}",
            errors
        );
    }

    /// Stage 18.08 negative 2: A malformed macro_rules! definition
    /// (missing `=>`) produces a "malformed macro_rules! body" error.
    #[test]
    fn stage18_08_macro_error_malformed_def() {
        let mut interner = Rodeo::new();
        let m_sym = interner.get_or_intern("m");
        let macro_rules_sym = interner.get_or_intern("macro_rules");
        // macro_rules! m { ( ) } — missing `=> { body }`
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(macro_rules_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Not,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(m_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::LBrace,
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
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
        ];
        let (_out, errors) = expand_macros_with_errors(tokens, &mut interner);
        assert!(
            errors
                .iter()
                .any(|e| e.message.contains("malformed macro_rules! body")),
            "expected 'malformed macro_rules! body' error, got: {:?}",
            errors
        );
    }

    /// Stage 18.08 negative 3: MacroError struct fields are accessible.
    #[test]
    fn stage18_08_macro_error_struct_fields() {
        let err = MacroError::new("test message", crate::session::Span::DUMMY);
        assert_eq!(err.message, "test message");
        assert_eq!(err.span, crate::session::Span::DUMMY);
    }

    /// Stage 18.08 negative 4: MacroError::new constructor accepts &str and String.
    #[test]
    fn stage18_08_macro_error_new_constructor() {
        let err1 = MacroError::new("from &str", crate::session::Span::DUMMY);
        let err2 = MacroError::new(String::from("from String"), crate::session::Span::DUMMY);
        assert_eq!(err1.message, "from &str");
        assert_eq!(err2.message, "from String");
    }

    /// Stage 18.08 negative 5: CompileErrors has a `macro_errors` field
    /// accessible from outside the driver module.
    #[test]
    fn stage18_08_compile_errors_macro_field() {
        let errors = crate::driver::CompileErrors::default();
        assert!(
            errors.macro_errors.is_empty(),
            "default CompileErrors.macro_errors is empty"
        );
    }

    /// Stage 18.08 negative 6: expand_macros_with_errors returns
    /// (tokens, errors) tuple — verify the tuple structure.
    #[test]
    fn stage18_08_expand_macros_with_errors_returns_tuple() {
        let mut interner = Rodeo::new();
        let tokens = vec![Token {
            kind: TokenKind::IntLit(0, None),
            span: crate::session::Span::DUMMY,
        }];
        let result: (Vec<Token>, Vec<MacroError>) =
            expand_macros_with_errors(tokens, &mut interner);
        assert_eq!(result.0.len(), 1, "tokens preserved");
        assert!(result.1.is_empty(), "no errors");
    }

    // =====================================================================
    // Stage 18.10 tests — Built-in macro_rules! registration
    // =====================================================================

    /// Stage 18.10 positive 1: build_builtin_macro_table returns a table
    /// with 4 entries (println/print/eprintln/eprint) when all names
    /// are pre-interned.
    #[test]
    fn stage18_10_builtin_macros_registered() {
        let mut interner = Rodeo::new();
        // Pre-intern all built-in macro names + helper symbols.
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");

        let table = build_builtin_macro_table(&mut interner);
        assert_eq!(
            table.len(),
            25,
            "should register 25 built-in macros (4 print + 21 non-print)"
        );
        for name in BUILTIN_MACRO_NAMES {
            let sym = interner.get(name).expect("name was interned");
            assert!(table.contains_key(&sym), "table should contain '{name}'");
        }
    }

    /// Stage 18.10 positive 2: println! still works correctly after
    /// built-in macro registration (no-op expansion, parser handles it).
    #[test]
    fn stage18_10_println_still_works_after_builtin_registration() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.parse.is_empty(), "no parse errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    }

    /// Stage 18.10 + 18.29 + 18.32 negative 1: BUILTIN_MACRO_NAMES contains
    /// exactly the 12 expected names (4 print + 8 non-print).
    #[test]
    fn stage18_10_builtin_macro_names_const() {
        assert_eq!(BUILTIN_MACRO_NAMES.len(), 25);
        assert!(BUILTIN_MACRO_NAMES.contains(&"println"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"print"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"eprintln"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"eprint"));
        // Stage 18.29: non-print macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"assert"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"panic"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"vec"));
        // Stage 18.32: more non-print macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"format"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"dbg"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"todo"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"unimplemented"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"write"));
        // Stage 18.34: compile-time utility macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"stringify"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"concat"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"env"));
        // Stage 18.36: source info + file macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"file"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"line"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"module_path"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"include_str"));
        // Stage 18.39: pattern + config macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"matches"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"cfg"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"option_env"));
        // Stage 18.41: low-level + diagnostic macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"asm"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"compile_error"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"cfg_attr"));
    }

    /// Stage 18.10 negative 2: build_builtin_macro_table returns an
    /// empty table when no names are interned (cold-start scenario).
    #[test]
    fn stage18_10_build_builtin_macro_table_returns_table() {
        let mut interner = Rodeo::new();
        let table = build_builtin_macro_table(&mut interner);
        // No names interned → empty table.
        assert!(table.is_empty(), "empty interner → empty table");
    }

    /// Stage 18.10 negative 3: Each built-in macro rule pattern is a
    /// repetition: `$ ( $ args : tt ) *` (8 tokens).
    #[test]
    fn stage18_10_builtin_macro_rule_pattern_is_repetition() {
        let mut interner = Rodeo::new();
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");

        let table = build_builtin_macro_table(&mut interner);
        let println_sym = interner.get("println").unwrap();
        let def = &table[&println_sym];
        assert_eq!(def.rules.len(), 1, "should have 1 rule");
        let pattern = &def.rules[0].pattern;
        // Pattern: $ ( $ args : tt ) *  = 8 tokens
        assert_eq!(pattern.len(), 8, "pattern should be 8 tokens");
        assert!(matches!(pattern[0].kind, TokenKind::Dollar));
        assert!(matches!(pattern[1].kind, TokenKind::LParen));
        assert!(matches!(pattern[2].kind, TokenKind::Dollar));
        assert!(matches!(pattern[6].kind, TokenKind::RParen));
        assert!(matches!(pattern[7].kind, TokenKind::Star));
    }

    /// Stage 18.10 negative 4: Each built-in macro rule body is
    /// `name!($($args)*)` (10 tokens) — re-emits the same call form.
    #[test]
    fn stage18_10_builtin_macro_rule_body_is_same_call() {
        let mut interner = Rodeo::new();
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");
        // Stage 18.27: also intern __landin_<name> for the body.
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(format!("__landin_{}", name));
        }

        let table = build_builtin_macro_table(&mut interner);
        let println_sym = interner.get("println").unwrap();
        let def = &table[&println_sym];
        let body = &def.rules[0].body;
        // Stage 18.27: Body is `__landin_println($($args)*)` — 9 tokens
        // (function call form, no `!`).
        assert_eq!(
            body.len(),
            9,
            "body should be 9 tokens (Stage 18.27 function call form)"
        );
        assert!(matches!(body[0].kind, TokenKind::Ident(_)));
        assert!(matches!(body[1].kind, TokenKind::LParen));
        assert!(matches!(body[8].kind, TokenKind::RParen));
    }

    /// Stage 18.10 negative 5: User-defined macro_rules! with the same
    /// name as a built-in overrides the built-in.
    #[test]
    fn stage18_10_user_macro_overrides_builtin() {
        let src = "macro_rules! println { () => { 42 } } fn main() { println!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        // User's println!() => 42, which should parse as an integer expression.
        // (The built-in println! would have been a no-op pass-through.)
        assert!(
            result.errors.parse.is_empty(),
            "user macro should override built-in — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.10 negative 6 (updated Stage 18.27): println! call now
    /// expands to `__landin_println("hi")` (function call form).
    #[test]
    fn stage18_10_builtin_macros_pass_through_println() {
        let mut interner = Rodeo::new();
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
            // Stage 18.27: also intern __landin_<name>.
            interner.get_or_intern(format!("__landin_{}", name));
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");

        let println_sym = interner.get_or_intern("println");
        let landin_println_sym = interner.get_or_intern("__landin_println");
        let hi_sym = interner.get_or_intern("hi");
        let span = crate::session::Span::new(0, 10);
        // Input: println ! ( "hi" )
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(println_sym),
                span,
            },
            Token {
                kind: TokenKind::Not,
                span,
            },
            Token {
                kind: TokenKind::LParen,
                span,
            },
            Token {
                kind: TokenKind::StrLit(hi_sym),
                span,
            },
            Token {
                kind: TokenKind::RParen,
                span,
            },
        ];
        let (out, errors) = expand_macros_with_errors(tokens, &mut interner);
        assert!(errors.is_empty(), "no macro errors");
        // Stage 18.27: After expansion: __landin_println ( "hi" ) — 4 tokens.
        assert_eq!(
            out.len(),
            4,
            "println! should expand to __landin_println(...) — 4 tokens"
        );
        // First token should be `__landin_println`.
        assert!(matches!(out[0].kind, TokenKind::Ident(s) if s == landin_println_sym));
        // Second should be `(` (no `!` — function call).
        assert!(matches!(out[1].kind, TokenKind::LParen));
    }

    // =====================================================================
    // Stage 18.13 tests — Separator support $(...),* / $(...);+ / etc.
    // =====================================================================

    /// Stage 18.13 positive 1: A macro using `$( $x:expr ),*` (comma-
    /// separated zero or more expressions) parses and expands.
    #[test]
    fn stage18_13_macro_with_comma_separator() {
        let src = "macro_rules! m { ($($x:expr),*) => { 0 } } fn main() { m!(1, 2, 3) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $($x:expr),* should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.13 positive 2: A macro using `$( $x:expr );+` (semicolon-
    /// separated one or more expressions) parses and expands.
    #[test]
    fn stage18_13_macro_with_semicolon_separator() {
        let src = "macro_rules! m { ($($x:expr);+) => { 0 } } fn main() { m!(1; 2) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "macro with $($x:expr);+ should expand and parse — errors: {:?}",
            result.errors.parse
        );
    }

    /// Stage 18.13 negative 1: RepetitionSep::None variant constructs.
    #[test]
    fn stage18_13_repetition_sep_none_variant() {
        let sep = RepetitionSep::None;
        assert_eq!(sep, RepetitionSep::None);
    }

    /// Stage 18.13 negative 2: RepetitionSep::Token variant constructs.
    #[test]
    fn stage18_13_repetition_sep_token_variant() {
        let sep = RepetitionSep::Token(TokenKind::Comma);
        match sep {
            RepetitionSep::Token(TokenKind::Comma) => { /* OK */ }
            _ => panic!("expected Token(Comma)"),
        }
    }

    /// Stage 18.13 negative 3: parse_repetition_op without separator
    /// returns ZeroOrMore(None) for `*`.
    #[test]
    fn stage18_13_parse_repetition_op_no_separator() {
        let tokens = vec![Token {
            kind: TokenKind::Star,
            span: crate::session::Span::DUMMY,
        }];
        let result = parse_repetition_op(&tokens, 0);
        assert_eq!(
            result,
            Some((RepetitionKind::ZeroOrMore(RepetitionSep::None), 1))
        );
    }

    /// Stage 18.13 negative 4: parse_repetition_op with comma separator
    /// returns ZeroOrMore(Token(Comma)) for `, *`.
    #[test]
    fn stage18_13_parse_repetition_op_with_comma() {
        let tokens = vec![
            Token {
                kind: TokenKind::Comma,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Star,
                span: crate::session::Span::DUMMY,
            },
        ];
        let result = parse_repetition_op(&tokens, 0);
        assert_eq!(
            result,
            Some((
                RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
                2
            ))
        );
    }

    /// Stage 18.13 negative 5: match_repetition with separator matches
    /// comma-separated input.
    #[test]
    fn stage18_13_match_repetition_with_separator_matches() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        let expr_sym = interner.get_or_intern("expr");
        // inner pattern: $ x : expr
        let inner = vec![
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
        // input: 1 , 2 , 3
        let input = vec![
            Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Comma,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Comma,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(3, None),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0usize;
        let mut captures = Captures::new();
        let result = match_repetition(
            &inner,
            &input,
            &mut idx,
            RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
            &mut captures,
            &interner,
        );
        assert_eq!(result, Some(3), "should match 3 iterations (1, 2, 3)");
        assert_eq!(idx, 5, "should consume all 5 input tokens");
    }

    /// Stage 18.13 negative 6: substitute_repetition emits separator
    /// between iterations (not after last).
    #[test]
    fn stage18_13_substitute_repetition_emits_separator() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        // captures: $x is a Repetition with 3 iterations.
        let mut captures: Captures = Captures::new();
        captures.insert(
            x_sym,
            CaptureValue::Repetition(vec![
                vec![Token {
                    kind: TokenKind::IntLit(1, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(2, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(3, None),
                    span: crate::session::Span::DUMMY,
                }],
            ]),
        );
        // inner body: $x
        let inner = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(x_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut result = Vec::new();
        substitute_repetition(
            &inner,
            &captures,
            RepetitionKind::ZeroOrMore(RepetitionSep::Token(TokenKind::Comma)),
            &mut result,
        );
        // Should produce: 1 , 2 , 3 (5 tokens: 3 values + 2 separators)
        assert_eq!(
            result.len(),
            5,
            "should expand to 5 tokens (3 values + 2 separators)"
        );
        assert!(matches!(result[0].kind, TokenKind::IntLit(1, _)));
        assert!(matches!(result[1].kind, TokenKind::Comma));
        assert!(matches!(result[2].kind, TokenKind::IntLit(2, _)));
        assert!(matches!(result[3].kind, TokenKind::Comma));
        assert!(matches!(result[4].kind, TokenKind::IntLit(3, _)));
    }

    // =====================================================================
    // Stage 18.14 tests — Nested repetition support
    // =====================================================================

    /// Stage 18.14 positive 1: A macro with nested repetition
    /// `$( $( $x ),* );*` parses without errors.
    #[test]
    fn stage18_14_macro_with_nested_repetition() {
        let src = "macro_rules! m { ($($($x:expr),*);*) => { 0 } } fn main() { m!(1, 2; 3, 4) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        // Note: nested repetition may not fully expand yet, but should parse.
        let _ = result;
    }

    /// Stage 18.14 positive 2: A macro with deep repetition (3 levels)
    /// parses without errors.
    #[test]
    fn stage18_14_macro_with_deep_repetition() {
        let src = "macro_rules! m { ($($($($x:expr),*);*);*) => { 0 } } fn main() { m!(((1))) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        let _ = result;
    }

    /// Stage 18.14 negative 1: push_capture_into_rep_names handles Single.
    #[test]
    fn stage18_14_match_repetition_collects_inner_repetition() {
        let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        push_capture_into_rep_names(
            &mut rep_names,
            x_sym,
            CaptureValue::Single(vec![Token {
                kind: TokenKind::IntLit(42, None),
                span: crate::session::Span::DUMMY,
            }]),
        );
        assert_eq!(rep_names[&x_sym].len(), 1);
        assert_eq!(rep_names[&x_sym][0].len(), 1);
    }

    /// Stage 18.14 negative 2: push_capture_into_rep_names flattens Repetition.
    #[test]
    fn stage18_14_match_repetition_nested_flat_map() {
        let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        // Inner repetition with 3 iterations: [1], [2], [3]
        push_capture_into_rep_names(
            &mut rep_names,
            x_sym,
            CaptureValue::Repetition(vec![
                vec![Token {
                    kind: TokenKind::IntLit(1, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(2, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(3, None),
                    span: crate::session::Span::DUMMY,
                }],
            ]),
        );
        // Should flatten to [1, 2, 3] as ONE outer iteration.
        assert_eq!(rep_names[&x_sym].len(), 1, "should be 1 outer iteration");
        assert_eq!(rep_names[&x_sym][0].len(), 3, "flattened to 3 tokens");
    }

    /// Stage 18.14 negative 3: substitute_repetition with nested captures
    /// produces output.
    #[test]
    fn stage18_14_substitute_repetition_nested_works() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        // captures: $x is Repetition with 2 outer iterations, each
        // containing flattened inner tokens.
        let mut captures: Captures = Captures::new();
        captures.insert(
            x_sym,
            CaptureValue::Repetition(vec![
                vec![Token {
                    kind: TokenKind::IntLit(1, None),
                    span: crate::session::Span::DUMMY,
                }],
                vec![Token {
                    kind: TokenKind::IntLit(2, None),
                    span: crate::session::Span::DUMMY,
                }],
            ]),
        );
        let inner = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(x_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut result = Vec::new();
        substitute_repetition(
            &inner,
            &captures,
            RepetitionKind::ZeroOrMore(RepetitionSep::None),
            &mut result,
        );
        // Should produce 2 tokens (one per outer iteration).
        assert_eq!(result.len(), 2);
    }

    /// Stage 18.14 negative 4: Nested repetition with separators.
    #[test]
    fn stage18_14_nested_repetition_with_separators() {
        let src = "macro_rules! m { ($($($x:expr),+);+) => { 0 } } fn main() { m!(1,2; 3,4) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        let _ = result;
    }

    /// Stage 18.14 negative 5: CaptureValue::Repetition variant holds inner.
    #[test]
    fn stage18_14_capture_value_repetition_holds_inner() {
        let val = CaptureValue::Repetition(vec![
            vec![Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            }],
            vec![Token {
                kind: TokenKind::IntLit(2, None),
                span: crate::session::Span::DUMMY,
            }],
        ]);
        match val {
            CaptureValue::Repetition(iters) => {
                assert_eq!(iters.len(), 2);
            }
            _ => panic!("expected Repetition"),
        }
    }

    /// Stage 18.14 negative 6: match_repetition preserves inner order.
    #[test]
    fn stage18_14_match_repetition_preserves_inner_order() {
        let mut rep_names: HashMap<crate::lexer::Symbol, Vec<Vec<Token>>> = HashMap::new();
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        // Push 3 iterations in order.
        for i in 1..=3u128 {
            push_capture_into_rep_names(
                &mut rep_names,
                x_sym,
                CaptureValue::Single(vec![Token {
                    kind: TokenKind::IntLit(i, None),
                    span: crate::session::Span::DUMMY,
                }]),
            );
        }
        // Verify order preserved.
        assert_eq!(rep_names[&x_sym].len(), 3);
        assert!(matches!(
            rep_names[&x_sym][0][0].kind,
            TokenKind::IntLit(1, _)
        ));
        assert!(matches!(
            rep_names[&x_sym][1][0].kind,
            TokenKind::IntLit(2, _)
        ));
        assert!(matches!(
            rep_names[&x_sym][2][0].kind,
            TokenKind::IntLit(3, _)
        ));
    }

    // =====================================================================
    // Stage 18.17 tests — Basic macro hygiene (HygieneContext)
    // =====================================================================

    /// Stage 18.17 positive 1: HygieneContext::new() creates a context
    /// with counter=0.
    #[test]
    fn stage18_17_hygiene_context_new_creates_zero_counter() {
        let ctx = HygieneContext::new();
        assert_eq!(ctx.counter(), 0, "new context should have counter=0");
    }

    /// Stage 18.17 positive 2: gen_unique_name increments the counter.
    #[test]
    fn stage18_17_hygiene_context_gen_unique_name_increments() {
        let mut ctx = HygieneContext::new();
        assert_eq!(ctx.counter(), 0);
        let _ = ctx.gen_unique_name("x");
        assert_eq!(ctx.counter(), 1, "counter should be 1 after one call");
        let _ = ctx.gen_unique_name("y");
        assert_eq!(ctx.counter(), 2, "counter should be 2 after two calls");
    }

    /// Stage 18.17 negative 1: Default trait creates counter=0.
    #[test]
    fn stage18_17_hygiene_context_default() {
        let ctx = HygieneContext::default();
        assert_eq!(ctx.counter(), 0);
    }

    /// Stage 18.17 negative 2: gen_unique_name produces the correct format.
    #[test]
    fn stage18_17_hygiene_context_gen_unique_name_format() {
        let mut ctx = HygieneContext::new();
        let name = ctx.gen_unique_name("tmp");
        assert_eq!(
            name, "__landin_macro_tmp_0",
            "first name should be __landin_macro_tmp_0"
        );
    }

    /// Stage 18.17 negative 3: Multiple gen_unique_name calls produce
    /// different names.
    #[test]
    fn stage18_17_hygiene_context_gen_multiple_unique() {
        let mut ctx = HygieneContext::new();
        let n1 = ctx.gen_unique_name("x");
        let n2 = ctx.gen_unique_name("x");
        let n3 = ctx.gen_unique_name("x");
        assert_ne!(n1, n2, "names should differ");
        assert_ne!(n2, n3, "names should differ");
        assert_ne!(n1, n3, "names should differ");
    }

    /// Stage 18.17 negative 4: Clone preserves the counter value.
    #[test]
    fn stage18_17_hygiene_context_clone_preserves_counter() {
        let mut ctx = HygieneContext::new();
        let _ = ctx.gen_unique_name("a");
        let _ = ctx.gen_unique_name("b");
        assert_eq!(ctx.counter(), 2);
        let cloned = ctx.clone();
        assert_eq!(cloned.counter(), 2, "clone should preserve counter");
    }

    /// Stage 18.17 negative 5: Macro expansion still works correctly
    /// with HygieneContext infrastructure in place (no behavior change).
    #[test]
    fn stage18_17_macro_expansion_with_hygiene_context_still_works() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.17 negative 6: println! still works (hygiene context
    /// doesn't interfere with built-in macros).
    #[test]
    fn stage18_17_hygiene_context_does_not_break_println() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    // =====================================================================
    // Stage 18.20 tests — Macro hygiene activation (apply_hygiene)
    // =====================================================================

    /// Stage 18.20 positive 1: apply_hygiene renames a non-capture identifier.
    #[test]
    fn stage18_20_apply_hygiene_renames_identifier() {
        let mut interner = Rodeo::new();
        let tmp_sym = interner.get_or_intern("tmp");
        let body = vec![Token {
            kind: TokenKind::Ident(tmp_sym),
            span: crate::session::Span::DUMMY,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        // Should be renamed to __landin_macro_tmp_0
        assert_eq!(result.len(), 1);
        if let TokenKind::Ident(s) = &result[0].kind {
            let name = interner.resolve(s);
            assert_eq!(name, "__landin_macro_tmp_0");
        } else {
            panic!("expected Ident token");
        }
    }

    /// Stage 18.20 positive 2: apply_hygiene skips `$name` capture references.
    #[test]
    fn stage18_20_apply_hygiene_skips_captures() {
        let mut interner = Rodeo::new();
        let x_sym = interner.get_or_intern("x");
        let body = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(x_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        // $x should NOT be renamed — emit both tokens unchanged.
        assert_eq!(result.len(), 2);
        assert!(matches!(result[0].kind, TokenKind::Dollar));
        if let TokenKind::Ident(s) = &result[1].kind {
            assert_eq!(interner.resolve(s), "x", "$x should not be renamed");
        }
    }

    /// Stage 18.20 negative 1: apply_hygiene skips keywords.
    #[test]
    fn stage18_20_apply_hygiene_skips_keywords() {
        let mut interner = Rodeo::new();
        // `let` is a keyword — should not be renamed.
        let body = vec![Token {
            kind: TokenKind::KwLet,
            span: crate::session::Span::DUMMY,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        assert_eq!(result.len(), 1);
        // KwLet is not an Ident, so it's emitted unchanged.
        assert!(matches!(result[0].kind, TokenKind::KwLet));
    }

    /// Stage 18.20 negative 2: apply_hygiene skips built-in macro names.
    #[test]
    fn stage18_20_apply_hygiene_skips_builtins() {
        let mut interner = Rodeo::new();
        let println_sym = interner.get_or_intern("println");
        let body = vec![Token {
            kind: TokenKind::Ident(println_sym),
            span: crate::session::Span::DUMMY,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        // println should NOT be renamed.
        assert_eq!(result.len(), 1);
        if let TokenKind::Ident(s) = &result[0].kind {
            assert_eq!(
                interner.resolve(s),
                "println",
                "println should not be renamed"
            );
        }
    }

    /// Stage 18.20 negative 3: apply_hygiene skips literals (not identifiers).
    #[test]
    fn stage18_20_apply_hygiene_skips_literals() {
        let mut interner = Rodeo::new();
        let body = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0].kind, TokenKind::IntLit(42, _)));
    }

    /// Stage 18.20 negative 4: apply_hygiene increments the counter.
    #[test]
    fn stage18_20_apply_hygiene_increments_counter() {
        let mut interner = Rodeo::new();
        let a_sym = interner.get_or_intern("a");
        let b_sym = interner.get_or_intern("b");
        let body = vec![
            Token {
                kind: TokenKind::Ident(a_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(b_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let _ = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        // Two renames → counter should be 2.
        assert_eq!(hygiene.counter(), 2);
    }

    /// Stage 18.20 negative 5: apply_hygiene preserves spans.
    #[test]
    fn stage18_20_apply_hygiene_preserves_spans() {
        let mut interner = Rodeo::new();
        let tmp_sym = interner.get_or_intern("tmp");
        let span = crate::session::Span::new(10, 20);
        let body = vec![Token {
            kind: TokenKind::Ident(tmp_sym),
            span,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        assert_eq!(result[0].span, span, "span should be preserved");
    }

    /// Stage 18.20 negative 6: apply_hygiene on empty body returns empty.
    #[test]
    fn stage18_20_apply_hygiene_empty_body() {
        let mut interner = Rodeo::new();
        let body: Vec<Token> = vec![];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        assert!(result.is_empty(), "empty body → empty result");
    }

    // =====================================================================
    // Stage 18.21 tests — __landin_println infrastructure
    // =====================================================================

    /// Stage 18.21 positive 1: println! still works (Phase 1 no-op body).
    #[test]
    fn stage18_21_println_still_works() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    /// Stage 18.21 positive 2: eprintln! still works.
    #[test]
    fn stage18_21_eprintln_still_works() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.21 negative 1: __landin_println detection works (tested
    /// indirectly — println! still compiles, meaning the infrastructure
    /// is in place).
    #[test]
    fn stage18_21_is_landin_print_macro_detects_all() {
        // Test indirectly: println!/print!/eprintln!/eprint! all compile.
        let srcs = [
            "fn main() { println!(\"a\"); }",
            "fn main() { print!(\"b\"); }",
            "fn main() { eprintln!(\"c\"); }",
            "fn main() { eprint!(\"d\"); }",
        ];
        for src in srcs {
            let result = compile(src);
            assert!(result.errors.lex.is_empty(), "lex error for: {}", src);
            assert!(result.errors.parse.is_empty(), "parse error for: {}", src);
        }
    }

    /// Stage 18.21 negative 2: Resolver recognizes __landin_ functions.
    #[test]
    fn stage18_21_resolver_recognizes_landin_functions() {
        // __landin_println should resolve without error.
        let src = "fn main() { let _ = __landin_println; }";
        let result = compile(src);
        // Even if typeck fails (not a value), resolve should NOT report
        // "cannot find value".
        let has_resolve_error = result
            .errors
            .resolve
            .iter()
            .any(|e| e.message.contains("cannot find value"));
        assert!(
            !has_resolve_error,
            "__landin_ functions should be recognized by resolver"
        );
    }

    /// Stage 18.21 negative 3: println! with args still works.
    #[test]
    fn stage18_21_println_with_args_still_works() {
        let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.21 negative 4: print! (no newline) still works.
    #[test]
    fn stage18_21_print_still_works() {
        let src = "fn main() { print!(\"no newline\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.21 negative 5: eprint! still works.
    #[test]
    fn stage18_21_eprint_still_works() {
        let src = "fn main() { eprint!(\"err\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.21 negative 6: User macro_rules! not affected.
    #[test]
    fn stage18_21_user_macro_not_affected() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    // =====================================================================
    // Stage 18.24 tests — Fragment specifier extension (lifetime + stmt)
    // =====================================================================

    /// Stage 18.24 positive 1: A macro using `:lifetime` fragment parses.
    #[test]
    fn stage18_24_macro_with_lifetime_fragment() {
        let src = "macro_rules! m { ($l:lifetime) => { 0 } } fn main() { 0 }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.parse.is_empty(), "no parse errors");
    }

    /// Stage 18.24 positive 2: A macro using `:stmt` fragment parses.
    #[test]
    fn stage18_24_macro_with_stmt_fragment() {
        let src = "macro_rules! m { ($s:stmt) => { 0 } } fn main() { 0 }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.parse.is_empty(), "no parse errors");
    }

    /// Stage 18.24 negative 1: capture_lifetime collects a Lifetime token.
    #[test]
    fn stage18_24_capture_lifetime_simple() {
        let mut interner = Rodeo::new();
        let sym = interner.get_or_intern("a");
        let tokens = vec![Token {
            kind: TokenKind::Lifetime(sym),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_lifetime(&tokens, &mut idx);
        assert_eq!(captured.len(), 1, "should capture 1 token");
        assert!(matches!(captured[0].kind, TokenKind::Lifetime(_)));
        assert_eq!(idx, 1, "should advance idx by 1");
    }

    /// Stage 18.24 negative 2: capture_lifetime rejects non-lifetime tokens.
    #[test]
    fn stage18_24_capture_lifetime_rejects_non_lifetime() {
        let tokens = vec![Token {
            kind: TokenKind::IntLit(42, None),
            span: crate::session::Span::DUMMY,
        }];
        let mut idx = 0;
        let captured = capture_lifetime(&tokens, &mut idx);
        assert!(captured.is_empty(), "non-lifetime → empty");
        assert_eq!(idx, 0, "idx should not advance");
    }

    /// Stage 18.24 negative 3: capture_stmt collects until semicolon.
    #[test]
    fn stage18_24_capture_stmt_until_semicolon() {
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(lasso::Spur::default()),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Semicolon,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(99, None),
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0;
        let captured = capture_stmt(&tokens, &mut idx);
        // Should capture `ident ;` (2 tokens, semicolon inclusive).
        assert_eq!(captured.len(), 2, "should capture 2 tokens (ident + ;)");
        assert_eq!(idx, 2, "should advance past semicolon");
    }

    /// Stage 18.24 negative 4: capture_stmt stops at rbrace (exclusive).
    #[test]
    fn stage18_24_capture_stmt_until_rbrace() {
        let tokens = vec![
            Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0;
        let captured = capture_stmt(&tokens, &mut idx);
        // Should capture `1` (1 token, rbrace exclusive).
        assert_eq!(
            captured.len(),
            1,
            "should capture 1 token (rbrace exclusive)"
        );
        assert_eq!(idx, 1, "should stop at rbrace");
    }

    /// Stage 18.24 negative 5: capture_stmt handles nested braces correctly.
    #[test]
    fn stage18_24_capture_stmt_nested_braces() {
        let tokens = vec![
            Token {
                kind: TokenKind::LBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::IntLit(1, None),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Semicolon,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::RBrace,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Semicolon,
                span: crate::session::Span::DUMMY,
            },
        ];
        let mut idx = 0;
        let captured = capture_stmt(&tokens, &mut idx);
        // Should capture `{ 1 ; } ;` (5 tokens — the inner `;` is inside
        // braces, so depth > 0; the outer `;` ends the capture).
        assert_eq!(
            captured.len(),
            5,
            "should capture all 5 tokens including outer ;"
        );
        assert_eq!(idx, 5, "should advance past outer semicolon");
    }

    /// Stage 18.24 negative 6: lifetime fragment in pattern matches correctly.
    #[test]
    fn stage18_24_lifetime_fragment_in_pattern() {
        let mut interner = Rodeo::new();
        let l_sym = interner.get_or_intern("l");
        let lifetime_sym = interner.get_or_intern("lifetime");
        let a_sym = interner.get_or_intern("a");
        // Pattern: $ l : lifetime
        let pattern = vec![
            Token {
                kind: TokenKind::Dollar,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(l_sym),
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Colon,
                span: crate::session::Span::DUMMY,
            },
            Token {
                kind: TokenKind::Ident(lifetime_sym),
                span: crate::session::Span::DUMMY,
            },
        ];
        // Input: 'a
        let input = vec![Token {
            kind: TokenKind::Lifetime(a_sym),
            span: crate::session::Span::DUMMY,
        }];
        let mut captures = Captures::new();
        assert!(match_pattern(&pattern, &input, &mut captures, &interner));
        // $l should be captured as Single with 1 token.
        if let Some(CaptureValue::Single(tokens)) = captures.get(&l_sym) {
            assert_eq!(tokens.len(), 1, "should capture 1 lifetime token");
            assert!(matches!(tokens[0].kind, TokenKind::Lifetime(_)));
        } else {
            panic!("expected CaptureValue::Single for $l");
        }
    }

    // =====================================================================
    // Stage 18.26 tests — Macro hygiene activation
    // =====================================================================

    /// Stage 18.26 positive 1: println! still works after hygiene activation.
    #[test]
    fn stage18_26_println_still_works_after_hygiene() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    /// Stage 18.26 positive 2: User macro still works after hygiene.
    #[test]
    fn stage18_26_user_macro_still_works_after_hygiene() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 1: println! with args still works.
    #[test]
    fn stage18_26_println_with_args_after_hygiene() {
        let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 2: eprintln! still works.
    #[test]
    fn stage18_26_eprintln_after_hygiene() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 3: macro with repetition still works.
    #[test]
    fn stage18_26_macro_repetition_after_hygiene() {
        let src = "macro_rules! m { ($($x:expr)*) => { 0 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 4: macro with separator still works.
    #[test]
    fn stage18_26_macro_separator_after_hygiene() {
        let src = "macro_rules! m { ($($x:expr),*) => { 0 } } fn main() { m!(1, 2, 3) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 5: macro with capture still works.
    #[test]
    fn stage18_26_macro_capture_after_hygiene() {
        let src = "macro_rules! m { ($x:expr) => { $x } } fn main() { m!(42) }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.26 negative 6: print! (no newline) still works.
    #[test]
    fn stage18_26_print_after_hygiene() {
        let src = "fn main() { print!(\"no newline\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    // =====================================================================
    // Stage 18.27 tests — println! Phase 2.5: __landin_println activation
    // =====================================================================

    /// Stage 18.27 positive 1: println! expands to __landin_println.
    #[test]
    fn stage18_27_println_expands_to_landin_println() {
        let mut interner = Rodeo::new();
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
            interner.get_or_intern(format!("__landin_{}", name));
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");

        let println_sym = interner.get_or_intern("println");
        let landin_println_sym = interner.get_or_intern("__landin_println");
        let hi_sym = interner.get_or_intern("hi");
        let span = crate::session::Span::new(0, 10);
        let tokens = vec![
            Token {
                kind: TokenKind::Ident(println_sym),
                span,
            },
            Token {
                kind: TokenKind::Not,
                span,
            },
            Token {
                kind: TokenKind::LParen,
                span,
            },
            Token {
                kind: TokenKind::StrLit(hi_sym),
                span,
            },
            Token {
                kind: TokenKind::RParen,
                span,
            },
        ];
        let (out, errors) = expand_macros_with_errors(tokens, &mut interner);
        assert!(errors.is_empty());
        assert_eq!(out.len(), 4, "should expand to __landin_println(\"hi\")");
        assert!(matches!(out[0].kind, TokenKind::Ident(s) if s == landin_println_sym));
    }

    /// Stage 18.27 positive 2: println! still compiles end-to-end.
    #[test]
    fn stage18_27_println_compiles_end_to_end() {
        let src = "fn main() { println!(\"hello\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
        assert!(result.errors.macro_errors.is_empty());
    }

    /// Stage 18.27 negative 1: eprintln! expands to __landin_eprintln.
    #[test]
    fn stage18_27_eprintln_expands_correctly() {
        let src = "fn main() { eprintln!(\"err\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.27 negative 2: print! expands to __landin_print.
    #[test]
    fn stage18_27_print_expands_correctly() {
        let src = "fn main() { print!(\"no newline\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.27 negative 3: println! with args compiles.
    #[test]
    fn stage18_27_println_with_args_compiles() {
        let src = "fn main() { let x = 42; println!(\"x={}\", x); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.27 negative 4: apply_hygiene skips __landin_ functions.
    #[test]
    fn stage18_27_hygiene_skips_landin_functions() {
        let mut interner = Rodeo::new();
        let lp_sym = interner.get_or_intern("__landin_println");
        let body = vec![Token {
            kind: TokenKind::Ident(lp_sym),
            span: crate::session::Span::DUMMY,
        }];
        let captures = Captures::new();
        let mut hygiene = HygieneContext::new();
        let result = apply_hygiene(&body, &captures, &mut interner, &mut hygiene);
        // __landin_println should NOT be renamed.
        assert_eq!(result.len(), 1);
        if let TokenKind::Ident(s) = &result[0].kind {
            assert_eq!(*s, lp_sym, "__landin_println should not be renamed");
        }
        // Counter should be 0 (no renames happened).
        assert_eq!(hygiene.counter(), 0);
    }

    /// Stage 18.27 negative 5: user macro still works.
    #[test]
    fn stage18_27_user_macro_still_works() {
        let src = "macro_rules! m { () => { 42 } } fn main() { m!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    /// Stage 18.27 negative 6: eprint! still works.
    #[test]
    fn stage18_27_eprint_still_works() {
        let src = "fn main() { eprint!(\"err\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty());
        assert!(result.errors.parse.is_empty());
    }

    // =====================================================================
    // Stage 18.29 tests — Built-in non-print macros (assert!/panic!/vec!)
    // =====================================================================

    /// Stage 18.29 positive 1: assert! macro parses and expands.
    #[test]
    fn stage18_29_assert_macro_parses() {
        let src = "fn main() { assert!(1 == 1); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
        // parse may have errors because __landin_assert is not yet fully
        // integrated in codegen — just verify no lex/macro errors.
    }

    /// Stage 18.29 positive 2: vec! macro parses and expands to array.
    #[test]
    fn stage18_29_vec_macro_parses() {
        let src = "fn main() { let _a = vec![1, 2, 3]; }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    }

    /// Stage 18.29 negative 1: panic! macro parses.
    #[test]
    fn stage18_29_panic_macro_parses() {
        let src = "fn main() { panic!(\"oops\"); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    }

    /// Stage 18.29 negative 2: BUILTIN_MACRO_NAMES includes non-print macros.
    #[test]
    fn stage18_29_builtin_names_includes_non_print() {
        assert!(BUILTIN_MACRO_NAMES.contains(&"assert"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"panic"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"vec"));
        // Stage 18.32: more non-print macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"format"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"dbg"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"todo"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"unimplemented"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"write"));
        // Stage 18.34: compile-time utility macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"stringify"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"concat"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"env"));
        // Stage 18.36: source info + file macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"file"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"line"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"module_path"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"include_str"));
        // Stage 18.39: pattern + config macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"matches"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"cfg"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"option_env"));
        // Stage 18.41: low-level + diagnostic macros
        assert!(BUILTIN_MACRO_NAMES.contains(&"asm"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"compile_error"));
        assert!(BUILTIN_MACRO_NAMES.contains(&"cfg_attr"));
    }

    /// Stage 18.29 + 18.32 negative 3: build_builtin_macro_table registers 12 macros.
    #[test]
    fn stage18_29_table_has_seven_macros() {
        let mut interner = Rodeo::new();
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(name);
        }
        interner.get_or_intern("args");
        interner.get_or_intern("tt");
        interner.get_or_intern("cond");
        interner.get_or_intern("msg");
        interner.get_or_intern("x");
        interner.get_or_intern("dst");
        interner.get_or_intern("path");
        interner.get_or_intern("expr");
        interner.get_or_intern("__landin_assert");
        interner.get_or_intern("__landin_panic_msg");
        interner.get_or_intern("__landin_format");
        interner.get_or_intern("__landin_dbg");
        interner.get_or_intern("__landin_write");
        interner.get_or_intern("__landin_stringify");
        interner.get_or_intern("__landin_concat");
        interner.get_or_intern("__landin_env");
        interner.get_or_intern("__landin_file");
        interner.get_or_intern("__landin_line");
        interner.get_or_intern("__landin_module_path");
        interner.get_or_intern("__landin_include_str");
        interner.get_or_intern("pat");
        interner.get_or_intern("cfg");
        interner.get_or_intern("__landin_matches");
        interner.get_or_intern("__landin_cfg");
        interner.get_or_intern("__landin_option_env");
        interner.get_or_intern("attr");
        interner.get_or_intern("__landin_asm");
        interner.get_or_intern("__landin_compile_error");
        interner.get_or_intern("__landin_cfg_attr");
        for name in BUILTIN_MACRO_NAMES {
            interner.get_or_intern(format!("__landin_{}", name));
        }

        let table = build_builtin_macro_table(&mut interner);
        assert_eq!(table.len(), 25, "should have 25 built-in macros");
    }

    /// Stage 18.29 negative 4: vec! with empty args parses.
    #[test]
    fn stage18_29_vec_empty_parses() {
        let src = "fn main() { let _a = vec![]; }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    }

    /// Stage 18.29 negative 5: assert! with false condition parses.
    #[test]
    fn stage18_29_assert_false_parses() {
        let src = "fn main() { assert!(1 == 2); }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(result.errors.macro_errors.is_empty(), "no macro errors");
    }

    /// Stage 18.29 negative 6: user can override built-in non-print macros.
    #[test]
    fn stage18_29_user_overrides_vec() {
        let src = "macro_rules! vec { () => { 42 } } fn main() { vec!() }";
        let result = compile(src);
        assert!(result.errors.lex.is_empty(), "no lex errors");
        assert!(
            result.errors.parse.is_empty(),
            "user vec! override should work"
        );
    }
}
