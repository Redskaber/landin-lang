//! Stage 18.157: Landin runtime C wrapper source.
//!
//! Landin codegen emits `landin_main` (not `main`), plus runtime stubs
//! (`__landin_println`, `__landin_panic_*`, etc.). This module provides
//! the C source that:
//! 1. Provides a `main()` that calls `landin_main()`
//! 2. Implements all `__landin_*` runtime symbols
//!
//! Both `landin-stage0` (src/bin/main.rs) and `landinc` (src/bin/landinc.rs)
//! use this shared constant to avoid duplication (DRY).
//!
//! Per §1.0 原則 6 (通解>特例): one runtime source for all build paths.
//! Per §13.4 J2 (单一职责): this module owns the C runtime definition.
//! Per §10: `LANDIN_C_WRAPPER` follows `<NOUN>_<NOUN>` constant pattern.

/// The C wrapper source for linking Landin executables.
///
/// This is written to a temp `.c` file and compiled + linked with the
/// Landin-generated object file via `cc -fno-pie -no-pie <wrapper.c> <obj.o> -o <exe> -lm`.
///
/// # Runtime stubs provided
///
/// - `main()` — calls `landin_main()`, returns exit code
/// - `__landin_panic_overflow(op, lhs, rhs)` — arithmetic overflow panic
/// - `__landin_panic_bounds_check(index, len)` — bounds check panic
/// - `__landin_panic_div_by_zero()` — division by zero panic
/// - `__landin_eprintf(fmt, ...)` — variadic stderr print (legacy)
/// - `__landin_str_eq(a, a_len, b, b_len)` — string content comparison
/// - `__landin_println(fmt, ...)` — stdout print + newline
/// - `__landin_print(fmt, ...)` — stdout print
/// - `__landin_eprintln(fmt, ...)` — stderr print + newline
/// - `__landin_eprint(fmt, ...)` — stderr print
/// - `__landin_assert(cond)` — assertion check
/// - `__landin_panic_msg(msg)` — panic with message
/// - `__landin_alloc(size)` — heap allocation (wraps malloc, panics on OOM)
/// - `__landin_dealloc(ptr)` — heap deallocation (wraps free, NULL-safe)
///
/// Stage 18.157: Extracted from `src/bin/main.rs` (Stage 13.8/13.10/13.13)
/// and `src/bin/landinc.rs` (Stage 18.156) to eliminate duplication.
pub const LANDIN_C_WRAPPER: &str = r#"#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
extern int landin_main(void);
/* Runtime stubs — codegen declares these as extern */
void __landin_panic_overflow(int op, int lhs, int rhs) {
    fprintf(stderr, "panic: arithmetic overflow (op=%d lhs=%d rhs=%d)\n", op, lhs, rhs);
    exit(1);
}
void __landin_panic_bounds_check(long long index, long long len) {
    fprintf(stderr, "panic: index out of bounds (index=%lld len=%lld)\n", index, len);
    exit(1);
}
void __landin_panic_div_by_zero(void) {
    fprintf(stderr, "panic: divide by zero\n");
    exit(1);
}
/* Stage 13.14/18.27: eprint!/eprintln! helpers.
   Stage 18.27: Replaced the old single-arg __landin_eprint and the
   variadic __landin_eprintf with unified variadic __landin_eprint and
   __landin_eprintln stubs (defined below, before main()).
   The old helpers were:
     void __landin_eprint(const char* s)  — single-arg, hardcoded "%s"
     void __landin_eprintf(const char* fmt, ...) — variadic, to stderr
   The new stubs are:
     int __landin_eprint(const char* fmt, ...) — variadic, to stderr
     int __landin_eprintln(const char* fmt, ...) — variadic + newline, to stderr
   Per §1.0 原則 6 "通用 > 特解": unified variadic interface. */
/* Stage 18.27: Keep __landin_eprintf for backward compat — emit_printf_call
   still references it for the stderr=true path. Will be removed in Phase 3
   when Println variant is removed. */
void __landin_eprintf(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    vfprintf(stderr, fmt, args);
    va_end(args);
}
/* Stage 14.69: String equality comparison — content comparison via memcmp.
   Codegen calls this for `==` and `!=` on &str (fat pointers {ptr, len}).
   Without this, string comparison was bitwise (pointer + length), which
   only worked for deduplicated string globals (same literal in same scope).
   For different allocations of the same content (e.g., function parameter
   vs. literal in function body), bitwise comparison returned false.
   Per api-naming-standard.md §8.1: __landin_<noun>_<verb> pattern. */
int __landin_str_eq(const char* a, long long a_len, const char* b, long long b_len) {
    if (a_len != b_len) return 0;
    if (a == b) return 1;  /* same pointer → definitely equal */
    /* Compare contents byte by byte (memcmp semantics) */
    for (long long i = 0; i < a_len; i++) {
        if (a[i] != b[i]) return 0;
    }
    return 1;
}
/* Stage 18.27: __landin_println / __landin_print / __landin_eprintln /
   __landin_eprint stubs. These are needed because MIR lowering creates
   `store ptr @__landin_println` (function pointer assignment) which
   references the symbol. The actual Call is intercepted by
   codegen_print_call (which emits printf directly), so these stubs are
   never actually called. They exist only to satisfy the linker.
   Per §1.0 原則 6 "通用 > 特解": one set of stubs for all 4 functions.
   Per api-naming-standard.md §8.1: __landin_<verb> pattern. */
int __landin_println(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int ret = vprintf(fmt, args);
    va_end(args);
    printf("\n");
    return ret;
}
int __landin_print(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int ret = vprintf(fmt, args);
    va_end(args);
    return ret;
}
int __landin_eprintln(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int ret = vfprintf(stderr, fmt, args);
    va_end(args);
    fprintf(stderr, "\n");
    return ret;
}
int __landin_eprint(const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    int ret = vfprintf(stderr, fmt, args);
    va_end(args);
    return ret;
}
/* Stage 18.29: Non-print built-in macro runtime stubs.
   assert! → __landin_assert(cond) — panics if cond is false
   panic! → __landin_panic_msg(msg) — prints message and exits
   Per §1.0 原則 6 "通用 > 特解": unified __landin_ runtime interface. */
void __landin_assert(int cond) {
    if (!cond) {
        fprintf(stderr, "panic: assertion failed\n");
        exit(1);
    }
}
void __landin_panic_msg(const char* msg) {
    fprintf(stderr, "panic: %s\n", msg);
    exit(1);
}
/* Stage 18.178 (TD-HEAP-ALLOC): Heap allocation runtime stubs.
   __landin_alloc(size) → malloc(size), panics on OOM
   __landin_dealloc(ptr) → free(ptr)
   These wrap libc malloc/free with OOM safety. Codegen declares them
   as extern and emits `call ... @__landin_alloc(...)` for Box::new(x)
   and `call void @__landin_dealloc(...)` for Box drop glue.

   Per §1.0 原則 6 (通解>特例): one allocation interface for all heap types
     (Box/Vec/String/Rc/Arc will all funnel through __landin_alloc).
   Per §1.0 原則 4 (报错>静默): OOM must panic, not return NULL.
   Per api-naming-standard.md §8.1: __landin_<verb>_<noun> pattern. */
void* __landin_alloc(long long size) {
    void* ptr = malloc((size_t)size);
    if (ptr == 0) {
        fprintf(stderr, "panic: memory allocation failed (size=%lld)\n", size);
        exit(1);
    }
    return ptr;
}
void __landin_dealloc(void* ptr) {
    if (ptr == 0) return;  /* free(NULL) is a no-op per C standard */
    free(ptr);
}
int main(void) {
    /* Stage 13.13: println! output is emitted inline within landin_main()
       via StatementKind::Println → printf("%s", <msg_global>).
       Stage 13.14: eprintln! output routes to __landin_eprint helper.
       No pre-main helper call needed.
       Stage 13.22: codegen always emits `define i32 @landin_main(...)` —
       when `fn main()` has no return type, codegen emits `ret i32 0`
       (verified by --emit-llvm-ir). The C wrapper declaration
       `extern int landin_main(void)` is therefore always correct —
       no UB, no ABI mismatch. The earlier "void landin_main" comment
       was inaccurate; codegen has never emitted a void landin_main.
       Stage 14.16 (GAP-20): comment corrected to reflect actual behavior. */
    int ret = landin_main();
    return ret;
}
"#;

#[cfg(test)]
mod tests {
    use super::LANDIN_C_WRAPPER;

    /// Stage 18.157 positive 1: C wrapper contains all required runtime stubs.
    #[test]
    fn stage18_157_c_wrapper_contains_all_stubs() {
        // Each __landin_ runtime symbol must be defined.
        let required = [
            "__landin_panic_overflow",
            "__landin_panic_bounds_check",
            "__landin_panic_div_by_zero",
            "__landin_eprintf",
            "__landin_str_eq",
            "__landin_println",
            "__landin_print",
            "__landin_eprintln",
            "__landin_eprint",
            "__landin_assert",
            "__landin_panic_msg",
            // Stage 18.178 (TD-HEAP-ALLOC): heap allocation stubs.
            "__landin_alloc",
            "__landin_dealloc",
        ];
        for sym in &required {
            assert!(
                LANDIN_C_WRAPPER.contains(sym),
                "C wrapper missing runtime stub: {}",
                sym
            );
        }
    }

    /// Stage 18.157 positive 2: C wrapper provides main() calling landin_main().
    #[test]
    fn stage18_157_c_wrapper_has_main_entry() {
        assert!(
            LANDIN_C_WRAPPER.contains("extern int landin_main(void);"),
            "must declare landin_main as extern"
        );
        assert!(
            LANDIN_C_WRAPPER.contains("int main(void)"),
            "must define main() entry point"
        );
        assert!(
            LANDIN_C_WRAPPER.contains("landin_main();"),
            "main() must call landin_main()"
        );
    }

    /// Stage 18.157 positive 3: C wrapper includes required C headers.
    #[test]
    fn stage18_157_c_wrapper_includes_headers() {
        assert!(LANDIN_C_WRAPPER.contains("#include <stdio.h>"));
        assert!(LANDIN_C_WRAPPER.contains("#include <stdlib.h>"));
        assert!(LANDIN_C_WRAPPER.contains("#include <stdarg.h>"));
    }

    /// Stage 18.178 positive 1: C wrapper defines __landin_alloc with OOM
    /// panic safety. Per §2 原則 4 (报错>静默): OOM must panic, not return NULL.
    #[test]
    fn stage18_178_c_wrapper_has_alloc_with_oom_panic() {
        // Must call malloc with size_t cast (Landin passes i64).
        assert!(
            LANDIN_C_WRAPPER.contains("void* __landin_alloc(long long size)"),
            "must declare __landin_alloc with i64 size param"
        );
        assert!(
            LANDIN_C_WRAPPER.contains("malloc((size_t)size)"),
            "must call malloc with size_t cast"
        );
        // Per §2 原則 4: OOM must panic, not silently return NULL.
        assert!(
            LANDIN_C_WRAPPER.contains("if (ptr == 0)"),
            "must check for NULL (OOM) and panic"
        );
        assert!(
            LANDIN_C_WRAPPER.contains("memory allocation failed"),
            "must print panic message on OOM"
        );
    }

    /// Stage 18.178 positive 2: C wrapper defines __landin_dealloc as NULL-safe
    /// free wrapper. free(NULL) is a no-op per C standard — we follow that.
    #[test]
    fn stage18_178_c_wrapper_has_dealloc_null_safe() {
        assert!(
            LANDIN_C_WRAPPER.contains("void __landin_dealloc(void* ptr)"),
            "must declare __landin_dealloc with void* param"
        );
        assert!(
            LANDIN_C_WRAPPER.contains("free(ptr)"),
            "must call free on the pointer"
        );
        // NULL safety: free(NULL) is well-defined no-op in C, but we make it
        // explicit for clarity and to match Rust's Box::from_raw(NULL) safety.
        assert!(
            LANDIN_C_WRAPPER.contains("if (ptr == 0) return;"),
            "must be NULL-safe (early return on NULL)"
        );
    }
}
