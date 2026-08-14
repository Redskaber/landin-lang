use clap::Parser as ClapParser;
use landin_compiler::driver;
use landin_compiler::parser::Parser;
use landin_compiler::session::SourceFile;
use lasso::Rodeo;
use std::path::PathBuf;

#[derive(ClapParser)]
#[command(name = "landin-stage0", version, about = "Landin compiler (stage 0)")]
struct Cli {
    /// Input file
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// Emit token stream only
    #[arg(long)]
    emit_tokens: bool,

    /// Emit AST only (don't proceed to later stages)
    #[arg(long)]
    emit_ast: bool,

    /// Full compile (lex + parse + resolve + typeck + borrowck + codegen)
    /// Exits 0 on success, 1 on compile error.
    #[arg(long)]
    compile: bool,

    /// Emit LLVM IR (implies --compile)
    #[arg(long)]
    emit_llvm_ir: bool,

    /// Emit object file (.o) — requires --features llvm-backend
    #[arg(long)]
    emit_obj: bool,

    /// Emit executable (link via cc/clang) — requires --features llvm-backend
    #[arg(long)]
    emit_bin: bool,

    /// Compile, link, and run the program — requires --features llvm-backend
    #[arg(long)]
    run: bool,

    /// Output file path (default: <input>.o or <input>.out)
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,

    /// Stage 15.19: Color output control (auto/always/never).
    /// Default: auto (colors when stderr is a terminal).
    #[arg(long, value_name = "WHEN", default_value = "auto")]
    color: String,

    /// Stage 18.89: Target triple for cross-compilation.
    /// Default: x86_64-unknown-linux-gnu
    /// Examples: aarch64-unknown-linux-gnu, x86_64-pc-windows-gnu
    #[arg(long, value_name = "TRIPLE")]
    target: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let source_file = match SourceFile::from_path(&cli.file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: cannot read file {}: {e}", cli.file.display());
            std::process::exit(2);
        }
    };

    let mut interner = Rodeo::new();

    // Lex
    let (tokens, lex_errors) = landin_compiler::lexer::tokenize(&source_file.src, &mut interner);

    for err in &lex_errors {
        eprintln!("lex error: {} at {}", err.message, err.span);
    }

    if cli.emit_tokens {
        for tok in &tokens {
            println!("{:?}", tok.kind);
        }
        return;
    }

    // Parse
    let mut parser = Parser::new(tokens, &mut interner);
    let krate = parser.parse_crate();
    let parse_errors = parser.into_errors();

    for err in &parse_errors {
        eprintln!("parse error: {} at {}", err.message, err.span);
    }

    if cli.emit_ast {
        println!("Crate with {} items", krate.items.len());
        for item in &krate.items {
            println!("  - {:?}", item.kind);
        }
    }

    if !lex_errors.is_empty() || !parse_errors.is_empty() {
        eprintln!(
            "error: aborting due to {} lex error(s) and {} parse error(s)",
            lex_errors.len(),
            parse_errors.len()
        );
        std::process::exit(1);
    }

    // If --compile, --emit-llvm-ir, --emit-obj, --emit-bin, or --run, run full pipeline
    if cli.compile || cli.emit_llvm_ir || cli.emit_obj || cli.emit_bin || cli.run {
        // Stage 18.73 P1-G: Use compile_binary to validate main exists.
        let mut result = driver::compile_binary(&source_file.src);

        if result.has_errors() {
            // Stage 15.19: Color output with --color flag (auto/always/never).
            // Default: auto (colors when stderr is a terminal).
            use landin_compiler::diagnostics::ColorConfig;
            use std::io::IsTerminal;
            let color = match cli.color.as_str() {
                "always" => ColorConfig::Always,
                "never" => ColorConfig::Never,
                _ => {
                    // "auto" or any other value — TTY auto-detection
                    if std::io::stderr().is_terminal() {
                        ColorConfig::Always
                    } else {
                        ColorConfig::Never
                    }
                }
            };
            let source_map = landin_compiler::session::SourceMap::new(&source_file.src);
            let error_str = result.errors.format_via_diagnostics_colored(
                &source_file.src,
                &source_file.name,
                &source_map,
                Some(&result.interner),
                color,
            );
            eprintln!("{}", error_str);
            std::process::exit(1);
        }

        // Stage 13.5: Emit LLVM IR (text)
        if cli.emit_llvm_ir {
            // Stage 18.89: Use --target if specified.
            let target = cli
                .target
                .as_ref()
                .map(|t| landin_compiler::codegen::TargetTriple::from_str(t))
                .unwrap_or_default();
            let llvm_ir = landin_compiler::codegen::codegen_crate_with_target(&result, target);
            println!("{}", llvm_ir);
            return;
        }

        // Stage 13.6-13.8: Emit object file, executable, or run via LLVMSysEmitter
        #[cfg(feature = "llvm-backend")]
        if cli.emit_obj || cli.emit_bin || cli.run {
            use landin_compiler::codegen::codegen_crate_to_module_with_target;

            // Stage 18.89: Use --target if specified.
            let target = cli
                .target
                .as_ref()
                .map(|t| landin_compiler::codegen::TargetTriple::from_str(t))
                .unwrap_or_default();

            // Stage 13.27: Check if the source contains `fn main()` before
            // attempting to link. Without `fn main()`, there's no `landin_main`
            // symbol for the C wrapper to call → linker error.
            // This gives a clear error message instead of a cryptic linker failure.
            if cli.emit_bin || cli.run {
                let has_main = result.body_metas.iter().any(|m| m.fn_name == "landin_main");
                if !has_main {
                    eprintln!("error: no `fn main()` found in source — cannot link or run");
                    eprintln!("hint: add `fn main() {{ }}` to your program");
                    std::process::exit(1);
                }
            }

            let emitter = codegen_crate_to_module_with_target(&result, target);

            // Stage 13.23: Determine object file path.
            // For --run: always use a temp directory to avoid polluting the
            // source/test directory with .o and .out files.
            // For --emit-obj/--emit-bin: use -o if specified, else alongside
            // the input file (user explicitly requested the output).
            let obj_path = if cli.run {
                // --run: use temp dir, cleaned up after execution
                std::env::temp_dir().join(format!(
                    "landin_run_{}_{}.o",
                    std::process::id(),
                    cli.file.file_name().unwrap_or_default().to_string_lossy()
                ))
            } else if let Some(ref o) = cli.output {
                o.with_extension("o")
            } else {
                cli.file.with_extension("o")
            };

            // Emit object file
            // Stage 18.78 P0-B: Codegen errors now populate CompileErrors.codegen
            // instead of being silently eprintln'd + exit. This allows the
            // diagnostic display path to show them properly.
            match emitter.to_object_file(obj_path.to_str().unwrap()) {
                Ok(()) => {
                    eprintln!("info: object file written to {}", obj_path.display());
                }
                Err(e) => {
                    // Stage 18.78 P0-B: Push to codegen errors for proper display.
                    result.errors.codegen.push(e);
                }
            }

            // Stage 18.78 P0-B: If codegen errors occurred, display them and exit.
            if !result.errors.codegen.is_empty() {
                use landin_compiler::diagnostics::ColorConfig;
                use std::io::IsTerminal;
                let color = match cli.color.as_str() {
                    "always" => ColorConfig::Always,
                    "never" => ColorConfig::Never,
                    _ => {
                        if std::io::stderr().is_terminal() {
                            ColorConfig::Auto
                        } else {
                            ColorConfig::Never
                        }
                    }
                };
                let source_map = landin_compiler::session::SourceMap::new(&source_file.src);
                let error_str = result.errors.format_via_diagnostics_colored(
                    &source_file.src,
                    &source_file.name,
                    &source_map,
                    Some(&result.interner),
                    color,
                );
                eprintln!("{}", error_str);
                std::process::exit(1);
            }

            // If --emit-bin or --run, link via cc/clang
            if cli.emit_bin || cli.run {
                let exe_path = if cli.run {
                    // --run: use temp dir, cleaned up after execution
                    std::env::temp_dir().join(format!(
                        "landin_run_{}_{}.out",
                        std::process::id(),
                        cli.file.file_name().unwrap_or_default().to_string_lossy()
                    ))
                } else if let Some(ref o) = cli.output {
                    o.clone()
                } else {
                    cli.file.with_extension("out")
                };

                // Stage 13.8/13.10/13.13: Generate a C wrapper that calls landin_main()
                // and provides runtime stubs for panic functions.
                //
                // Stage 13.13 simplification: the previous weak-symbol trick
                // (a separate println-helper function declared as weak extern
                // and called before landin_main) has been REMOVED because
                // Stage 13.13 now emits println! output INLINE within
                // landin_main() itself (via StatementKind::Println →
                // inline `printf("%s", <msg_global>)`). The C wrapper no
                // longer needs to call a separate println helper before
                // landin_main() — that approach (Stage 13.12) broke
                // output ordering for loops and conditionals.
                let wrapper_c =
                    std::env::temp_dir().join(format!("landin_wrapper_{}.c", std::process::id()));
                let wrapper_src = r#"#include <stdio.h>
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
                if let Err(e) = std::fs::write(&wrapper_c, wrapper_src) {
                    eprintln!("error: cannot write wrapper: {e}");
                    std::process::exit(1);
                }

                let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
                let status = std::process::Command::new(&cc)
                    .arg("-fno-pie")
                    .arg("-no-pie")
                    .arg(&wrapper_c)
                    .arg(&obj_path)
                    .arg("-o")
                    .arg(&exe_path)
                    .arg("-lm")
                    .status();

                let _ = std::fs::remove_file(&wrapper_c);

                match status {
                    Ok(s) if s.success() => {
                        eprintln!("info: executable written to {}", exe_path.display());
                    }
                    Ok(s) => {
                        eprintln!("error: linker failed with status {s}");
                        std::process::exit(1);
                    }
                    Err(e) => {
                        eprintln!("error: cannot invoke linker '{cc}': {e}");
                        std::process::exit(1);
                    }
                }

                // Stage 13.8: If --run, execute the program
                if cli.run {
                    eprintln!("info: running {}", exe_path.display());
                    let run_status = std::process::Command::new(&exe_path).status();

                    match run_status {
                        Ok(s) => {
                            let _ = std::fs::remove_file(&exe_path);
                            let _ = std::fs::remove_file(&obj_path);
                            std::process::exit(s.code().unwrap_or(1));
                        }
                        Err(e) => {
                            eprintln!("error: cannot execute '{}': {e}", exe_path.display());
                            let _ = std::fs::remove_file(&exe_path);
                            let _ = std::fs::remove_file(&obj_path);
                            std::process::exit(1);
                        }
                    }
                }
            }

            return;
        }

        #[cfg(not(feature = "llvm-backend"))]
        if cli.emit_obj || cli.emit_bin || cli.run {
            eprintln!("error: --emit-obj/--emit-bin/--run requires --features llvm-backend");
            eprintln!("hint: rebuild with: cargo build --features llvm-backend");
            std::process::exit(1);
        }

        // --compile: just report success
        eprintln!(
            "info: successfully compiled {} items",
            result.hir.as_ref().map(|h| h.owners.len()).unwrap_or(0)
        );
        return;
    }

    eprintln!("info: successfully parsed {} items", krate.items.len());
}
