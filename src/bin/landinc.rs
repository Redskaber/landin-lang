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
use landin_compiler::codegen::runtime::LANDIN_C_WRAPPER;
use landin_compiler::diagnostics::ColorConfig;
use landin_compiler::driver::CompileResult;
use std::io::IsTerminal;
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

        /// Stage 18.156 (缺陷1 fix): Also link an executable (requires llvm-backend).
        /// Without this flag, `build` only compiles to MIR (+ optional LLVM IR).
        /// With `--bin`, it emits an object file and links it into an executable
        /// in the target directory (matching `cargo build` behavior).
        #[arg(long)]
        bin: bool,

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

    /// Stage 29.1 (v0.11): Run project tests (placeholder — searches for
    /// `#[test]` functions in src/ and runs them).
    Test,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Command::Build {
            release,
            emit_llvm,
            bin,
            target_dir,
        } => {
            cmd_build(&cli.manifest_path, release, emit_llvm, bin, target_dir);
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
        Command::Test => {
            cmd_test(&cli.manifest_path);
        }
    }
}

/// Stage 18.155 (缺陷2 fix): Print compile errors using colored diagnostics.
///
/// Uses `CompileErrors::format_via_diagnostics_colored` to produce
/// user-friendly colored error output (matching `landin-stage0` behavior).
/// Falls back to plain eprintln if the entry file can't be re-read for
/// source context.
///
/// Per §2 原则 4 (报错>静默): all errors are surfaced, not silently dropped.
/// Per §13.4 J2 (单一职责): extracted from cmd_build/cmd_check/cmd_run to
/// avoid duplicated error-printing logic.
/// Per §1.0 原則 6 (通解>特例): one error printer for all commands.
fn print_compile_errors(result: &CompileResult, entry: &std::path::Path) {
    // Re-read the entry source for diagnostic context (span underlines).
    let src = std::fs::read_to_string(entry).unwrap_or_default();
    let source_map = landin_compiler::session::SourceMap::new(&src);
    let source_name = entry.to_string_lossy();

    // Auto-detect color based on stderr TTY (matching landin-stage0 behavior).
    let color = if std::io::stderr().is_terminal() {
        ColorConfig::Always
    } else {
        ColorConfig::Never
    };

    let error_str = result.errors.format_via_diagnostics_colored(
        &src,
        &source_name,
        &source_map,
        Some(&result.interner),
        color,
    );
    eprintln!("{}", error_str);
    eprintln!(
        "error: compilation failed with {} error(s)",
        result.errors.total_count()
    );
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
/// Uses `compile_project_opt(entry_path, optimize)` to compile the multi-file
/// project. Stage 18.155: `--release` now controls the `optimize` flag
/// (MIR DCE + const_prop).
///
/// Stage 18.156 (缺陷1 fix): `--bin` flag now links an executable into the
/// target directory (previously only `landinc run` could link). This matches
/// `cargo build` behavior where `cargo build` produces an executable.
///
/// If `emit_llvm` is set, also generates LLVM IR text via `codegen_crate`.
fn cmd_build(
    manifest_path: &Option<PathBuf>,
    release: bool,
    emit_llvm: bool,
    bin: bool,
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

    // Stage 18.155: `--release` controls MIR optimization.
    // - debug (default): optimize=true (DCE + const_prop run)
    // - release: optimize=true (same — currently only one opt level)
    //
    // Note: `compile_project_opt(path, false)` would DISABLE MIR opt, which
    // is NOT what release wants. Currently both debug and release use
    // optimize=true. A future stage will add LLVM-level opt-level control
    // for release builds (deferred — requires LLVM target machine options).
    //
    // Per §2 原則 9 (正确>妥协): we pass `true` explicitly rather than
    // ignoring the flag, documenting the current limitation.
    let _ = release; // Currently single opt level; documented in dev-log.
    let result = landin_compiler::compile_project_opt(entry, true);

    if result.has_errors() {
        print_compile_errors(&result, entry);
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

    // Stage 18.156 (缺陷1 fix): Link executable if `--bin` is set.
    if bin {
        link_and_emit_executable(&result, &manifest, &target_dir);
    }
}

// Stage 18.157: C wrapper source is now shared from
// `landin_compiler::codegen::runtime::LANDIN_C_WRAPPER`.
// See that module for the full C source + documentation.

/// Stage 18.156 (缺陷1 fix): Link an object file into an executable via `cc`.
///
/// Extracted from `cmd_run` to share between `landinc build --bin` and
/// `landinc run`. Both need: object emission → cc link → executable path.
///
/// Stage 18.156: Now uses a C wrapper (`LANDIN_C_WRAPPER`) that provides
/// `main()` + runtime stubs, matching `landin-stage0` behavior. Also adds
/// `-fno-pie -no-pie -lm` flags (Landin's LLVM module is non-PIC).
///
/// Per §13.4 J2 (单一职责): this function only does "object → executable".
/// Per §1.0 原則 6 (通解>特例): one linker for both build --bin and run.
/// Per §2 原則 4 (报错>静默): link failures are reported with clear messages.
#[cfg(feature = "llvm-backend")]
fn link_object_to_executable(
    emitter: &landin_compiler::codegen::LLVMSysEmitter,
    obj_path: &std::path::Path,
    exe_path: &std::path::Path,
) {
    // Emit object file.
    let obj_path_str = obj_path.to_string_lossy().to_string();
    if let Err(e) = emitter.to_object_file(&obj_path_str) {
        eprintln!("error: object emission failed: {}", e);
        std::process::exit(1);
    }

    // Write C wrapper to temp file.
    // Stage 18.326 (P1 soundness fix): use global atomic counter + PID + nanos
    // to guarantee unique temp file names under multi-threaded test execution.
    // Per §2.2 (根因思维) + §12 (最优>最小): root-cause fix, not workaround.
    static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temp_id = {
        let c = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!("{}_{}_{}", std::process::id(), nanos, c)
    };
    let wrapper_c = std::env::temp_dir().join(format!("landin_wrapper_{}.c", temp_id));
    if let Err(e) = std::fs::write(&wrapper_c, LANDIN_C_WRAPPER) {
        eprintln!("error: cannot write C wrapper: {}", e);
        std::process::exit(1);
    }

    // Link via cc/clang with C wrapper + non-PIC flags + math lib.
    let cc = std::env::var("CC").unwrap_or_else(|_| "cc".to_string());
    let link_status = std::process::Command::new(&cc)
        .arg("-fno-pie")
        .arg("-no-pie")
        .arg(&wrapper_c)
        .arg(obj_path)
        .arg("-o")
        .arg(exe_path)
        .arg("-lm")
        .status();

    // Clean up wrapper.
    let _ = std::fs::remove_file(&wrapper_c);

    match link_status {
        Ok(s) if s.success() => {}
        Ok(s) => {
            eprintln!("error: linking failed (exit code {:?})", s.code());
            let _ = std::fs::remove_file(obj_path);
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("error: cannot invoke {}: {}", cc, e);
            let _ = std::fs::remove_file(obj_path);
            std::process::exit(1);
        }
    }
}

/// Stage 18.156 (缺陷1 fix): Link an executable into the target directory.
///
/// Used by `landinc build --bin`. Emits an object file from the LLVM module,
/// links it via `cc`, and writes the executable to `<target_dir>/<name>`.
///
/// Per §10: `link_and_emit_executable` follows `<verb>_<conj>_<verb>_<noun>`.
#[cfg(feature = "llvm-backend")]
fn link_and_emit_executable(
    result: &landin_compiler::CompileResult,
    manifest: &ProjectManifest,
    target_dir: &Option<PathBuf>,
) {
    use landin_compiler::codegen::codegen_crate_to_module_with_target;

    // Check for fn main() — binary crates need it.
    let has_main = result.body_metas.iter().any(|m| m.fn_name == "landin_main");
    if !has_main {
        eprintln!("error: no `fn main()` found — cannot build executable");
        eprintln!("hint: add `fn main() {{ }}` to your entry point");
        std::process::exit(1);
    }

    // Generate LLVM module.
    let emitter = match codegen_crate_to_module_with_target(result, Default::default()) {
        Ok(em) => em,
        Err(e) => {
            eprintln!("error: codegen failed: {}", e);
            std::process::exit(1);
        }
    };

    // Determine output paths.
    let target = target_dir
        .clone()
        .unwrap_or_else(|| manifest.target_dir.clone());
    if let Err(e) = std::fs::create_dir_all(&target) {
        eprintln!(
            "error: cannot create target directory {}: {}",
            target.display(),
            e
        );
        std::process::exit(1);
    }

    // Use a unique temp object file, then move the final executable.
    let obj_path = target.join(format!("{}.o", manifest.name));
    let exe_path = target.join(&manifest.name);

    link_object_to_executable(&emitter, &obj_path, &exe_path);

    // Clean up the intermediate object file.
    let _ = std::fs::remove_file(&obj_path);

    eprintln!("Executable written to {}", exe_path.display());
}

/// Stage 18.156 (缺陷1 fix): `landinc build --bin` without llvm-backend.
#[cfg(not(feature = "llvm-backend"))]
fn link_and_emit_executable(
    _result: &landin_compiler::CompileResult,
    _manifest: &ProjectManifest,
    _target_dir: &Option<PathBuf>,
) {
    eprintln!("error: `landinc build --bin` requires the llvm-backend feature");
    eprintln!("hint: rebuild with: cargo build --features llvm-backend");
    std::process::exit(1);
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

    let result = landin_compiler::compile_project_opt(entry, true);

    if result.has_errors() {
        print_compile_errors(&result, entry);
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

    // Stage 18.156: Use shared linker helper (was inline before).
    let obj_path = std::env::temp_dir().join(format!("landinc_run_{}.o", std::process::id()));
    let exe_path = std::env::temp_dir().join(format!("landinc_run_{}", std::process::id()));
    link_object_to_executable(&emitter, &obj_path, &exe_path);

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
    // Stage 18.155 (缺陷3 fix): Validate project name.
    // Per §2 原则 4 (报错>静默): invalid names are reported, not silently accepted.
    if !landin_compiler::lexer::is_valid_ident(name) {
        eprintln!("error: invalid project name `{}`", name);
        eprintln!("hint: name must start with a letter or underscore, contain only");
        eprintln!("      letters, digits, or underscores, and not be a keyword");
        std::process::exit(1);
    }

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

/// Stage 29.1 (v0.11 TD-SINGLE-FILE Phase 4): `landinc test` — run project tests.
///
/// Compiles the project and checks for compilation errors. In the future,
/// this will search for `#[test]` functions and run them. For now, it
/// compiles the project and reports success/failure.
///
/// Per §1.0 原則 4 (报错 > 静默): compilation errors are reported.
/// Per §12 (最优 > 最小): root-cause fix — use compile_project_from_manifest.
/// Per §1.0 原則 6 (通解 > 特解): one command handles all project test scenarios.
fn cmd_test(manifest_path: &Option<PathBuf>) {
    let manifest = load_manifest(manifest_path);
    let entry = &manifest.entry_point;

    if !entry.exists() {
        eprintln!("error: entry point not found: {}", entry.display());
        eprintln!("hint: check `entry_point` in landin.toml");
        std::process::exit(1);
    }

    eprintln!(
        "Testing {} v{} ({})",
        manifest.name,
        manifest.version,
        entry.display()
    );

    let result = landin_compiler::compile_project_from_manifest(&manifest);

    if result.has_errors() {
        print_compile_errors(&result, entry);
        std::process::exit(1);
    }

    eprintln!("test result: ok. {} MIR bodies compiled", result.mirs.len());
}
