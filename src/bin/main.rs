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

    /// Stage 119 (TD-PROCESS-PER-TEST-ISOLATION): Check errors and output
    /// counts as JSON to stdout. Enables process-per-test isolation —
    /// tests call `landin-stage0 --check-errors <file>` in a subprocess,
    /// getting fresh LLVM C++ state each time.
    #[arg(long)]
    check_errors: bool,
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

    // If --compile, --emit-llvm-ir, --emit-obj, --emit-bin, --run, or --check-errors, run full pipeline
    if cli.compile
        || cli.emit_llvm_ir
        || cli.emit_obj
        || cli.emit_bin
        || cli.run
        || cli.check_errors
    {
        let mut result = driver::compile_binary(&source_file.src);

        // Stage 119 (TD-PROCESS-PER-TEST-ISOLATION): --check-errors outputs
        // error counts as JSON to stdout, then exits. This gives each test
        // fresh LLVM C++ state (subprocess isolation).
        if cli.check_errors {
            let has_errors = result.has_errors();
            println!(
                r#"{{"has_errors":{},"lex":{},"parse":{},"lower":{},"resolve":{},"typeck":{},"borrowck":{},"trait_errors":{},"macro_errors":{},"codegen":{},"module_load":{},"total":{}}}"#,
                has_errors,
                result.errors.lex.len(),
                result.errors.parse.len(),
                result.errors.lower.len(),
                result.errors.resolve.len(),
                result.errors.typeck.len(),
                result.errors.borrowck.len(),
                result.errors.trait_errors.len(),
                result.errors.macro_errors.len(),
                result.errors.codegen.len(),
                result.errors.module_load.len(),
                result.errors.total_count(),
            );
            std::process::exit(if has_errors { 1 } else { 0 });
        }

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
            // Stage 18.151 (TD-CODEGEN-RESULT): codegen_crate_with_target
            // now returns `CodegenResult<String>`. Surface errors to user.
            let llvm_ir = match landin_compiler::codegen::codegen_crate_with_target(&result, target)
            {
                Ok(ir) => ir,
                Err(e) => {
                    eprintln!("error: codegen failed: {}", e);
                    std::process::exit(1);
                }
            };
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

            let emitter = match codegen_crate_to_module_with_target(&result, target) {
                Ok(em) => em,
                Err(e) => {
                    eprintln!("error: codegen failed: {}", e);
                    std::process::exit(1);
                }
            };

            // Stage 13.23: Determine object file path.
            // For --run: always use a temp directory to avoid polluting the
            // source/test directory with .o and .out files.
            // For --emit-obj/--emit-bin: use -o if specified, else alongside
            // the input file (user explicitly requested the output).
            //
            // Stage 18.326 (P1 soundness fix): use global atomic counter + PID + nanos
            // to guarantee unique temp file names under multi-threaded test execution.
            // Previously used only `std::process::id()` which caused intermittent
            // /tmp file races when multiple `landin-stage0 --run` subprocesses
            // executed concurrently (e.g., cargo test --test-threads=N).
            // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix, not workaround.
            static TEMP_COUNTER: std::sync::atomic::AtomicU64 =
                std::sync::atomic::AtomicU64::new(0);
            let temp_id = {
                let c = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let nanos = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or(0);
                format!("{}_{}_{}", std::process::id(), nanos, c)
            };
            // Stage 13.23: Determine object file path.
            // For --run: use the input file's parent directory (which is a
            // unique temp subdir created by the test harness, per Stage 18.326).
            // This ensures all artifacts (.o, .out, .c) live in the same unique
            // subdir, eliminating /tmp file races under multi-threaded execution.
            // For --emit-obj/--emit-bin: use -o if specified, else alongside
            // the input file (user explicitly requested the output).
            //
            // Stage 18.326 (P1 soundness fix): previously used std::env::temp_dir()
            // which put all artifacts in /tmp root — multiple concurrent
            // `landin-stage0 --run` processes could race on the same /tmp paths.
            // Now we use the input file's parent dir, which is unique per test
            // invocation (test harness creates /tmp/landin_test_{pid}_{nanos}_{counter}/).
            // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix.
            let obj_path = if cli.run {
                // --run: use input file's parent dir (unique temp subdir in tests)
                let parent = cli
                    .file
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("."));
                parent.join(format!("landin_run_{}.o", temp_id,))
            } else if let Some(ref o) = cli.output {
                o.with_extension("o")
            } else {
                cli.file.with_extension("o")
            };

            // Emit object file
            // Stage 18.78 P0-B: Codegen errors now populate CompileErrors.codegen
            // instead of being silently eprintln'd + exit. This allows the
            // diagnostic display path to show them properly.
            // Stage 18.93: Use to_string_lossy to handle non-UTF8 paths safely.
            let obj_path_str = obj_path.to_string_lossy();
            match emitter.to_object_file(&obj_path_str) {
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
                    // --run: use input file's parent dir (unique temp subdir in tests)
                    // Stage 18.326: same parent dir as obj_path to avoid /tmp races
                    let parent = cli
                        .file
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    parent.join(format!("landin_run_{}.out", temp_id))
                } else if let Some(ref o) = cli.output {
                    o.clone()
                } else {
                    cli.file.with_extension("out")
                };

                // Stage 13.8/13.10/13.13: Generate a C wrapper that calls landin_main()
                // and provides runtime stubs for panic functions.
                //
                // Stage 18.157: C wrapper source is now shared from
                // `landin_compiler::codegen::runtime::LANDIN_C_WRAPPER`.
                // Both `landin-stage0` and `landinc` use the same source,
                // eliminating duplication (DRY per §1.0 原則 6 通解>特例).
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
                let wrapper_c = {
                    // --run: use input file's parent dir (unique temp subdir in tests)
                    // Stage 18.326: same parent dir as obj_path/exe_path
                    let parent = cli
                        .file
                        .parent()
                        .unwrap_or_else(|| std::path::Path::new("."));
                    parent.join(format!("landin_wrapper_{}.c", temp_id))
                };
                let wrapper_src = landin_compiler::codegen::runtime::LANDIN_C_WRAPPER;
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
