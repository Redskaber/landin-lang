//! Type error type.

use crate::mir::ty::Ty;
use crate::session::Span;

/// A type error encountered during type checking.
///
/// Non-fatal: type checking continues after an error, producing
/// `TyKind::Error` for the affected types. This allows the compiler
/// to report multiple errors in one pass.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub message: String,
    pub span: Span,
    pub expected: Option<Ty>,
    pub found: Option<Ty>,
}

impl TypeError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            expected: None,
            found: None,
        }
    }

    pub fn mismatch(expected: Ty, found: Ty, span: Span) -> Self {
        // Stage 15.80: use human-readable type names instead of Debug
        // format. Previously: `expected {:?}, found {:?}` leaked
        // `Int(I32)`, `Infer(IntVar(IntVid(0)))`, etc. into user-facing
        // messages. Now: `expected i32, found {integer}` etc.
        //
        // Per §1.0 原則 3 "显式 > 隐式": user-facing type names are
        // explicit (e.g., "i32", not "Int(I32)").
        // Per §1.0 原則 4 "报错 > 静默": the error message is clear about
        // what types mismatched, not cryptic about internal enum variants.
        use crate::mir::ty::type_kind_to_string;
        Self {
            message: format!(
                "mismatched types: expected {}, found {}",
                type_kind_to_string(&expected.kind),
                type_kind_to_string(&found.kind),
            ),
            span,
            expected: Some(expected),
            found: Some(found),
        }
    }

    /// Stage 16.80: Construct a mismatch error with resolver-backed type names.
    ///
    /// Unlike `mismatch`, this resolves `Adt` type names via the resolver,
    /// producing messages like "expected MyStruct, found i32" instead of
    /// "expected <adt>, found i32". Also resolves `Param` and `Projection`
    /// types to their names.
    ///
    /// Per §1.0 原則 3 "显式 > 隐式": user-facing type names are explicit.
    /// Per §23 (API Naming): `mismatch_with_resolver` follows
    /// `<noun>_<prep>_<noun>` pattern.
    pub fn mismatch_with_resolver(
        expected: Ty,
        found: Ty,
        span: Span,
        resolver: &crate::traits::TraitResolver,
        interner: &lasso::Rodeo,
    ) -> Self {
        use crate::mir::ty::type_kind_to_string_with_resolver;
        Self {
            message: format!(
                "mismatched types: expected {}, found {}",
                type_kind_to_string_with_resolver(&expected.kind, resolver, interner),
                type_kind_to_string_with_resolver(&found.kind, resolver, interner),
            ),
            span,
            expected: Some(expected),
            found: Some(found),
        }
    }

    pub fn unresolved(span: Span) -> Self {
        Self {
            message: "type annotations needed".into(),
            span,
            expected: None,
            found: None,
        }
    }
}

// Stage 15.16: implement `Spanned` for uniform span access.
impl crate::diagnostics::Spanned for TypeError {
    fn span(&self) -> crate::session::Span {
        self.span
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "type error: {} at {}", self.message, self.span)
    }
}

// Stage 3.64 (P2 fix): implement `std::error::Error` for `TypeError`.
impl std::error::Error for TypeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;
    use crate::mir::ty::{type_to_string_with_resolver, Ty, TyKind};
    use crate::session::Span;

    /// Stage 16.80 positive 1: Adt type resolves to actual struct name.
    #[test]
    fn stage16_80_adt_resolves_name() {
        let src =
            "struct MyStruct { x: i32 } fn main() { let s: MyStruct = MyStruct { x: 42 }; 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        // Find the MyStruct DefId via type_by_def_id
        for (def_id, spur) in &resolver.type_by_def_id {
            let name = interner.resolve(spur);
            if name == "MyStruct" {
                let ty = Ty::new(TyKind::Adt(*def_id, Vec::new().into()), Span::DUMMY);
                let formatted = type_to_string_with_resolver(&ty, resolver, interner);
                assert_eq!(
                    formatted, "MyStruct",
                    "Adt type should resolve to 'MyStruct', got '{}'",
                    formatted
                );
                return;
            }
        }
        panic!("MyStruct not found in type_by_def_id");
    }

    /// Stage 16.80 positive 2: Primitive types are unchanged by resolver variant.
    #[test]
    fn stage16_80_primitive_unchanged() {
        let src = "fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        let ty = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let formatted = type_to_string_with_resolver(&ty, resolver, interner);
        assert_eq!(formatted, "i32");
    }

    /// Stage 16.80 negative 1: Unknown Adt DefId shows <adt#N>.
    #[test]
    fn stage16_80_unknown_adt_shows_id() {
        let src = "fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        // Use a DefId that doesn't exist in type_by_def_id.
        let unknown_def_id = crate::hir::DefId(9999);
        let ty = Ty::new(TyKind::Adt(unknown_def_id, Vec::new().into()), Span::DUMMY);
        let formatted = type_to_string_with_resolver(&ty, resolver, interner);
        assert!(
            formatted.starts_with("<adt#"),
            "Unknown Adt should show <adt#N>, got '{}'",
            formatted
        );
    }

    /// Stage 16.80 negative 2: Type mismatch error shows struct name.
    #[test]
    fn stage16_80_mismatch_shows_struct_name() {
        let src = "struct MyStruct { x: i32 } fn main() { let s: MyStruct = 42; 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        // Find the MyStruct DefId
        let mut struct_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "MyStruct" {
                struct_def_id = Some(*def_id);
                break;
            }
        }
        let def_id = struct_def_id.expect("MyStruct not found");
        let expected = Ty::new(TyKind::Adt(def_id, Vec::new().into()), Span::DUMMY);
        let found = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let error =
            TypeError::mismatch_with_resolver(expected, found, Span::DUMMY, resolver, interner);
        assert!(
            error.message.contains("MyStruct"),
            "Error message should contain 'MyStruct', got: {}",
            error.message
        );
        assert!(
            error.message.contains("i32"),
            "Error message should contain 'i32', got: {}",
            error.message
        );
    }

    /// Stage 16.80 negative 3: Type mismatch error shows enum name.
    #[test]
    fn stage16_80_mismatch_shows_enum_name() {
        let src = "enum MyEnum { A, B } fn main() { let e: MyEnum = 42; 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        let mut enum_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "MyEnum" {
                enum_def_id = Some(*def_id);
                break;
            }
        }
        let def_id = enum_def_id.expect("MyEnum not found");
        let expected = Ty::new(TyKind::Adt(def_id, Vec::new().into()), Span::DUMMY);
        let found = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let error =
            TypeError::mismatch_with_resolver(expected, found, Span::DUMMY, resolver, interner);
        assert!(
            error.message.contains("MyEnum"),
            "Error message should contain 'MyEnum', got: {}",
            error.message
        );
    }

    /// Stage 16.80 negative 4: Mismatch struct vs int — full message format.
    #[test]
    fn stage16_80_mismatch_struct_vs_int_full_message() {
        let src = "struct Foo { x: i32 } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        let mut foo_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            if interner.resolve(spur) == "Foo" {
                foo_def_id = Some(*def_id);
                break;
            }
        }
        let def_id = foo_def_id.expect("Foo not found");
        let expected = Ty::new(TyKind::Adt(def_id, Vec::new().into()), Span::DUMMY);
        let found = Ty::new(TyKind::Int(crate::ast::IntTy::I32), Span::DUMMY);
        let error =
            TypeError::mismatch_with_resolver(expected, found, Span::DUMMY, resolver, interner);
        assert_eq!(
            error.message, "mismatched types: expected Foo, found i32",
            "Full message should be exact, got: {}",
            error.message
        );
    }

    /// Stage 16.80 negative 5: Mismatch two structs — both names shown.
    #[test]
    fn stage16_80_mismatch_two_structs() {
        let src = "struct Foo { x: i32 } struct Bar { y: i32 } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        let mut foo_def_id = None;
        let mut bar_def_id = None;
        for (def_id, spur) in &resolver.type_by_def_id {
            let name = interner.resolve(spur);
            if name == "Foo" {
                foo_def_id = Some(*def_id);
            } else if name == "Bar" {
                bar_def_id = Some(*def_id);
            }
        }
        let foo = foo_def_id.expect("Foo not found");
        let bar = bar_def_id.expect("Bar not found");
        let expected = Ty::new(TyKind::Adt(foo, Vec::new().into()), Span::DUMMY);
        let found = Ty::new(TyKind::Adt(bar, Vec::new().into()), Span::DUMMY);
        let error =
            TypeError::mismatch_with_resolver(expected, found, Span::DUMMY, resolver, interner);
        assert_eq!(
            error.message, "mismatched types: expected Foo, found Bar",
            "Two-struct mismatch should show both names, got: {}",
            error.message
        );
    }

    /// Stage 16.80 negative 6: Param type resolves to parameter name.
    #[test]
    fn stage16_80_param_shows_name() {
        // Use a source that has a generic type parameter T.
        let src = "fn f<T>(x: T) -> T { x } fn main() { 0 }";
        let result = compile(src);
        let resolver = &result.trait_resolver;
        let interner = &result.interner;
        // "T" should be interned because it appears in the source.
        let name_spur = interner.get("T").expect("T should be interned from source");
        let param = crate::mir::ty::ParamTy {
            index: 0,
            name: name_spur,
        };
        let ty = Ty::new(TyKind::Param(param), Span::DUMMY);
        let formatted = type_to_string_with_resolver(&ty, resolver, interner);
        assert_eq!(
            formatted, "T",
            "Param type should resolve to 'T', got '{}'",
            formatted
        );
    }
}
