//! Mini-cargo: Landin project manifest parser + build orchestrator.
//!
//! Stage 5.24: MVP implementation of `landinc` — the Landin package
//! manager + build tool. This module provides:
//! - `ProjectManifest` — parse `landin.toml` project manifest files
//! - `BuildConfig` — build configuration (target dir, optimization, etc.)
//! - `build_project()` — orchestrate compilation of all source files
//!
//! Per §16: mini-cargo is a *driver-level* orchestrator — it calls
//! `compile()` on each source file and collects results. It does NOT
//! access HIR/MIR/typeck internals.
//!
//! Per API-naming-standard §3: all types use `<Noun><Noun>` pattern
//! (e.g. `ProjectManifest`, `BuildConfig`); methods use `<verb>_<noun>`
//! pattern (e.g. `parse_manifest`, `build_project`).

use std::path::{Path, PathBuf};

/// Stage 5.24: A Landin project manifest (`landin.toml`).
///
/// Minimal MVP format:
/// ```toml
/// [package]
/// name = "my_project"
/// version = "0.1.0"
/// edition = "v0"
///
/// [dependencies]
/// # (empty for now — dependency resolution deferred)
/// ```
#[derive(Debug, Clone)]
pub struct ProjectManifest {
    /// Project name (e.g. "my_project").
    pub name: String,
    /// Project version (e.g. "0.1.0").
    pub version: String,
    /// Edition (e.g. "v0" — always "v0" for now).
    pub edition: String,
    /// Source directory (default: "src/").
    pub src_dir: PathBuf,
    /// Entry point file (default: "src/main.lin").
    pub entry_point: PathBuf,
    /// Output directory (default: "target/").
    pub target_dir: PathBuf,
}

impl Default for ProjectManifest {
    fn default() -> Self {
        Self {
            name: String::new(),
            version: String::new(),
            edition: "v0".to_string(),
            src_dir: PathBuf::from("src"),
            entry_point: PathBuf::from("src/main.lin"),
            target_dir: PathBuf::from("target"),
        }
    }
}

impl ProjectManifest {
    /// Stage 5.24: Parse a `landin.toml` manifest from a string.
    ///
    /// Per API-naming-standard §3: `parse_manifest` follows
    /// `parse_<noun>` pattern consistent with `parse_crate`.
    pub fn parse_manifest(content: &str) -> Self {
        let mut manifest = Self::default();
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.is_empty() || line.starts_with('[') {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"');
                match key {
                    "name" => manifest.name = value.to_string(),
                    "version" => manifest.version = value.to_string(),
                    "edition" => manifest.edition = value.to_string(),
                    "src_dir" => manifest.src_dir = PathBuf::from(value),
                    "entry_point" => manifest.entry_point = PathBuf::from(value),
                    "target_dir" => manifest.target_dir = PathBuf::from(value),
                    _ => {}
                }
            }
        }
        manifest
    }

    /// Stage 5.24: Load a `landin.toml` manifest from a file path.
    ///
    /// Per API-naming-standard §3: `load_manifest` follows
    /// `load_<noun>` pattern.
    pub fn load_manifest(path: &Path) -> std::io::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        Ok(Self::parse_manifest(&content))
    }
}

/// Stage 5.24: Build configuration for `build_project()`.
#[derive(Debug, Clone, Default)]
pub struct BuildConfig {
    /// Optimization level (0 = debug, 1 = release).
    pub optimization: u8,
    /// Whether to emit LLVM IR.
    pub emit_llvm: bool,
    /// Output file name (without extension).
    pub output_name: Option<String>,
}

/// Stage 5.24: Result of building a project.
#[derive(Debug)]
pub struct BuildResult {
    /// Whether the build succeeded (0 errors).
    pub success: bool,
    /// Total error count across all compiled files.
    pub error_count: usize,
    /// Number of files compiled.
    pub files_compiled: usize,
    /// Generated LLVM IR (if `emit_llvm` was set).
    pub llvm_ir: Option<String>,
    /// Error messages (if any).
    pub errors: Vec<String>,
}

/// Stage 5.24: Build a Landin project — compile the entry point file
/// and return the result.
///
/// This is the main entry point for `landinc build`. It:
/// 1. Reads the manifest
/// 2. Compiles the entry point via `compile()`
/// 3. Optionally generates LLVM IR via `codegen_crate()`
/// 4. Returns a `BuildResult`
///
/// Per §16: uses only the public `compile()` + `codegen_crate()` APIs.
/// Per API-naming-standard §3: `build_project` follows `build_<noun>` pattern.
pub fn build_project(manifest: &ProjectManifest, config: &BuildConfig) -> BuildResult {
    let entry = &manifest.entry_point;
    let src = match std::fs::read_to_string(entry) {
        Ok(s) => s,
        Err(e) => {
            return BuildResult {
                success: false,
                error_count: 1,
                files_compiled: 0,
                llvm_ir: None,
                errors: vec![format!("cannot read {}: {}", entry.display(), e)],
            };
        }
    };

    let result = crate::compile(&src);
    let error_count = result.errors.total_count();

    let llvm_ir = if config.emit_llvm && error_count == 0 {
        Some(crate::codegen::codegen_crate(&result))
    } else {
        None
    };

    BuildResult {
        success: error_count == 0,
        error_count,
        files_compiled: 1,
        llvm_ir,
        errors: if error_count > 0 {
            vec![result
                .errors
                .format_for_user(Some(&src), Some(&result.interner))]
        } else {
            Vec::new()
        },
    }
}
