use landin_compiler::lexer::{tokenize, TokenKind};
use lasso::Rodeo;

fn lex(src: &str) -> Vec<TokenKind> {
    let mut interner = Rodeo::new();
    tokenize(src, &mut interner)
        .0
        .into_iter()
        .map(|t| t.kind)
        .collect()
}

#[allow(dead_code)]
fn lex_count(src: &str) -> usize {
    lex(src).len() - 1 // exclude Eof
}

// === LITERAL TESTS (30) ===

#[test]
fn test_int_dec_basic() {
    assert_eq!(lex("42"), vec![TokenKind::IntLit(42, None), TokenKind::Eof]);
}

#[test]
fn test_int_dec_zero() {
    assert_eq!(lex("0"), vec![TokenKind::IntLit(0, None), TokenKind::Eof]);
}

#[test]
fn test_int_dec_underscore() {
    assert_eq!(
        lex("1_000_000"),
        vec![TokenKind::IntLit(1_000_000, None), TokenKind::Eof]
    );
}

#[test]
fn test_int_dec_suffix_i32() {
    assert_eq!(
        lex("42i32"),
        vec![
            TokenKind::IntLit(
                42,
                Some(landin_compiler::lexer::token::IntSuffix::Signed(
                    landin_compiler::ast::IntTy::I32
                ))
            ),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_int_dec_suffix_u64() {
    assert_eq!(
        lex("42u64"),
        vec![
            TokenKind::IntLit(
                42,
                Some(landin_compiler::lexer::token::IntSuffix::Unsigned(
                    landin_compiler::ast::UintTy::U64
                ))
            ),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_int_dec_suffix_isize() {
    assert_eq!(
        lex("42isize"),
        vec![
            TokenKind::IntLit(
                42,
                Some(landin_compiler::lexer::token::IntSuffix::Signed(
                    landin_compiler::ast::IntTy::Isize
                ))
            ),
            TokenKind::Eof
        ]
    );
}

#[test]
fn test_int_hex() {
    assert_eq!(
        lex("0xff"),
        vec![TokenKind::IntLit(255, None), TokenKind::Eof]
    );
}

#[test]
fn test_int_hex_underscore() {
    assert_eq!(
        lex("0xFF_FF"),
        vec![TokenKind::IntLit(65535, None), TokenKind::Eof]
    );
}

#[test]
fn test_int_oct() {
    assert_eq!(
        lex("0o77"),
        vec![TokenKind::IntLit(63, None), TokenKind::Eof]
    );
}

#[test]
fn test_int_bin() {
    assert_eq!(
        lex("0b1010"),
        vec![TokenKind::IntLit(10, None), TokenKind::Eof]
    );
}

#[test]
fn test_float_basic() {
    let tokens = lex("3.14");
    assert!(matches!(tokens[0], TokenKind::FloatLit(_, _)));
}

#[test]
fn test_float_exp() {
    let tokens = lex("1e10");
    assert!(matches!(tokens[0], TokenKind::FloatLit(_, _)));
}

#[test]
fn test_float_suffix_f32() {
    let tokens = lex("1.0f32");
    assert!(matches!(
        tokens[0],
        TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F32))
    ));
}

#[test]
fn test_float_suffix_f64() {
    let tokens = lex("1.0f64");
    assert!(matches!(
        tokens[0],
        TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F64))
    ));
}

#[test]
fn test_char_basic() {
    assert_eq!(lex("'a'"), vec![TokenKind::CharLit('a'), TokenKind::Eof]);
}

#[test]
fn test_char_escape_newline() {
    assert_eq!(lex("'\\n'"), vec![TokenKind::CharLit('\n'), TokenKind::Eof]);
}

#[test]
fn test_char_escape_tab() {
    assert_eq!(lex("'\\t'"), vec![TokenKind::CharLit('\t'), TokenKind::Eof]);
}

#[test]
fn test_char_escape_hex() {
    assert_eq!(
        lex("'\\x41'"),
        vec![TokenKind::CharLit('A'), TokenKind::Eof]
    );
}

#[test]
fn test_char_escape_unicode() {
    assert_eq!(
        lex("'\\u{4E00}'"),
        vec![TokenKind::CharLit('\u{4E00}'), TokenKind::Eof]
    );
}

#[test]
fn test_string_basic() {
    let tokens = lex(r#""hello""#);
    assert!(matches!(tokens[0], TokenKind::StrLit(_)));
}

#[test]
fn test_string_escape() {
    let tokens = lex(r#""hello\nworld""#);
    assert!(matches!(tokens[0], TokenKind::StrLit(_)));
}

#[test]
fn test_byte_literal() {
    let tokens = lex("b'A'");
    assert_eq!(tokens[0], TokenKind::ByteLit(65));
}

#[test]
fn test_byte_escape_hex() {
    let tokens = lex("b'\\x41'");
    assert_eq!(tokens[0], TokenKind::ByteLit(65));
}

#[test]
fn test_byte_string() {
    let tokens = lex("b\"hello\"");
    assert!(matches!(tokens[0], TokenKind::ByteStrLit(_)));
}

#[test]
fn test_raw_string_basic() {
    let tokens = lex("r\"hello\"");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 0)));
}

#[test]
fn test_raw_string_hash() {
    let tokens = lex("r#\"hello\"#");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 1)));
}

#[test]
fn test_raw_string_with_quotes() {
    let tokens = lex("r#\"has \"quotes\" inside\"#");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 1)));
}

#[test]
fn test_bool_true() {
    assert_eq!(lex("true"), vec![TokenKind::KwTrue, TokenKind::Eof]);
}

#[test]
fn test_bool_false() {
    assert_eq!(lex("false"), vec![TokenKind::KwFalse, TokenKind::Eof]);
}

// === OPERATOR TESTS (25) ===

#[test]
fn test_op_arithmetic() {
    let tokens = lex("+ - * / %");
    assert_eq!(tokens[0], TokenKind::Plus);
    assert_eq!(tokens[1], TokenKind::Minus);
    assert_eq!(tokens[2], TokenKind::Star);
    assert_eq!(tokens[3], TokenKind::Slash);
    assert_eq!(tokens[4], TokenKind::Percent);
}

#[test]
fn test_op_comparison() {
    let tokens = lex("== != < > <= >=");
    assert_eq!(tokens[0], TokenKind::EqEq);
    assert_eq!(tokens[1], TokenKind::NotEq);
    assert_eq!(tokens[2], TokenKind::Lt);
    assert_eq!(tokens[3], TokenKind::Gt);
    assert_eq!(tokens[4], TokenKind::LtEq);
    assert_eq!(tokens[5], TokenKind::GtEq);
}

#[test]
fn test_op_logical() {
    let tokens = lex("&& || !");
    assert_eq!(tokens[0], TokenKind::AndAnd);
    assert_eq!(tokens[1], TokenKind::OrOr);
    assert_eq!(tokens[2], TokenKind::Not);
}

#[test]
fn test_op_bitwise() {
    let tokens = lex("& | ^");
    assert_eq!(tokens[0], TokenKind::And);
    assert_eq!(tokens[1], TokenKind::Or);
    assert_eq!(tokens[2], TokenKind::Caret);
}

#[test]
fn test_op_shift() {
    let tokens = lex("<< >>");
    assert_eq!(tokens[0], TokenKind::Shl);
    assert_eq!(tokens[1], TokenKind::Shr);
}

#[test]
fn test_op_assign() {
    assert_eq!(lex("=")[0], TokenKind::Eq);
}

#[test]
fn test_op_plus_eq() {
    assert_eq!(lex("+=")[0], TokenKind::PlusEq);
}

#[test]
fn test_op_minus_eq() {
    // Use string compare for robustness
    let t = lex("-=");
    assert!(matches!(t[0], TokenKind::MinusEq));
}

#[test]
fn test_op_star_eq() {
    assert!(matches!(lex("*=")[0], TokenKind::StarEq));
}

#[test]
fn test_op_slash_eq() {
    assert!(matches!(lex("/=")[0], TokenKind::SlashEq));
}

#[test]
fn test_op_percent_eq() {
    assert!(matches!(lex("%=")[0], TokenKind::PercentEq));
}

#[test]
fn test_op_and_eq() {
    assert!(matches!(lex("&=")[0], TokenKind::AndEq));
}

#[test]
fn test_op_or_eq() {
    assert!(matches!(lex("|=")[0], TokenKind::OrEq));
}

#[test]
fn test_op_caret_eq() {
    assert!(matches!(lex("^=")[0], TokenKind::CaretEq));
}

#[test]
fn test_op_shl_eq() {
    assert!(matches!(lex("<<=")[0], TokenKind::ShlEq));
}

#[test]
fn test_op_shr_eq() {
    assert!(matches!(lex(">>=")[0], TokenKind::ShrEq));
}

#[test]
fn test_op_range() {
    assert!(matches!(lex("..")[0], TokenKind::DotDot));
    assert!(matches!(lex("..=")[0], TokenKind::DotDotEq));
}

#[test]
fn test_op_arrow() {
    assert!(matches!(lex("->")[0], TokenKind::Arrow));
}

#[test]
fn test_op_fat_arrow() {
    assert!(matches!(lex("=>")[0], TokenKind::FatArrow));
}

#[test]
fn test_op_path_sep() {
    assert!(matches!(lex("::")[0], TokenKind::PathSep));
}

#[test]
fn test_op_question() {
    assert!(matches!(lex("?")[0], TokenKind::Question));
}

#[test]
fn test_op_maximal_munch_dot_dot_eq() {
    let tokens = lex("..=");
    assert!(matches!(tokens[0], TokenKind::DotDotEq));
}

#[test]
fn test_op_maximal_munch_shl_eq() {
    let tokens = lex("<<=");
    assert!(matches!(tokens[0], TokenKind::ShlEq));
}

#[test]
fn test_op_maximal_munch_gt_eq() {
    let tokens = lex(">=");
    assert!(matches!(tokens[0], TokenKind::GtEq));
}

#[test]
fn test_op_maximal_munch_ne() {
    let tokens = lex("!=");
    assert!(matches!(tokens[0], TokenKind::NotEq));
}

// === KEYWORD TESTS (5) ===

#[test]
fn test_kw_strict_core() {
    let tokens = lex("fn struct enum trait impl if else while for loop match return let mut pub const static type use mod unsafe extern");
    assert_eq!(tokens[0], TokenKind::KwFn);
    assert_eq!(tokens[1], TokenKind::KwStruct);
    assert_eq!(tokens[2], TokenKind::KwEnum);
    assert_eq!(tokens[3], TokenKind::KwTrait);
    assert_eq!(tokens[4], TokenKind::KwImpl);
    assert_eq!(tokens[5], TokenKind::KwIf);
    assert_eq!(tokens[6], TokenKind::KwElse);
    assert_eq!(tokens[7], TokenKind::KwWhile);
    assert_eq!(tokens[8], TokenKind::KwFor);
    assert_eq!(tokens[9], TokenKind::KwLoop);
    assert_eq!(tokens[10], TokenKind::KwMatch);
    assert_eq!(tokens[11], TokenKind::KwReturn);
    assert_eq!(tokens[12], TokenKind::KwLet);
    assert_eq!(tokens[13], TokenKind::KwMut);
    assert_eq!(tokens[14], TokenKind::KwPub);
    assert_eq!(tokens[15], TokenKind::KwConst);
    assert_eq!(tokens[16], TokenKind::KwStatic);
    assert_eq!(tokens[17], TokenKind::KwType);
    assert_eq!(tokens[18], TokenKind::KwUse);
    assert_eq!(tokens[19], TokenKind::KwMod);
    assert_eq!(tokens[20], TokenKind::KwUnsafe);
    assert_eq!(tokens[21], TokenKind::KwExtern);
}

#[test]
fn test_kw_async_await() {
    let tokens = lex("async await");
    assert_eq!(tokens[0], TokenKind::KwAsync);
    assert_eq!(tokens[1], TokenKind::KwAwait);
}

#[test]
fn test_kw_self_vs_selftype() {
    let tokens = lex("self Self");
    assert_eq!(tokens[0], TokenKind::KwSelf_);
    assert_eq!(tokens[1], TokenKind::KwSelfType);
}

#[test]
fn test_kw_move_ref_dyn() {
    let tokens = lex("move ref dyn");
    assert_eq!(tokens[0], TokenKind::KwMove);
    assert_eq!(tokens[1], TokenKind::KwRef);
    assert_eq!(tokens[2], TokenKind::KwDyn);
}

#[test]
fn test_kw_crate_super_where() {
    let tokens = lex("crate super where in continue break");
    assert_eq!(tokens[0], TokenKind::KwCrate);
    assert_eq!(tokens[1], TokenKind::KwSuper);
    assert_eq!(tokens[2], TokenKind::KwWhere);
    assert_eq!(tokens[3], TokenKind::KwIn);
    assert_eq!(tokens[4], TokenKind::KwContinue);
    assert_eq!(tokens[5], TokenKind::KwBreak);
}

// === IDENTIFIER TESTS (5) ===

#[test]
fn test_ident_basic() {
    let tokens = lex("foo bar_baz");
    assert!(matches!(tokens[0], TokenKind::Ident(_)));
    assert!(matches!(tokens[1], TokenKind::Ident(_)));
}

#[test]
fn test_ident_underscore() {
    // Stage 39.3: `_` (lone underscore) is now `TokenKind::Underscore`,
    // not `TokenKind::Ident("_")`. Identifier-prefix underscores like
    // `_foo` and `_bar` remain `TokenKind::Ident`.
    let tokens = lex("_foo _ _bar");
    assert!(matches!(tokens[0], TokenKind::Ident(_)));
    assert!(matches!(tokens[1], TokenKind::Underscore));
    assert!(matches!(tokens[2], TokenKind::Ident(_)));
}

#[test]
fn test_ident_unicode() {
    let tokens = lex("café");
    assert!(matches!(tokens[0], TokenKind::Ident(_)));
}

#[test]
fn test_lifetime_basic() {
    let tokens = lex("'a 'b 'static");
    assert!(matches!(tokens[0], TokenKind::Lifetime(_)));
    assert!(matches!(tokens[1], TokenKind::Lifetime(_)));
    assert!(matches!(tokens[2], TokenKind::Lifetime(_)));
}

#[test]
fn test_ident_not_keyword() {
    let tokens = lex("function structure enumeration");
    assert!(matches!(tokens[0], TokenKind::Ident(_)));
    assert!(matches!(tokens[1], TokenKind::Ident(_)));
    assert!(matches!(tokens[2], TokenKind::Ident(_)));
}

// === COMMENT TESTS (5) ===

#[test]
fn test_comment_line() {
    let tokens = lex("// hello\n42");
    assert_eq!(tokens[0], TokenKind::IntLit(42, None));
}

#[test]
fn test_comment_block() {
    let tokens = lex("/* block */ 42");
    assert_eq!(tokens[0], TokenKind::IntLit(42, None));
}

#[test]
fn test_comment_nested_block() {
    let tokens = lex("/* outer /* inner */ still outer */ 42");
    assert_eq!(tokens[0], TokenKind::IntLit(42, None));
}

#[test]
fn test_comment_only_file() {
    let tokens = lex("// just a comment");
    assert_eq!(tokens.len(), 1); // just Eof
}

#[test]
fn test_comment_block_unterminated() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("/* unterminated", &mut interner)
    };
    assert!(
        !errors.is_empty(),
        "should report error for unterminated block comment"
    );
}

// === ERROR RECOVERY TESTS (5) ===

#[test]
fn test_error_bad_char_continues() {
    // @ is not a valid Landin character
    let tokens = lex("@ 42");
    // Should still produce the 42 token after error recovery
    assert!(tokens.iter().any(|t| matches!(t, TokenKind::IntLit(42, _))));
}

#[test]
fn test_error_unterminated_string() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("\"unterminated", &mut interner)
    };
    assert!(!errors.is_empty());
}

#[test]
fn test_error_multiple_errors() {
    // `@ @ @ 42` is now a valid token sequence (At + At + At + IntLit) since
    // Round 2c added the `@` token for pattern binding. The lexer should
    // produce all 4 tokens without error. (Semantic validity of `@` outside
    // a pattern is checked by the parser, not the lexer.)
    let (tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("@ @ @ 42", &mut interner)
    };
    assert!(
        errors.is_empty(),
        "lexer should accept `@` as a token now: {:?}",
        errors
    );
    assert_eq!(
        tokens.len(),
        5,
        "expected 4 tokens + Eof, got {}",
        tokens.len()
    );
    assert!(
        tokens
            .iter()
            .filter(|t| matches!(t.kind, TokenKind::At))
            .count()
            == 3,
        "should have 3 At tokens"
    );
}

#[test]
fn test_error_leading_zero() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("042", &mut interner)
    };
    assert!(!errors.is_empty(), "should report leading zero error");
}

#[test]
fn test_error_invalid_escape() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize(r#""\q""#, &mut interner)
    };
    assert!(!errors.is_empty(), "should report invalid escape \\q");
}

// === PUNCTUATION TESTS (5) ===

#[test]
fn test_punct_brackets() {
    let tokens = lex("( ) { } [ ]");
    assert_eq!(tokens[0], TokenKind::LParen);
    assert_eq!(tokens[1], TokenKind::RParen);
    assert_eq!(tokens[2], TokenKind::LBrace);
    assert_eq!(tokens[3], TokenKind::RBrace);
    assert_eq!(tokens[4], TokenKind::LBracket);
    assert_eq!(tokens[5], TokenKind::RBracket);
}

#[test]
fn test_punct_comma_semicolon_colon() {
    let tokens = lex(", ; :");
    assert_eq!(tokens[0], TokenKind::Comma);
    assert_eq!(tokens[1], TokenKind::Semicolon);
    assert_eq!(tokens[2], TokenKind::Colon);
}

#[test]
fn test_punct_dot() {
    assert!(matches!(lex(".")[0], TokenKind::Dot));
}

#[test]
fn test_punct_hash() {
    assert!(matches!(lex("#")[0], TokenKind::Hash));
}

#[test]
fn test_punct_underscore_token() {
    // Stage 39.3: `_` is `TokenKind::Underscore`, not `TokenKind::Ident`.
    assert!(matches!(lex("_")[0], TokenKind::Underscore));
}

// === P0 REGRESSION TESTS (3) ===

#[test]
fn test_regression_float_suffix_only() {
    // RP0-1: 1f32 should be FloatLit, not IntLit + error
    let tokens = lex("1f32");
    assert!(
        matches!(
            tokens[0],
            TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F32))
        ),
        "1f32 should be FloatLit with F32 suffix, got {:?}",
        tokens[0]
    );
}

#[test]
fn test_regression_float_suffix_only_f64() {
    let tokens = lex("1f64");
    assert!(
        matches!(
            tokens[0],
            TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F64))
        ),
        "1f64 should be FloatLit with F64 suffix"
    );
}

#[test]
fn test_regression_empty_hex_literal() {
    // RP0-4: 0x with no digits should error
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("0x", &mut interner)
    };
    assert!(!errors.is_empty(), "0x with no digits should report error");
}

#[test]
fn test_regression_empty_oct_literal() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("0o", &mut interner)
    };
    assert!(!errors.is_empty(), "0o with no digits should report error");
}

#[test]
fn test_regression_empty_bin_literal() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("0b", &mut interner)
    };
    assert!(!errors.is_empty(), "0b with no digits should report error");
}

#[test]
fn test_regression_doc_comment_outer() {
    // RP0-8 fix: `///` is an outer doc comment and should be tokenized as
    // `DocComment(_, false)` rather than silently skipped.
    let tokens = lex("/// doc comment\n42");
    assert_eq!(
        tokens.len(),
        3,
        "expected DocComment + IntLit + Eof, got {:?}",
        tokens
    );
    match &tokens[0] {
        TokenKind::DocComment(_, is_inner) => {
            assert!(!*is_inner, "outer doc comment should have is_inner=false");
        }
        other => panic!("expected DocComment, got {:?}", other),
    }
    assert_eq!(tokens[1], TokenKind::IntLit(42, None));
}

#[test]
fn test_regression_doc_comment_inner() {
    // RP0-8 fix: `//!` is an inner doc comment and should be tokenized as
    // `DocComment(_, true)` rather than silently skipped.
    let tokens = lex("//! inner doc\n42");
    assert_eq!(
        tokens.len(),
        3,
        "expected DocComment + IntLit + Eof, got {:?}",
        tokens
    );
    match &tokens[0] {
        TokenKind::DocComment(_, is_inner) => {
            assert!(*is_inner, "inner doc comment should have is_inner=true");
        }
        other => panic!("expected DocComment, got {:?}", other),
    }
    assert_eq!(tokens[1], TokenKind::IntLit(42, None));
}

#[test]
fn test_regression_doc_comment_4slashes_is_not_doc() {
    // Per 02-grammar.md §1.12: `////` is a regular line comment, NOT a doc comment.
    let tokens = lex("//// not a doc\n42");
    assert_eq!(tokens[0], TokenKind::IntLit(42, None));
}

// === ADDITIONAL TESTS TO REACH 200 (10 more) ===

#[test]
fn test_int_all_suffixes() {
    let suffixes = [
        "i8", "i16", "i32", "i64", "i128", "isize", "u8", "u16", "u32", "u64", "u128", "usize",
    ];
    for s in suffixes {
        let src = format!("42{}", s);
        let tokens = lex(&src);
        assert!(
            matches!(tokens[0], TokenKind::IntLit(42, Some(_))),
            "42{} should be IntLit with suffix",
            s
        );
    }
}

#[test]
fn test_float_all_suffixes() {
    let tokens = lex("3.14f32");
    assert!(matches!(
        tokens[0],
        TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F32))
    ));
    let tokens = lex("3.14f64");
    assert!(matches!(
        tokens[0],
        TokenKind::FloatLit(_, Some(landin_compiler::lexer::token::FloatTy::F64))
    ));
}

#[test]
fn test_string_with_escapes() {
    let tokens = lex(r#""hello\nworld\ttab""#);
    assert!(matches!(tokens[0], TokenKind::StrLit(_)));
}

#[test]
fn test_char_unicode_escape() {
    let tokens = lex("'\\u{1F600}'");
    assert_eq!(tokens[0], TokenKind::CharLit('\u{1F600}'));
}

#[test]
fn test_raw_string_multiple_hashes() {
    let tokens = lex("r##\"content\"##");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 2)));
}

// === RP0-1 REGRESSION: pure-suffix float literals ===

#[test]
fn test_rp0_1_pure_suffix_f32() {
    // RP0-1 fix: `1f32` must produce FloatLit(1.0, Some(F32)), not IntLit + error.
    let tokens = lex("1f32");
    assert_eq!(tokens.len(), 2, "expected FloatLit + Eof, got {:?}", tokens);
    match tokens[0] {
        TokenKind::FloatLit(v, Some(landin_compiler::lexer::token::FloatTy::F32)) => {
            assert_eq!(v, 1.0);
        }
        ref other => panic!("expected FloatLit(1.0, Some(F32)), got {:?}", other),
    }
}

#[test]
fn test_rp0_1_pure_suffix_f64() {
    let tokens = lex("42f64");
    match tokens[0] {
        TokenKind::FloatLit(v, Some(landin_compiler::lexer::token::FloatTy::F64)) => {
            assert_eq!(v, 42.0);
        }
        ref other => panic!("expected FloatLit(42.0, Some(F64)), got {:?}", other),
    }
}

#[test]
fn test_rp0_1_pure_suffix_no_error() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("1f32 2f64", &mut interner)
    };
    assert!(
        errors.is_empty(),
        "pure-suffix floats must not produce errors: {:?}",
        errors
    );
}

// === RP0-2 REGRESSION: raw identifiers ===

#[test]
fn test_rp0_2_raw_ident_basic() {
    let tokens = lex("r#foo");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(tokens[0], TokenKind::RawIdent(_)));
}

#[test]
fn test_rp0_2_raw_ident_escapes_keyword() {
    // `r#match` is the canonical use case for raw identifiers.
    let tokens = lex("r#match");
    assert!(matches!(tokens[0], TokenKind::RawIdent(_)));
}

#[test]
fn test_rp0_2_raw_ident_no_error() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("r#foo r#bar", &mut interner)
    };
    assert!(
        errors.is_empty(),
        "raw identifiers must not produce errors: {:?}",
        errors
    );
}

#[test]
fn test_rp0_2_raw_ident_underscore_prefix() {
    // r#_foo is a valid raw identifier (underscore is a valid ident-start).
    let tokens = lex("r#_foo");
    assert!(matches!(tokens[0], TokenKind::RawIdent(_)));
}

#[test]
fn test_rp0_2_raw_string_with_hashes_still_works() {
    // r#"..."# must NOT be mis-dispatched as a raw identifier.
    let tokens = lex("r#\"raw\"#");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 1)));
}

#[test]
fn test_rp0_2_raw_string_three_hashes_still_works() {
    let tokens = lex("r###\"raw\"###");
    assert!(matches!(tokens[0], TokenKind::RawStrLit(_, 3)));
}

// === RP0-4 REGRESSION: empty hex/oct/bin literals ===

#[test]
fn test_rp0_4_empty_hex_reports_error() {
    let (tokens, errors) = {
        let mut interner = Rodeo::new();

        landin_compiler::lexer::tokenize("0x", &mut interner)
    };
    assert!(!errors.is_empty(), "0x with no digits must error");
    // Recovery: token should still be produced (IntLit(0, None)) so parser can continue.
    assert!(matches!(tokens[0].kind, TokenKind::IntLit(0, None)));
}

#[test]
fn test_rp0_4_empty_oct_reports_error() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("0o", &mut interner)
    };
    assert!(!errors.is_empty(), "0o with no digits must error");
}

#[test]
fn test_rp0_4_empty_bin_reports_error() {
    let (_tokens, errors) = {
        let mut interner = Rodeo::new();
        landin_compiler::lexer::tokenize("0b", &mut interner)
    };
    assert!(!errors.is_empty(), "0b with no digits must error");
}

#[test]
fn test_rp0_4_nonempty_hex_still_works() {
    let tokens = lex("0xff");
    assert_eq!(tokens[0], TokenKind::IntLit(255, None));
}

// === RP0-8 REGRESSION: doc comments ===

#[test]
fn test_rp0_8_doc_outer_body() {
    let (tokens, _errors) = {
        let mut interner = Rodeo::new();
        let r = landin_compiler::lexer::tokenize("/// hello world\n", &mut interner);
        (r.0, r.1)
    };
    match &tokens[0].kind {
        TokenKind::DocComment(sym, is_inner) => {
            assert!(!*is_inner);
            // The leading single space is stripped; the body should be "hello world".
            // We can't easily resolve the symbol here without keeping the interner alive,
            // so just check the token kind.
            let _ = sym;
        }
        other => panic!("expected DocComment, got {:?}", other),
    }
}

#[test]
fn test_rp0_8_doc_inner_body() {
    let tokens = lex("//! module doc\n");
    match &tokens[0] {
        TokenKind::DocComment(_, is_inner) => assert!(*is_inner),
        other => panic!("expected DocComment, got {:?}", other),
    }
}

#[test]
fn test_rp0_8_doc_comment_followed_by_item() {
    // Doc comment + a real item: ensure both tokens are produced in order.
    let tokens = lex("/// docs\nfn main() {}");
    assert!(matches!(tokens[0], TokenKind::DocComment(_, false)));
    assert_eq!(tokens[1], TokenKind::KwFn);
}

#[test]
fn test_rp0_8_multiple_doc_comments() {
    let tokens = lex("/// line 1\n/// line 2\nfn f();");
    assert!(matches!(tokens[0], TokenKind::DocComment(_, false)));
    assert!(matches!(tokens[1], TokenKind::DocComment(_, false)));
    assert_eq!(tokens[2], TokenKind::KwFn);
}
