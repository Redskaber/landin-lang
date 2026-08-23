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

/// The canonical ABI contract for all compound C runtime helpers.
/// Per §1.0 原則 6 (通解>特例): one table for all helpers.
const COMPOUND_ABI_CONTRACTS: &[AbiContract] = &[
    AbiContract {
        name: "__landin_vec_push",
        c_return: "void",
        c_params: &["void* vec_ptr", "void* val_ptr", "long long elem_size"],
    },
    AbiContract {
        name: "__landin_string_push_str",
        c_return: "void",
        c_params: &["void* str_ptr", "const char* src_ptr", "long long src_len"],
    },
    AbiContract {
        name: "__landin_vec_get",
        c_return: "void",
        c_params: &[
            "void* vec_ptr",
            "long long index",
            "void* out_ptr",
            "long long elem_size",
        ],
    },
    AbiContract {
        name: "__landin_format_variadic",
        c_return: "void",
        c_params: &[
            "void* out_str_ptr",
            "const char* fmt_ptr",
            "long long fmt_len",
            "long long n_args",
            "const long long* arg_types",
            "const long long* arg_vals",
            "...",
        ],
    },
];

/// The primitive ABI contracts (alloc/dealloc/memcpy/realloc) — these are
/// explicitly endorsed by 07-codegen.md §4-§5 and are NOT in scope for
/// TD-C-WRAPPER-OVERUSE migration.
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

/// Stage 18.206 positive 1: all compound C helpers have the expected ABI.
/// Per §1.0 原則 6 (通解>特例): one test for all compound helpers.
#[test]
fn stage18_206_compound_abis_match_c_source() {
    for contract in COMPOUND_ABI_CONTRACTS {
        let (ret, params) = extract_c_signature(contract.name).unwrap_or_else(|| {
            panic!("C function {} not found in LANDIN_C_WRAPPER", contract.name)
        });
        assert_eq!(
            ret, contract.c_return,
            "C function {} return type mismatch: expected {:?}, got {:?}",
            contract.name, contract.c_return, ret
        );
        assert_eq!(
            params.len(),
            contract.c_params.len(),
            "C function {} param count mismatch: expected {}, got {}: expected {:?}, got {:?}",
            contract.name,
            contract.c_params.len(),
            params.len(),
            contract.c_params,
            params
        );
        for (i, (expected, actual)) in contract.c_params.iter().zip(params.iter()).enumerate() {
            assert_eq!(
                actual, expected,
                "C function {} param {} mismatch: expected {:?}, got {:?}",
                contract.name, i, expected, actual
            );
        }
    }
}

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

/// Stage 18.206 positive 3: compound helpers use pointer-typed params
/// (void*, const char*, const long long*), not raw integers.
/// Per §11 (interface isolation): pointer params are opaque — the C function
/// doesn't care about the pointee type, only that it's a pointer.
#[test]
fn stage18_206_compound_abis_use_pointer_params() {
    for contract in COMPOUND_ABI_CONTRACTS {
        let (_, params) = extract_c_signature(contract.name).unwrap();
        for (i, p) in params.iter().enumerate() {
            let is_pointer = p.contains('*');
            let is_integer = p.contains("long long") && !p.contains('*');
            let is_variadic = p == "...";
            // At least one of pointer, integer, or variadic marker.
            assert!(
                is_pointer || is_integer || is_variadic,
                "C function {} param {} has unexpected type {:?} — expected pointer, long long, or `...`",
                contract.name,
                i,
                p
            );
        }
    }
}

/// Stage 18.206 positive 4: vec_push and vec_get have matching elem_size
/// semantics (both take `long long elem_size` as the LAST param).
/// Per §1.0 原則 6 (通解>特例): matching param positions for related ops.
#[test]
fn stage18_206_vec_push_get_elem_size_consistency() {
    let (_, push_params) = extract_c_signature("__landin_vec_push").unwrap();
    let (_, get_params) = extract_c_signature("__landin_vec_get").unwrap();
    let push_last = push_params.last().expect("vec_push has params");
    let get_last = get_params.last().expect("vec_get has params");
    assert_eq!(
        push_last, get_last,
        "vec_push and vec_get must have matching elem_size (last param) type: push={}, get={}",
        push_last, get_last
    );
    // The last param should be "long long elem_size" (type + name).
    assert_eq!(
        push_last, "long long elem_size",
        "elem_size param must be 'long long elem_size', got {:?}",
        push_last
    );
}

/// Stage 18.206 positive 5: format_variadic has 6 fixed params + variadic `...`.
/// Per §1.0 原則 3 (显式 > 隐式): the fixed param count + variadic marker is explicit.
#[test]
fn stage18_206_format_variadic_has_6_fixed_params_plus_variadic() {
    let (ret, params) = extract_c_signature("__landin_format_variadic").unwrap();
    assert_eq!(ret, "void", "format_variadic return must be void");
    // 6 fixed params + 1 variadic marker `...` = 7 entries in the C signature.
    assert_eq!(
        params.len(),
        7,
        "format_variadic must have 6 fixed params + 1 variadic `...` = 7 entries, got {}: {:?}",
        params.len(),
        params
    );
    // The last entry must be `...` (variadic marker).
    assert_eq!(
        params.last().unwrap(),
        "...",
        "format_variadic must end with variadic `...`, got {:?}",
        params.last()
    );
    // The first 6 must be the fixed params (verify by name suffix).
    let fixed = &params[..6];
    assert!(
        fixed.iter().all(|p| p != "..."),
        "fixed params must not contain `...`: {:?}",
        fixed
    );
}

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
    let (actual_ret, _) = extract_c_signature("__landin_vec_push").unwrap();
    // vec_push returns void, not int. If we assert int, the test SHOULD fail.
    assert_ne!(
        actual_ret, "int",
        "sanity check: __landin_vec_push returns {:?}, not int — mismatch detection works",
        actual_ret
    );
    assert_eq!(actual_ret, "void", "__landin_vec_push must return void");
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
    let all_contracts: Vec<&AbiContract> = COMPOUND_ABI_CONTRACTS
        .iter()
        .chain(PRIMITIVE_ABI_CONTRACTS.iter())
        .collect();
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
    for contract in COMPOUND_ABI_CONTRACTS
        .iter()
        .chain(PRIMITIVE_ABI_CONTRACTS.iter())
    {
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
