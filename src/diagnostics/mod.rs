//! Diagnostics: error/warning collection and formatting.

use crate::session::{LineCol, Span};
use std::fmt;

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

/// Buffer collecting all diagnostics during compilation.
#[derive(Debug, Default)]
pub struct DiagnosticBuffer {
    pub diagnostics: Vec<Diagnostic>,
    pub error_count: usize,
    pub warning_count: usize,
    pub error_limit: usize,
}

impl DiagnosticBuffer {
    pub fn new() -> Self {
        Self {
            error_limit: 128,
            ..Default::default()
        }
    }

    pub fn emit(&mut self, diag: Diagnostic) {
        match diag.level {
            Level::Error | Level::Fatal | Level::Bug => self.error_count += 1,
            Level::Warning => self.warning_count += 1,
            _ => {}
        }
        self.diagnostics.push(diag);
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
}
