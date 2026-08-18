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
/// - `__landin_realloc(ptr, old, new)` — heap reallocation (wraps realloc, panics on OOM)
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
/* Stage 18.185 (TD-STRING-INTRINSICS): Memory copy for String::from_str.
   __landin_memcpy(dst, src, n) → copies n bytes from src to dst.
   Used by String::from_str to copy &str bytes to heap-allocated buffer.
   Per §1.0 原則 6 (通解>特例): one memcpy for all byte copy operations. */
void __landin_memcpy(void* dst, const void* src, long long n) {
    char* d = (char*)dst;
    const char* s = (const char*)src;
    for (long long i = 0; i < n; i++) {
        d[i] = s[i];
    }
}
/* Stage 18.194: Heap reallocation for Vec/String growth.
   __landin_realloc(ptr, old_size, new_size) → realloc(ptr, new_size), panics on OOM.
   Wraps libc realloc which handles in-place extension when possible.
   Per §1.0 原則 6 (通解>特例): one realloc for all heap growth operations.
   Per §1.0 原則 4 (报错>静默): OOM must panic, not return NULL. */
void* __landin_realloc(void* ptr, long long old_size, long long new_size) {
    void* new_ptr = realloc(ptr, (size_t)new_size);
    if (new_ptr == 0) {
        fprintf(stderr, "panic: memory reallocation failed (old=%lld new=%lld)\n", old_size, new_size);
        exit(1);
    }
    return new_ptr;
}
/* Stage 18.197 (TD-VEC-PUSH): Vec push helper.
   __landin_vec_push(vec_ptr, val_ptr, elem_size) → grows if needed, stores val, increments len.
   vec_ptr points to the Vec struct { ptr: *mut T, len: i64, cap: i64 }.
   val_ptr points to the value to push.
   Per §1.0 原則 6 (通解>特例): one function for all Vec<T> types. */
void __landin_vec_push(void* vec_ptr, void* val_ptr, long long elem_size) {
    void** ptr_field = (void**)vec_ptr;           /* offset 0: *mut T */
    long long* len_field = (long long*)((char*)vec_ptr + 8);  /* offset 8: i64 len */
    long long* cap_field = (long long*)((char*)vec_ptr + 16); /* offset 16: i64 cap */
    long long len = *len_field;
    long long cap = *cap_field;
    if (len >= cap) {
        long long new_cap = (cap == 0) ? 4 : cap * 2;
        long long new_bytes = new_cap * elem_size;
        void* new_ptr = (cap == 0)
            ? malloc((size_t)new_bytes)
            : realloc(*ptr_field, (size_t)new_bytes);
        if (new_ptr == 0) {
            fprintf(stderr, "panic: vec push grow failed (old_cap=%lld new_cap=%lld)\n", cap, new_cap);
            exit(1);
        }
        *ptr_field = new_ptr;
        *cap_field = new_cap;
    }
    /* Store val at ptr[len] */
    char* dest = (char*)(*ptr_field) + (len * elem_size);
    char* src = (char*)val_ptr;
    for (long long i = 0; i < elem_size; i++) {
        dest[i] = src[i];
    }
    /* Increment len */
    *len_field = len + 1;
}
/* Stage 18.198 (TD-STRING-INTRINSICS): String::push_str helper.
   __landin_string_push_str(str_ptr, src_ptr, src_len) → appends src to String.
   str_ptr points to String { ptr: *mut u8, len: i64, cap: i64 }.
   src_ptr/src_len describe the &str to append.
   Grows capacity if needed, copies bytes, increments len.
   Per §1.0 原則 6 (通解>特例): one function for all String::push_str calls. */
void __landin_string_push_str(void* str_ptr, const char* src_ptr, long long src_len) {
    void** ptr_field = (void**)str_ptr;           /* offset 0: *mut u8 */
    long long* len_field = (long long*)((char*)str_ptr + 8);  /* offset 8: i64 len */
    long long* cap_field = (long long*)((char*)str_ptr + 16); /* offset 16: i64 cap */
    long long len = *len_field;
    long long cap = *cap_field;
    long long new_len = len + src_len;
    /* Grow if needed */
    if (new_len > cap) {
        long long new_cap = (cap == 0) ? 4 : cap;
        while (new_cap < new_len) new_cap *= 2;
        long long new_bytes = new_cap;
        void* new_ptr = (cap == 0)
            ? malloc((size_t)new_bytes)
            : realloc(*ptr_field, (size_t)new_bytes);
        if (new_ptr == 0) {
            fprintf(stderr, "panic: string push_str grow failed (old_cap=%lld new_cap=%lld)\n", cap, new_cap);
            exit(1);
        }
        *ptr_field = new_ptr;
        *cap_field = new_cap;
    }
    /* Copy src bytes to ptr[len] */
    char* dest = (char*)(*ptr_field) + len;
    for (long long i = 0; i < src_len; i++) {
        dest[i] = src_ptr[i];
    }
    /* Update len */
    *len_field = new_len;
}
/* Stage 18.200: Vec::get helper.
   __landin_vec_get(vec_ptr, index, out_ptr, elem_size) → copies element at index to out_ptr.
   Panics if index >= len.
   Per §1.0 原則 6 (通解>特例): one function for all Vec<T> types.
   Per §1.0 原則 4 (报错>静默): OOB panics. */
void __landin_vec_get(void* vec_ptr, long long index, void* out_ptr, long long elem_size) {
    void** ptr_field = (void**)vec_ptr;
    long long* len_field = (long long*)((char*)vec_ptr + 8);
    long long len = *len_field;
    if (index < 0 || index >= len) {
        fprintf(stderr, "panic: vec get index out of bounds (index=%lld len=%lld)\n", index, len);
        exit(1);
    }
    char* src = (char*)(*ptr_field) + (index * elem_size);
    char* dst = (char*)out_ptr;
    for (long long i = 0; i < elem_size; i++) {
        dst[i] = src[i];
    }
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
            // Stage 18.185 (TD-STRING-INTRINSICS): memcpy stub.
            "__landin_memcpy",
            // Stage 18.194: realloc stub for Vec/String growth.
            "__landin_realloc",
            // Stage 18.197: Vec push helper.
            "__landin_vec_push",
            // Stage 18.198: String::push_str helper.
            "__landin_string_push_str",
            // Stage 18.200: Vec::get helper.
            "__landin_vec_get",
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
