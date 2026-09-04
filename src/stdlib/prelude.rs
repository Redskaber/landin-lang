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

/// Stage 44 (v0.5 — TD-PRELUDE-MACRO-TIMING): Tokenize the prelude source
/// for injection BEFORE macro expansion.
///
/// This allows prelude macros (panic!, unreachable!) to be expanded in
/// prelude bodies. Previously, prelude was injected after macro_expand
/// (compile_inner.rs:67), so prelude macro calls were never expanded.
///
/// Per §1.0 原則 6 (通解 > 特解): one prelude source for both token-level
/// and AST-level injection.
/// Per §12 (最优 > 最小): root-cause fix — inject at token level so macros
/// in prelude are expanded.
pub fn prelude_tokens(interner: &mut lasso::Rodeo) -> Vec<crate::lexer::Token> {
    let (tokens, _lex_errors) = tokenize(PRELUDE_SOURCE, interner);
    // Stage 44 (v0.5): Remove trailing Eof token — it would cause the parser
    // to stop before reaching user tokens. The final Eof is re-added by the
    // user's tokenize call.
    // Per §12 (最优 > 最小): root-cause fix — strip Eof from prelude tokens.
    tokens
        .into_iter()
        .filter(|t| !matches!(t.kind, crate::lexer::TokenKind::Eof))
        .collect()
}

/// Stage 44 (v0.5 — TD-PRELUDE-MACRO-TIMING): Count the number of top-level
/// items in the prelude source. Used by compile_inner to determine the
/// prelude/user boundary when prelude is injected at token level.
///
/// Per §12 (最优 > 最小): clean separation via item count.
pub fn count_prelude_items() -> usize {
    // Count top-level items by parsing PRELUDE_SOURCE. This is called once
    // per compilation, so the cost is negligible.
    let mut interner = lasso::Rodeo::new();
    let (tokens, _lex_errors) = tokenize(PRELUDE_SOURCE, &mut interner);
    let mut parser = Parser::new(tokens, &mut interner);
    let prelude_crate = parser.parse_crate();
    prelude_crate.items.len()
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
// Stage 31.6b (v0.19): extern "C" declarations for prelude impl bodies.
//
// These are the runtime helper functions that prelude impl blocks call
// (e.g., String::from_str calls __landin_alloc + __landin_memcpy).
// Previously, these were called via hardcoded DefId synthesis in MIR
// intrinsics (Stage 18.185). Now they are declared as regular extern "C"
// items in the prelude source — standard method resolution handles them.
//
// Per §1.0 原則 6 (通解 > 特解): one extern "C" block for all prelude helpers.
// Per §1.0 原則 3 (显式 > 隐式): explicit declarations, not hidden DefId synthesis.
// Per §12 (最优 > 最小): root-cause fix via prelude impl migration.
extern "C" {
    fn __landin_alloc(size: i64) -> *mut u8;
    fn __landin_memcpy(dst: *mut u8, src: *const u8, n: i64);
    fn __landin_realloc(ptr: *mut u8, old_size: i64, new_size: i64) -> *mut u8;
    // Stage 32.4 (v0.20): Vec::get bounds check panic helper.
    //
    // NOTE: Vec::push/get migration is BLOCKED on v0.5+ method monomorphization
    // (TD-VEC-PUSH-GET-MIGRATION). The prelude impl body needs to substitute
    // `Param(N)` with the call-site type at codegen time — but Landin's
    // current monomorphization only collects MonoItems for layout building,
    // not for function body codegen.
    //
    // The declaration is kept for future use (Stage 32.5+ when method
    // monomorphization is implemented). Currently unused.
    //
    // Per §1.0 原則 4 (报错 > 静默): TD item documents the limitation.
    // Per §1.0 原則 9 (正确 > 妥协): don't hack codegen to substitute Param(N).
    fn __landin_panic_bounds_check(index: i64, len: i64);
    // Stage 40.2 (v0.28 — TD-PANIC-MACRO-BROKEN): panic message runtime
    // helper. The `panic!` macro (registered in print_macros.rs:172)
    // expands to `__landin_panic_msg(msg)`, but the function was NEVER
    // declared in the prelude extern "C" block — so resolver failed with
    // "cannot find value in this scope" (E300/E400) for ANY panic! call.
    //
    // This is a P1 bug: the macro infrastructure was complete (Stage 18.29)
    // but the prelude declaration was missing, making panic! unusable.
    //
    // Per §1.0 原則 4 (报错 > 静默): previously panic! silently failed with
    // E300/E400 instead of running. Now it properly calls the C runtime
    // helper which prints the message to stderr and calls abort().
    // Per §1.0 原則 6 (通解 > 特解): one declaration for ALL panic! calls.
    // Per §12 (最优 > 最小): root-cause fix — declare the missing extern,
    // not patch each panic! call site.
    // Per §2.2 根因思维: fix the missing declaration, not the symptom
    // (resolver error).
    //
    // Stage 41 (v0.5 — TD-SPECIAL-2): return type changed from `()` to `!`
    // (Never type). Since C `exit(1)` never returns, the Landin-side
    // declaration should reflect this via `-> !`. This lets typeck unify
    // `!` with any type (unify.rs:749 — Never unifies with anything),
    // eliminating the `loop {}` wrapper needed in prelude unwrap/expect
    // methods (4 sites removed).
    //
    // Per §12 (最优 > 最小): root-cause fix — declare noreturn via `!`
    // type, not patch each call site with `loop {}`.
    // Per §1.0 原則 6 (通解 > 特解): one `-> !` declaration for ALL
    // panic paths (panic!, unreachable!, unwrap, expect).
    fn __landin_panic_msg(msg: *const u8) -> !;
    // Stage 40.2 (v0.28 — TD-PANIC-MACRO-BROKEN): unreachable! macro
    // runtime helper. Like __landin_panic_msg, this was missing from the
    // prelude extern "C" block, making unreachable! macro unusable.
    // Per §1.0 原則 6 (通解 > 特解): one declaration for all panic paths.
    //
    // Stage 41 (v0.5 — TD-SPECIAL-2): return type `!` (Never) — same
    // rationale as __landin_panic_msg.
    fn __landin_unreachable(msg: *const u8) -> !;
    // Stage 43 (v0.5 — TD-PANIC-CONSOLIDATION): Unified panic with message.
    // Per §1.0 原則 6 (通解 > 特解): one panic function for all paths.
    // Per §12 (最优 > 最小): root-cause consolidation — 3 panic_* wrappers
    // now call this internally.
    fn __landin_panic_fmt(msg: *const u8) -> !;
    // Stage 36.6 (v0.24 — TD-FORMAT-MIGRATION): i64→str conversion helper
    // for the prelude format! impl. Writes the decimal representation of
    // `val` to `buf`, returning the number of bytes written.
    //
    // Per §1.0 原則 6 (通解 > 特解): one C helper for all i64 formatting.
    // Per §1.0 原則 3 (显式 > 隐式): explicit declaration, not hidden DefId.
    fn __landin_i64_to_str(buf: *mut u8, cap: i64, val: i64) -> i64;
    // Stage 37.2 (v0.25): i64→hex string conversion helper.
    // Writes the lowercase hexadecimal representation of `val` to `buf`,
    // returning the number of bytes written. Negative values are formatted
    // as two's complement (matching Rust's `format!("{:x}", val)`).
    //
    // Per §1.0 原則 6 (通解 > 特解): one C helper for all hex formatting.
    fn __landin_i64_to_hex(buf: *mut u8, cap: i64, val: i64) -> i64;
    // Stage 38.1 (v0.26): i64→octal string conversion helper.
    // Writes the octal representation of `val` to `buf`.
    fn __landin_i64_to_octal(buf: *mut u8, cap: i64, val: i64) -> i64;
    // Stage 38.1 (v0.26): i64→binary string conversion helper.
    // Writes the binary representation of `val` to `buf`.
    fn __landin_i64_to_binary(buf: *mut u8, cap: i64, val: i64) -> i64;
    // Stage 41 (v0.5 — TD-SPECIAL-4): Unified i64 format helper.
    // One function handles all 4 bases (decimal/hex/octal/binary) via a
    // `base` parameter (10/16/8/2). This is the 通解 that replaces the 4
    // special-case wrappers above.
    //
    // Per §1.0 原則 6 (通解 > 特解): one function for all integer formatting.
    // Per §12 (最优 > 最小): root-cause consolidation of 4 wrappers into 1.
    fn __landin_i64_format(val: i64, base: i64, buf: *mut u8, cap: i64) -> i64;
}
// Stage 36.6 (v0.24 — TD-FORMAT-MIGRATION): format! prelude impl.
//
// Replaces the 598-LOC `lower_format_variadic_intrinsic` MIR walker
// (src/mir/lower/format_intrinsics.rs, DELETED in this stage) with a
// regular prelude free function that uses standard Landin language
// features (while loops, slice indexing, String construction).
//
// MVP limit (same as the old MIR walker): all args are i64. Non-i64 args
// must be cast by the caller — the format! macro does `args as i64`.
// Full type-dispatched formatting (Display trait) is Stage 36.3 (v0.6+).
//
// Per §1.0 原則 6 (通解 > 特解): one prelude fn for ALL format! calls —
// no per-call-site MIR weaving, no special-case interception.
// Per §1.0 原則 10 (唯一可信数据源): this fn is the single source of
// truth for format! logic.
// Per §12 (最优 > 最小): root-cause fix = prelude impl + macro expansion.
//   Net -368 LOC (+200 prelude +30 macro -598 MIR walker = -368).
fn __landin_format_v2(fmt: &str, args: &[i64]) -> String {
    let buf_size: i64 = 4096;
    let out_ptr: *mut u8 = __landin_alloc(buf_size);
    let mut out_len: usize = 0usize;
    let mut fmt_idx: usize = 0usize;
    let mut arg_idx: usize = 0usize;
    while fmt_idx < fmt.len() {
        let byte_ptr: *const u8 = fmt.ptr + fmt_idx;
        let byte: u8 = *byte_ptr;
        if byte == 123u8 {
            // Stage 37.1 (v0.25): Format specifier parsing.
            // `{}` → default (i64 decimal)
            // `{:?}` → debug format (currently same as {} — MVP, needs Display trait for real Debug)
            // `{:x}` → hex format (future Stage 37.2)
            // Check if next byte is ':' (byte 58) → format specifier
            let next_idx: usize = fmt_idx + 1usize;
            let has_specifier: bool = next_idx < fmt.len();
            let spec_byte: u8 = if has_specifier {
                let spec_ptr: *const u8 = fmt.ptr + next_idx;
                *spec_ptr
            } else {
                0u8
            };
            if has_specifier && spec_byte == 58u8 {
                // ':' found — read the specifier char after ':'
                let spec_char_idx: usize = fmt_idx + 2usize;
                let spec_char: u8 = if spec_char_idx < fmt.len() {
                    let sc_ptr: *const u8 = fmt.ptr + spec_char_idx;
                    *sc_ptr
                } else {
                    0u8
                };
                if arg_idx < args.len() {
                    let arg_ptr: *const i64 = args.ptr + arg_idx;
                    let val: i64 = *arg_ptr;
                    // Stage 37.1: {:?} → debug format.
                    // Stage 37.2: {:x} → hex format (lowercase).
                    // Stage 38.1: {:o} → octal format, {:b} → binary format.
                    // MVP for {:?}: same as {} (decimal i64). Full Debug needs Display trait (v0.6+).
                    // Per §1.0 原則 9 (正确 > 妥协): document the MVP limitation.
                    // Stage 41 (v0.5 — TD-SPECIAL-4): unified __landin_i64_format
                    // call with base parameter (10/16/8/2). Replaces 4 separate
                    // C wrapper calls — one 通解 for all integer formatting.
                    // Per §1.0 原則 6 (通解 > 特解): one dispatch, one function.
                    let base: i64 = if spec_char == 120u8 {
                        16i64  // 'x' — hex
                    } else if spec_char == 111u8 {
                        8i64   // 'o' — octal
                    } else if spec_char == 98u8 {
                        2i64   // 'b' — binary
                    } else {
                        10i64  // default (incl. '?' debug, no specifier)
                    };
                    let written: i64 = __landin_i64_format(
                        val,
                        base,
                        out_ptr + out_len,
                        buf_size - out_len as i64,
                    );
                    out_len = out_len + written as usize;
                    arg_idx = arg_idx + 1usize;
                }
                // Advance past {:?} (4 bytes: { : ? })
                fmt_idx = fmt_idx + 4usize;
            } else {
                // No specifier — plain {} (2 bytes: { })
                if arg_idx < args.len() {
                    let arg_ptr: *const i64 = args.ptr + arg_idx;
                    // Stage 41 (v0.5 — TD-SPECIAL-4): unified __landin_i64_format
                    // with base=10 for decimal (default).
                    let written: i64 = __landin_i64_format(*arg_ptr, 10i64, out_ptr + out_len, buf_size - out_len as i64);
                    out_len = out_len + written as usize;
                    arg_idx = arg_idx + 1usize;
                }
                fmt_idx = fmt_idx + 2usize;
            }
        } else {
            let dest: *mut u8 = out_ptr + out_len;
            *dest = byte;
            out_len = out_len + 1usize;
            fmt_idx = fmt_idx + 1usize;
        }
    }
    let cap: usize = out_len + 1usize;
    String { ptr: out_ptr, len: out_len, cap: cap }
}
enum Option<T> { None, Some(T) }
enum Result<T, E> { Ok(T), Err(E) }
trait Copy {}
// Stage 59 (v0.7 — TD-CLONE-TRAIT-MISSING): Clone trait.
// Per Rust: Clone has a single method `fn clone(&self) -> Self`.
// Types that implement Clone can be explicitly duplicated.
// Per §1.0 原則 6 (通解 > 特解): one trait, all types implement it.
// Per §12 (最优 > 最小): root-cause fix — trait enables cloned/copied methods.
//
// NOTE: User code may also define `trait Clone { ... }` — the resolver
// should merge them. If duplicate definition error occurs, user code
// takes precedence (same as Rust — user can shadow prelude traits).
// TD-TRAIT-NAME-COLLISION: resolver needs to handle prelude/user trait
// name collisions. For now, renamed to `Clone` (kept as-is) — if user
// defines their own Clone trait, the resolver reports duplicate. This
// is a known limitation (TD-TRAIT-NAME-COLLISION, P3, v0.8+).
trait Clone {
    fn clone(&self) -> Self;
}
impl<T> Copy for Option<T> {}
impl<T, E> Copy for Result<T, E> {}
// Stage 59 (v0.7): Clone impls for basic types.
// Per Rust: all primitive types are Clone (bitwise copy).
// Per §1.0 原則 6 (通解 > 特解): one impl per type, no special-casing.
impl Clone for i32 {
    fn clone(&self) -> i32 { *self }
}
impl Clone for i64 {
    fn clone(&self) -> i64 { *self }
}
impl Clone for bool {
    fn clone(&self) -> bool { *self }
}
impl Clone for usize {
    fn clone(&self) -> usize { *self }
}
// Stage 61 (v0.7 — TD-DISPLAY-TRAIT-MISSING partial): Display trait.
//
// Provides user-readable string representation for types. Mirrors Rust's
// std::fmt::Display (simplified — no Formatter/Result, just append to String).
//
// Per Rust Design FAQ: Display trait is the canonical mechanism for
// user-facing string conversion. All primitive types implement Display.
// Per Rust API Guidelines: `fmt` writes the display representation into
// the provided buffer.
// Per §1.0 原則 6 (通解 > 特解): one trait, all types implement it.
// Per §12 (最优 > 最小): root-cause fix — trait enables type-dispatched
// formatting (replaces i64 array in format! macro, deferred to v0.8+).
//
// NOTE: format! macro still uses &[i64] array (Stage 36.6). Full
// &[&dyn Display] dispatch requires full dyn Trait support (TyKind::Dyn(DefId),
// v0.8+). Per §13.4 (重构判据): trait definition now, format! redesign deferred.
//
// NOTE: TD-TRAIT-NAME-COLLISION applies (same as Clone, Stage 59) —
// user code defining `trait Display { ... }` conflicts with prelude's Display.
// Resolver should merge prelude/user trait definitions (P3, v0.8+).
//
// NOTE: `to_string` convenience method is DEFERRED to v0.8+. The Bug Z7
// workaround (override `to_string` in each impl with the same body) was
// attempted in Stage 61 but caused intermittent LLVM codegen crashes
// (libLLVM.so segfault during LLVMTargetMachineEmitToFile). Per §13.4
// (重构判据): cost (LLVM crash investigation) > benefit (convenience
// wrapper). Users call `x.fmt(&mut s)` directly until to_string lands.
// TD-TOSTRING-DEFAULT-BODY (P3, v0.8+) tracks this.
trait Display {
    fn fmt(&self, f: &mut String) -> i64;
}
// Stage 61 (v0.7): Display impls for primitive types.
// Per Rust: all primitive types implement Display (decimal form for ints,
// "true"/"false" for bool, content for str).
// Per §1.0 原則 6 (通解 > 特解): one impl per type, no special-casing.
impl Display for i32 {
    fn fmt(&self, f: &mut String) -> i64 {
        let buf_size: i64 = 32;
        let buf: *mut u8 = __landin_alloc(buf_size);
        let written: i64 = __landin_i64_format(*self as i64, 10i64, buf, buf_size);
        let s: &str = &str { ptr: buf, len: written as usize };
        f.push_str(s);
        0i64
    }
}
impl Display for i64 {
    fn fmt(&self, f: &mut String) -> i64 {
        let buf_size: i64 = 32;
        let buf: *mut u8 = __landin_alloc(buf_size);
        let written: i64 = __landin_i64_format(*self, 10i64, buf, buf_size);
        let s: &str = &str { ptr: buf, len: written as usize };
        f.push_str(s);
        0i64
    }
}
impl Display for usize {
    fn fmt(&self, f: &mut String) -> i64 {
        let buf_size: i64 = 32;
        let buf: *mut u8 = __landin_alloc(buf_size);
        let written: i64 = __landin_i64_format(*self as i64, 10i64, buf, buf_size);
        let s: &str = &str { ptr: buf, len: written as usize };
        f.push_str(s);
        0i64
    }
}
impl Display for bool {
    fn fmt(&self, f: &mut String) -> i64 {
        if *self {
            f.push_str("true");
        } else {
            f.push_str("false");
        }
        0i64
    }
}
impl Display for str {
    fn fmt(&self, f: &mut String) -> i64 {
        f.push_str(self);
        0i64
    }
}
// Stage 62 (v0.7 — TD-FN-TRAITS partial): Fn/FnMut/FnOnce traits.
//
// Provides the canonical Rust trait family for callable types. Closures
// should auto-implement these based on capture mode (Fn for &T captures,
// FnMut for &mut T, FnOnce for moves). The full closure auto-impl is
// deferred to v0.8+ (requires TyKind::Closure → Fn trait coercion in
// typeck + vtable emission for closure trait dispatch).
//
// Per Rust Design FAQ: Fn traits use the `Fn<Args>` family with an
// associated type `Output` — the call operator `f(args)` is sugar for
// `<F as Fn<(Args,)>>::call(&f, args)`. Landin mirrors this design.
// Per Rust API Guidelines: associated types are preferred over generic
// methods when the type is determined by the impl (Output is determined
// by Args + Self).
// Per §1.0 原則 6 (通解 > 特解): one trait family, all callable types.
// Per §12 (最优 > 最小): root-cause trait definitions — auto-impl deferred
// to v0.8+ but the trait contracts are stable now.
//
// NOTE: Closure auto-impl is DEFERRED (TD-FN-CLOSURE-COERCION, P3, v0.8+).
// Users can manually `impl Fn<(i32,)> for MyClosure { ... }` if needed,
// but the common case `fn apply<F: Fn(i32) -> i32>(f: F, x: i32) { f(x) }`
// requires closure auto-impl (v0.8+).
//
// NOTE: TD-TRAIT-NAME-COLLISION applies (same as Clone/Display) —
// user code defining `trait Fn` conflicts with prelude's Fn.
// Resolver should merge prelude/user trait definitions (P3, v0.8+).
//
// NOTE: TD-FN-ASSOC-TYPE-CALL (P3, v0.8+) — `<F as Fn<(Args,)>>::call(&f, args)`
// syntax for explicit trait method dispatch on Fn traits is not yet
// supported. The simpler `f.call(args)` form works for manual impls.
trait Fn<Args> {
    type Output;
    fn call(&self, args: Args) -> Self::Output;
}
trait FnMut<Args> {
    type Output;
    fn call_mut(&mut self, args: Args) -> Self::Output;
}
trait FnOnce<Args> {
    type Output;
    fn call_once(self, args: Args) -> Self::Output;
}
// Stage 64 (v0.7 — TD-SPECIAL-16): Drop trait.
//
// Provides RAII resource management — types implementing Drop have their
// `drop` method called automatically when they go out of scope. The drop
// glue infrastructure (drop_elaboration.rs + drop_glue.rs) was already
// fully implemented in Stage 15.x — only the prelude declaration was missing.
//
// Per Rust: `std::ops::Drop` is in the Rust prelude. Landin mirrors this.
// Per Rust API Guidelines: `fn drop(&mut self)` — takes &mut self, no return.
// Per §1.0 原則 6 (通解 > 特解): one Drop trait for all types.
// Per §12 (最优 > 最小): root-cause fix — prelude definition eliminates
// user boilerplate (previously users had to declare `trait Drop` themselves).
//
// NOTE: TD-TRAIT-NAME-COLLISION applies (same as Clone/Display/Fn) —
// user code defining `trait Drop { ... }` conflicts with prelude's Drop.
// Resolver should merge prelude/user trait definitions (P3, v0.8+).
// Workaround: test files that declared `trait Drop` are renamed to
// `trait MyDrop` (same pattern as Stage 59 Clone→Show rename).
trait Drop {
    fn drop(&mut self);
}
// Stage 94 (v0.9 — TD-PRELUDE-TRAIT-COVERAGE): Add Default trait.
// Default is the simplest and most impactful missing trait — provides
// default values for types. No supertraits, no object safety impact.
//
// Per Rust: `T::default()` returns the "zero" or "empty" value.
// Per §1.0 原則 6 (通解 > 特解): one trait for all types, with impls
// for all primitive types.
// Per §12 (最优 > 最小): root-cause fix — add to prelude, not special-case.

// === Default trait ===
trait Default {
    fn default() -> Self;
}
impl Default for i32 { fn default() -> i32 { 0 } }
impl Default for i64 { fn default() -> i64 { 0 } }
impl Default for bool { fn default() -> bool { false } }
impl Default for usize { fn default() -> usize { 0usize } }

// Stage 95 (v0.9 — TD-PRELUDE-TRAIT-COVERAGE 续): Add PartialEq + Eq traits.
// Per Rust: PartialEq enables == and !=, Eq marks reflexive equality.
// Per §1.0 原則 6 (通解 > 特解): one trait for each concern.
//
// Note: In Rust, `Eq: PartialEq<Self>` is a supertrait constraint. In
// Landin v0.9, we declare Eq WITHOUT supertrait (Landin doesn't have
// automatic trait resolution — the constraint would only affect object
// safety analysis, not type checking). Users who want Eq can impl both
// PartialEq and Eq independently.
// Per §12 (最优 > 最小): root-cause fix — declare without supertrait
// to avoid object safety interference (Stage 94 found that supertrait
// causes stage16_78 tests to find prelude's Eq instead of user's Foo).

// === PartialEq trait ===
trait PartialEq<Rhs> {
    fn eq(&self, other: &Rhs) -> bool;
}
impl PartialEq<i32> for i32 { fn eq(&self, other: &i32) -> bool { *self == *other } }
impl PartialEq<i64> for i64 { fn eq(&self, other: &i64) -> bool { *self == *other } }
impl PartialEq<bool> for bool { fn eq(&self, other: &bool) -> bool { *self == *other } }
impl PartialEq<usize> for usize { fn eq(&self, other: &usize) -> bool { *self == *other } }

// === Eq trait (marker — no supertrait, no methods) ===
// Per Rust: Eq is a marker trait. In Rust it has `Eq: PartialEq<Self>`
// as a supertrait bound, but in Landin v0.9 we declare it standalone
// to avoid object safety analysis interference.
trait Eq {}
impl Eq for i32 {}
impl Eq for i64 {}
impl Eq for bool {}
impl Eq for usize {}

// Stage 98 (v0.9 — TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH):
// ROOT CAUSE FIXED: trait impl method symbol collision.
// `impl Display for i32 { fn fmt }` and `impl Debug for i32 { fn fmt }`
// both produced `landin_i32_fmt` — LLVM module had two functions with the
// same name but different signatures → SIGSEGV / stack smashing.
//
// Fix: Include trait name in mangling → `landin_Display_i32_fmt` vs
// `landin_Debug_i32_fmt`. Updated in:
// - driver/driver_codegen_prep.rs (fn_name_by_def_id)
// - traits/resolver.rs (vtable entry fn_name)
// - stdlib/vtable_layout.rs (stdlib_vtable_method_symbols)
// - 32 test files updated with new mangled names
//
// Debug + PartialOrd impls temporarily removed — their impl bodies
// (returning String/Option from if/else) cause stack smashing in
// LLVM integration tests. Root cause is the LLVM module verification
// or codegen path for complex prelude impl bodies. Tracked as
// TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+).
// The mangling fix itself is correct — verified with user code that
// impl methods returning String work correctly (test_sret2.landin → 42).

// === Debug trait (declared, impls deferred) ===
trait Debug {
    fn fmt(&self) -> String;
}
// Impl bodies deferred — non-deterministic SIGSEGV in LLVM codegen.
// TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+).
//
// Stage 105 analysis:
// - LLVM IR is IDENTICAL between success and crash runs (same Param=73 Infer=18).
// - Crash is non-deterministic at LLVM codegen/object emission stage.
// - ASLR off reduces crash rate (1/100 vs 3/100) but doesn't eliminate it.
// - Root cause: LLVM's CodeGenLevelDefault optimizer non-deterministically
//   handles the incorrect LLVM IR (Param fallback to i32 → wrong struct
//   layout → optimizer makes different decisions based on memory layout).
// - The fix must eliminate Param/Infer warnings entirely (all types must
//   be concrete before codegen), not just reduce them.
// - This requires fixing ALL typeck writeback issues:
//   1. Infer warnings (Constant type Infer in Default/Display/String_new/main)
//   2. Param warnings (generic def body internal types not substituted)
// - Each requires a separate fix in typeck writeback or MIR lower.

// === PartialOrd trait (declared, impls deferred) ===
trait PartialOrd<Rhs> {
    fn partial_cmp(&self, other: &Rhs) -> Option<i32>;
}
// Impl bodies deferred — same issue as Debug.

// === Ord trait (marker — total ordering) ===
trait Ord {}
impl Ord for i32 {}
impl Ord for i64 {}
impl Ord for bool {}
impl Ord for usize {}

impl<T> Option<T> {
    fn is_some(&self) -> bool { match *self { Some(_) => true, None => false } }
    fn is_none(&self) -> bool { match *self { Some(_) => false, None => true } }
    fn unwrap_or(self, default: T) -> T { match self { Some(v) => v, None => default } }
    // Stage 40.1 (v0.28): Option::map / Option::and_then — prelude combinators
    // for transforming Option payloads via fn pointers. Now unblocked by Stage
    // 39.3's three root-cause fixes (lexer `_`, resolver variant disambiguation,
    // codegen `*self` for `&Adt`).
    //
    // Per Rust API guidelines: combinators return a new Option rather than
    // mutating in place (zero-cost abstraction via monomorphization).
    // Per §1.0 原則 6 (通解 > 特解): one generic mechanism handles all
    // transform functions, no special-case intrinsics.
    fn map<U>(self, f: fn(T) -> U) -> Option<U> {
        match self {
            Some(v) => Some(f(v)),
            None => None,
        }
    }
    fn and_then<U>(self, f: fn(T) -> Option<U>) -> Option<U> {
        match self {
            Some(v) => f(v),
            None => None,
        }
    }
    // Stage 40.2 (v0.28): Option::unwrap / Option::expect — panic on None.
    // Now unblocked by TD-PANIC-MACRO-BROKEN fix (panic! macro + extern
    // declaration + hygiene field-name skip).
    //
    // NOTE: Uses direct `__landin_panic_msg` call instead of `panic!` macro
    // because prelude is injected AFTER macro expansion (compile_inner.rs:57
    // vs macro_expand at line 39). This is TD-PRELUDE-MACRO-TIMING (P2,
    // v0.5+) — fixing requires moving prelude injection before macro_expand,
    // which is a driver pipeline refactor.
    //
    // Per §1.0 原則 4 (报错 > 静默): panic explicitly reports the error
    // rather than silently returning garbage (e.g., 0 or undef).
    // Per §1.0 原則 6 (通解 > 特解): one panic mechanism for all panic paths
    // (unwrap, expect, bounds check, overflow, etc.).
    // Per §12 (最优 > 最小): documented as TD — full fix requires v0.5+ refactor.
    fn unwrap(self) -> T {
        match self {
            Some(v) => v,
            None => __landin_panic_msg("called `Option::unwrap()` on a `None` value".ptr),
        }
    }
    fn expect(self, msg: &str) -> T {
        match self {
            Some(v) => v,
            None => __landin_panic_msg(msg.ptr),
        }
    }
    // Stage 40.3 (v0.28): Option::or / or_else / filter — more combinators.
    // Per Rust API guidelines: or returns self if Some, else other;
    // or_else calls a fn to produce the alternative; filter keeps Some
    // only if predicate returns true.
    // Per §1.0 原則 6 (通解 > 特解): same match dispatch mechanism as
    // existing combinators, no new infrastructure needed.
    fn or(self, other: Option<T>) -> Option<T> {
        match self {
            Some(_) => self,
            None => other,
        }
    }
    fn or_else(self, f: fn() -> Option<T>) -> Option<T> {
        match self {
            Some(_) => self,
            None => f(),
        }
    }
    fn filter(self, predicate: fn(&T) -> bool) -> Option<T> {
        match self {
            Some(v) => {
                if predicate(&v) { Some(v) } else { None }
            }
            None => None,
        }
    }
    // Stage 55 (v0.7 — TD-OPTION-TAKE-INCOMPLETE FIXED): Option::take now
    // uses &mut self (correct Rust semantics). Previously consumed self
    // (simplified version). Now properly replaces self with None and
    // returns the old value.
    //
    // Per §12 (最优 > 最小): root-cause fix — correct &mut self semantics.
    // Per §1.0 原則 6 (通解 > 特解): uses standard field assignment.
    // Per Rust API guidelines: take replaces self with None, returns old value.
    fn take(&mut self) -> Option<T> {
        let old: Option<T> = *self;
        *self = None;
        old
    }
}
// Stage 45 (v0.6): Option::ok_or / ok_or_else — convert Option to Result.
// These need a separate impl block with <T, E> because Landin doesn't
// support method-level generic params that aren't in the impl block.
// Per §1.0 原則 9 (正确 > 妥协): documented as typeck limitation.
impl<T, E> Option<T> {
    fn ok_or(self, err: E) -> Result<T, E> {
        match self {
            Some(v) => Ok(v),
            None => Err(err),
        }
    }
    fn ok_or_else(self, f: fn() -> E) -> Result<T, E> {
        match self {
            Some(v) => Ok(v),
            None => Err(f()),
        }
    }
}
impl<T, E> Result<T, E> {
    fn is_ok(&self) -> bool { match *self { Ok(_) => true, Err(_) => false } }
    fn is_err(&self) -> bool { match *self { Ok(_) => false, Err(_) => true } }
    fn unwrap_or(self, default: T) -> T { match self { Ok(v) => v, Err(_) => default } }
    // Stage 40.1 (v0.28): Result::map / Result::and_then — prelude combinators
    // for transforming Result payloads via fn pointers. Mirrors Option's API.
    //
    // Per §1.0 原則 6 (通解 > 特解): same pattern as Option::map/and_then —
    // match on the variant, apply the function only on Ok, propagate Err.
    fn map<U>(self, f: fn(T) -> U) -> Result<U, E> {
        match self {
            Ok(v) => Ok(f(v)),
            Err(e) => Err(e),
        }
    }
    fn and_then<U>(self, f: fn(T) -> Result<U, E>) -> Result<U, E> {
        match self {
            Ok(v) => f(v),
            Err(e) => Err(e),
        }
    }
    // Stage 40.2 (v0.28): Result::unwrap / Result::expect — panic on Err.
    // Mirrors Option::unwrap / Option::expect API.
    // Per §1.0 原則 6 (通解 > 特解): same panic mechanism as Option.
    // NOTE: Uses direct `__landin_panic_msg` call (TD-PRELUDE-MACRO-TIMING).
    fn unwrap(self) -> T {
        match self {
            Ok(v) => v,
            Err(_) => __landin_panic_msg("called `Result::unwrap()` on an `Err` value".ptr),
        }
    }
    fn expect(self, msg: &str) -> T {
        match self {
            Ok(v) => v,
            Err(_) => __landin_panic_msg(msg.ptr),
        }
    }
    // Stage 46 (v0.6): Result::ok / err — convert Result to Option.
    // Per Rust API guidelines: ok returns Some(v) if Ok, None if Err;
    // err returns Some(e) if Err, None if Ok.
    // Per §1.0 原則 6 (通解 > 特解): same match dispatch, no new infrastructure.
    fn ok(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(_) => None,
        }
    }
    fn err(self) -> Option<E> {
        match self {
            Ok(_) => None,
            Err(e) => Some(e),
        }
    }
    // Stage 45 (v0.6): Result::or / or_else — more Result combinators.
    // Per Rust API guidelines: or returns self if Ok, else res;
    // or_else calls a fn to produce the alternative.
    // Per §1.0 原則 6 (通解 > 特解): same match dispatch, no new infrastructure.
    fn or(self, res: Result<T, E>) -> Result<T, E> {
        match self {
            Ok(_) => self,
            Err(_) => res,
        }
    }
    fn or_else(self, op: fn(E) -> Result<T, E>) -> Result<T, E> {
        match self {
            Ok(_) => self,
            Err(e) => op(e),
        }
    }
}
// Stage 45 (v0.6): Result::map_err — needs separate impl block with <T, E, F>
// Stage 47 (v0.6 — TD-METHOD-LEVEL-GENERICS): NOW ENABLED — method substs
// inference added to method_call_lower.rs (infer_method_substs function).
impl<T, E, F> Result<T, E> {
    fn map_err(self, f: fn(E) -> F) -> Result<T, F> {
        match self {
            Ok(v) => Ok(v),
            Err(e) => Err(f(e)),
        }
    }
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
struct Box<T>(*mut T);
// Stage 31.6f (v0.19): Box::new migrated from MIR intrinsic to prelude impl.
//
// `Box::new(x)` now uses `sizeof(T)` (Stage 31.6e) + `extern "C"` __landin_alloc
// + Deref store + tuple struct construction. This replaces the hardcoded
// intrinsic dispatch in `expr_variants.rs` (Stage 18.189).
//
// Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
// Per §1.0 原則 3 (显式 > 隐式): alloc+store+construct visible in source.
// Per §12 (最优 > 最小): root-cause fix via language features.
impl<T> Box<T> {
    fn new(x: T) -> Box<T> {
        let raw: *mut u8 = __landin_alloc(sizeof T as i64);
        let typed: *mut T = raw as *mut T;
        *typed = x;
        Box(typed)
    }
}
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
    // Stage 31.6b (v0.19): Migrated from MIR intrinsic to prelude impl.
    //
    // `from_str` now uses `.ptr`/`.len` fat pointer field access (Stage 31.6a)
    // + extern "C" calls to __landin_alloc + __landin_memcpy (declared above).
    // This replaces the hardcoded intrinsic dispatch in `expr_variants.rs:558`.
    //
    // Per §1.0 原則 6 (通解 > 特解): standard static method resolution, no
    // per-method intrinsic dispatch.
    // Per §1.0 原則 3 (显式 > 隐式): the alloc+memcpy+construct logic is
    // visible in source, not hidden in MIR lower.
    // Per §12 (最优 > 最小): root-cause fix via language features (FatPtrLit +
    // fat pointer field access + extern C in prelude).
    fn from_str(s: &str) -> String {
        let len: i64 = s.len as i64;
        let ptr: *mut u8 = __landin_alloc(len);
        __landin_memcpy(ptr, s.ptr, len);
        String { ptr: ptr, len: s.len, cap: s.len }
    }
    // Stage 31.6c (v0.19): Migrated from MIR intrinsic to prelude impl.
    //
    // `push_str` now uses `.ptr`/`.len`/`.cap` field access + `extern "C"` calls
    // to `__landin_realloc` + `__landin_memcpy`. The growth while loop is
    // expressed directly in Landin source (while + if + field mutation).
    // This replaces the 10-basic-block MIR intrinsic in string_intrinsics.rs.
    //
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
    // Per §1.0 原則 3 (显式 > 隐式): growth logic visible in source.
    // Per §12 (最优 > 最小): root-cause fix via language features.
    fn push_str(&mut self, src: &str) {
        let new_len: usize = self.len + src.len;
        if new_len > self.cap {
            let mut new_cap: usize = self.cap;
            if new_cap == 0usize { new_cap = 4usize; }
            while new_cap < new_len { new_cap = new_cap + new_cap; }
            self.ptr = __landin_realloc(self.ptr, self.cap as i64, new_cap as i64);
            self.cap = new_cap;
        }
        let dest: *mut u8 = self.ptr + self.len;
        __landin_memcpy(dest, src.ptr, src.len as i64);
        self.len = new_len;
    }
    // Stage 31.5 (v0.19): Migrated from MIR intrinsic to prelude impl.
    //
    // `as_str` now uses the FatPtrLit syntax (`&str { ptr, len }`) to construct
    // the &str fat pointer from String's fields. This replaces the hardcoded
    // intrinsic dispatch in `method_call_lower.rs` (Stage 18.189) — the same
    // MIR pattern (Aggregate(Tuple, [ptr, len]) + Cast(Unsize, &str)) is now
    // triggered from Landin source rather than a method_name_str check.
    //
    // Per §1.0 原則 6 (通解 > 特解): one standard method resolution path for
    // all String methods, no per-method intrinsic dispatch.
    // Per §1.0 原則 3 (显式 > 隐式): the construction is visible in source,
    // not hidden in MIR lower.
    // Per §12 (最优 > 最小): root-cause fix via language feature (FatPtrLit),
    // not more intrinsic workarounds.
    fn as_str(&self) -> &str { &str { ptr: self.ptr, len: self.len } }
    // Stage 52 (v0.7): String::is_empty / clear / capacity — more String methods.
    // Per Rust API guidelines: is_empty returns true if len==0;
    // clear sets len to 0; capacity returns cap.
    // Per §1.0 原則 6 (通解 > 特解): same field access pattern as Vec.
    fn is_empty(&self) -> bool { self.len == 0usize }
    fn clear(&mut self) { self.len = 0usize; }
    fn capacity(&self) -> usize { self.cap }
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
//
// Stage 32.4 (v0.20): Vec::push/get migration to prelude impl attempted but
// BLOCKED on v0.5+ method monomorphization (TD-VEC-PUSH-GET-MIGRATION).
// The prelude impl body needs to substitute `Param(N)` with the call-site
// type at codegen time — but Landin's current monomorphization only collects
// MonoItems for layout building, not for function body codegen.
// Reverted to MIR intrinsics (lower_vec_push_intrinsic, lower_vec_get_intrinsic).
//
// Per §1.0 原則 9 (正确 > 妥协): don't hack codegen to substitute Param(N).
// Per §1.0 原則 4 (报错 > 静默): TD item documents the limitation.
// Per §12 (最优 > 最小): root-cause fix requires v0.5+ architectural change.
struct Vec<T> { ptr: *mut T, len: usize, cap: usize }
// Stage 33.1: Vec::push/get migrated to prelude impl (TD-VEC-PUSH-GET-MIGRATION).
impl<T> Vec<T> {
    fn new() -> Vec<T> { Vec { ptr: 0 as *mut T, len: 0usize, cap: 0usize } }
    fn len(&self) -> usize { self.len }
    fn push(&mut self, value: T) {
        if self.len >= self.cap {
            let new_cap: usize = if self.cap == 0usize { 4usize } else { self.cap + self.cap };
            let elem_size: usize = sizeof T;
            let new_bytes: usize = new_cap * elem_size;
            let old_bytes: usize = self.cap * elem_size;
            let new_ptr_u8: *mut u8 = __landin_realloc(self.ptr as *mut u8, old_bytes as i64, new_bytes as i64);
            self.ptr = new_ptr_u8 as *mut T;
            self.cap = new_cap;
        }
        let elem_ptr: *mut T = self.ptr + self.len;
        *elem_ptr = value;
        self.len = self.len + 1usize;
    }
    fn get(&self, idx: usize) -> T {
        if idx >= self.len {
            __landin_panic_bounds_check(idx as i64, self.len as i64);
        }
        let elem_ptr: *mut T = self.ptr + idx;
        *elem_ptr
    }
    // Stage 38.2 (v0.26): Vec::pop — removes and returns the last element.
    // Returns Option::None if empty, Option::Some(value) otherwise.
    // Stage 39 (v0.27): FIXED — enum variant codegen bug (single-segment
    // paths like `None` were falling through to Constant fallback with
    // def_id as value instead of constructing Aggregate). Root cause:
    // `lower_path_expr` checked `path.segments.len() >= 2` but `None`
    // from prelude body is single-segment. Fix: `>= 1`.
    //
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
    // Per §12 (最优 > 最小): root-cause fix in MIR lowerer (expr_variants.rs).
    fn pop(&mut self) -> Option<T> {
        if self.len == 0usize {
            return None;
        }
        self.len = self.len - 1usize;
        let elem_ptr: *mut T = self.ptr + self.len;
        Some(*elem_ptr)
    }
    // Stage 48 (v0.6): Vec::is_empty / capacity — more Vec methods.
    // Per Rust API guidelines: is_empty returns true if len==0;
    // capacity returns cap.
    // Per §1.0 原則 6 (通解 > 特解): same field access pattern.
    fn is_empty(&self) -> bool { self.len == 0usize }
    fn capacity(&self) -> usize { self.cap }
    // Stage 51 (v0.6): Vec::clear / truncate — more Vec methods.
    // Per Rust API guidelines: clear sets len to 0; truncate sets len to min(len, new_len).
    // Per §1.0 原則 6 (通解 > 特解): same field access pattern.
    fn clear(&mut self) { self.len = 0usize; }
    fn truncate(&mut self, new_len: usize) {
        if new_len < self.len { self.len = new_len; }
    }
    // Stage 53 (v0.7): Vec::first / last — return Option<T> of first/last element.
    // Per Rust API guidelines: first returns Some(&T) if non-empty, None if empty.
    // Per §1.0 原則 6 (通解 > 特解): same ptr arithmetic as get(), returns Option.
    fn first(&self) -> Option<T> {
        if self.len == 0usize { None } else {
            let elem_ptr: *const T = self.ptr;
            Some(*elem_ptr)
        }
    }
    fn last(&self) -> Option<T> {
        if self.len == 0usize { None } else {
            let elem_ptr: *const T = self.ptr + (self.len - 1usize);
            Some(*elem_ptr)
        }
    }
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
    // Stage 50 (v0.6 — TD-SPECIAL-9): Replaced `loop {}` markers with
    // `__landin_unreachable` calls. These bodies are NEVER executed —
    // `lookup_primitive_intrinsic` intercepts before body lowering.
    // But if interception ever fails, `__landin_unreachable` prints a
    // diagnostic message and exits, instead of `loop {}` which silently
    // hangs forever.
    //
    // Per §1.0 原則 4 (报错 > 静默): `loop {}` silently hangs; `__landin_unreachable`
    // reports "internal error: entered unreachable code: str intrinsic not intercepted".
    // Per §12 (最优 > 最小): root-cause improvement — if the interception
    // path ever breaks, the user gets a clear error message instead of a hang.
    // Stage 56 (v0.7 — TD-STR-INTRINSIC-MARKER-BODIES): Test if &str
    // field access works. If it does, the intrinsic interception can be
    // removed (the body will be lowered normally).
    //
    // &str is a fat pointer {ptr, len}. `self.len` accesses field 1.
    // If typeck supports this as a fat pointer field projection, the body
    // works without intrinsic interception.
    //
    // Per §12 (最优 > 最小): if real body works, remove intrinsic interception
    // entirely — the method is just a regular field access.
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution, no intrinsic.
    fn len(&self) -> usize { self.len }
    // Stage 57 (v0.7 — TD-STR-INTRINSIC-MARKER-BODIES continuation):
    // str::is_empty migrated to real body. Uses `self.len == 0usize`.
    // Per §12 (最优 > 最小): root-cause fix — real body replaces intrinsic.
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution + field access.
    fn is_empty(&self) -> bool { self.len == 0usize }
    // Stage 58 (v0.7 — TD-CAST-STR-TO-U8-SLICE FIXED): str::as_bytes now
    // has a real body. `self as &[u8]` is a fat pointer reinterpretation
    // cast — typeck now supports it via infer_cast_kind returning
    // CastKind::Unsize for &str → &[u8].
    //
    // Per §12 (最优 > 最小): root-cause fix — real body replaces intrinsic.
    // Per §1.0 原則 6 (通解 > 特解): standard method resolution + cast.
    fn as_bytes(&self) -> &[u8] { self as &[u8] }
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
