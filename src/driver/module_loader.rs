//! Stage 18.152 (TD-SINGLE-FILE Phase 1): Multi-file module loader.
//!
//! Resolves `mod foo;` declarations to filesystem paths (`foo.lin` or
//! `foo/mod.lin`), reads + parses those files, and populates the AST
//! `ModDecl::Loaded { items }` with the parsed items.
//!
//! Per `docs/lang-design/10-toolchain.md` §3.3 (project layout):
//! - `mod foo;` → `foo.lin` (single-file module) OR `foo/mod.lin` (directory module)
//! - `foo.lin` takes precedence over `foo/mod.lin` (Rust semantics)
//! - Nested modules: `mod a::b;` → `a/b.lin` or `a/b/mod.lin`
//!
//! Per §11 (interface isolation): ModuleLoader is a driver-level concern.
//! It runs AFTER parsing (which produces the initial AST with empty
//! `Loaded` items) and BEFORE HIR lowering (which consumes the populated
//! items). The parser does NOT do file IO; the loader does.
//!
//! Per §2 原则 4 (报错>静默): missing files, circular dependencies, and
//! parse errors in loaded modules are reported as `ModuleLoadError`, not
//! silently ignored.
//!
//! Per §2 原则 9 (正确>妥协): the loader carries loaded items directly in
//! the AST (`ModDecl::Loaded { items }`), not in a side table — the AST
//! is the single source of truth.
//!
//! Per §10 (API naming): `ModuleLoader` follows `<Noun>Loader` (-er suffix
//! for context type); `load_module_tree` follows `<verb>_<noun>_<noun>`.

use crate::ast::{self, Crate};
use crate::lexer::tokenize;
use crate::parser::macro_expand::expand_macros_with_errors;
use crate::parser::Parser;
use crate::session::Span;
use lasso::{Key, Rodeo};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

/// Stage 18.152: Error encountered while loading a module from disk.
///
/// Per §10.1.8: error types use `Error` suffix with `{ message, span }`
/// minimal form. `ModuleLoadError` extends with `path` for filesystem
/// context (the file that failed to load or wasn't found).
#[derive(Debug, Clone)]
pub struct ModuleLoadError {
    /// Human-readable error message.
    pub message: String,
    /// Span of the `mod foo;` declaration that triggered the load.
    pub span: Span,
    /// Filesystem path that was attempted (for diagnostics).
    pub path: Option<PathBuf>,
}

impl std::fmt::Display for ModuleLoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.path {
            Some(p) => write!(
                f,
                "module load error: {} (path: {})",
                self.message,
                p.display()
            ),
            None => write!(f, "module load error: {}", self.message),
        }
    }
}

impl std::error::Error for ModuleLoadError {}

/// Stage 18.152 (TD-SINGLE-FILE Phase 1): Multi-file module loader.
///
/// Walks an AST crate's `mod foo;` declarations, resolves each to a
/// filesystem path (`foo.lin` or `foo/mod.lin`), reads + parses the file,
/// and populates `ModDecl::Loaded { items }` with the parsed items.
///
/// Recursively loads nested modules (a module file can itself contain
/// `mod bar;` declarations).
///
/// # Circular dependency detection
///
/// Maintains a `visited` set of canonicalized paths. If a path is already
/// in the set, reports a `ModuleLoadError` instead of recursing infinitely.
///
/// # Path resolution rules
///
/// For `mod foo;` declared in file `/proj/src/main.lin`:
/// 1. Try `/proj/src/foo.lin` (single-file module)
/// 2. Else try `/proj/src/foo/mod.lin` (directory module)
/// 3. Else report `ModuleLoadError` (file not found)
///
/// `foo.lin` takes precedence over `foo/mod.lin` (Rust semantics).
///
/// Per §10: `ModuleLoader` follows `<Noun>Loader` (-er suffix).
pub struct ModuleLoader {
    /// Canonicalized paths of files already loaded (cycle detection).
    visited: HashSet<PathBuf>,
}

impl Default for ModuleLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl ModuleLoader {
    /// Create a new `ModuleLoader` with an empty visited set.
    ///
    /// Per §10: `new` follows `<verb>` pattern.
    pub fn new() -> Self {
        Self {
            visited: HashSet::new(),
        }
    }

    /// Walk `krate.items` and populate every `ModDecl::Loaded { items }`.
    ///
    /// `base_dir` is the directory containing the entry file (e.g., for
    /// `/proj/src/main.lin`, `base_dir = /proj/src`). Submodule paths are
    /// resolved relative to `base_dir`.
    ///
    /// Returns a list of `ModuleLoadError` for any modules that failed
    /// to load (file not found, parse error, circular dependency). The
    /// caller is responsible for surfacing these as user-visible diagnostics.
    ///
    /// Per §10: `load_module_tree` follows `<verb>_<noun>_<noun>` pattern.
    /// Per §11: driver-level (no cross-stage calls except parser).
    pub fn load_module_tree(
        &mut self,
        krate: &mut Crate,
        base_dir: &Path,
        interner: &mut Rodeo,
    ) -> Vec<ModuleLoadError> {
        let mut errors = Vec::new();
        for item in &mut krate.items {
            Self::load_item_modules(item, base_dir, interner, &mut self.visited, &mut errors);
        }
        errors
    }

    /// Recursively walk an `Item`, populating `ModDecl::Loaded { items }`.
    ///
    /// For inline modules (`mod foo { ... }`), the base_dir shifts to
    /// `base_dir/foo/` (Rust semantics: inline mod's children load from
    /// a subdirectory named after the mod).
    ///
    /// For loaded modules (`mod foo;`), resolve `foo.lin` or `foo/mod.lin`,
    /// parse it, populate `items`, then recurse into those items.
    fn load_item_modules(
        item: &mut ast::Item,
        base_dir: &Path,
        interner: &mut Rodeo,
        visited: &mut HashSet<PathBuf>,
        errors: &mut Vec<ModuleLoadError>,
    ) {
        if let ast::ItemKind::Mod(mod_decl) = &mut item.kind {
            Self::load_mod_decl(mod_decl, base_dir, interner, visited, errors);
        }
    }

    /// Process a single `ModDecl`, populating `Loaded { items }` if needed.
    fn load_mod_decl(
        m: &mut ast::ModDecl,
        base_dir: &Path,
        interner: &mut Rodeo,
        visited: &mut HashSet<PathBuf>,
        errors: &mut Vec<ModuleLoadError>,
    ) {
        match m {
            ast::ModDecl::Inline { ident, items, .. } => {
                // Inline mod: children load from `base_dir/<ident>/`.
                // Resolve the module name via the interner (Symbol → &str).
                let mod_name = interner
                    .try_resolve(&ident.name)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("<symbol#{}>", ident.name.into_usize()));
                let sub_dir = base_dir.join(&mod_name);
                for item in items.iter_mut() {
                    Self::load_item_modules(item, &sub_dir, interner, visited, errors);
                }
            }
            ast::ModDecl::Loaded { ident, items, span } => {
                // Resolve `foo.lin` or `foo/mod.lin` relative to base_dir.
                let mod_name = interner
                    .try_resolve(&ident.name)
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| format!("<symbol#{}>", ident.name.into_usize()));
                let file_path = Self::resolve_module_path(base_dir, &mod_name);

                let resolved = match file_path {
                    Some(p) => p,
                    None => {
                        errors.push(ModuleLoadError {
                            message: format!(
                                "module file not found: tried `{}.lin` and `{}/mod.lin`",
                                mod_name, mod_name
                            ),
                            span: *span,
                            path: Some(base_dir.join(format!("{}.lin", mod_name))),
                        });
                        return;
                    }
                };

                // Cycle detection.
                let canonical = match resolved.canonicalize() {
                    Ok(c) => c,
                    Err(e) => {
                        errors.push(ModuleLoadError {
                            message: format!("cannot canonicalize {}: {}", resolved.display(), e),
                            span: *span,
                            path: Some(resolved.clone()),
                        });
                        return;
                    }
                };
                if !visited.insert(canonical.clone()) {
                    errors.push(ModuleLoadError {
                        message: format!(
                            "circular module dependency: `{}` was already loaded",
                            canonical.display()
                        ),
                        span: *span,
                        path: Some(canonical),
                    });
                    return;
                }

                // Read + parse the file.
                let src = match std::fs::read_to_string(&resolved) {
                    Ok(s) => s,
                    Err(e) => {
                        errors.push(ModuleLoadError {
                            message: format!(
                                "cannot read module file {}: {}",
                                resolved.display(),
                                e
                            ),
                            span: *span,
                            path: Some(resolved.clone()),
                        });
                        return;
                    }
                };

                let (tokens, lex_errors) = tokenize(&src, interner);
                if !lex_errors.is_empty() {
                    for le in lex_errors {
                        errors.push(ModuleLoadError {
                            message: format!("lex error in {}: {}", resolved.display(), le.message),
                            span: *span,
                            path: Some(resolved.clone()),
                        });
                    }
                    return;
                }

                let (tokens, macro_errs) = expand_macros_with_errors(tokens, interner);
                if !macro_errs.is_empty() {
                    errors.push(ModuleLoadError {
                        message: format!("macro expansion error in {}", resolved.display()),
                        span: *span,
                        path: Some(resolved.clone()),
                    });
                    return;
                }

                let mut parser = Parser::new(tokens, interner);
                let sub_krate = parser.parse_crate();
                let parse_errors = parser.into_errors();
                if !parse_errors.is_empty() {
                    for pe in parse_errors {
                        errors.push(ModuleLoadError {
                            message: format!(
                                "parse error in {}: {}",
                                resolved.display(),
                                pe.message
                            ),
                            span: *span,
                            path: Some(resolved.clone()),
                        });
                    }
                    return;
                }

                // The parsed file is itself a Crate (top-level items).
                // Move its items into this ModDecl::Loaded.
                *items = sub_krate.items;

                // Recurse into the loaded items (they may contain their own `mod bar;`).
                // The base_dir for nested modules is the directory containing
                // the loaded file (or its parent for `foo/mod.lin`).
                let nested_base_dir =
                    if resolved.file_name().and_then(|s| s.to_str()) == Some("mod.lin") {
                        // `foo/mod.lin` → nested base is `foo/`
                        resolved
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| base_dir.to_path_buf())
                    } else {
                        // `foo.lin` → nested base is the same dir as `foo.lin`
                        resolved
                            .parent()
                            .map(|p| p.to_path_buf())
                            .unwrap_or_else(|| base_dir.to_path_buf())
                    };

                for item in items.iter_mut() {
                    Self::load_item_modules(item, &nested_base_dir, interner, visited, errors);
                }
            }
        }
    }

    /// Resolve `mod foo;` to a filesystem path.
    ///
    /// Tries `<base_dir>/foo.lin` first, then `<base_dir>/foo/mod.lin`.
    /// Returns `Some(path)` if a file exists, `None` if neither exists.
    ///
    /// Per §2 原则 4 (报错>静默): returns None rather than panicking; the
    /// caller reports a clear error to the user.
    fn resolve_module_path(base_dir: &Path, mod_name: &str) -> Option<PathBuf> {
        let file_path = base_dir.join(format!("{}.lin", mod_name));
        if file_path.is_file() {
            return Some(file_path);
        }
        let dir_path = base_dir.join(mod_name).join("mod.lin");
        if dir_path.is_file() {
            return Some(dir_path);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    /// Stage 18.152 positive 1: single-file project still compiles (no mod decls).
    #[test]
    fn stage18_152_single_file_no_mod_decls() {
        // No `mod foo;` → no loading needed → compile() works as before.
        let result = compile("fn main() { }");
        assert!(!result.has_errors(), "single file should compile cleanly");
    }

    /// Stage 18.152 positive 2: inline mod is unchanged (no file IO).
    #[test]
    fn stage18_152_inline_mod_unchanged() {
        let src = "mod foo { fn bar() -> i32 { 42 } } fn main() -> i32 { foo::bar() }";
        let result = compile(src);
        // Note: name resolution for `foo::bar` may or may not work depending
        // on stage; we just verify no module-load errors crash compile().
        let _ = result;
    }

    /// Stage 18.152 negative 1: ModuleLoader reports missing file.
    #[test]
    fn stage18_152_module_loader_missing_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "landin_stage18_152_missing_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let entry_path = temp_dir.join("main.lin");
        std::fs::write(&entry_path, "mod nonexistent; fn main() { }").unwrap();

        let src = std::fs::read_to_string(&entry_path).unwrap();
        let mut interner = Rodeo::new();
        let (tokens, _) = tokenize(&src, &mut interner);
        let (tokens, _) = expand_macros_with_errors(tokens, &mut interner);
        let mut parser = Parser::new(tokens, &mut interner);
        let mut krate = parser.parse_crate();

        let mut loader = ModuleLoader::new();
        let errors = loader.load_module_tree(&mut krate, &temp_dir, &mut interner);
        assert_eq!(errors.len(), 1, "should report 1 missing module error");
        assert!(
            errors[0].message.contains("not found"),
            "error should mention 'not found', got: {}",
            errors[0].message
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Stage 18.152 positive 3: ModuleLoader loads `foo.lin`.
    #[test]
    fn stage18_152_module_loader_loads_file() {
        let temp_dir = std::env::temp_dir().join(format!(
            "landin_stage18_152_file_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        std::fs::write(temp_dir.join("main.lin"), "mod helper; fn main() { }").unwrap();
        std::fs::write(temp_dir.join("helper.lin"), "fn answer() -> i32 { 42 }").unwrap();

        let src = std::fs::read_to_string(temp_dir.join("main.lin")).unwrap();
        let mut interner = Rodeo::new();
        let (tokens, _) = tokenize(&src, &mut interner);
        let (tokens, _) = expand_macros_with_errors(tokens, &mut interner);
        let mut parser = Parser::new(tokens, &mut interner);
        let mut krate = parser.parse_crate();

        let mut loader = ModuleLoader::new();
        let errors = loader.load_module_tree(&mut krate, &temp_dir, &mut interner);
        assert!(
            errors.is_empty(),
            "should have no load errors: {:?}",
            errors
        );

        // Verify the helper module's items were loaded.
        let mut found_helper = false;
        for item in &krate.items {
            if let ast::ItemKind::Mod(ast::ModDecl::Loaded { items, .. }) = &item.kind {
                if !items.is_empty() {
                    found_helper = true;
                    break;
                }
            }
        }
        assert!(found_helper, "helper module items should be populated");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Stage 18.152 positive 4: ModuleLoader loads `foo/mod.lin`.
    #[test]
    fn stage18_152_module_loader_loads_dir() {
        let temp_dir = std::env::temp_dir().join(format!(
            "landin_stage18_152_dir_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp_dir.join("foo")).unwrap();

        std::fs::write(temp_dir.join("main.lin"), "mod foo; fn main() { }").unwrap();
        std::fs::write(
            temp_dir.join("foo").join("mod.lin"),
            "fn bar() -> i32 { 7 }",
        )
        .unwrap();

        let src = std::fs::read_to_string(temp_dir.join("main.lin")).unwrap();
        let mut interner = Rodeo::new();
        let (tokens, _) = tokenize(&src, &mut interner);
        let (tokens, _) = expand_macros_with_errors(tokens, &mut interner);
        let mut parser = Parser::new(tokens, &mut interner);
        let mut krate = parser.parse_crate();

        let mut loader = ModuleLoader::new();
        let errors = loader.load_module_tree(&mut krate, &temp_dir, &mut interner);
        assert!(
            errors.is_empty(),
            "should have no load errors: {:?}",
            errors
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Stage 18.152 negative 2: circular module dependency is detected.
    #[test]
    fn stage18_152_module_loader_circular_dep() {
        let temp_dir = std::env::temp_dir().join(format!(
            "landin_stage18_152_circ_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        // a.lin: mod b;
        // b.lin: mod a;  ← circular
        std::fs::write(temp_dir.join("a.lin"), "mod b;").unwrap();
        std::fs::write(temp_dir.join("b.lin"), "mod a;").unwrap();

        let src = "mod a; fn main() { }";
        let mut interner = Rodeo::new();
        let (tokens, _) = tokenize(src, &mut interner);
        let (tokens, _) = expand_macros_with_errors(tokens, &mut interner);
        let mut parser = Parser::new(tokens, &mut interner);
        let mut krate = parser.parse_crate();

        let mut loader = ModuleLoader::new();
        let errors = loader.load_module_tree(&mut krate, &temp_dir, &mut interner);
        assert!(
            errors.iter().any(|e| e.message.contains("circular")),
            "should detect circular dependency, got: {:?}",
            errors
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    /// Stage 18.152 positive 5: nested modules load recursively.
    #[test]
    fn stage18_152_module_loader_nested() {
        let temp_dir = std::env::temp_dir().join(format!(
            "landin_stage18_152_nested_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(temp_dir.join("outer").join("inner")).unwrap();

        // main.lin: mod outer;
        // outer/mod.lin: mod inner;
        // outer/inner/mod.lin: fn deep() -> i32 { 99 }
        std::fs::write(temp_dir.join("main.lin"), "mod outer; fn main() { }").unwrap();
        std::fs::write(temp_dir.join("outer").join("mod.lin"), "mod inner;").unwrap();
        std::fs::write(
            temp_dir.join("outer").join("inner").join("mod.lin"),
            "fn deep() -> i32 { 99 }",
        )
        .unwrap();

        let src = std::fs::read_to_string(temp_dir.join("main.lin")).unwrap();
        let mut interner = Rodeo::new();
        let (tokens, _) = tokenize(&src, &mut interner);
        let (tokens, _) = expand_macros_with_errors(tokens, &mut interner);
        let mut parser = Parser::new(tokens, &mut interner);
        let mut krate = parser.parse_crate();

        let mut loader = ModuleLoader::new();
        let errors = loader.load_module_tree(&mut krate, &temp_dir, &mut interner);
        assert!(
            errors.is_empty(),
            "should have no load errors: {:?}",
            errors
        );

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
