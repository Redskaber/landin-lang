//! Stage 18.154 (TD-SINGLE-FILE Phase 3): `landinc` — Landin build tool.
//!
//! Per `docs/lang-design/10-toolchain.md` §3: `landinc` is the package
//! manager + build tool. It orchestrates multi-file project compilation
//! via `compile_project()` (Stage 18.152).
//!
//! ## Subcommands
//!
//! - `landinc build` — compile the project (debug mode)
//! - `landinc build --release` — compile with optimizations
//! - `landinc run` — compile + execute (requires llvm-backend)
//! - `landinc check` — type-check without codegen
//! - `landinc new <name>` — create a new project skeleton
//! - `landinc new --lib <name>` — create a library project
//! - `landinc clean` — remove target/
//!
//! ## Separation from `landin-stage0`
//!
//! Per §13.4 J2 (single responsibility):
//! - `landin-stage0` = compiler (single-file: `landin-stage0 <file> --compile`)
//! - `landinc` = build tool (multi-file project: `landinc build`)
//!
//! Per §10 (API naming): `landinc` follows the design doc naming.
//! Per §11 (interface isolation): `landinc` uses only public APIs
//! (`compile_project`, `ProjectManifest`, `codegen_crate`).

use clap::{Parser, Subcommand};
use landin_compiler::cargo::ProjectManifest;
use std::path::PathBuf;

/// Stage 18.154: `landinc` CLI entry point.
#[derive(Parser)]
#[command(
    name = "landinc",
    version,
    about = "Landin build tool — compile, run, and manage Landin projects"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Manifest path (default: ./landin.toml)
    #[arg(long, value_name = "PATH", global = true)]
    manifest_path: Option<PathBuf>,
}

/// Stage 18.154: Subcommands for `landinc`.
#[derive(Subcommand)]
enum Command {
    /// Compile the project (debug mode by default)
    Build {
        /// Release mode (optimizations on)
        #[arg(long)]
        release: bool,

        /// Emit LLVM IR alongside compilation
        #[arg(long)]
        emit_llvm: bool,

        /// Output directory (default: ./target)
        #[arg(long, value_name = "DIR")]
        target_dir: Option<PathBuf>,
    },

    /// Compile and run the project (requires llvm-backend feature)
    Run {
        /// Release mode (optimizations on)
        #[arg(long)]
        release: bool,

        /// Arguments to pass to the program
        #[arg(last = true)]
        args: Vec<String>,
    },

    /// Type-check the project without generating code
    Check,

    /// Create a new Landin project
    New {
        /// Project name
        name: String,

        /// Create a library project (src/lib.lin) instead of binary (src/main.lin)
        #[arg(long)]
        lib: bool,
    },

    /// Remove the target directory
    Clean,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Build {
            release,
            emit_llvm,
            target_dir,
        } => {
            cmd_build(&cli.manifest_path, release, emit_llvm, target_dir);
        }
        Command::Run { release, args } => {
            cmd_run(&cli.manifest_path, release, &args);
        }
        Command::Check => {
            cmd_check(&cli.manifest_path);
        }
        Command::New { name, lib } => {
            cmd_new(&name, lib);
        }
        Command::Clean => {
            cmd_clean(&cli.manifest_path);
        }
    }
}

/// Stage 18.154: Resolve the manifest path.
///
/// If `--manifest-path` is given, use it. Otherwise, look for `landin.toml`
/// in the current directory.
fn resolve_manifest_path(manifest_path: &Option<PathBuf>) -> PathBuf {
    manifest_path
        .clone()
        .unwrap_or_else(|| PathBuf::from("landin.toml"))
}

/// Stage 18.154: Load the project manifest from the given path.
///
/// Per §2 原则 4 (报错>静默): missing/unreadable manifest is a clear error.
fn load_manifest(manifest_path: &Option<PathBuf>) -> ProjectManifest {
    let path = resolve_manifest_path(manifest_path);
    match ProjectManifest::load_manifest(&path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("error: cannot read manifest {}: {}", path.display(), e);
            eprintln!("hint: are you in a Landin project directory? (needs landin.toml)");
            std::process::exit(1);
        }
    }
}

/// Stage 18.154: `landinc build` — compile the project.
///
/// Uses `compile_project(entry_path)` to compile the multi-file project.
/// If `emit_llvm` is set, also generates LLVM IR text via `codegen_crate`.
fn cmd_build(
    manifest_path: &Option<PathBuf>,
    _release: bool,
    emit_llvm: bool,
    target_dir: Option<PathBuf>,
) {
    let manifest = load_manifest(manifest_path);
    let entry = &manifest.entry_point;

    // Verify entry point exists.
    if !entry.exists() {
        eprintln!("error: entry point not found: {}", entry.display());
        eprintln!("hint: check `entry_point` in landin.toml");
        std::process::exit(1);
    }

    eprintln!(
        "Compiling {} v{} ({})",
        manifest.name,
        manifest.version,
        entry.display()
    );

    let result = landin_compiler::compile_project(entry);

    if result.has_errors() {
        // Print errors.
        for err in &result.errors.lex {
            eprintln!("lex error: {} at {}", err.message, err.span);
        }
        for err in &result.errors.parse {
            eprintln!("parse error: {} at {}", err.message, err.span);
        }
        for err in &result.errors.lower {
            eprintln!("lower error: {}", err.message);
        }
        for err in &result.errors.resolve {
            eprintln!("resolve error: {}", err.message);
        }
        eprintln!(
            "error: compilation failed with {} error(s)",
            result.errors.total_count()
        );
        std::process::exit(1);
    }

    eprintln!("Compiling finished ({} MIR bodies)", result.mirs.len());

    // Emit LLVM IR if requested.
    if emit_llvm {
        match landin_compiler::codegen::codegen_crate(&result) {
            Ok(ir) => {
                let target = target_dir
                    .clone()
                    .unwrap_or_else(|| manifest.target_dir.clone());
                std::fs::create_dir_all(&target).ok();
                let ir_path = target.join(format!("{}.ll", manifest.name));
                match std::fs::write(&ir_path, &ir) {
                    Ok(_) => eprintln!("LLVM IR written to {}", ir_path.display()),
                    Err(e) => {
                        eprintln!("error: cannot write LLVM IR: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            Err(e) => {
                eprintln!("error: codegen failed: {}", e);
                std::process::exit(1);
            }
        }
    }
}

/// Stage 18.154: `landinc run` — compile and run the project.
///
/// Requires the `llvm-backend` feature. Compiles the project, generates
/// an object file, links it into an executable, and runs it.
#[cfg(feature = "llvm-backend")]
fn cmd_run(manifest_path: &Option<PathBuf>, _release: bool, args: &[String]) {
    use landin_compiler::codegen::codegen_crate_to_module_with_target;

    let manifest = load_manifest(manifest_path);
    let entry = &manifest.entry_point;

    if !entry.exists() {
        eprintln!("error: entry point not found: {}", entry.display());
        std::process::exit(1);
    }

    eprintln!(
        "Running {} v{} ({})",
        manifest.name,
        manifest.version,
        entry.display()
    );

    let result = landin_compiler::compile_project(entry);

    if result.has_errors() {
        eprintln!(
            "error: compilation failed with {} error(s)",
            result.errors.total_count()
        );
        std::process::exit(1);
    }

    // Check for fn main()
    let has_main = result.body_metas.iter().any(|m| m.fn_name == "landin_main");
    if !has_main {
        eprintln!("error: no `fn main()` found — cannot run");
        std::process::exit(1);
    }

    // Generate LLVM module.
    let emitter = match codegen_crate_to_module_with_target(&result, Default::default()) {
        Ok(em) => em,
        Err(e) => {
            eprintln!("error: codegen failed: {}", e);
            std::process::exit(1);
        }
    };

    // Emit object file.
    let obj_path = std::env::temp_dir().join(format!("landinc_run_{}.o", std::process::id()));
    let obj_path_str = obj_path.to_string_lossy().to_string();
    match emitter.to_object_file(&obj_path_str) {
        Ok(_) => {}
        Err(e) => {
            eprintln!("error: object emission failed: {}", e);
            std::process::exit(1);
        }
    }

    // Link via cc/clang.
    let exe_path = std::env::temp_dir().join(format!("landinc_run_{}", std::process::id()));
    let link_status = std::process::Command::new("cc")
        .arg(&obj_path)
        .arg("-o")
        .arg(&exe_path)
        .status();
    match link_status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("error: linking failed (exit code {:?})", s.code());
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: cannot invoke cc: {}", e);
            std::process::exit(1);
        }
    }

    // Run.
    let run_status = std::process::Command::new(&exe_path).args(args).status();
    let _ = std::fs::remove_file(&obj_path);
    let _ = std::fs::remove_file(&exe_path);
    match run_status {
        Ok(s) => std::process::exit(s.code().unwrap_or(1)),
        Err(e) => {
            eprintln!("error: cannot run executable: {}", e);
            std::process::exit(1);
        }
    }
}

/// Stage 18.154: `landinc run` without llvm-backend — prints error.
#[cfg(not(feature = "llvm-backend"))]
fn cmd_run(_manifest_path: &Option<PathBuf>, _release: bool, _args: &[String]) {
    eprintln!("error: `landinc run` requires the llvm-backend feature");
    eprintln!("hint: rebuild with: cargo build --features llvm-backend");
    std::process::exit(1);
}

/// Stage 18.154: `landinc check` — type-check without codegen.
fn cmd_check(manifest_path: &Option<PathBuf>) {
    let manifest = load_manifest(manifest_path);
    let entry = &manifest.entry_point;

    if !entry.exists() {
        eprintln!("error: entry point not found: {}", entry.display());
        std::process::exit(1);
    }

    eprintln!(
        "Checking {} v{} ({})",
        manifest.name,
        manifest.version,
        entry.display()
    );

    let result = landin_compiler::compile_project(entry);

    if result.has_errors() {
        for err in &result.errors.lex {
            eprintln!("lex error: {} at {}", err.message, err.span);
        }
        for err in &result.errors.parse {
            eprintln!("parse error: {} at {}", err.message, err.span);
        }
        for err in &result.errors.lower {
            eprintln!("lower error: {}", err.message);
        }
        for err in &result.errors.resolve {
            eprintln!("resolve error: {}", err.message);
        }
        eprintln!(
            "error: check failed with {} error(s)",
            result.errors.total_count()
        );
        std::process::exit(1);
    }

    eprintln!("Check passed ({} MIR bodies)", result.mirs.len());
}

/// Stage 18.154: `landinc new <name>` — create a new project skeleton.
///
/// Creates:
/// ```text
/// <name>/
/// ├── landin.toml
/// ├── src/
/// │   └── main.lin   (or lib.lin if --lib)
/// └── .gitignore
/// ```
fn cmd_new(name: &str, lib: bool) {
    let project_dir = PathBuf::from(name);

    // Verify directory doesn't exist.
    if project_dir.exists() {
        eprintln!("error: directory already exists: {}", project_dir.display());
        std::process::exit(1);
    }

    // Create directories.
    let src_dir = project_dir.join("src");
    if let Err(e) = std::fs::create_dir_all(&src_dir) {
        eprintln!("error: cannot create project directory: {}", e);
        std::process::exit(1);
    }

    // Create landin.toml.
    let manifest_content = if lib {
        format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "v0"
entry_point = "src/lib.lin"
target_dir = "target"
"#,
            name
        )
    } else {
        format!(
            r#"[package]
name = "{}"
version = "0.1.0"
edition = "v0"
entry_point = "src/main.lin"
target_dir = "target"
"#,
            name
        )
    };
    let manifest_path = project_dir.join("landin.toml");
    if let Err(e) = std::fs::write(&manifest_path, &manifest_content) {
        eprintln!("error: cannot write manifest: {}", e);
        std::process::exit(1);
    }

    // Create entry point file.
    let (entry_path, entry_content) = if lib {
        (
            src_dir.join("lib.lin"),
            "//! Library crate.\n\npub fn version() -> i32 { 1 }\n",
        )
    } else {
        (
            src_dir.join("main.lin"),
            "//! Binary crate.\n\nfn main() {\n    println!(\"Hello, Landin!\");\n}\n",
        )
    };
    if let Err(e) = std::fs::write(&entry_path, entry_content) {
        eprintln!("error: cannot write entry file: {}", e);
        std::process::exit(1);
    }

    // Create .gitignore.
    let gitignore = "/target\n";
    if let Err(e) = std::fs::write(project_dir.join(".gitignore"), gitignore) {
        eprintln!("warning: cannot write .gitignore: {}", e);
    }

    eprintln!(
        "Created {} project `{}`",
        if lib { "library" } else { "binary" },
        name
    );
    eprintln!("  cd {} && landinc build", name);
}

/// Stage 18.154: `landinc clean` — remove the target directory.
fn cmd_clean(manifest_path: &Option<PathBuf>) {
    let manifest = load_manifest(manifest_path);
    let target = &manifest.target_dir;

    if !target.exists() {
        eprintln!("Nothing to clean (target dir does not exist)");
        return;
    }

    match std::fs::remove_dir_all(target) {
        Ok(_) => eprintln!("Removed {}", target.display()),
        Err(e) => {
            eprintln!("error: cannot remove {}: {}", target.display(), e);
            std::process::exit(1);
        }
    }
}
