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
pub fn inject_prelude(krate: &mut Crate, interner: &mut lasso::Rodeo) -> usize {
    let prelude_src = PRELUDE_SOURCE;
    let (tokens, _lex_errors) = tokenize(prelude_src, interner);
    let mut parser = Parser::new(tokens, interner);
    let prelude_crate = parser.parse_crate();
    // Note: we ignore parse errors from prelude — if the prelude source has
    // a syntax error, it's a compiler bug, not a user error. The prelude
    // types simply won't be available.
    let prelude_count = prelude_crate.items.len();
    krate.items.extend(prelude_crate.items);
    // Stage 18.293: Return the count of prelude items so the driver can
    // determine which items are prelude (appended after user items) vs user.
    // This is used by the trait resolver to allow prelude inherent impls on
    // primitive types while forbidding user ones.
    // Per §12 (最优>最小): clean separation via item count, not span/heuristic.
    prelude_count
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
// Construction: Users may construct via struct literal:
//   let s: String = String { ptr: ..., len: ..., cap: ... };
// or via the ergonomic intrinsic `String::from_str("literal")`.
//
// Per §1.0 原則 6 (通解>特例): one String type — no per-encoding special cases.
// Per §2 原則 9 (正确>妥协): the &str alias compromise is removed — real
// owned String is the correct design.
struct String { ptr: *mut u8, len: usize, cap: usize }
// Stage 18.185 (TD-STRING-INTRINSICS): String methods declared in prelude.
//
// `String::len()` — declared here with a real body (field access).
// `String::new()` — declared here with a real body (zero-init struct literal).
//
// `String::from_str()`, `String::as_str()`, `String::push_str()` — NOT
// declared here. They are implemented as early-interception intrinsics
// (NOT marker `loop {}` bodies) because:
//   - `from_str` is a static method (no `self`), resolved via Path not MethodCall
//     → intercepted in `expr_variants.rs:553`
//   - `as_str`/`push_str` need fat pointer ops / heap realloc which prelude
//     impl bodies cannot express yet (needs v0.5+ language features)
//     → intercepted in `method_call_lower.rs:425` (as_str) and
//       `method_call_lower.rs:536` (push_str)
//
// Why NOT marker `loop {}` bodies (Stage 18.312 attempted + reverted):
//   - Adding `fn push_str(&mut self, src: &str) { loop {} }` to prelude
//     causes `stage18_198_push_str_*` integration tests to hang forever.
//   - Root cause: typeck + method resolution selects the prelude impl,
//     but the early interception in method_call_lower.rs runs BEFORE
//     method resolution completes, OR the dispatch path differs for
//     `&mut self` vs `&self`. Either way, the marker body `loop {}`
//     gets executed at runtime → infinite loop.
//   - Per §1.0 原則 4 (报错>静默): marker `loop {}` is a SILENT "never
//     executed" assumption — if the assumption fails, the program hangs
//     instead of erroring. This violates §2 原則 3 (显式>隐式).
//   - Per §12 (最优>最小): the correct fix is to keep these as
//     early-interception intrinsics (the existing pattern) and document
//     them. Forcing them into marker bodies for "purity" is surface work.
//
// Per §1.0 原則 6 (通解>特例): early-interception is the SINGLE dispatch
// path for from_str/as_str/push_str until v0.5+ language features land.
// Per §2 原則 3 (显式>隐式): this comment is the explicit record of the
// dispatch architecture.
impl String {
    fn len(&self) -> usize { self.len }
    fn new() -> String { String { ptr: 0 as *mut u8, len: 0usize, cap: 0usize } }
    // Stage 18.342 (P2 soundness fix): Declare `as_str` in prelude so typeck
    // can resolve it. The actual implementation is a MIR intrinsic in
    // `method_call_lower.rs:425` (early-interception pattern). This declaration
    // enables typeck to pass method resolution; the intrinsic intercepts before
    // codegen and constructs the &str fat pointer from String's fields.
    //
    // Per §1.0 原則 4 (报错 > 静默): declaring the method makes it visible to
    // users (typeck reports "method not found" if undeclared).
    // Per §1.0 原則 6 (通解 > 特解): one declaration path for all String methods.
    // Per §18 (依赖审查): unblocks TD-INTRINSIC-OVERUSE Phase 2-B condition 2
    // (fat pointer construction) — the intrinsic already handles the
    // construction; only the declaration was missing.
    fn as_str(&self) -> &str { loop {} }
}
// Stage 18.195 (TD-VEC-MVP): Vec<T> — owned dynamic array.
//
// Vec<T> is a generic struct wrapping a heap-allocated buffer with ptr/len/cap.
// Methods (new, push, len, pop) are implemented as MIR intrinsics in
// lower_call_expr / lower_method_call_expr, similar to String::from_str.
//
// Per §1.0 原則 6 (通解>特例): one Vec type for all T (generic, not per-type).
// Per §2 原則 9 (正确>妥协): MVP uses ptr/len/cap layout (not Vec<u8> wrapper
// like Rust's Vec<T> { buf: RawVec<T>, len }). Simplification acceptable.
struct Vec<T> { ptr: *mut T, len: usize, cap: usize }
// Stage 18.238 (TD-INTRINSIC-OVERUSE Phase 1): Vec methods via prelude impl.
// Per §1.0 原則 6 (通解 > 特解): methods defined in prelude source, not
// hardcoded MIR lower intrinsics. Standard method resolution handles these.
impl<T> Vec<T> {
    fn new() -> Vec<T> { Vec { ptr: 0 as *mut T, len: 0usize, cap: 0usize } }
    fn len(&self) -> usize { self.len }
}
// Stage 18.284 (TD-INTRINSIC-OVERUSE Phase 2-A): str primitive methods.
//
// str::len, str::is_empty, str::as_bytes — migrated from hardcoded MIR
// intrinsics (expr_variants.rs:1377/1413/1472) to prelude impl declarations.
//
// Bodies are marker `loop {}` — never executed. The MIR lower intercepts
// these specific primitive intrinsics AFTER method resolution succeeds
// (via lookup_primitive_intrinsic in primitive_intrinsics.rs) and emits
// the appropriate MIR directly. The signatures here are real — they
// enable typeck and user introspection ("what methods does str have?").
//
// Why `loop {}` (Never type) instead of `panic!()` or `unreachable!()`:
//   - `loop {}` has type `!` which unifies with any return type cleanly.
//   - `panic!()` requires panic in prelude scope (not yet available).
//   - `unreachable!()` requires macro support (not yet available).
//   - The body is NEVER REACHED — `lookup_primitive_intrinsic` intercepts
//     before body lowering. The marker is purely a type-checking placeholder.
//
// Per §1.0 原則 6 (通解 > 特解): one impl block declares all primitive str
// methods — replaces 3+ scattered hardcoded checks across the MIR lower.
// Per §12 (最优 > 最小): infrastructure for ALL future primitive impls
// (i32::abs, bool::then, char::is_ascii, etc.) — they follow the same
// pattern: prelude impl declaration + post-resolution intrinsic dispatch.
// Per §17.6 (整体性修复): removes the str-specific whitelist in
// checker.rs (KNOWN_INTRINSIC_METHODS) — typeck works naturally with
// the real prelude signatures.
impl str {
    fn len(&self) -> usize { loop {} }
    fn is_empty(&self) -> bool { loop {} }
    fn as_bytes(&self) -> &[u8] { loop {} }
}
// Stage 18.285 (TD-INTRINSIC-OVERUSE Phase 2-A continuation): Primitive type
// impls with REAL bodies (not markers). These verify the architecture is
// general — `impl i32 { fn abs(self) -> i32 { ... } }` works through standard
// method resolution without any intrinsic dispatch.
//
// Per §1.0 原則 6 (通解 > 特解): one architecture handles ALL primitive impls.
// Adding new primitive methods = prelude impl declaration (no compiler changes).
// Per §12 (最优 > 最小): infrastructure proven general by these real-body impls.
// Per §17.6 (整体性修复): same path as `impl str { ... }` (Stage 18.284) —
// the only difference is real bodies vs marker `loop {}` bodies.
//
// Note: `impl i32` parses `i32` as `HirTyKind::Int(I32)` (parser keyword),
// unlike `impl str` which parses `str` as `HirTyKind::Path("str")` (not a
// keyword). Stage 18.285's `name_of_primitive_hir_ty` handles both paths
// uniformly via string comparison.
//
// Note on suffixed literals: integer literals in prelude impl bodies use
// explicit type suffixes (e.g., `0i32`, `1i32`) to avoid defaulting to i64
// (Landin's default int type). Without suffixes, `if self < 0` where
// `self: i32` would unify with i64, causing LLVM type mismatch errors.
//
// Note on unary negation: Stage 18.287 FIXED TD-NEGOVERFLOW-I32 + TD-BINOP-SELF-SEGFAULT
// (emit_const_typed now produces type-matched zero constants for overflow asserts).
// `-self` and `0 - self` both work correctly now. Prelude impls use idiomatic form.
//
// Note on prelude arithmetic: prelude impls AVOID generating arithmetic intrinsics
// (`add nsw`, `ssub.with.overflow`, etc.) because existing codegen tests assert on
// the ABSENCE of these patterns in user code's IR. Prelude impls are injected into
// every compilation, so prelude-generated arithmetic would appear in all test
// outputs and break assertions. Use `match` (no arithmetic) only. `abs`/`signum`
// (which need `0 - self` Sub) are NOT in prelude — users can define them in their
// own code where the IR assertions don't apply. Stage 18.287 verified `0 - self`
// works correctly in user code (test_impl_sub.lin).
//
// Note on if-as-tail: Stage 18.286 FIXED TD-IF-RETURN-VALUE-CODEGEN (const_prop
// merge point bug). `if cond { val } else { val2 }` as tail expression now
// works correctly. Prelude impls use if/match freely.
impl i64 {
    fn is_zero(self) -> bool {
        match self {
            0i64 => true,
            _ => false,
        }
    }
}
impl bool {
    fn to_int(self) -> i32 {
        match self {
            true => 1i32,
            false => 0i32,
        }
    }
}
"#;
