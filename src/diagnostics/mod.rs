//! Diagnostics: error/warning collection and formatting.
//!
//! Stage 15.13 (v0.2): Improved diagnostics system — added `format_snippet`
//! (moved from driver.rs), `DiagnosticBuilder` for ergonomic construction,
//! and `DiagnosticBuffer::format_with_source` for rustc-style display with
//! source code snippets. This module is now the single source of truth for
//! error display formatting.
//!
//! Stage 15.16 (v0.2): Added `Spanned` trait (uniform span access for all
//! error types) and `ErrorCode` catalog (stable error codes E001-E999).
//!
//! Per §1.0 原则 3 "显式 > 隐式": the snippet format is explicit in this
//! module, not hidden in driver.rs.
//! Per §23 (API Naming): `DiagnosticBuilder` follows the `<Noun>Builder`
//! pattern consistent with Rust API guidelines.

use crate::session::{LineCol, Span};
use std::fmt;

/// Stage 15.16: Trait for types that carry a source span.
///
/// All error types implement this trait so that `to_diagnostics` and other
/// consumers can access the span uniformly without knowing the concrete type.
///
/// Per §1.0 原则 6 "通用 > 特例": one trait handles all error types.
/// Per §23 (API Naming): `Spanned` follows the `<Adj>` pattern (trait name
/// describes a capability, consistent with `Clone`, `Copy`).
pub trait Spanned {
    /// Returns the source span of this item.
    fn span(&self) -> Span;
}

/// Stage 15.16: Error code catalog.
///
/// Stable error codes for each error category. These codes appear in
/// diagnostics as `error[E001]: message`, matching the rustc convention.
/// Users can look up the code in documentation to understand the error.
///
/// Per §1.0 原则 3 "显式 > 隐式": the code is explicit, not implicit.
/// Per §23 (API Naming): `ErrorCode` follows the `<Noun>Code` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCode {
    /// Lexer errors (E001-E099)
    Lex,
    /// Parser errors (E100-E199)
    Parse,
    /// HIR lowering errors (E200-E299)
    Lower,
    /// Name resolution errors (E300-E399)
    Resolve,
    /// Type checking errors (E400-E499)
    Type,
    /// Borrow checking errors (E500-E599)
    Borrow,
    /// Trait coherence/completeness errors (E600-E699)
    Trait,
    /// Internal compiler error (E900)
    Internal,
}

impl ErrorCode {
    /// Get the numeric code (e.g., E001, E100, E400).
    ///
    /// Per §23 (API Naming): `code` follows the `<noun>` pattern for
    /// property accessors (Rust getter convention — no `get_` prefix).
    pub fn code(self) -> &'static str {
        match self {
            ErrorCode::Lex => "E001",
            ErrorCode::Parse => "E100",
            ErrorCode::Lower => "E200",
            ErrorCode::Resolve => "E300",
            ErrorCode::Type => "E400",
            ErrorCode::Borrow => "E500",
            ErrorCode::Trait => "E600",
            ErrorCode::Internal => "E900",
        }
    }

    /// Get the category name (e.g., "lex", "parse", "type").
    ///
    /// Per §23 (API Naming): `category` follows the `<noun>` pattern.
    pub fn category(self) -> &'static str {
        match self {
            ErrorCode::Lex => "lex",
            ErrorCode::Parse => "parse",
            ErrorCode::Lower => "lower",
            ErrorCode::Resolve => "resolve",
            ErrorCode::Type => "type",
            ErrorCode::Borrow => "borrow",
            ErrorCode::Trait => "trait",
            ErrorCode::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.code())
    }
}

/// Stage 15.17: Color configuration for diagnostics display.
///
/// Controls whether ANSI color codes are emitted. When `Auto`, colors are
/// enabled only if stderr is a TTY (checked by the caller).
///
/// Per §1.0 原则 3 "显式 > 隐式": the color choice is explicit.
/// Per §23 (API Naming): `ColorConfig` follows the `<Noun>Config` pattern.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorConfig {
    /// Always use colors (even if output is not a TTY).
    Always,
    /// Never use colors (plain text).
    Never,
    /// Auto-detect: use colors only if stderr is a TTY (caller checks).
    #[default]
    Auto,
}

/// Stage 15.17: ANSI color codes for terminal output.
///
/// Per §23 (API Naming): `Color` follows the `<Noun>` pattern for enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Yellow,
    Cyan,
    Green,
    Bold,
    Reset,
}

impl Color {
    fn code(self) -> &'static str {
        match self {
            Color::Red => "\x1b[31m",
            Color::Yellow => "\x1b[33m",
            Color::Cyan => "\x1b[36m",
            Color::Green => "\x1b[32m",
            Color::Bold => "\x1b[1m",
            Color::Reset => "\x1b[0m",
        }
    }
}

/// Stage 15.17: Apply color to a string if colors are enabled.
fn colorize(text: &str, color: Color, config: ColorConfig) -> String {
    match config {
        ColorConfig::Always => format!("{}{}{}", color.code(), text, Color::Reset.code()),
        ColorConfig::Never => text.to_string(),
        ColorConfig::Auto => text.to_string(), // Auto is resolved by caller
    }
}

/// Stage 15.17: Get the color for a diagnostic level.
fn level_color(level: Level) -> Color {
    match level {
        Level::Error | Level::Fatal | Level::Bug => Color::Red,
        Level::Warning => Color::Yellow,
        Level::Note => Color::Cyan,
        Level::Help => Color::Green,
    }
}

/// Severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Error,
    Warning,
    Note,
    Help,
    Fatal,
    Bug, // ICE
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Level::Error => write!(f, "error"),
            Level::Warning => write!(f, "warning"),
            Level::Note => write!(f, "note"),
            Level::Help => write!(f, "help"),
            Level::Fatal => write!(f, "fatal error"),
            Level::Bug => write!(f, "internal compiler error"),
        }
    }
}

/// A single diagnostic message.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: Level,
    pub code: Option<String>,
    pub message: String,
    pub span: Span,
    pub children: Vec<SubDiagnostic>,
}

#[derive(Debug, Clone)]
pub struct SubDiagnostic {
    pub level: Level,
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(level: Level, message: impl Into<String>, span: Span) -> Self {
        Self {
            level,
            code: None,
            message: message.into(),
            span,
            children: vec![],
        }
    }

    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Error, message, span)
    }

    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self::new(Level::Warning, message, span)
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    pub fn with_note(mut self, message: impl Into<String>, span: Span) -> Self {
        self.children.push(SubDiagnostic {
            level: Level::Note,
            message: message.into(),
            span,
        });
        self
    }

    pub fn with_help(mut self, message: impl Into<String>, span: Span) -> Self {
        self.children.push(SubDiagnostic {
            level: Level::Help,
            message: message.into(),
            span,
        });
        self
    }
}

/// Stage 15.13: Builder for ergonomic `Diagnostic` construction.
///
/// Provides a fluent API for building diagnostics with notes, helps, and
/// codes. The builder is consumed by `DiagnosticBuffer::emit`.
///
/// Per §23 (API Naming): `DiagnosticBuilder` follows the `<Noun>Builder`
/// pattern consistent with Rust API guidelines (cf. `std::process::Command`).
///
/// # Example
/// ```ignore
/// let diag = DiagnosticBuilder::error("mismatched types", span)
///     .with_code("E0308")
///     .with_note("expected `i32`, found `bool`", span)
///     .with_help("try using `as i32` to convert", span)
///     .build();
/// buffer.emit(diag);
/// ```
#[derive(Debug, Clone)]
pub struct DiagnosticBuilder {
    diag: Diagnostic,
}

impl DiagnosticBuilder {
    /// Create a new error-level diagnostic builder.
    pub fn error(message: impl Into<String>, span: Span) -> Self {
        Self {
            diag: Diagnostic::error(message, span),
        }
    }

    /// Create a new warning-level diagnostic builder.
    pub fn warning(message: impl Into<String>, span: Span) -> Self {
        Self {
            diag: Diagnostic::warning(message, span),
        }
    }

    /// Create a new note-level diagnostic builder.
    pub fn note(message: impl Into<String>, span: Span) -> Self {
        Self {
            diag: Diagnostic::new(Level::Note, message, span),
        }
    }

    /// Create a new help-level diagnostic builder.
    pub fn help(message: impl Into<String>, span: Span) -> Self {
        Self {
            diag: Diagnostic::new(Level::Help, message, span),
        }
    }

    /// Create a new fatal-level diagnostic builder.
    pub fn fatal(message: impl Into<String>, span: Span) -> Self {
        Self {
            diag: Diagnostic::new(Level::Fatal, message, span),
        }
    }

    /// Add an error code (e.g., "E0308").
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.diag.code = Some(code.into());
        self
    }

    /// Add a note sub-diagnostic.
    pub fn with_note(mut self, message: impl Into<String>, span: Span) -> Self {
        self.diag.children.push(SubDiagnostic {
            level: Level::Note,
            message: message.into(),
            span,
        });
        self
    }

    /// Add a help sub-diagnostic.
    pub fn with_help(mut self, message: impl Into<String>, span: Span) -> Self {
        self.diag.children.push(SubDiagnostic {
            level: Level::Help,
            message: message.into(),
            span,
        });
        self
    }

    /// Build the final `Diagnostic`.
    pub fn build(self) -> Diagnostic {
        self.diag
    }
}

/// Buffer collecting all diagnostics during compilation.
#[derive(Debug, Default)]
pub struct DiagnosticBuffer {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
    pub error_limit: usize,
    /// Stage 15.13: Flag to emit the "error limit reached" note only once.
    limit_reached_emitted: bool,
}

impl DiagnosticBuffer {
    pub fn new() -> Self {
        Self {
            error_limit: 128,
            ..Default::default()
        }
    }

    pub fn emit(&mut self, diag: Diagnostic) {
        // Stage 15.13: Respect error_limit — stop emitting after limit reached.
        // This prevents overwhelming the user with cascading errors.
        if self.error_count >= self.error_limit
            && matches!(diag.level, Level::Error | Level::Fatal | Level::Bug)
        {
            // Skip this error — limit reached. Emit a summary ONCE.
            // Use a flag to avoid emitting the "limit reached" note multiple times.
            if !self.limit_reached_emitted {
                self.diagnostics.push(Diagnostic::new(
                    Level::Note,
                    format!(
                        "error limit reached ({} errors); suppressing further errors",
                        self.error_limit
                    ),
                    Span::DUMMY,
                ));
                self.limit_reached_emitted = true;
            }
            return;
        }
        match diag.level {
            Level::Error | Level::Fatal | Level::Bug => self.error_count += 1,
            Level::Warning => self.warning_count += 1,
            _ => {}
        }
        self.diagnostics.push(diag);
    }

    /// Stage 15.13: Emit a diagnostic from a builder (convenience method).
    pub fn emit_builder(&mut self, builder: DiagnosticBuilder) {
        self.emit(builder.build());
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    /// Format diagnostics for display, using SourceMap for line/col info.
    pub fn format(&self, source_name: &str, source_map: &crate::session::SourceMap) -> String {
        let mut out = String::new();
        for diag in &self.diagnostics {
            let LineCol { line, col } = source_map.line_col(diag.span.lo);
            if let Some(ref code) = diag.code {
                out.push_str(&format!("{}[{}]: {}\n", diag.level, code, diag.message));
            } else {
                out.push_str(&format!("{}: {}\n", diag.level, diag.message));
            }
            out.push_str(&format!("  --> {}:{}:{}\n", source_name, line, col));
            for child in &diag.children {
                let LineCol { line, col } = source_map.line_col(child.span.lo);
                out.push_str(&format!(
                    "  {} {}:{}:{}: {}\n",
                    child.level, source_name, line, col, child.message
                ));
            }
            out.push('\n');
        }
        if self.error_count > 0 {
            out.push_str(&format!(
                "error: aborting due to {} previous error{}\n",
                self.error_count,
                if self.error_count > 1 { "s" } else { "" }
            ));
        }
        out
    }

    /// Stage 15.13: Format diagnostics with source code snippets (rustc-style).
    ///
    /// Produces output like:
    /// ```text
    /// error[E0308]: mismatched types
    ///   --> main.lin:5:13
    ///    |
    ///  5 | let x: i32 = true;
    ///    |             ^^^^ expected `i32`, found `bool`
    ///    |
    /// help: try using `as i32` to convert
    ///   --> main.lin:5:13
    ///    |
    ///  5 | let x: i32 = true as i32;
    ///    |             ^^^^^^^^^^^^
    /// ```
    ///
    /// Per "显示友好": users see the actual source line with the span
    /// underlined, making it easy to locate the error.
    pub fn format_with_source(
        &self,
        source_name: &str,
        source_map: &crate::session::SourceMap,
        source: &str,
    ) -> String {
        let mut out = String::new();
        for diag in &self.diagnostics {
            // Header line: "error[E0308]: message" or "error: message"
            if let Some(ref code) = diag.code {
                out.push_str(&format!("{}[{}]: {}\n", diag.level, code, diag.message));
            } else {
                out.push_str(&format!("{}: {}\n", diag.level, diag.message));
            }
            // Location line: "  --> source_name:line:col"
            let LineCol { line, col } = source_map.line_col(diag.span.lo);
            out.push_str(&format!("  --> {}:{}:{}\n", source_name, line, col));
            // Source snippet with ^^^ underline
            out.push_str(&format_snippet(source, &diag.span));
            // Children (notes/helps)
            for child in &diag.children {
                out.push_str(&format!("\n{}: {}\n", child.level, child.message));
                let LineCol { line, col } = source_map.line_col(child.span.lo);
                out.push_str(&format!("  --> {}:{}:{}\n", source_name, line, col));
                out.push_str(&format_snippet(source, &child.span));
            }
            out.push('\n');
        }
        if self.error_count > 0 {
            out.push_str(&format!(
                "error: aborting due to {} previous error{}\n",
                self.error_count,
                if self.error_count > 1 { "s" } else { "" }
            ));
        }
        out
    }

    /// Stage 15.17: Format diagnostics with source snippets and ANSI colors.
    ///
    /// Same as `format_with_source` but with color:
    /// - Error/Fatal/Bug: red
    /// - Warning: yellow
    /// - Note: cyan
    /// - Help: green
    /// - `^^^` underline: colored by level
    ///
    /// Per "显示友好": colored output makes it easier to distinguish
    /// errors from warnings at a glance.
    ///
    /// Per §23 (API Naming): `format_with_source_colored` follows
    /// `<verb>_<prep>_<noun>_<adj>` pattern.
    pub fn format_with_source_colored(
        &self,
        source_name: &str,
        source_map: &crate::session::SourceMap,
        source: &str,
        color: ColorConfig,
    ) -> String {
        let mut out = String::new();
        for diag in &self.diagnostics {
            let lc = level_color(diag.level);
            // Header line: colored "error[E0308]: message"
            let level_str = colorize(&diag.level.to_string(), lc, color);
            if let Some(ref code) = diag.code {
                out.push_str(&format!("{}[{}]: {}\n", level_str, code, diag.message));
            } else {
                out.push_str(&format!("{}: {}\n", level_str, diag.message));
            }
            // Location line
            let LineCol { line, col } = source_map.line_col(diag.span.lo);
            out.push_str(&format!("  --> {}:{}:{}\n", source_name, line, col));
            // Source snippet with colored ^^^ underline
            out.push_str(&format_snippet_colored(source, &diag.span, lc, color));
            // Children
            for child in &diag.children {
                let cc = level_color(child.level);
                let child_level_str = colorize(&child.level.to_string(), cc, color);
                out.push_str(&format!("\n{}: {}\n", child_level_str, child.message));
                let LineCol { line, col } = source_map.line_col(child.span.lo);
                out.push_str(&format!("  --> {}:{}:{}\n", source_name, line, col));
                out.push_str(&format_snippet_colored(source, &child.span, cc, color));
            }
            out.push('\n');
        }
        if self.error_count > 0 {
            let err_str = colorize("error", Color::Red, color);
            out.push_str(&format!(
                "{}: aborting due to {} previous error{}\n",
                err_str,
                self.error_count,
                if self.error_count > 1 { "s" } else { "" }
            ));
        }
        out
    }
}

/// Stage 15.13: Format a source snippet around a span, with a `^` underline.
///
/// Moved from `src/driver.rs` (was private `format_snippet`). Now the single
/// source of truth for snippet formatting — both `driver::format_for_user`
/// and `DiagnosticBuffer::format_with_source` use this function.
///
/// ```text
///   |
/// 5 | let x: bool = 42;
///   |                ^^
///   |
/// ```
///
/// For dummy spans (lo == hi == 0), returns an empty string (no snippet).
///
/// Per §23 (API Naming): `format_snippet` follows `<verb>_<noun>` pattern.
pub fn format_snippet(src: &str, span: &Span) -> String {
    if span.is_dummy() {
        return String::new();
    }
    let lo = span.lo as usize;
    let hi = span.hi as usize;
    if lo >= src.len() || hi > src.len() {
        return String::new();
    }

    // Find the line containing `lo`.
    let mut line_start = 0;
    let mut line_end = src.len();
    let mut line_no = 1;
    for (i, c) in src.char_indices() {
        if i < lo {
            if c == '\n' {
                line_start = i + 1;
                line_no += 1;
            }
        } else if c == '\n' {
            line_end = i;
            break;
        }
    }
    if line_end < line_start {
        line_end = src.len();
    }
    let line = &src[line_start..line_end.min(src.len())];

    // Compute column offsets within the line.
    let col_lo = lo.saturating_sub(line_start);
    let col_hi = hi.saturating_sub(line_start).max(col_lo + 1);

    // Stage 15.21: Clamp the span to the line width.
    // If the span extends beyond the line (e.g., multi-line span or span
    // includes trailing whitespace), clamp col_hi to line.len().
    // This prevents ^^^ from extending past the source line.
    let line_len = line.len();
    let col_hi_clamped = col_hi.min(line_len);
    let col_lo_clamped = col_lo.min(line_len);

    // Stage 15.22: Fix gutter alignment.
    // The source line format is: "{line_no_str} | {line}"
    //   e.g. "3 |   x" — '3' at pos 0, ' ' at pos 1, '|' at pos 2, ' ' at pos 3, source at pos 4+
    // The gutter line format is: "{pad} |"
    //   where pad = " " * len(line_no_str), so ' ' at pos 0, ' ' at pos 1... NO.
    //   Actually: pad should be " " * len(line_no_str), then " |" follows.
    //   For line_no_str="3" (1 char): pad=" " (1 space), gutter = " |" — | at pos 2. ✓
    //   For line_no_str="10" (2 chars): pad="  " (2 spaces), gutter = "  |" — | at pos 3. ✓
    // The underline format is: "{pad} | {spaces}{^}"
    //   For line_no_str="3": " | " + "  " + "^" — ^ at pos 6 (matching x at pos 6 in "3 |   x"). ✓
    let line_no_str = line_no.to_string();
    let pad = " ".repeat(line_no_str.len());

    let mut out = String::new();
    out.push_str(&format!("{} |\n", pad)); // gutter: " |"
    out.push_str(&format!("{} | {}\n", line_no_str, line)); // source: "3 |   x"
    out.push_str(&format!("{} | ", pad)); // underline prefix: " | "
    out.push_str(&" ".repeat(col_lo_clamped));
    let span_len = col_hi_clamped.saturating_sub(col_lo_clamped).max(1);
    out.push_str(&"^".repeat(span_len));
    out.push('\n');
    out
}

/// Stage 15.17: Format a source snippet with colored `^` underline.
///
/// Same as `format_snippet` but the `^^^` underline is colored.
/// Per §23 (API Naming): `format_snippet_colored` follows
/// `<verb>_<noun>_<adj>` pattern.
pub fn format_snippet_colored(src: &str, span: &Span, color: Color, config: ColorConfig) -> String {
    if span.is_dummy() {
        return String::new();
    }
    let lo = span.lo as usize;
    let hi = span.hi as usize;
    if lo >= src.len() || hi > src.len() {
        return String::new();
    }

    let mut line_start = 0;
    let mut line_end = src.len();
    let mut line_no = 1;
    for (i, c) in src.char_indices() {
        if i < lo {
            if c == '\n' {
                line_start = i + 1;
                line_no += 1;
            }
        } else if c == '\n' {
            line_end = i;
            break;
        }
    }
    if line_end < line_start {
        line_end = src.len();
    }
    let line = &src[line_start..line_end.min(src.len())];

    let col_lo = lo.saturating_sub(line_start);
    let col_hi = hi.saturating_sub(line_start).max(col_lo + 1);

    // Stage 15.21: Clamp the span to the line width (same as format_snippet).
    let line_len = line.len();
    let col_hi_clamped = col_hi.min(line_len);
    let col_lo_clamped = col_lo.min(line_len);

    // Stage 15.22: Fix gutter alignment (same as format_snippet).
    let line_no_str = line_no.to_string();
    let pad = " ".repeat(line_no_str.len());

    let mut out = String::new();
    out.push_str(&format!("{} |\n", pad));
    out.push_str(&format!("{} | {}\n", line_no_str, line));
    out.push_str(&format!("{} | ", pad));
    out.push_str(&" ".repeat(col_lo_clamped));
    let span_len = col_hi_clamped.saturating_sub(col_lo_clamped).max(1);
    let underline = "^".repeat(span_len);
    out.push_str(&colorize(&underline, color, config));
    out.push('\n');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage15_13_diagnostic_builder_error() {
        let diag = DiagnosticBuilder::error("test error", Span::DUMMY)
            .with_code("E001")
            .with_note("this is a note", Span::DUMMY)
            .with_help("try this instead", Span::DUMMY)
            .build();
        assert_eq!(diag.level, Level::Error);
        assert_eq!(diag.message, "test error");
        assert_eq!(diag.code.as_deref(), Some("E001"));
        assert_eq!(diag.children.len(), 2);
        assert_eq!(diag.children[0].level, Level::Note);
        assert_eq!(diag.children[1].level, Level::Help);
    }

    #[test]
    fn stage15_13_diagnostic_builder_warning() {
        let diag = DiagnosticBuilder::warning("test warning", Span::DUMMY).build();
        assert_eq!(diag.level, Level::Warning);
        assert_eq!(diag.message, "test warning");
    }

    #[test]
    fn stage15_13_diagnostic_buffer_emit_and_count() {
        let mut buf = DiagnosticBuffer::new();
        buf.emit(Diagnostic::error("error 1", Span::DUMMY));
        buf.emit(Diagnostic::warning("warning 1", Span::DUMMY));
        buf.emit(Diagnostic::error("error 2", Span::DUMMY));
        assert_eq!(buf.error_count, 2);
        assert_eq!(buf.warning_count, 1);
        assert!(buf.has_errors());
        assert_eq!(buf.diagnostics.len(), 3);
    }

    #[test]
    fn stage15_13_diagnostic_buffer_emit_builder() {
        let mut buf = DiagnosticBuffer::new();
        buf.emit_builder(DiagnosticBuilder::error("builder error", Span::DUMMY).with_code("E002"));
        assert_eq!(buf.error_count, 1);
        assert_eq!(buf.diagnostics[0].code.as_deref(), Some("E002"));
    }

    #[test]
    fn stage15_13_diagnostic_buffer_error_limit() {
        let mut buf = DiagnosticBuffer::new();
        buf.error_limit = 3;
        for i in 0..5 {
            buf.emit(Diagnostic::error(format!("error {}", i), Span::DUMMY));
        }
        // Should have 3 errors + 1 "limit reached" note = 4 diagnostics
        assert_eq!(buf.error_count, 3);
        assert!(buf.diagnostics.len() <= 4);
    }

    #[test]
    fn stage15_13_format_snippet_dummy_span() {
        let result = format_snippet("let x = 42;", &Span::DUMMY);
        assert!(result.is_empty(), "dummy span should produce empty snippet");
    }

    #[test]
    fn stage15_13_format_snippet_real_span() {
        let src = "fn main() { let x = 42; }";
        // Span covering "42" (positions 20-22)
        let span = Span::new(20, 22);
        let result = format_snippet(src, &span);
        assert!(
            result.contains("|"),
            "snippet should contain gutter, got: {}",
            result
        );
        assert!(
            result.contains("42"),
            "snippet should contain the source line, got: {}",
            result
        );
        assert!(
            result.contains("^"),
            "snippet should contain ^ underline, got: {}",
            result
        );
    }

    #[test]
    fn stage15_13_level_display() {
        assert_eq!(Level::Error.to_string(), "error");
        assert_eq!(Level::Warning.to_string(), "warning");
        assert_eq!(Level::Note.to_string(), "note");
        assert_eq!(Level::Help.to_string(), "help");
        assert_eq!(Level::Fatal.to_string(), "fatal error");
        assert_eq!(Level::Bug.to_string(), "internal compiler error");
    }

    // Stage 15.16 tests

    #[test]
    fn stage15_16_error_code_codes() {
        assert_eq!(ErrorCode::Lex.code(), "E001");
        assert_eq!(ErrorCode::Parse.code(), "E100");
        assert_eq!(ErrorCode::Lower.code(), "E200");
        assert_eq!(ErrorCode::Resolve.code(), "E300");
        assert_eq!(ErrorCode::Type.code(), "E400");
        assert_eq!(ErrorCode::Borrow.code(), "E500");
        assert_eq!(ErrorCode::Trait.code(), "E600");
        assert_eq!(ErrorCode::Internal.code(), "E900");
    }

    #[test]
    fn stage15_16_error_code_categories() {
        assert_eq!(ErrorCode::Lex.category(), "lex");
        assert_eq!(ErrorCode::Parse.category(), "parse");
        assert_eq!(ErrorCode::Type.category(), "type");
        assert_eq!(ErrorCode::Borrow.category(), "borrow");
        assert_eq!(ErrorCode::Trait.category(), "trait");
    }

    #[test]
    fn stage15_16_error_code_display() {
        assert_eq!(ErrorCode::Lex.to_string(), "E001");
        assert_eq!(ErrorCode::Type.to_string(), "E400");
        assert_eq!(ErrorCode::Trait.to_string(), "E600");
    }

    #[test]
    fn stage15_16_spanned_trait_lex_error() {
        use crate::lexer::LexError;
        let err = LexError {
            message: "test".to_string(),
            span: Span::new(10, 20),
        };
        assert_eq!(err.span().lo, 10);
        assert_eq!(err.span().hi, 20);
    }

    #[test]
    fn stage15_16_spanned_trait_type_error() {
        use crate::typeck::TypeError;
        let err = TypeError::new("test", Span::new(5, 15));
        assert_eq!(err.span().lo, 5);
        assert_eq!(err.span().hi, 15);
    }

    #[test]
    fn stage15_16_spanned_trait_resolve_error() {
        use crate::resolve::ResolveError;
        let err = ResolveError::new("test", Span::new(0, 10));
        assert_eq!(err.span().lo, 0);
        assert_eq!(err.span().hi, 10);
    }

    #[test]
    fn stage15_16_spanned_trait_borrow_error() {
        use crate::borrowck::BorrowError;
        use crate::borrowck::BorrowErrorKind;
        let err = BorrowError {
            message: "test".to_string(),
            span: Span::new(3, 7),
            kind: BorrowErrorKind::UseAfterMove,
        };
        assert_eq!(err.span().lo, 3);
        assert_eq!(err.span().hi, 7);
    }

    #[test]
    fn stage15_16_spanned_trait_parse_error() {
        use crate::parser::ParseError;
        let err = ParseError {
            message: "test".to_string(),
            span: Span::new(100, 110),
        };
        assert_eq!(err.span().lo, 100);
        assert_eq!(err.span().hi, 110);
    }

    // Stage 15.17 tests

    #[test]
    fn stage15_17_color_config_default() {
        assert_eq!(ColorConfig::default(), ColorConfig::Auto);
    }

    #[test]
    fn stage15_17_colorize_always() {
        let result = colorize("error", Color::Red, ColorConfig::Always);
        assert!(result.contains("\x1b[31m"), "should contain red ANSI code");
        assert!(result.contains("\x1b[0m"), "should contain reset ANSI code");
        assert!(result.contains("error"), "should contain the text");
    }

    #[test]
    fn stage15_17_colorize_never() {
        let result = colorize("error", Color::Red, ColorConfig::Never);
        assert_eq!(result, "error", "Never should produce plain text");
        assert!(!result.contains("\x1b"), "should not contain ANSI codes");
    }

    #[test]
    fn stage15_17_colorize_auto() {
        // Auto resolves to plain text (caller resolves Auto to Always/Never)
        let result = colorize("error", Color::Red, ColorConfig::Auto);
        assert_eq!(result, "error", "Auto should produce plain text by default");
    }

    #[test]
    fn stage15_17_level_color_mapping() {
        assert_eq!(level_color(Level::Error), Color::Red);
        assert_eq!(level_color(Level::Fatal), Color::Red);
        assert_eq!(level_color(Level::Bug), Color::Red);
        assert_eq!(level_color(Level::Warning), Color::Yellow);
        assert_eq!(level_color(Level::Note), Color::Cyan);
        assert_eq!(level_color(Level::Help), Color::Green);
    }

    #[test]
    fn stage15_17_format_snippet_colored_never() {
        let src = "fn main() { let x = 42; }";
        let span = Span::new(20, 22);
        let result = format_snippet_colored(src, &span, Color::Red, ColorConfig::Never);
        assert!(result.contains("42"), "should contain source line");
        assert!(!result.contains("\x1b"), "Never should not have ANSI codes");
    }

    #[test]
    fn stage15_17_format_snippet_colored_always() {
        let src = "fn main() { let x = 42; }";
        let span = Span::new(20, 22);
        let result = format_snippet_colored(src, &span, Color::Red, ColorConfig::Always);
        assert!(result.contains("42"), "should contain source line");
        assert!(
            result.contains("\x1b[31m"),
            "should contain red ANSI code for ^^^"
        );
        assert!(result.contains("\x1b[0m"), "should contain reset ANSI code");
    }

    #[test]
    fn stage15_17_format_with_source_colored_never() {
        let mut buf = DiagnosticBuffer::new();
        buf.emit(Diagnostic::error("test error", Span::new(0, 5)));
        let src = "fn main() { }";
        let sm = crate::session::SourceMap::new(src);
        let result = buf.format_with_source_colored("test.lin", &sm, src, ColorConfig::Never);
        assert!(result.contains("error"), "should contain 'error'");
        assert!(!result.contains("\x1b"), "Never should not have ANSI codes");
    }

    #[test]
    fn stage15_17_format_with_source_colored_always() {
        let mut buf = DiagnosticBuffer::new();
        buf.emit(Diagnostic::error("test error", Span::new(0, 5)));
        let src = "fn main() { }";
        let sm = crate::session::SourceMap::new(src);
        let result = buf.format_with_source_colored("test.lin", &sm, src, ColorConfig::Always);
        assert!(result.contains("\x1b[31m"), "should contain red ANSI code");
        assert!(result.contains("test error"), "should contain message");
    }
}
