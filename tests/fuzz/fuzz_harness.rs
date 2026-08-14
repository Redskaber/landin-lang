//! Stage 18.85: Lightweight fuzz harness for the Landin compiler.
//!
//! Generates random (syntactically valid) Landin source code and verifies
//! the compiler does not panic or crash. Errors are OK — crashes are not.
//!
//! Per §1.0 原則 4 "报错 > 静默": the compiler must report errors, not crash.
//! Per §1.0 原則 6 "通用 > 特例": one generator covers all statement types.
//!
//! This is NOT cargo-fuzz (which requires nightly + no_std). It's a simple
//! deterministic PRNG-based generator that produces valid-ish Landin code.

use landin_compiler::compile;

/// Simple deterministic PRNG (xorshift64).
fn xorshift64(state: &mut u64) -> u64 {
    let mut x = *state;
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

/// Generate a random Landin expression string.
fn gen_expr(state: &mut u64, depth: u32) -> String {
    if depth >= 3 {
        // Base case: return a simple literal
        return match xorshift64(state) % 5 {
            0 => "42".to_string(),
            1 => "true".to_string(),
            2 => "false".to_string(),
            3 => "0".to_string(),
            _ => "'x'".to_string(),
        };
    }
    match xorshift64(state) % 6 {
        0 => format!("{}", xorshift64(state) % 1000),
        1 => match xorshift64(state) % 2 {
            0 => "true".to_string(),
            _ => "false".to_string(),
        },
        2 => format!(
            "{} + {}",
            gen_expr(state, depth + 1),
            gen_expr(state, depth + 1)
        ),
        3 => format!(
            "{} - {}",
            gen_expr(state, depth + 1),
            gen_expr(state, depth + 1)
        ),
        4 => format!(
            "if true {{ {} }} else {{ {} }}",
            gen_expr(state, depth + 1),
            gen_expr(state, depth + 1)
        ),
        _ => format!("({})", gen_expr(state, depth + 1)),
    }
}

/// Generate a random Landin statement string.
fn gen_stmt(state: &mut u64, depth: u32) -> String {
    if depth >= 4 {
        return format!("let _x = {};", gen_expr(state, 0));
    }
    match xorshift64(state) % 4 {
        0 => format!("let _v{} = {};", xorshift64(state) % 10, gen_expr(state, 0)),
        1 => format!(
            "if true {{ {} }} else {{ {} }}",
            gen_stmt(state, depth + 1),
            gen_stmt(state, depth + 1)
        ),
        2 => format!("let _t = ({}, {});", gen_expr(state, 0), gen_expr(state, 0)),
        _ => format!("let _x = {};", gen_expr(state, 0)),
    }
}

/// Generate a complete Landin program with random statements.
fn gen_program(seed: u64) -> String {
    let mut state = seed;
    let n_stmts = (xorshift64(&mut state) % 10) + 1;
    let mut prog = String::from("fn main() {\n");
    for _ in 0..n_stmts {
        prog.push_str("    ");
        prog.push_str(&gen_stmt(&mut state, 0));
        prog.push('\n');
    }
    prog.push_str("    0\n");
    prog.push_str("}\n");
    prog
}

/// Fuzz test: compile 50 random programs, verify no panic.
#[test]
fn fuzz_random_programs_no_crash() {
    for seed in 0..50u64 {
        let code = gen_program(seed);
        // The compiler must not panic. Errors are OK.
        let _result = compile(&code);
        // If we reach here, the compiler didn't crash. Success.
    }
}

/// Fuzz test: compile malformed programs, verify no panic.
#[test]
fn fuzz_malformed_programs_no_crash() {
    let long_id = "a".repeat(256);
    let deep_nest = "if true { ".repeat(50);
    let many_stmts = "let _ = 1;".repeat(100);
    let nested_tuple = (0..20).fold("1".to_string(), |acc, _| format!("({}, 1)", acc));
    let malformed_inputs: Vec<String> = vec![
        // Empty
        "".to_string(),
        // Just whitespace
        "   \n\t  ".to_string(),
        // Unclosed brace
        "fn main() {".to_string(),
        // Unclosed string
        "fn main() { let x = \"hello; }".to_string(),
        // Unmatched parens
        "fn main() { let x = (1 + 2; }".to_string(),
        // Invalid token
        "fn main() { let x = @; }".to_string(),
        // Very long identifier
        format!("fn main() {{ let {} = 0; }}", long_id),
        // Deeply nested
        format!("fn main() {{ {}0; }}", deep_nest),
        // Many semicolons
        format!("fn main() {{ {}0; }}", many_stmts),
        // Binary ops chain
        "fn main() { let x = 1 + 2 - 3 * 4 / 5 % 6 + 7 - 8; 0 }".to_string(),
        // Mixed types
        "fn main() { let x = true + 42; 0 }".to_string(),
        // Nested tuples
        format!("fn main() {{ let x = {}; 0 }}", nested_tuple),
    ];

    for input in &malformed_inputs {
        // Must not panic
        let _result = compile(input);
    }
}

/// Fuzz test: stress test with large match expression.
#[test]
fn fuzz_large_match_no_crash() {
    let mut code = String::from("fn main() {\n    let x = 1;\n    let r = match x {\n");
    for i in 0..50 {
        code.push_str(&format!("        {} => {},\n", i, i * 2));
    }
    code.push_str("        _ => 0,\n    }\n    r\n}\n");
    let _result = compile(&code);
}

/// Fuzz test: stress test with large struct.
#[test]
fn fuzz_large_struct_no_crash() {
    let mut code = String::from("struct Big {\n");
    for i in 0..30 {
        code.push_str(&format!("    f{}: i32,\n", i));
    }
    code.push_str("}\nfn main() {\n    let b = Big {\n");
    for i in 0..30 {
        code.push_str(&format!("        f{}: {},\n", i, i));
    }
    code.push_str("    };\n    0\n}\n");
    let _result = compile(&code);
}

/// Fuzz test: stress test with large array.
#[test]
fn fuzz_large_array_no_crash() {
    let elems: Vec<String> = (0..200).map(|i| i.to_string()).collect();
    let code = format!(
        "fn main() {{\n    let a = [{}];\n    a[0]\n}}\n",
        elems.join(", ")
    );
    let _result = compile(&code);
}

/// Fuzz test: deeply nested if-else (20 levels — within stack limits).
#[test]
fn fuzz_deep_if_nesting_no_crash() {
    let mut code = String::from("fn main() {\n    let x = ");
    for _ in 0..20 {
        code.push_str("if true { ");
    }
    code.push_str("42");
    for _ in 0..20 {
        code.push_str(" } else { 0 }");
    }
    code.push_str(";\n    x\n}\n");
    let _result = compile(&code);
}

/// Fuzz test: many functions.
#[test]
fn fuzz_many_functions_no_crash() {
    let mut code = String::new();
    for i in 0..30 {
        code.push_str(&format!("fn f{}(x: i32) -> i32 {{ x + {} }}\n", i, i));
    }
    code.push_str("fn main() {\n    let r = f0(1)");
    for i in 1..30 {
        code.push_str(&format!(" + f{}({})", i, i));
    }
    code.push_str(";\n    r\n}\n");
    let _result = compile(&code);
}
