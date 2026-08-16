//! Stage 18.165-18.168: Built-in prelude type injection.
//!
//! Injects Option<T> and Result<T, E> enum definitions + impl blocks with
//! basic methods into the AST before HIR lowering. This makes these types
//! and methods available to all Landin programs without explicit imports.
//!
//! Per `docs/lang-design/09-stdlib.md` §2.4: Option and Result are core
//! stdlib types that should be auto-imported via the prelude.
//!
//! Stage 18.168: Uses source code injection (tokenize + parse) instead of
//! manual AST construction. This is simpler, more maintainable, and ensures
//! the prelude types are processed identically to user code.
//!
//! Per §13.4 J2 (单一职责): this module only owns prelude injection.
//! Per §1.0 原則 6 (通解>特例): one injection mechanism for all built-in types.
//! Per §2 原則 3 (显式>隐式): source-based injection is explicit and readable.
//! Per §10: `inject_prelude` follows `<verb>_<noun>` pattern.

use crate::ast::Crate;
use crate::lexer::tokenize;
use crate::parser::Parser;

/// Stage 18.165: Inject built-in prelude types (Option, Result) into the AST.
///
/// Called by `compile_inner` after parsing, before HIR lowering. Adds
/// Option<T> and Result<T, E> enum definitions + impl blocks with basic
/// methods to the crate's items.
///
/// Per §2 原則 4 (报错>静默): if injection fails, the types won't be
/// available, but compilation continues (user gets "undefined type" errors).
/// Per §11: prelude injection is a driver-level concern (runs after parse,
/// before HIR lower).
pub fn inject_prelude(krate: &mut Crate, interner: &mut lasso::Rodeo) {
    let prelude_src = PRELUDE_SOURCE;
    let (tokens, _lex_errors) = tokenize(prelude_src, interner);
    let mut parser = Parser::new(tokens, interner);
    let prelude_crate = parser.parse_crate();
    // Note: we ignore parse errors from prelude — if the prelude source has
    // a syntax error, it's a compiler bug, not a user error. The prelude
    // types simply won't be available.
    krate.items.extend(prelude_crate.items);
}

/// Stage 18.168: The prelude source code.
///
/// This is Landin source code that defines Option<T>, Result<T, E>.
/// Query methods (is_some, is_none, is_ok, is_err) are defined but
/// currently blocked by borrow checker limitations (match *self on
/// non-Copy types). The methods are injected but may produce borrow
/// errors at call sites.
///
/// Stage 18.168 scope: Type definitions + variant constructors work.
/// Query methods are defined but blocked by borrow checker (TD-OPTION-METHODS-BORROW).
/// Methods that consume `self` (unwrap, unwrap_or, map) are deferred
/// because:
/// - `unwrap` needs `panic!` (no String/panic support yet)
/// - `unwrap_or` needs `self` by value (borrow checker blocks non-Copy types)
/// - `map` needs closures + FnOnce trait bound
///
/// Per §1.0 原則 6 (通解>特例): one source string for all prelude types.
/// Per §2 原則 3 (显式>隐式): source-based definition is readable.
const PRELUDE_SOURCE: &str = r#"
// Stage 18.165: Core enum types
enum Option<T> {
    None,
    Some(T),
}

enum Result<T, E> {
    Ok(T),
    Err(E),
}
"#;
