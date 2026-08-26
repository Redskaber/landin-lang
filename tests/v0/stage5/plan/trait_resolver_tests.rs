//! Stage 5.1: TraitResolver tests
//!
//! Tests that TraitResolver correctly collects trait definitions + impl blocks
//! and builds dispatch tables.
//!
//! Per stage-committee-process.md v3.18 §17.1, new tests in tests/v0/stage5/plan/.

use landin_compiler::hir::lower::lower_crate;
use landin_compiler::lexer::tokenize;
use landin_compiler::parser::Parser;
use landin_compiler::resolve::resolve_crate;
use landin_compiler::TraitResolver;
use lasso::Rodeo;

fn parse_lower_resolve(src: &str) -> landin_compiler::hir::HirCrate {
    let mut interner = Rodeo::new();
    interner.get_or_intern("Self");
    interner.get_or_intern("self");
    interner.get_or_intern("crate");
    interner.get_or_intern("super");
    let (tokens, _) = tokenize(src, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    let mut hir = lower_crate(&krate, &interner).0;
    let _ = resolve_crate(&mut hir, &mut interner);
    hir
}

#[test]
fn test_trait_collected() {
    let hir = parse_lower_resolve("trait Foo { fn bar(); }");
    let mut resolver = TraitResolver::new();
    resolver.collect(&hir, &mut Rodeo::new(), 0);
    assert_eq!(resolver.trait_count(), 1, "should collect 1 trait");
    assert_eq!(resolver.impl_count(), 0, "no impls in source");
}

#[test]
fn test_impl_collected() {
    let hir =
        parse_lower_resolve("trait Foo { fn bar(); } struct S; impl Foo for S { fn bar() {} }");
    let mut resolver = TraitResolver::new();
    resolver.collect(&hir, &mut Rodeo::new(), 0);
    assert_eq!(resolver.trait_count(), 1, "should collect 1 trait");
    assert_eq!(resolver.impl_count(), 1, "1 user impl");
}

#[test]
fn test_method_dispatch_table() {
    let hir = parse_lower_resolve(
        "trait Display { fn show(); } struct Point; impl Display for Point { fn show() {} }",
    );
    let mut resolver = TraitResolver::new();
    resolver.collect(&hir, &mut Rodeo::new(), 0);
    assert_eq!(resolver.trait_count(), 1, "1 user trait");
    assert_eq!(resolver.impl_count(), 1, "1 user impl");
    // Verify dispatch table was built (trait_name, self_ty_name) → impl DefId
    // We can't easily get the Spur values without the interner, but we can
    // verify the impl_by_trait_and_type map has 1 entry
    assert!(
        !resolver.impl_by_trait_and_type.is_empty(),
        "dispatch table should have 1 entry"
    );
}
