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

    // If --compile or --emit-llvm-ir, run the full pipeline via driver::compile
    if cli.compile || cli.emit_llvm_ir {
        let result = driver::compile(&source_file.src);

        if result.has_errors() {
            // Print all errors
            let error_str = result.errors.format_for_user(Some(&source_file.src));
            eprintln!("{}", error_str);
            eprintln!(
                "error: aborting due to {} error(s)",
                result.errors.total_count()
            );
            std::process::exit(1);
        }

        // Success
        if cli.emit_llvm_ir {
            let llvm_ir = landin_compiler::codegen::codegen_crate(&result);
            println!("{}", llvm_ir);
        } else {
            eprintln!(
                "info: successfully compiled {} items",
                result.hir.as_ref().map(|h| h.owners.len()).unwrap_or(0)
            );
        }
        return;
    }

    eprintln!("info: successfully parsed {} items", krate.items.len());
}
