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

    /// Output file path (default: <input>.o or <input>.out)
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
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

    // If --compile, --emit-llvm-ir, --emit-obj, or --emit-bin, run full pipeline
    if cli.compile || cli.emit_llvm_ir || cli.emit_obj || cli.emit_bin {
        let result = driver::compile(&source_file.src);

        if result.has_errors() {
            let error_str = result.errors.format_for_user(Some(&source_file.src));
            eprintln!("{}", error_str);
            eprintln!(
                "error: aborting due to {} error(s)",
                result.errors.total_count()
            );
            std::process::exit(1);
        }

        // Stage 13.5: Emit LLVM IR (text)
        if cli.emit_llvm_ir {
            let llvm_ir = landin_compiler::codegen::codegen_crate(&result);
            println!("{}", llvm_ir);
            return;
        }

        // Stage 13.6: Emit object file or executable via LLVMSysEmitter
        #[cfg(feature = "llvm-backend")]
        if cli.emit_obj || cli.emit_bin {
            use landin_compiler::codegen::codegen_crate_to_module;

            let emitter = codegen_crate_to_module(&result);

            // Determine output path
            let obj_path = if let Some(ref o) = cli.output {
                o.with_extension("o")
            } else {
                cli.file.with_extension("o")
            };

            // Emit object file
            match emitter.to_object_file(obj_path.to_str().unwrap()) {
                Ok(()) => {
                    eprintln!("info: object file written to {}", obj_path.display());
                }
                Err(e) => {
                    eprintln!("error: object file generation failed: {e}");
                    std::process::exit(1);
                }
            }

            // If --emit-bin, link via cc/clang
            if cli.emit_bin {
                let exe_path = if let Some(ref o) = cli.output {
                    o.clone()
                } else {
                    cli.file.with_extension("out")
                };

                let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
                let status = std::process::Command::new(&cc)
                    .arg(&obj_path)
                    .arg("-o")
                    .arg(&exe_path)
                    .arg("-lm")
                    .status();

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
            }

            return;
        }

        #[cfg(not(feature = "llvm-backend"))]
        if cli.emit_obj || cli.emit_bin {
            eprintln!("error: --emit-obj/--emit-bin requires --features llvm-backend");
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
