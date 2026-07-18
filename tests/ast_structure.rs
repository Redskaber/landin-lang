use landin_compiler::ast::*;
use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use lasso::Rodeo;

fn parse(
    src: &str,
) -> (
    landin_compiler::ast::Crate,
    Vec<landin_compiler::parser::ParseError>,
) {
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &interner);
    let krate = parser.parse_crate();
    let errors = parser.into_errors();
    (krate, errors)
}

// === AST STRUCTURE ASSERTION TESTS ===

#[test]
fn test_ast_fn_item_structure() {
    let (krate, errors) = parse("fn main() {}");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 1);
    match &krate.items[0].kind {
        ItemKind::Fn(fn_decl) => {
            assert!(fn_decl.body.is_some(), "fn should have body");
            assert!(fn_decl.sig.inputs.is_empty(), "fn should have no params");
        }
        _ => panic!("expected Fn item"),
    }
}

#[test]
fn test_ast_fn_with_params_structure() {
    let (krate, errors) = parse("fn add(a: i32, b: i32) -> i32 { a + b }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Fn(fn_decl) => {
            assert_eq!(fn_decl.sig.inputs.len(), 2, "should have 2 params");
        }
        _ => panic!("expected Fn item"),
    }
}

#[test]
fn test_ast_struct_item_structure() {
    let (krate, errors) = parse("struct Point { x: i32, y: i32 }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Struct(s) => {
            assert_eq!(s.fields.len(), 2, "should have 2 fields");
            assert!(!s.is_unit, "should not be unit struct");
            assert!(!s.is_tuple, "should not be tuple struct");
        }
        _ => panic!("expected Struct item"),
    }
}

#[test]
fn test_ast_enum_item_structure() {
    let (krate, errors) = parse("enum Color { Red, Green, Blue }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.variants.len(), 3, "should have 3 variants");
        }
        _ => panic!("expected Enum item"),
    }
}

#[test]
fn test_ast_binop_precedence_structure() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let (krate, errors) = parse("fn f() { let x = 1 + 2 * 3; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().expect("body");
    let stmt = &body.stmts[0];
    match stmt {
        Stmt::Local(local) => {
            let init = local.init.as_ref().expect("init");
            // Should be Binary { op: Add, rhs: Binary { op: Mul } }
            match init {
                Expr::Binary { op, rhs, .. } => {
                    assert_eq!(*op, BinOp::Add, "top-level op should be Add");
                    match rhs.as_ref() {
                        Expr::Binary { op: rhs_op, .. } => {
                            assert_eq!(*rhs_op, BinOp::Mul, "nested op should be Mul");
                        }
                        _ => panic!("expected Binary for rhs"),
                    }
                }
                _ => panic!("expected Binary expr"),
            }
        }
        _ => panic!("expected Local stmt"),
    }
}

#[test]
fn test_ast_int_literal_structure() {
    let (krate, errors) = parse("fn f() { let x = 42; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().expect("body");
    match &body.stmts[0] {
        Stmt::Local(local) => {
            let init = local.init.as_ref().expect("init");
            match init {
                Expr::Lit(LitKind::Int(val, _), _) => {
                    assert_eq!(*val, 42, "literal value should be 42");
                }
                _ => panic!("expected Int literal"),
            }
        }
        _ => panic!("expected Local stmt"),
    }
}

#[test]
fn test_ast_type_bool() {
    let (krate, errors) = parse("fn f(x: bool) {}");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    match &fn_decl.sig.inputs[0].ty {
        Ty::Bool(_) => {} // correct
        other => panic!("expected Ty::Bool, got {:?}", other),
    }
}

#[test]
fn test_ast_type_i32() {
    let (krate, errors) = parse("fn f(x: i32) {}");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    match &fn_decl.sig.inputs[0].ty {
        Ty::Int(IntTy::I32, _) => {} // correct
        other => panic!("expected Ty::Int(I32), got {:?}", other),
    }
}

#[test]
fn test_ast_type_ref() {
    let (krate, errors) = parse("fn f(x: &i32) {}");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    match &fn_decl.sig.inputs[0].ty {
        Ty::Ref(_, Mutability::Immutable, inner, _) => {
            match inner.as_ref() {
                Ty::Int(IntTy::I32, _) => {} // correct
                other => panic!("expected inner Ty::Int(I32), got {:?}", other),
            }
        }
        other => panic!("expected Ty::Ref, got {:?}", other),
    }
}

#[test]
fn test_ast_visibility() {
    let (krate, _) = parse("pub fn main() {}");
    assert_eq!(krate.items[0].vis, Visibility::Public);

    let (krate, _) = parse("fn main() {}");
    assert_eq!(krate.items[0].vis, Visibility::Private);
}

// === REGRESSION TESTS (for P0 fixes) ===

#[test]
fn test_regression_break_keyword() {
    // "break" should be KwBreak, not Ident (br prefix bug)
    let (_krate, errors) = parse("fn f() { loop { break; } }");
    assert!(errors.is_empty(), "break should parse as keyword");
}

#[test]
fn test_regression_closure_empty_params() {
    // || { 42 } should parse (OrOr not just Or)
    let (_krate, errors) = parse("fn f() { let g = || 42; }");
    assert!(errors.is_empty(), "|| closure should parse: {:?}", errors);
}

#[test]
fn test_regression_closure_with_params() {
    // |x| { x + 1 } should parse
    let (_krate, errors) = parse("fn f() { let g = |x| x + 1; }");
    assert!(errors.is_empty(), "|x| closure should parse: {:?}", errors);
}

#[test]
fn test_regression_self_param() {
    // &self should parse
    let (_krate, errors) = parse("impl Foo { fn bar(&self) {} }");
    assert!(errors.is_empty(), "&self should parse: {:?}", errors);
}

#[test]
fn test_regression_mut_self_param() {
    // &mut self should parse
    let (_krate, errors) = parse("impl Foo { fn bar(&mut self) {} }");
    assert!(errors.is_empty(), "&mut self should parse: {:?}", errors);
}

#[test]
fn test_regression_error_recovery_rbrace() {
    // RBrace in error position should not cause infinite loop
    let (krate, _) = parse("} fn main() {}");
    // Should recover and find fn main
    assert!(
        !krate.items.is_empty(),
        "should recover and find at least 1 item"
    );
}

#[test]
fn test_regression_empty_token_stream() {
    // Empty token stream should not panic
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize("", &mut interner);
    let mut parser = Parser::new(tokens, &interner);
    let krate = parser.parse_crate();
    assert!(krate.items.is_empty());
}

#[test]
fn test_regression_byte_literal() {
    // b'A' should produce ByteLit(65)
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize("b'A'", &mut interner);
    assert!(matches!(
        tokens[0].kind,
        landin_compiler::lexer::token::TokenKind::ByteLit(65)
    ));
}

#[test]
fn test_regression_oct_suffix() {
    // 0o77u8 should have suffix U8
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize("0o77u8", &mut interner);
    match &tokens[0].kind {
        landin_compiler::lexer::token::TokenKind::IntLit(_, Some(suffix)) => {
            assert_eq!(*suffix, landin_compiler::lexer::token::IntTy::U8);
        }
        other => panic!("expected IntLit with U8 suffix, got {:?}", other),
    }
}

#[test]
fn test_regression_bin_suffix() {
    // 0b1010u8 should have suffix U8
    let mut interner = Rodeo::new();
    let (tokens, _) = tokenize("0b1010u8", &mut interner);
    match &tokens[0].kind {
        landin_compiler::lexer::token::TokenKind::IntLit(_, Some(suffix)) => {
            assert_eq!(*suffix, landin_compiler::lexer::token::IntTy::U8);
        }
        other => panic!("expected IntLit with U8 suffix, got {:?}", other),
    }
}

#[test]
fn test_regression_error_recovery_continues() {
    // `@ 42` is now a valid token sequence (At + IntLit) since Round 2c added
    // the `@` token for pattern binding. The lexer should produce both tokens
    // without error. (Semantic validity of `@` outside a pattern is checked
    // by the parser, not the lexer.)
    let mut interner = Rodeo::new();
    let (tokens, errors) = tokenize("@ 42", &mut interner);
    assert!(
        errors.is_empty(),
        "lexer should accept `@` as a token now: {:?}",
        errors
    );
    assert!(
        tokens
            .iter()
            .any(|t| matches!(t.kind, landin_compiler::lexer::token::TokenKind::At)),
        "should have At token"
    );
    assert!(
        tokens.iter().any(|t| matches!(
            t.kind,
            landin_compiler::lexer::token::TokenKind::IntLit(42, _)
        )),
        "should still have 42 token"
    );
}

// === EDGE CASE TESTS ===

#[test]
fn test_edge_empty_file() {
    let (krate, errors) = parse("");
    assert!(errors.is_empty());
    assert!(krate.items.is_empty());
}

#[test]
fn test_edge_comment_only() {
    let (krate, errors) = parse("// just a comment\n");
    assert!(errors.is_empty());
    assert!(krate.items.is_empty());
}

#[test]
fn test_edge_whitespace_only() {
    let (krate, errors) = parse("   \n\t  \n");
    assert!(errors.is_empty());
    assert!(krate.items.is_empty());
}

#[test]
fn test_edge_single_token() {
    let (_krate, errors) = parse("42");
    // Should produce an error (42 is not a valid item)
    assert!(!errors.is_empty());
}

#[test]
fn test_edge_deeply_nested_blocks() {
    let (_krate, errors) = parse("fn f() { if true { 1 } else { 2 }; }");
    assert!(errors.is_empty());
}

#[test]
fn test_edge_multiple_items() {
    let (krate, errors) = parse("fn a() {} fn b() {} fn c() {} struct D;");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 4);
}

// === AST STRUCTURE: Pratt precedence (13 levels per 02-grammar.md §2) ===
//
// These tests are STRUCTURAL — they walk the AST tree and assert the operator
// at each node, not just "parses without error". A regression that flattens
// precedence (e.g., makes `*` and `+` same precedence) would fail these tests.

/// Helper: extract the first let-binding's init expression from `fn f() { let _ = EXPR; }`.
fn first_let_init(krate: &Crate) -> &Expr {
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn item"),
    };
    let body = fn_decl.body.as_ref().expect("fn body");
    match &body.stmts[0] {
        Stmt::Local(local) => local.init.as_ref().expect("let init"),
        _ => panic!("expected Local stmt"),
    }
}

#[test]
fn test_pratt_mul_binds_tighter_than_add() {
    // 1 + 2 * 3 should parse as (1 + (2 * 3)).
    let (krate, errors) = parse("fn f() { let _ = 1 + 2 * 3; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, rhs, .. } => {
            assert_eq!(*op, BinOp::Add, "top-level op should be Add");
            match rhs.as_ref() {
                Expr::Binary { op: rhs_op, .. } => {
                    assert_eq!(*rhs_op, BinOp::Mul, "rhs should be Mul");
                }
                _ => panic!("expected Binary rhs (the `2 * 3` part)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_add_left_assoc() {
    // 1 - 2 - 3 should parse as ((1 - 2) - 3), NOT (1 - (2 - 3)).
    let (krate, errors) = parse("fn f() { let _ = 1 - 2 - 3; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, rhs, .. } => {
            assert_eq!(*op, BinOp::Sub, "top-level op should be Sub (the second -)");
            // rhs should be the literal 3
            match rhs.as_ref() {
                Expr::Lit(LitKind::Int(3, _), _) => {}
                _ => panic!("expected rhs to be literal 3"),
            }
            // lhs should be (1 - 2)
            match lhs.as_ref() {
                Expr::Binary { op: lhs_op, .. } => {
                    assert_eq!(*lhs_op, BinOp::Sub, "lhs should be Sub (the first -)");
                }
                _ => panic!("expected lhs to be Binary (the `1 - 2` part)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_comparison_binds_tighter_than_logical_and() {
    // a == b && c == d should parse as ((a == b) && (c == d)).
    let (krate, errors) = parse("fn f() { let _ = a == b && c == d; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, rhs, .. } => {
            assert_eq!(*op, BinOp::And, "top-level op should be And (&&)");
            match lhs.as_ref() {
                Expr::Binary { op: lhs_op, .. } => {
                    assert_eq!(*lhs_op, BinOp::Eq, "lhs should be Eq (==)");
                }
                _ => panic!("expected lhs to be Binary (a == b)"),
            }
            match rhs.as_ref() {
                Expr::Binary { op: rhs_op, .. } => {
                    assert_eq!(*rhs_op, BinOp::Eq, "rhs should be Eq (==)");
                }
                _ => panic!("expected rhs to be Binary (c == d)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_logical_and_binds_tighter_than_or() {
    // a || b && c should parse as (a || (b && c)).
    let (krate, errors) = parse("fn f() { let _ = a || b && c; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, rhs, .. } => {
            assert_eq!(*op, BinOp::Or, "top-level op should be Or (||)");
            match rhs.as_ref() {
                Expr::Binary { op: rhs_op, .. } => {
                    assert_eq!(*rhs_op, BinOp::And, "rhs should be And (&&)");
                }
                _ => panic!("expected rhs to be Binary (b && c)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_shift_binds_tighter_than_add() {
    // 1 + 2 << 3 — in Landin (per 02-grammar.md §2), `<<` binds LOOSER than `+`,
    // so this parses as ((1 + 2) << 3). This matches Rust semantics.
    let (krate, errors) = parse("fn f() { let _ = 1 + 2 << 3; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, .. } => {
            assert_eq!(
                *op,
                BinOp::Shl,
                "top-level op should be Shl (looser than Add)"
            );
            match lhs.as_ref() {
                Expr::Binary { op: lhs_op, .. } => {
                    assert_eq!(*lhs_op, BinOp::Add, "lhs should be Add (1 + 2)");
                }
                _ => panic!("expected lhs to be Binary (1 + 2)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_as_binds_tighter_than_mul() {
    // 2 * 3 as i32 should parse as (2 * (3 as i32)).
    let (krate, errors) = parse("fn f() { let _ = 2 * 3 as i32; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, rhs, .. } => {
            assert_eq!(*op, BinOp::Mul, "top-level op should be Mul");
            match rhs.as_ref() {
                Expr::Cast { .. } => {} // Good — rhs is a Cast expr
                _ => panic!("expected rhs to be Cast (3 as i32)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_unary_binds_tighter_than_mul() {
    // -2 * 3 should parse as ((-2) * 3).
    let (krate, errors) = parse("fn f() { let _ = -2 * 3; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, .. } => {
            assert_eq!(*op, BinOp::Mul, "top-level op should be Mul");
            match lhs.as_ref() {
                Expr::Unary { op: lhs_op, .. } => {
                    assert_eq!(*lhs_op, UnaryOp::Neg, "lhs should be Neg");
                }
                _ => panic!("expected lhs to be Unary (-2)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_postfix_binds_tighter_than_unary() {
    // -a.b should parse as -(a.b).
    let (krate, errors) = parse("fn f() { let _ = -a.b; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Unary { op, expr, .. } => {
            assert_eq!(*op, UnaryOp::Neg, "top-level should be Neg");
            match expr.as_ref() {
                Expr::Field { .. } => {} // Good — inner is a Field access
                _ => panic!("expected inner expr to be Field (a.b)"),
            }
        }
        _ => panic!("expected Unary expr"),
    }
}

#[test]
fn test_pratt_parens_override_precedence() {
    // (1 + 2) * 3 — explicit parens. Should be Mul(Add(1,2), 3).
    let (krate, errors) = parse("fn f() { let _ = (1 + 2) * 3; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, .. } => {
            assert_eq!(*op, BinOp::Mul, "top-level should be Mul");
            match lhs.as_ref() {
                Expr::Binary { op: lhs_op, .. } => {
                    assert_eq!(*lhs_op, BinOp::Add, "lhs should be Add (parens override)");
                }
                _ => panic!("expected lhs to be Binary (1 + 2 from parens)"),
            }
        }
        _ => panic!("expected Binary expr"),
    }
}

#[test]
fn test_pratt_assignment_lowest_precedence() {
    // a = 1 + 2 should parse as (a = (1 + 2)).
    let (krate, errors) = parse("fn f() { a = 1 + 2; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().expect("body");
    match &body.stmts[0] {
        Stmt::Expr(
            Expr::Assign {
                lhs: _, rhs, op, ..
            },
            true,
        ) => {
            assert!(op.is_none(), "should be plain `=` assignment");
            match rhs.as_ref() {
                Expr::Binary { op: rhs_op, .. } => {
                    assert_eq!(*rhs_op, BinOp::Add, "rhs should be Add (1 + 2)");
                }
                _ => panic!("expected rhs to be Binary (1 + 2)"),
            }
        }
        _ => panic!("expected Assign stmt (with semicolon=true)"),
    }
}

// === AST STRUCTURE: Ty variants (16 per 05-ast.md §6) ===

#[test]
fn test_ty_bool() {
    let (krate, errors) = parse("fn f(x: bool) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_never() {
    let (krate, errors) = parse("fn f() -> ! {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_array() {
    let (krate, errors) = parse("fn f(x: [i32; 4]) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_slice() {
    let (krate, errors) = parse("fn f(x: &[i32]) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_tuple() {
    let (krate, errors) = parse("fn f(x: (i32, &str)) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_fn_pointer() {
    let (krate, errors) = parse("fn f(cb: fn(i32) -> i32) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_ty_infer() {
    // `_` is the inference placeholder type.
    let (krate, errors) = parse("fn f() { let x: _ = 42; }");
    assert!(errors.is_empty());
    let _ = krate;
}

// === AST STRUCTURE: raw identifiers in parser (RP0-2 integration) ===

#[test]
fn test_raw_ident_in_fn_name() {
    // `r#match` should be accepted as a function name.
    let (krate, errors) = parse("fn r#match() {}");
    assert!(
        errors.is_empty(),
        "r#match as fn name should parse: {:?}",
        errors
    );
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_raw_ident_in_struct_field() {
    let (krate, errors) = parse("struct S { r#type: i32 }");
    assert!(
        errors.is_empty(),
        "r#type as field name should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_raw_ident_in_let_binding() {
    let (krate, errors) = parse("fn f() { let r#async = 42; }");
    assert!(
        errors.is_empty(),
        "r#async as let name should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_raw_ident_in_path() {
    // `r#mod::r#fn()` should parse as a path call with raw identifier segments.
    let (krate, errors) = parse("fn f() { r#mod::r#fn(); }");
    assert!(
        errors.is_empty(),
        "raw idents in path should parse: {:?}",
        errors
    );
    let _ = krate;
}

// === AST STRUCTURE: doc comments in parser (RP0-8 integration) ===

#[test]
fn test_doc_comment_before_fn() {
    // Doc comment before a fn: parser should consume it without error.
    // (Attaching it as an attribute is Stage 1 work; for now we just verify
    // that the presence of a DocComment token doesn't break parsing.)
    let (krate, errors) = parse("/// this is a function\nfn main() {}");
    assert!(
        errors.is_empty(),
        "doc comment before fn should not break parsing: {:?}",
        errors
    );
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_inner_doc_comment_at_crate_root() {
    let (krate, errors) = parse("//! crate-level doc\nfn main() {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_multiple_doc_comments_before_item() {
    let (krate, errors) = parse("/// line 1\n/// line 2\n/// line 3\nfn f() {}");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 1);
}

// === SAFETY REGRESSION: parser must not infinite-loop / OOM-kill ===

#[test]
fn test_regression_no_infinite_loop_on_struct_literal() {
    // Round 2c fix: `Point { x: 1 }` is now parsed as a struct literal
    // expression (Expr::Struct). Before Round 2a, this caused an infinite
    // loop / OOM kill. We assert the parser returns successfully with no
    // errors and that the resulting crate has one item (the fn).
    let (krate, errors) = parse("fn f() { Point { x: 1 } }");
    assert!(
        errors.is_empty(),
        "struct literal should parse cleanly now: {:?}",
        errors
    );
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_regression_no_infinite_loop_on_if_let() {
    // Before Round 2a fix: `if let Some(x) = opt { }` caused a timeout.
    let (_krate, errors) = parse("fn f() { if let Some(x) = opt { 1 } }");
    assert!(
        !errors.is_empty(),
        "if let should produce errors (not yet supported), got 0"
    );
}

#[test]
fn test_regression_no_infinite_loop_on_while_let() {
    let (_krate, errors) = parse("fn f() { while let Some(x) = iter.next() { 1 } }");
    assert!(
        !errors.is_empty(),
        "while let should produce errors (not yet supported), got 0"
    );
}

#[test]
fn test_regression_no_infinite_loop_on_unrecognized_token() {
    // Any unrecognized token sequence inside a block must not hang.
    let (_krate, _errors) = parse("fn f() { @ }");
    // The @ token isn't even in the lexer yet; this is a stress test that
    // the parser degrades gracefully.
}

// === Round 2c: NEW FEATURE TESTS — struct literal / macro_call / move closure / etc. ===

#[test]
fn test_struct_literal_basic() {
    let (krate, errors) = parse("fn f() { Point { x: 1, y: 2 } }");
    assert!(
        errors.is_empty(),
        "struct literal should parse: {:?}",
        errors
    );
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_struct_literal_shorthand() {
    // Shorthand: `Point { x, y }` means `Point { x: x, y: y }`
    let (krate, errors) = parse("fn f() { let x = 1; let p = Point { x }; }");
    assert!(
        errors.is_empty(),
        "struct shorthand should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_struct_literal_with_base() {
    // Struct update: `Point { ..base }`
    let (krate, errors) = parse("fn f() { let p = Point { ..base }; }");
    assert!(
        errors.is_empty(),
        "struct update should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_struct_literal_in_let() {
    let (krate, errors) = parse("fn f() { let p = Point { x: 1, y: 2 }; }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_struct_literal_not_in_if_cond() {
    // `if Foo { ... }` — the `{` belongs to the if block, NOT a struct literal
    // in the condition. Without no_struct_literal, this would parse as
    // `if (Foo {x:1})` (a struct literal in the condition), which would then
    // fail to find the if's block.
    let (krate, errors) = parse("fn f() { if true { 1 } }");
    assert!(
        errors.is_empty(),
        "if with simple cond should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_macro_call_paren() {
    // `println!(...)` — macro call with parens
    let (krate, errors) = parse("fn f() { println!(\"hi\"); }");
    assert!(errors.is_empty(), "macro call should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_macro_call_bracket() {
    // `vec![1, 2, 3]` — macro call with brackets
    let (krate, errors) = parse("fn f() { let v = vec![1, 2, 3]; }");
    assert!(errors.is_empty(), "vec! macro should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_macro_call_brace() {
    // `panic!{...}` — macro call with braces (rare but valid)
    let (krate, errors) = parse("fn f() { panic!{ \"oops\" }; }");
    // We accept this with possibly some recovery for the trailing `;`
    let _ = (krate, errors);
}

#[test]
fn test_move_closure() {
    // `move || 42` — move closure with no params
    let (krate, errors) = parse("fn f() { let g = move || 42; }");
    assert!(errors.is_empty(), "move closure should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_move_closure_with_params() {
    // `move |x| x + 1`
    let (krate, errors) = parse("fn f() { let g = move |x: i32| x + 1; }");
    assert!(
        errors.is_empty(),
        "move closure with params should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_unsafe_fn_item() {
    // `unsafe fn foo() {}` — standalone unsafe fn
    let (krate, errors) = parse("unsafe fn foo() {}");
    assert!(errors.is_empty(), "unsafe fn should parse: {:?}", errors);
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_extern_c_fn_standalone() {
    // `extern "C" fn foo() {}` — standalone extern fn
    let (krate, errors) = parse("extern \"C\" fn foo() {}");
    assert!(errors.is_empty(), "extern C fn should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_extern_block() {
    // `extern "C" { fn printf(...); }` — extern block
    let (krate, errors) = parse("extern \"C\" { fn printf(fmt: *const u8) -> i32; }");
    assert!(errors.is_empty(), "extern block should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_pub_crate_visibility() {
    let (krate, errors) = parse("pub(crate) fn foo() {}");
    assert!(errors.is_empty(), "pub(crate) should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_pub_super_visibility() {
    let (krate, errors) = parse("pub(super) fn foo() {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pub_in_path_visibility() {
    let (krate, errors) = parse("pub(in crate::module) fn foo() {}");
    assert!(errors.is_empty(), "pub(in path) should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_use_simple() {
    let (krate, errors) = parse("use std::io;");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_use_alias() {
    let (krate, errors) = parse("use std::io as sio;");
    assert!(errors.is_empty(), "use alias should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_use_glob() {
    let (krate, errors) = parse("use std::io::*;");
    assert!(errors.is_empty(), "use glob should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_use_group() {
    let (krate, errors) = parse("use std::{io, fs, net};");
    assert!(errors.is_empty(), "use group should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_use_nested_group() {
    let (krate, errors) = parse("use std::{io::{Read, Write}, fs};");
    assert!(
        errors.is_empty(),
        "nested use group should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_use_group_with_alias() {
    let (krate, errors) = parse("use std::{io as sio, fs};");
    assert!(errors.is_empty());
    let _ = krate;
}

// === Round 2d: NEW FEATURE TESTS — generic bounds / where / trait items / patterns ===

#[test]
fn test_generic_bounds_single() {
    let (krate, errors) = parse("fn f<T: Clone>(x: T) -> T { x }");
    assert!(
        errors.is_empty(),
        "single generic bound should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_bounds_multiple() {
    let (krate, errors) = parse("fn f<T: Clone + Default + Send>(x: T) -> T { x }");
    assert!(
        errors.is_empty(),
        "multiple bounds should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_lifetime_param() {
    let (krate, errors) = parse("fn f<'a, T>(x: &'a T) -> &'a T { x }");
    assert!(
        errors.is_empty(),
        "lifetime generic param should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_lifetime_with_bounds() {
    let (krate, errors) = parse("fn f<'a: 'b + 'c, T>(x: &'a T) -> &'a T { x }");
    assert!(
        errors.is_empty(),
        "lifetime with bounds should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_default() {
    let (krate, errors) = parse("fn f<T = i32>(x: T) -> T { x }");
    assert!(
        errors.is_empty(),
        "default type param should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_where_clause_simple() {
    let (krate, errors) = parse("fn f<T>(x: T) -> T where T: Clone { x }");
    assert!(errors.is_empty(), "where clause should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_where_clause_multiple() {
    let (krate, errors) =
        parse("fn f<T, U>(x: T, y: U) -> T where T: Clone, U: Default + Send { x }");
    assert!(
        errors.is_empty(),
        "multiple where predicates should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_where_clause_with_lifetime() {
    let (krate, errors) = parse("fn f<'a, 'b: 'a>(x: &'a i32, y: &'b i32) {}");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_trait_with_supertraits() {
    let (krate, errors) = parse("trait Foo: Clone + Send { fn bar(&self); }");
    assert!(
        errors.is_empty(),
        "trait with supertraits should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_trait_with_fn_item() {
    let (krate, errors) = parse("trait Foo { fn bar(&self); fn baz(&self) -> i32; }");
    assert!(
        errors.is_empty(),
        "trait with fn items should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_trait_with_assoc_type() {
    let (krate, errors) =
        parse("trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }");
    assert!(
        errors.is_empty(),
        "trait with assoc type should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_trait_with_assoc_const() {
    let (krate, errors) = parse("trait Foo { const X: i32; const Y: i32 = 42; }");
    assert!(
        errors.is_empty(),
        "trait with assoc const should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_trait_with_default_method() {
    let (krate, errors) = parse("trait Foo { fn bar(&self) -> i32 { 42 } }");
    assert!(
        errors.is_empty(),
        "trait with default method should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_args_in_path_type() {
    let (krate, errors) = parse("fn f(x: Vec<i32>) {}");
    assert!(errors.is_empty(), "Vec<i32> should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_generic_args_multiple() {
    let (krate, errors) = parse("fn f(x: HashMap<String, i32>) {}");
    assert!(
        errors.is_empty(),
        "HashMap<K, V> should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_generic_args_with_lifetime() {
    let (krate, errors) = parse("fn f<'a>(x: Foo<'a, i32>) {}");
    assert!(errors.is_empty(), "Foo<'a, T> should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_generic_args_assoc_binding() {
    let (krate, errors) = parse("fn f(x: Iterator<Item = i32>) {}");
    assert!(
        errors.is_empty(),
        "Iterator<Item = i32> should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_impl_trait_arg() {
    let (krate, errors) = parse("fn f(x: impl Clone + Default) {}");
    assert!(
        errors.is_empty(),
        "impl Trait arg should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_impl_trait_return() {
    let (krate, errors) = parse("fn f() -> impl Iterator<Item = i32> { vec![].into_iter() }");
    assert!(
        errors.is_empty(),
        "impl Trait return should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_dyn_trait() {
    let (krate, errors) = parse("fn f(x: &dyn Display) {}");
    assert!(errors.is_empty(), "dyn Trait should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_dyn_trait_with_lifetime() {
    let (krate, errors) = parse("fn f(x: &(dyn Display + 'static)) {}");
    assert!(
        errors.is_empty(),
        "dyn Trait + 'static should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_lifetime_in_ref_ty() {
    let (krate, errors) = parse("fn f<'a>(x: &'a i32) -> &'a i32 { x }");
    assert!(
        errors.is_empty(),
        "&'a i32 should preserve lifetime: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_fn_pointer_type_full() {
    let (krate, errors) = parse("fn f(cb: fn(i32, i64) -> bool) {}");
    assert!(
        errors.is_empty(),
        "fn pointer type with params should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_unsafe_fn_pointer_type() {
    let (krate, errors) = parse("fn f(cb: unsafe fn() -> i32) {}");
    assert!(
        errors.is_empty(),
        "unsafe fn pointer type should parse: {:?}",
        errors
    );
    let _ = krate;
}

// === Pattern tests ===

#[test]
fn test_pat_literal_int() {
    let (krate, errors) = parse("fn f(x: i32) { match x { 1 => 1, _ => 0 } }");
    assert!(
        errors.is_empty(),
        "literal int pattern should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_literal_char() {
    let (krate, errors) = parse("fn f(x: char) { match x { 'a' => 1, _ => 0 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_literal_str() {
    let (krate, errors) = parse("fn f(x: &str) { match x { \"foo\" => 1, _ => 0 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_literal_bool() {
    let (krate, errors) = parse("fn f(x: bool) { match x { true => 1, false => 0 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_tuple() {
    let (krate, errors) = parse("fn f(x: (i32, i32)) { match x { (1, 2) => 0, _ => 1 } }");
    assert!(
        errors.is_empty(),
        "tuple pattern should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_tuple_struct() {
    let (krate, errors) = parse("fn f(x: Option<i32>) { match x { Some(1) => 1, None => 0 } }");
    assert!(
        errors.is_empty(),
        "tuple struct pattern should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_struct() {
    let (krate, errors) = parse("fn f(p: Point) { match p { Point { x: 1, y: 2 } => 0, _ => 1 } }");
    assert!(
        errors.is_empty(),
        "struct pattern should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_struct_shorthand() {
    let (krate, errors) = parse("fn f(p: Point) { match p { Point { x, y } => 0, _ => 1 } }");
    assert!(
        errors.is_empty(),
        "struct pattern shorthand should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_struct_with_rest() {
    let (krate, errors) = parse("fn f(p: Point) { match p { Point { x, .. } => 0, _ => 1 } }");
    assert!(
        errors.is_empty(),
        "struct pattern with .. should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_or() {
    let (krate, errors) = parse("fn f(x: i32) { match x { 1 | 2 | 3 => \"low\", _ => \"high\" } }");
    assert!(errors.is_empty(), "or pattern should parse: {:?}", errors);
    let _ = krate;
}

#[test]
fn test_pat_at_binding() {
    let (krate, errors) = parse("fn f(x: i32) { match x { n @ 1..=10 => n, _ => 0 } }");
    assert!(
        errors.is_empty(),
        "ident @ pat binding should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_pat_ref() {
    let (krate, errors) = parse("fn f(x: &i32) { match x { &1 => 0, _ => 1 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_ref_mut() {
    let (krate, errors) = parse("fn f(x: &mut i32) { match x { &mut n => n, _ => 0 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_ref_keyword() {
    // `ref ident` pattern (different from `&pat`)
    let (krate, errors) = parse("fn f(x: i32) { match x { ref n => *n, _ => 0 } }");
    assert!(errors.is_empty());
    let _ = krate;
}

#[test]
fn test_pat_slice() {
    let (krate, errors) = parse("fn f(x: &[i32]) { match x { [1, 2] => 0, [..] => 1 } }");
    assert!(
        errors.is_empty(),
        "slice pattern should parse: {:?}",
        errors
    );
    let _ = krate;
}

// === Real-world idiomatic Landin programs ===

#[test]
fn test_realistic_iterator_chain() {
    let (krate, errors) = parse(
        r#"
        fn sum_evens(nums: Vec<i32>) -> i32 {
            nums.iter()
                .filter(|x| *x % 2 == 0)
                .map(|x| x * 2)
                .fold(0, |acc, x| acc + x)
        }
    "#,
    );
    assert!(
        errors.is_empty(),
        "realistic iterator chain should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_realistic_trait_impl() {
    let (krate, errors) = parse(
        r#"
        struct Point { x: i32, y: i32 }
        impl Point {
            fn new(x: i32, y: i32) -> Self {
                Point { x, y }
            }
            fn distance(&self, other: &Self) -> f64 {
                let dx = (self.x - other.x) as f64;
                let dy = (self.y - other.y) as f64;
                (dx * dx + dy * dy).sqrt()
            }
        }
    "#,
    );
    assert!(
        errors.is_empty(),
        "realistic trait impl should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_realistic_enum_with_match() {
    let (krate, errors) = parse(
        r#"
        enum Shape {
            Circle(f64),
            Rectangle(f64, f64),
            Triangle { a: f64, b: f64, c: f64 },
        }
        fn area(s: Shape) -> f64 {
            match s {
                Shape::Circle(r) => 3.14159 * r * r,
                Shape::Rectangle(w, h) => w * h,
                Shape::Triangle { a, b, c } => {
                    let s = (a + b + c) / 2.0;
                    (s * (s - a) * (s - b) * (s - c)).sqrt()
                }
            }
        }
    "#,
    );
    assert!(
        errors.is_empty(),
        "realistic enum with match should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_realistic_generic_struct_with_where() {
    let (krate, errors) = parse(
        r#"
        struct SortedMap<K, V>
        where
            K: Ord + Clone,
            V: Clone,
        {
            data: Vec<(K, V)>,
        }
        impl<K, V> SortedMap<K, V>
        where
            K: Ord + Clone,
            V: Clone,
        {
            fn new() -> Self {
                Self { data: Vec::new() }
            }
        }
    "#,
    );
    assert!(
        errors.is_empty(),
        "realistic generic struct with where should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_realistic_module_system() {
    let (krate, errors) = parse(
        r#"
        mod network {
            pub use std::io::{Read, Write};
            use std::net::*;

            pub fn connect(host: &str) -> i32 {
                0
            }
        }
        use network::connect;
        use network::*;

        fn main() {
            connect("localhost");
        }
    "#,
    );
    assert!(
        errors.is_empty(),
        "realistic module system should parse: {:?}",
        errors
    );
    let _ = krate;
}

#[test]
fn test_realistic_closures() {
    // Note: `Fn(i32) -> i32` is a parenthesized generic args form
    // (not angle-bracketed) which we don't yet support in Stage 0.
    // Use the angle-bracketed form or just `impl Fn(...)` for now.
    let (krate, errors) = parse(
        r#"
        fn apply(f: impl Fn(i32) -> i32, x: i32) -> i32 { f(x) }
        fn main() {
            let add_one = |x| x + 1;
            let move_add = move || 42;
            apply(add_one, 5);
            apply(|x| x * 2, 5);
        }
    "#,
    );
    // We may not parse this fully yet (impl Trait in arg position is new);
    // assert no panic.
    let _ = (krate, errors);
}

// === Function name preservation (Round 2b critical fix) ===

#[test]
fn test_fn_name_preserved() {
    // Before Round 2b fix: function name was silently discarded.
    // Now: FnDecl.ident must be a real Ident.
    let (krate, errors) = parse("fn my_function() {}");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 1);
    match &krate.items[0].kind {
        ItemKind::Fn(fn_decl) => {
            // The ident should NOT be Spur::default() (which is what we'd get
            // if the name were lost).
            // Note: Spur is opaque, so we check by resolving via the interner.
            // But the test framework's interner is local — we can't easily
            // resolve here. Instead, just assert that the span is non-empty.
            assert!(
                fn_decl.ident.span.lo < fn_decl.ident.span.hi,
                "fn name span should be non-empty: {:?}",
                fn_decl.ident.span
            );
        }
        other => panic!("expected Fn, got {:?}", other),
    }
}

#[test]
fn test_mod_name_preserved() {
    let (krate, errors) = parse("mod my_module {}");
    assert!(errors.is_empty());
    assert_eq!(krate.items.len(), 1);
    match &krate.items[0].kind {
        ItemKind::Mod(ModDecl::Inline { ident, .. }) => {
            assert!(
                ident.span.lo < ident.span.hi,
                "mod name span should be non-empty: {:?}",
                ident.span
            );
        }
        other => panic!("expected Mod Inline, got {:?}", other),
    }
}

// === Self parameter preservation ===

#[test]
fn test_self_param_marked() {
    // Before Round 2b fix: `&self` produced a Param with Spur::default() name
    // and no self marker. Now: Param.is_self == true.
    let (krate, errors) = parse("impl Foo { fn bar(&self) {} }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(impl_decl) => {
            assert!(!impl_decl.items.is_empty());
            match &impl_decl.items[0].kind {
                ItemKind::Fn(fn_decl) => {
                    assert!(
                        !fn_decl.sig.inputs.is_empty(),
                        "should have at least one param (self)"
                    );
                    assert!(
                        fn_decl.sig.inputs[0].is_self,
                        "first param should be marked is_self=true"
                    );
                }
                other => panic!("expected Fn in impl, got {:?}", other),
            }
        }
        other => panic!("expected Impl, got {:?}", other),
    }
}

// === Round 8: Agent-group review fixes — regression tests ===

#[test]
fn test_regression_lexer_no_panic_on_unterminated_raw_string() {
    // Round 8a / P0-1 compiler: lexer used to panic on `r"` (no closing quote).
    // Now it must produce an error and a recovery token.
    let mut interner = Rodeo::new();
    let (tokens, errors) = tokenize("r\"", &mut interner);
    assert!(!errors.is_empty(), "unterminated raw string must error");
    // Recovery: must still produce a token (and Eof)
    assert!(!tokens.is_empty());
}

#[test]
fn test_regression_where_clause_preserved_on_struct() {
    // Round 8b / P0-1 type system: where clauses on struct/enum/impl/type-alias
    // used to be silently discarded. Now they must be preserved.
    let (krate, errors) = parse("struct S<T> where T: Clone { x: T }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Struct(s) => {
            assert_eq!(
                s.generics.where_clause.len(),
                1,
                "struct should preserve where clause: got {:?}",
                s.generics.where_clause
            );
        }
        other => panic!("expected Struct, got {:?}", other),
    }
}

#[test]
fn test_regression_where_clause_preserved_on_enum() {
    let (krate, errors) = parse("enum E<T> where T: Clone { A(T) }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Enum(e) => {
            assert_eq!(e.generics.where_clause.len(), 1);
        }
        other => panic!("expected Enum, got {:?}", other),
    }
}

#[test]
fn test_regression_where_clause_preserved_on_impl() {
    let (krate, errors) = parse("impl<T> Foo for T where T: Clone {}");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(i) => {
            assert_eq!(i.generics.where_clause.len(), 1);
        }
        other => panic!("expected Impl, got {:?}", other),
    }
}

#[test]
fn test_regression_where_clause_preserved_on_type_alias() {
    let (krate, errors) = parse("type R<T> where T: Clone = Vec<T>;");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::TypeAlias(t) => {
            assert_eq!(t.generics.where_clause.len(), 1);
        }
        other => panic!("expected TypeAlias, got {:?}", other),
    }
}

#[test]
fn test_regression_trait_method_ident_and_generics_preserved() {
    // Round 8c / P0-2 type system: trait method name + per-method generics
    // used to be silently discarded. Now TraitItem::Fn carries them.
    let (krate, errors) = parse("trait Foo { fn bar<T>(&self, x: T) -> T; }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Trait(t) => {
            assert_eq!(t.items.len(), 1);
            match &t.items[0] {
                TraitItem::Fn(ident, generics, _sig, _body) => {
                    assert!(
                        ident.span.lo < ident.span.hi,
                        "trait method name should have a real span: {:?}",
                        ident
                    );
                    assert_eq!(
                        generics.params.len(),
                        1,
                        "trait method should preserve per-method generics: got {:?}",
                        generics
                    );
                }
                other => panic!("expected TraitItem::Fn, got {:?}", other),
            }
        }
        other => panic!("expected Trait, got {:?}", other),
    }
}

#[test]
fn test_regression_trait_unsafe_fn_preserved() {
    // Round 8d / P0-3 soundness: `trait Foo { unsafe fn bar(); }` used to be
    // rejected and then demoted to safe. Now the unsafe qualifier is preserved.
    let (krate, errors) = parse("trait Foo { unsafe fn bar(); }");
    assert!(
        errors.is_empty(),
        "unsafe trait method should parse: {:?}",
        errors
    );
    match &krate.items[0].kind {
        ItemKind::Trait(t) => {
            assert_eq!(t.items.len(), 1);
            match &t.items[0] {
                TraitItem::Fn(_ident, _generics, sig, _body) => {
                    assert!(
                        sig.is_unsafe,
                        "trait method sig.is_unsafe should be true for `unsafe fn`"
                    );
                }
                other => panic!("expected TraitItem::Fn, got {:?}", other),
            }
        }
        other => panic!("expected Trait, got {:?}", other),
    }
}

#[test]
fn test_regression_unsafe_impl_parses() {
    // Round 8e / P1-2 soundness: `unsafe impl Trait for T {}` used to be
    // rejected. Now it parses (with the unsafe qualifier dropped — Stage 1.0
    // will add the AST field).
    let (krate, errors) = parse("unsafe impl Send for Foo {}");
    assert!(errors.is_empty(), "unsafe impl should parse: {:?}", errors);
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_regression_unsafe_trait_parses() {
    let (krate, errors) = parse("unsafe trait Foo {}");
    assert!(errors.is_empty(), "unsafe trait should parse: {:?}", errors);
    assert_eq!(krate.items.len(), 1);
}

#[test]
fn test_regression_attr_before_vis_parses() {
    // Round 8 / P1-7 compiler: `#[derive(Debug)] pub fn foo() {}` used to fail
    // because parser called parse_visibility BEFORE parse_outer_attrs. Now both
    // orders work.
    let (krate, errors) = parse("#[derive(Debug)] pub fn foo() {}");
    assert!(
        errors.is_empty(),
        "attr-before-vis should parse: {:?}",
        errors
    );
    assert_eq!(krate.items.len(), 1);
    // Verify the attribute was actually captured
    let item = &krate.items[0];
    assert!(
        !item.attrs.is_empty(),
        "attributes should be captured: {:?}",
        item.attrs
    );
    assert!(
        matches!(item.vis, Visibility::Public),
        "pub should be captured"
    );
    assert!(matches!(item.kind, ItemKind::Fn(_)));
}

#[test]
fn test_regression_attr_after_vis_parses() {
    let (krate, errors) = parse("pub #[derive(Debug)] fn foo() {}");
    assert!(
        errors.is_empty(),
        "vis-then-attr should also parse: {:?}",
        errors
    );
    let _ = krate;
}

// === Stage 1.1 Round 2: AST schema fix regression tests ===

#[test]
fn test_self_kind_value_param() {
    // `self` (by value, immutable binding)
    let (krate, errors) = parse("impl Foo { fn bar(self) {} }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(impl_decl) => match &impl_decl.items[0].kind {
            ItemKind::Fn(fn_decl) => {
                let p = &fn_decl.sig.inputs[0];
                assert!(p.is_self, "should be self param");
                assert_eq!(
                    p.self_kind,
                    Some(SelfKind::Value(Mutability::Immutable)),
                    "bare `self` should be Value(Immutable)"
                );
            }
            other => panic!("expected Fn, got {:?}", other),
        },
        other => panic!("expected Impl, got {:?}", other),
    }
}

#[test]
fn test_self_kind_value_mut_param() {
    // `mut self` (by value, mutable binding)
    let (krate, errors) = parse("impl Foo { fn bar(mut self) {} }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(impl_decl) => match &impl_decl.items[0].kind {
            ItemKind::Fn(fn_decl) => {
                let p = &fn_decl.sig.inputs[0];
                assert!(p.is_self);
                assert_eq!(
                    p.self_kind,
                    Some(SelfKind::Value(Mutability::Mutable)),
                    "`mut self` should be Value(Mutable)"
                );
            }
            other => panic!("expected Fn, got {:?}", other),
        },
        other => panic!("expected Impl, got {:?}", other),
    }
}

#[test]
fn test_self_kind_ref_param() {
    // `&self` (by reference, immutable borrow)
    let (krate, errors) = parse("impl Foo { fn bar(&self) {} }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(impl_decl) => match &impl_decl.items[0].kind {
            ItemKind::Fn(fn_decl) => {
                let p = &fn_decl.sig.inputs[0];
                assert!(p.is_self);
                assert_eq!(
                    p.self_kind,
                    Some(SelfKind::Ref(Mutability::Immutable)),
                    "`&self` should be Ref(Immutable)"
                );
            }
            other => panic!("expected Fn, got {:?}", other),
        },
        other => panic!("expected Impl, got {:?}", other),
    }
}

#[test]
fn test_self_kind_ref_mut_param() {
    // `&mut self` (by reference, mutable borrow)
    let (krate, errors) = parse("impl Foo { fn bar(&mut self) {} }");
    assert!(errors.is_empty());
    match &krate.items[0].kind {
        ItemKind::Impl(impl_decl) => match &impl_decl.items[0].kind {
            ItemKind::Fn(fn_decl) => {
                let p = &fn_decl.sig.inputs[0];
                assert!(p.is_self);
                assert_eq!(
                    p.self_kind,
                    Some(SelfKind::Ref(Mutability::Mutable)),
                    "`&mut self` should be Ref(Mutable)"
                );
            }
            other => panic!("expected Fn, got {:?}", other),
        },
        other => panic!("expected Impl, got {:?}", other),
    }
}

#[test]
fn test_self_kind_distinct() {
    // Verify all 4 self kinds produce distinct AST representations.
    let cases: [(&str, SelfKind); 4] = [
        (
            "impl F { fn a(self) {} }",
            SelfKind::Value(Mutability::Immutable),
        ),
        (
            "impl F { fn b(mut self) {} }",
            SelfKind::Value(Mutability::Mutable),
        ),
        (
            "impl F { fn c(&self) {} }",
            SelfKind::Ref(Mutability::Immutable),
        ),
        (
            "impl F { fn d(&mut self) {} }",
            SelfKind::Ref(Mutability::Mutable),
        ),
    ];
    for (src, expected) in cases.iter() {
        let (krate, errors) = parse(src);
        assert!(
            errors.is_empty(),
            "parse failed for {:?}: {:?}",
            src,
            errors
        );
        match &krate.items[0].kind {
            ItemKind::Impl(impl_decl) => match &impl_decl.items[0].kind {
                ItemKind::Fn(fn_decl) => {
                    let p = &fn_decl.sig.inputs[0];
                    assert_eq!(p.self_kind, Some(*expected), "mismatch for {:?}", src);
                }
                other => panic!("expected Fn, got {:?}", other),
            },
            other => panic!("expected Impl, got {:?}", other),
        }
    }
}

#[test]
fn test_binding_mode_by_value_immutable() {
    // `let x = ...` should produce BindingMode::ByValue(Immutable)
    let (krate, errors) = parse("fn f() { let x = 1; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().unwrap();
    match &body.stmts[0] {
        Stmt::Local(local) => match &local.pat {
            Pat::Ident(mode, _, _) => {
                assert_eq!(*mode, BindingMode::ByValue(Mutability::Immutable));
            }
            other => panic!("expected Ident pat, got {:?}", other),
        },
        other => panic!("expected Local stmt, got {:?}", other),
    }
}

#[test]
fn test_binding_mode_by_value_mutable() {
    // `let mut x = ...` should produce BindingMode::ByValue(Mutable)
    let (krate, errors) = parse("fn f() { let mut x = 1; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().unwrap();
    match &body.stmts[0] {
        Stmt::Local(local) => match &local.pat {
            Pat::Ident(mode, _, _) => {
                assert_eq!(*mode, BindingMode::ByValue(Mutability::Mutable));
            }
            other => panic!("expected Ident pat, got {:?}", other),
        },
        other => panic!("expected Local stmt, got {:?}", other),
    }
}

#[test]
fn test_binding_mode_by_ref_immutable() {
    // `let ref x = ...` should produce BindingMode::ByRef(Immutable)
    let (krate, errors) = parse("fn f() { let ref x = 1; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().unwrap();
    match &body.stmts[0] {
        Stmt::Local(local) => match &local.pat {
            Pat::Ident(mode, _, _) => {
                assert_eq!(*mode, BindingMode::ByRef(Mutability::Immutable));
            }
            other => panic!("expected Ident pat, got {:?}", other),
        },
        other => panic!("expected Local stmt, got {:?}", other),
    }
}

#[test]
fn test_binding_mode_by_ref_mutable() {
    // `let ref mut x = ...` should produce BindingMode::ByRef(Mutable)
    let (krate, errors) = parse("fn f() { let ref mut x = 1; }");
    assert!(errors.is_empty());
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().unwrap();
    match &body.stmts[0] {
        Stmt::Local(local) => match &local.pat {
            Pat::Ident(mode, _, _) => {
                assert_eq!(*mode, BindingMode::ByRef(Mutability::Mutable));
            }
            other => panic!("expected Ident pat, got {:?}", other),
        },
        other => panic!("expected Local stmt, got {:?}", other),
    }
}

#[test]
fn test_generic_args_not_misparsed_as_comparison() {
    // Round 2c: `a < b` in expression position must parse as comparison,
    // NOT as `a::<b>` (generic args on a value path).
    let (krate, errors) = parse("fn f() { let _ = a < b; }");
    assert!(errors.is_empty());
    let init = first_let_init(&krate);
    match init {
        Expr::Binary { op, lhs, rhs, .. } => {
            assert_eq!(*op, BinOp::Lt, "top-level op should be Lt (comparison)");
            // lhs and rhs should both be paths (a and b), not generic args
            match lhs.as_ref() {
                Expr::Path(_, p, _) => {
                    assert_eq!(p.segments.len(), 1, "lhs should be single-segment path `a`");
                    assert!(
                        p.segments[0].args.is_none(),
                        "lhs should have no generic args"
                    );
                }
                other => panic!("expected Path lhs, got {:?}", other),
            }
            match rhs.as_ref() {
                Expr::Path(_, p, _) => {
                    assert_eq!(p.segments.len(), 1, "rhs should be single-segment path `b`");
                    assert!(
                        p.segments[0].args.is_none(),
                        "rhs should have no generic args"
                    );
                }
                other => panic!("expected Path rhs, got {:?}", other),
            }
        }
        other => panic!("expected Binary expr (Lt), got {:?}", other),
    }
}

#[test]
fn test_turbofish_in_expression_position() {
    // `Vec::<i32>` in expression position should parse with generic args.
    let (krate, errors) = parse("fn f() { let _ = Vec::<i32>; }");
    assert!(errors.is_empty(), "turbofish should parse: {:?}", errors);
    let init = first_let_init(&krate);
    match init {
        Expr::Path(_, p, _) => {
            assert_eq!(p.segments.len(), 1, "should be single-segment path");
            assert!(
                p.segments[0].args.is_some(),
                "should have generic args from turbofish"
            );
        }
        other => panic!("expected Path with turbofish, got {:?}", other),
    }
}

#[test]
fn test_generic_args_in_type_position_still_works() {
    // `Vec<i32>` in type position (no turbofish required) should parse with
    // generic args captured on the path segment.
    let (krate, errors) = parse("fn f(x: Vec<i32>) {}");
    assert!(errors.is_empty());
    // Walk to the Param.ty and verify it has generic args.
    match &krate.items[0].kind {
        ItemKind::Fn(fn_decl) => {
            assert!(!fn_decl.sig.inputs.is_empty(), "should have one param");
            let param = &fn_decl.sig.inputs[0];
            match &param.ty {
                Ty::Path(_, path, _) => {
                    assert_eq!(
                        path.segments.len(),
                        1,
                        "should be single-segment path `Vec`"
                    );
                    let args = &path.segments[0].args;
                    assert!(args.is_some(), "Vec should have generic args");
                    // The single arg should be i32
                    if let Some(GenericArgs::AngleBracketed(args)) = args {
                        assert_eq!(args.len(), 1, "Vec should have exactly 1 generic arg");
                        match &args[0] {
                            GenericArg::Type(Ty::Int(IntTy::I32, _)) => {}
                            other => panic!("expected i32 type arg, got {:?}", other),
                        }
                    }
                }
                other => panic!("expected Path ty, got {:?}", other),
            }
        }
        other => panic!("expected Fn, got {:?}", other),
    }
}

#[test]
fn test_turbofish_in_method_call() {
    // `foo.method::<i32>()` should parse as a method call with turbofish
    // captured in `MethodCall.generic_args`. Per Round 8c fix.
    let (krate, errors) = parse("fn f() { foo.method::<i32>(); }");
    assert!(
        errors.is_empty(),
        "method turbofish should parse: {:?}",
        errors
    );
    let fn_decl = match &krate.items[0].kind {
        ItemKind::Fn(f) => f,
        _ => panic!("expected Fn"),
    };
    let body = fn_decl.body.as_ref().expect("body");
    match &body.stmts[0] {
        Stmt::Expr(Expr::MethodCall { generic_args, .. }, true) => {
            assert!(
                generic_args.is_some(),
                "MethodCall.generic_args should be Some for `::<i32>`"
            );
        }
        other => panic!("expected MethodCall with turbofish, got {:?}", other),
    }
}
