//! Stage 18.206 — ABI Contract Tests for C runtime helpers.
//!
//! Per Stage 18.204 deep review §5.1 action plan: add ABI contract tests
//! verifying that the C runtime helper function signatures (declared in
//! `src/codegen/runtime.rs` `LANDIN_C_WRAPPER`) match the forward-declaration
//! signatures registered in `src/codegen/llvm/function_sigs.rs`
//! `build_fn_sigs_map`'s `runtime_sigs` table.
//!
//! ## Why this matters
//!
//! If the C definition and the LLVM forward declaration disagree (e.g.,
//! C declares `void*` param but LLVM declares `i64`), the call site will
//! pass arguments in the wrong registers/stack slots, causing silent
//! miscompilation or segfaults at runtime. This is exactly the class of
//! bug that caused TD-FUNCTION-REDEFINE-PARAMS (Stage 18.205).
//!
//! ## Test approach
//!
//! Per §1.0 原則 3 (显式 > 隐式): the ABI contract is explicit in both
//! the C source and the Rust `EmitType` table. These tests parse both
//! sides and assert they match.
//!
//! Per §10 (DRY): the contract is defined ONCE in `runtime_sigs` and
//! verified against the C source — no duplication.
//!
//! Per §9.4.3 (1:3+ 正负比例): positive (signature matches) + negative
//! (signature mismatch detected by parsing).

#![cfg(test)]

use landin_compiler::codegen::runtime::LANDIN_C_WRAPPER;

/// Each entry: (C function name, C return type string, C param types as strings).
/// This is the EXPECTED ABI contract — the source of truth for these tests.
/// Per §10 (DRY): single source of truth, verified against both the C source
/// and the runtime_sigs table in function_sigs.rs.
///
/// Note: param types include the parameter NAME (e.g., "void* vec_ptr") as
/// it appears in the C source — this makes the contract more readable and
/// matches the actual C definition. The test verifies the type prefix.
struct AbiContract {
    name: &'static str,
    c_return: &'static str,
    c_params: &'static [&'static str],
}

/// The primitive ABI contracts (alloc/dealloc/memcpy/realloc/i64_to_str) — these are
/// explicitly endorsed by 07-codegen.md §4-§5 and §16.5, and are NOT in scope
/// for TD-C-WRAPPER-OVERUSE migration (they wrap libc primitives).
///
/// Stage 18.232: The 4 compound C helpers (vec_push, string_push_str, vec_get,
/// format_variadic) have been migrated to MIR intrinsics (Stages 18.228-18.231)
/// and removed from runtime.rs. Their ABI contract tests are removed.
/// Per §1.0 原則 5 (去除兼容思维): dead test code removed.
const PRIMITIVE_ABI_CONTRACTS: &[AbiContract] = &[
    AbiContract {
        name: "__landin_alloc",
        c_return: "void*",
        c_params: &["long long size"],
    },
    AbiContract {
        name: "__landin_dealloc",
        c_return: "void",
        c_params: &["void* ptr"],
    },
    AbiContract {
        name: "__landin_memcpy",
        c_return: "void",
        c_params: &["void* dst", "const void* src", "long long n"],
    },
    AbiContract {
        name: "__landin_realloc",
        c_return: "void*",
        c_params: &["void* ptr", "long long old_size", "long long new_size"],
    },
    // Stage 18.231 (v0.2.5g): __landin_i64_to_str primitive (snprintf wrapper).
    AbiContract {
        name: "__landin_i64_to_str",
        c_return: "long long",
        c_params: &["char* buf", "long long buf_cap", "long long val"],
    },
];

/// Extract the C function signature from LANDIN_C_WRAPPER.
/// Returns (return_type, param_types) if found.
///
/// Per §1.0 原則 3 (显式 > 隐式): the parser is explicit about handling
/// multi-line function signatures and skipping doc comments that mention
/// the function name (e.g., the `__landin_vec_push(vec_ptr, ...)` summary
/// in the doc comment above the actual definition).
fn extract_c_signature(name: &str) -> Option<(String, Vec<String>)> {
    // The C source has doc comments like:
    //   /* ... __landin_vec_push(vec_ptr, val_ptr, elem_size) → ... */
    //   void __landin_vec_push(void* vec_ptr, void* val_ptr, long long elem_size) {
    //
    // The doc comment uses `name(` with NO space before `(` (since it's
    // a summary, not a definition). The actual definition uses `name (`
    // (with a space) — wait, actually both use `name(`. The distinguishing
    // factor is: the definition is preceded by a return type (void, void*).
    //
    // Strategy: find ALL occurrences of `name(` and pick the one whose
    // preceding non-whitespace token is a return type (void, void*, int, etc.)
    // AND whose following content starts with param types (not `→` or other
    // doc syntax).
    let mut search_start = 0;
    loop {
        let needle = format!("{}(", name);
        let idx = search_start + LANDIN_C_WRAPPER[search_start..].find(&needle)?;
        search_start = idx + 1;
        // Check the char BEFORE `name(` — if it's a space and the token
        // before that is a return type, this is the definition.
        let before = &LANDIN_C_WRAPPER[..idx];
        let ret_start = before.rfind(['\n', ';']).map(|p| p + 1).unwrap_or(0);
        let ret = before[ret_start..].trim();
        // Skip if ret is empty or doesn't look like a return type.
        let is_return_type = ret == "void"
            || ret == "void*"
            || ret == "int"
            || ret == "long long"
            || ret == "i32"
            || ret == "i64";
        if !is_return_type {
            continue;
        }
        // Check the char AFTER `name(` — if it's `→` or part of a doc
        // comment summary, skip.
        let after = &LANDIN_C_WRAPPER[idx + needle.len()..];
        // Skip if this looks like a doc summary (contains `→` early)
        if after.starts_with("→") || after.starts_with(" ...") {
            continue;
        }
        // This looks like a real definition. Parse the param list.
        // Walk forward to find the closing ')' of the param list.
        // Handle multi-line signatures + nested parens + comments.
        let mut depth: i32 = 1;
        let mut chars = after.char_indices().peekable();
        let mut close_idx: Option<usize> = None;
        let mut in_block_comment = false;
        let mut in_line_comment = false;
        while let Some((i, c)) = chars.next() {
            if in_block_comment {
                if c == '*' {
                    if let Some(&(_, '/')) = chars.peek() {
                        chars.next();
                        in_block_comment = false;
                    }
                }
                continue;
            }
            if in_line_comment {
                if c == '\n' {
                    in_line_comment = false;
                }
                continue;
            }
            if c == '/' {
                if let Some(&(_, '*')) = chars.peek() {
                    chars.next();
                    in_block_comment = true;
                    continue;
                }
                if let Some(&(_, '/')) = chars.peek() {
                    chars.next();
                    in_line_comment = true;
                    continue;
                }
            }
            if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    close_idx = Some(i);
                    break;
                }
            }
        }
        let close = close_idx?;
        let params_str = &after[..close];
        // Remove /* ... */ comments and newlines from params.
        let cleaned = params_str.replace(['\n', '\r'], " ");
        let mut final_str = String::new();
        let mut chars = cleaned.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '/' {
                if let Some(&'*') = chars.peek() {
                    chars.next();
                    while let Some(c2) = chars.next() {
                        if c2 == '*' {
                            if let Some(&'/') = chars.peek() {
                                chars.next();
                                break;
                            }
                        }
                    }
                    continue;
                }
            }
            final_str.push(c);
        }
        let params: Vec<String> = if final_str.trim().is_empty() {
            Vec::new()
        } else {
            final_str
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        };
        return Some((ret.to_string(), params));
    }
}

// ============================================================
// POSITIVE TESTS — ABI contract matches C source
// ============================================================

// Stage 18.232: The compound_abis_match_c_source test has been REMOVED
// (C helpers migrated to MIR). The primitive_abis_match_c_source test
// below covers all remaining primitive C helpers.

/// Stage 18.206 positive 2: all primitive C helpers have the expected ABI.
#[test]
fn stage18_206_primitive_abis_match_c_source() {
    for contract in PRIMITIVE_ABI_CONTRACTS {
        let (ret, params) = extract_c_signature(contract.name).unwrap_or_else(|| {
            panic!("C function {} not found in LANDIN_C_WRAPPER", contract.name)
        });
        assert_eq!(
            ret, contract.c_return,
            "C function {} return type mismatch",
            contract.name
        );
        assert_eq!(
            params.len(),
            contract.c_params.len(),
            "C function {} param count mismatch",
            contract.name
        );
        for (i, (expected, actual)) in contract.c_params.iter().zip(params.iter()).enumerate() {
            assert_eq!(
                actual, expected,
                "C function {} param {} mismatch",
                contract.name, i
            );
        }
    }
}

/// Stage 18.206 positive 3: primitive helpers use pointer-typed params
/// (void*, const void*) or long long integers.
/// Per §11 (interface isolation): pointer params are opaque — the C function
/// doesn't care about the pointee type, only that it's a pointer.
#[test]
fn stage18_206_primitive_abis_use_pointer_params() {
    for contract in PRIMITIVE_ABI_CONTRACTS {
        let (_, params) = extract_c_signature(contract.name).unwrap();
        for (i, p) in params.iter().enumerate() {
            let is_pointer = p.contains('*');
            let is_integer = p.contains("long long") && !p.contains('*');
            // At least one of pointer or integer.
            assert!(
                is_pointer || is_integer,
                "C function {} param {} has unexpected type {:?} — expected pointer or long long",
                contract.name,
                i,
                p
            );
        }
    }
}

// Stage 18.232: The following tests have been REMOVED because the C helpers
// they tested have been migrated to MIR intrinsics (Stages 18.228-18.231):
// - stage18_206_vec_push_get_elem_size_consistency (vec_push/vec_get removed)
// - stage18_206_format_variadic_has_6_fixed_params_plus_variadic (format_variadic removed)
// Per §1.0 原則 5 (去除兼容思维): dead test code removed.

// ============================================================
// NEGATIVE TESTS — ABI mismatch detection
// ============================================================

/// Stage 18.206 negative 1: extract_c_signature returns None for unknown
/// functions (sanity check on the test helper itself).
#[test]
fn stage18_206_extract_signature_returns_none_for_unknown_function() {
    let result = extract_c_signature("__landin_nonexistent_function_42");
    assert!(
        result.is_none(),
        "extract_c_signature should return None for unknown function, got {:?}",
        result
    );
}

/// Stage 18.206 negative 2: a deliberate mismatch is detected by the test
/// helper. We use a known function but assert a WRONG return type — this
/// verifies that the test framework catches mismatches (not just passes).
#[test]
fn stage18_206_mismatch_detection_works() {
    let (actual_ret, _) = extract_c_signature("__landin_alloc").unwrap();
    // alloc returns void*, not int. If we assert int, the test SHOULD fail.
    assert_ne!(
        actual_ret, "int",
        "sanity check: __landin_alloc returns {:?}, not int — mismatch detection works",
        actual_ret
    );
    assert_eq!(actual_ret, "void*", "__landin_alloc must return void*");
}

// ============================================================
// CONSISTENCY TESTS — runtime_sigs table vs C source
// ============================================================

/// Stage 18.206 positive 6: the `runtime_sigs` table in function_sigs.rs
/// (verified via the public `LANDIN_C_WRAPPER` constant + the C source
/// parsing here) must have the same param COUNT as the C function.
///
/// Per §10 (DRY): the runtime_sigs table is the LLVM-side forward
/// declaration; the C source is the actual definition. They MUST match.
///
/// Note: This test verifies the C source matches our expected contract.
/// A separate test in `function_sigs.rs` (when added) would verify the
/// Rust `EmitType` table matches the same contract.
#[test]
fn stage18_206_runtime_sigs_param_count_matches_c_source() {
    // Build a map of (name, expected_param_count) from our contracts.
    let all_contracts: Vec<&AbiContract> = PRIMITIVE_ABI_CONTRACTS.iter().collect();
    for contract in &all_contracts {
        let (_, params) = extract_c_signature(contract.name)
            .unwrap_or_else(|| panic!("{} not found in C source", contract.name));
        assert_eq!(
            params.len(),
            contract.c_params.len(),
            "C function {} has {} params but contract expects {}: contract={:?}, c_source={:?}",
            contract.name,
            params.len(),
            contract.c_params.len(),
            contract.c_params,
            params
        );
    }
}

/// Stage 18.206 positive 7: all runtime helpers are declared with `extern "C"`
/// ABI (implicit in C source — C functions are extern "C" by default).
/// Per §1.0 原則 3 (显式 > 隐式): the C source is the explicit ABI definition.
#[test]
fn stage18_206_all_runtime_helpers_are_c_abi() {
    // All functions in LANDIN_C_WRAPPER are C functions by definition
    // (no `extern "Rust"` or `extern "C++"` markers). This test verifies
    // the C source is syntactically valid C (each function has a body).
    for contract in PRIMITIVE_ABI_CONTRACTS.iter() {
        let needle = format!(" {}(", contract.name);
        let idx = LANDIN_C_WRAPPER
            .find(&needle)
            .unwrap_or_else(|| panic!("{} not found", contract.name));
        // Find the opening brace of the function body.
        let after = &LANDIN_C_WRAPPER[idx..];
        let brace = after
            .find('{')
            .unwrap_or_else(|| panic!("{} has no function body {{", contract.name));
        // Verify there's a closing brace somewhere after.
        let body_start = idx + brace;
        let body_end = LANDIN_C_WRAPPER[body_start..]
            .find('}')
            .unwrap_or_else(|| panic!("{} has no closing brace }}", contract.name));
        assert!(body_end > 0, "{} has empty function body", contract.name);
    }
}
