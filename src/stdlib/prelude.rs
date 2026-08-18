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

/// Stage 18.169: The prelude source code.
///
/// This is Landin source code that defines Option<T>, Result<T, E>, and
/// their basic query methods (is_some, is_none, is_ok, is_err).
///
/// Stage 18.169 fix: `match *self` on non-Copy types now works because
/// the borrow checker no longer checks Copy-ness for SwitchInt
/// discriminants (match scrutinees are reads, not moves).
///
/// Per §1.0 原則 6 (通解>特例): one source string for all prelude types.
/// Per §2 原則 3 (显式>隐式): source-based definition is readable.
const PRELUDE_SOURCE: &str = r#"
enum Option<T> { None, Some(T) }
enum Result<T, E> { Ok(T), Err(E) }
trait Copy {}
impl<T> Copy for Option<T> {}
impl<T, E> Copy for Result<T, E> {}
impl<T> Option<T> {
    fn is_some(&self) -> bool { match *self { Some(_) => true, None => false } }
    fn is_none(&self) -> bool { match *self { Some(_) => false, None => true } }
    fn unwrap_or(self, default: T) -> T { match self { Some(v) => v, None => default } }
}
impl<T, E> Result<T, E> {
    fn is_ok(&self) -> bool { match *self { Ok(_) => true, Err(_) => false } }
    fn is_err(&self) -> bool { match *self { Ok(_) => false, Err(_) => true } }
    fn unwrap_or(self, default: T) -> T { match self { Ok(v) => v, Err(_) => default } }
}
// Stage 18.179 (TD-HEAP-ALLOC): Box<T> — owned heap pointer wrapper.
//
// MVP: Box<T> is a tuple struct wrapping a *mut T. Users construct it via
// `Box(p)` where `p` is obtained from `__landin_alloc`. Access via `b.0`
// (field 0 = the pointer), then `*b.0` to dereference. Manual cleanup via
// `__landin_dealloc(b.0 as *mut u8)`.
//
// Deferred to Stage 18.180:
//   - `Box::new(x)` sugar (intrinsic that calls alloc + store + construct)
//   - Auto-drop (drop glue that calls `__landin_dealloc`)
//
// Per §1.0 原則 6 (通解>特例): one Box type for all T — no per-type special cases.
// Per §2 原則 9 (正确>妥协): the MVP is a temporary compromise; real Box with
// auto-drop is the correct design (recorded as TD-BOX-AUTO-DROP).
struct Box<T>(*mut T)
// Stage 18.180 (TD-STRING-AS-STR-ALIAS fix): String — owned heap string.
//
// String is now a REAL struct type (not a &str alias). It wraps a heap-
// allocated buffer (ptr) with length (len) and capacity (cap) fields.
//
// This fixes the design violation from Stage 18.176 where String was
// mapped to PrimTy::Str (a stack-allocated fat pointer). Per the design
// doc (09-stdlib.md §3.4), String must be an owned heap type.
//
// MVP limitation: Users must construct String manually via struct literal:
//   let s: String = String { ptr: ..., len: ..., cap: ... };
// Ergonomic intrinsics (String::from_str, push_str, len, as_str) are
// deferred to Stage 18.181 (TD-STRING-INTRINSICS).
//
// Per §1.0 原則 6 (通解>特例): one String type — no per-encoding special cases.
// Per §2 原則 9 (正确>妥协): the &str alias compromise is removed — real
// owned String is the correct design.
struct String { ptr: *mut u8, len: i64, cap: i64 }
"#;
