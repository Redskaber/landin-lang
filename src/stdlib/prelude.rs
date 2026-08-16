//! Stage 18.165: Built-in prelude type injection.
//!
//! Injects Option<T> and Result<T, E> enum definitions into the AST before
//! HIR lowering. This makes these types available to all Landin programs
//! without explicit imports.
//!
//! Per `docs/lang-design/09-stdlib.md` §2.4: Option and Result are core
//! stdlib types that should be auto-imported via the prelude.
//!
//! Per §13.4 J2 (单一职责): this module only owns prelude injection.
//! Per §1.0 原則 6 (通解>特例): one injection mechanism for all built-in types.
//! Per §10: `inject_prelude` follows `<verb>_<noun>` pattern.

use crate::ast::*;
use crate::session::Span;

/// Stage 18.165: Inject built-in prelude types (Option, Result) into the AST.
///
/// Called by `compile_inner` after parsing, before HIR lowering. Adds
/// Option<T> and Result<T, E> enum definitions to the crate's items.
///
/// Per §2 原則 4 (报错>静默): if injection fails, the types won't be
/// available, but compilation continues (user gets "undefined type" errors).
/// Per §11: prelude injection is a driver-level concern (runs after parse,
/// before HIR lower).
pub fn inject_prelude(krate: &mut Crate, interner: &mut lasso::Rodeo) {
    krate.items.push(make_option_enum(interner));
    krate.items.push(make_result_enum(interner));
}

/// Create the `Option<T>` enum definition.
///
/// ```landin
/// enum Option<T> {
///     None,
///     Some(T),
/// }
/// ```
fn make_option_enum(interner: &mut lasso::Rodeo) -> Item {
    Item {
        vis: Visibility::Public,
        attrs: vec![],
        kind: ItemKind::Enum(EnumDecl {
            ident: make_ident(interner, "Option"),
            generics: make_generics(interner, &["T"]),
            variants: vec![
                EnumVariant {
                    ident: make_ident(interner, "None"),
                    data: VariantData::Unit(Span::DUMMY),
                    span: Span::DUMMY,
                },
                EnumVariant {
                    ident: make_ident(interner, "Some"),
                    data: VariantData::Tuple(
                        vec![make_anon_field(make_type_param_path(interner, "T"))],
                        Span::DUMMY,
                    ),
                    span: Span::DUMMY,
                },
            ],
            span: Span::DUMMY,
        }),
        span: Span::DUMMY,
    }
}

/// Create the `Result<T, E>` enum definition.
///
/// ```landin
/// enum Result<T, E> {
///     Ok(T),
///     Err(E),
/// }
/// ```
fn make_result_enum(interner: &mut lasso::Rodeo) -> Item {
    Item {
        vis: Visibility::Public,
        attrs: vec![],
        kind: ItemKind::Enum(EnumDecl {
            ident: make_ident(interner, "Result"),
            generics: make_generics(interner, &["T", "E"]),
            variants: vec![
                EnumVariant {
                    ident: make_ident(interner, "Ok"),
                    data: VariantData::Tuple(
                        vec![make_anon_field(make_type_param_path(interner, "T"))],
                        Span::DUMMY,
                    ),
                    span: Span::DUMMY,
                },
                EnumVariant {
                    ident: make_ident(interner, "Err"),
                    data: VariantData::Tuple(
                        vec![make_anon_field(make_type_param_path(interner, "E"))],
                        Span::DUMMY,
                    ),
                    span: Span::DUMMY,
                },
            ],
            span: Span::DUMMY,
        }),
        span: Span::DUMMY,
    }
}

/// Helper: create an Ident from a string name, interning it.
fn make_ident(interner: &mut lasso::Rodeo, name: &str) -> Ident {
    Ident {
        name: interner.get_or_intern(name),
        span: Span::DUMMY,
    }
}

/// Helper: create Generics with the given type parameter names.
fn make_generics(interner: &mut lasso::Rodeo, params: &[&str]) -> Generics {
    Generics {
        params: params
            .iter()
            .map(|&name| {
                GenericParam::Type(TypeParam {
                    ident: make_ident(interner, name),
                    bounds: vec![],
                    default: None,
                    span: Span::DUMMY,
                })
            })
            .collect(),
        where_clause: vec![],
        span: Span::DUMMY,
    }
}

/// Helper: create a type that references a generic parameter (e.g., `T`).
fn make_type_param_path(interner: &mut lasso::Rodeo, name: &str) -> Ty {
    Ty::Path(
        QSelf {
            ty: None,
            position: 0,
        },
        Path {
            segments: vec![PathSegment {
                ident: make_ident(interner, name),
                args: None,
            }],
            leading: PathLeading::None,
            span: Span::DUMMY,
        },
        Span::DUMMY,
    )
}

/// Helper: create an anonymous (unnamed) struct field.
fn make_anon_field(ty: Ty) -> StructField {
    StructField {
        ident: None,
        ty,
        vis: Visibility::Public,
        span: Span::DUMMY,
    }
}
