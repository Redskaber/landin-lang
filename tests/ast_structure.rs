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
        krate.items.len() >= 1,
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
    // @ 42 should still produce 42 after error recovery
    let mut interner = Rodeo::new();
    let (tokens, errors) = tokenize("@ 42", &mut interner);
    assert!(!errors.is_empty(), "should have error for @");
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
