//! Compiler session: source files, spans, error collection.

use std::path::PathBuf;
use std::sync::Arc;

/// A source file in the compilation.
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// File path on disk (or synthetic name for REPL/inline).
    pub path: Option<PathBuf>,
    /// File content as UTF-8 string.
    pub src: Arc<str>,
    /// A name used in diagnostics (e.g. "src/main.lin").
    pub name: String,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, src: impl Into<Arc<str>>) -> Self {
        Self {
            path: None,
            src: src.into(),
            name: name.into(),
        }
    }

    pub fn from_path(path: impl Into<PathBuf>) -> std::io::Result<Self> {
        let path = path.into();
        let src = std::fs::read_to_string(&path)?;
        let name = path.display().to_string();
        Ok(Self {
            path: Some(path),
            src: src.into(),
            name,
        })
    }
}

/// Byte position in a source file.
pub type BytePos = u32;

/// A span: [lo, hi) byte range in a source file.
/// Uses u32 to keep size small (4 bytes lo + 4 bytes hi = 8 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub lo: BytePos,
    pub hi: BytePos,
}

impl Span {
    pub fn new(lo: BytePos, hi: BytePos) -> Self {
        debug_assert!(lo <= hi, "span lo > hi: {lo} > {hi}");
        Self { lo, hi }
    }

    /// Dummy span for synthetic nodes.
    pub const DUMMY: Span = Span { lo: 0, hi: 0 };

    pub fn is_dummy(self) -> bool {
        self.lo == 0 && self.hi == 0
    }

    pub fn len(self) -> u32 {
        self.hi - self.lo
    }

    /// Returns true if this span has zero length (`lo == hi`).
    /// Required by clippy::len_without_is_empty whenever `len` is defined.
    pub fn is_empty(self) -> bool {
        self.lo == self.hi
    }

    pub fn contains(self, pos: BytePos) -> bool {
        self.lo <= pos && pos < self.hi
    }

    /// Merge two spans into one covering both.
    pub fn to(self, other: Span) -> Span {
        Span {
            lo: self.lo.min(other.lo),
            hi: self.hi.max(other.hi),
        }
    }
}

impl std::fmt::Display for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.lo, self.hi)
    }
}

/// Line/column info derived from a BytePos.
#[derive(Debug, Clone, Copy)]
pub struct LineCol {
    pub line: usize, // 1-indexed
    pub col: usize,  // 1-indexed
}

/// Map BytePos → LineCol for a source file.
#[derive(Debug, Clone)]
pub struct SourceMap {
    /// Byte offset of each line start (0-indexed: line_starts[0] = 0).
    line_starts: Vec<BytePos>,
}

impl SourceMap {
    pub fn new(src: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self { line_starts }
    }

    pub fn line_col(&self, pos: BytePos) -> LineCol {
        let line_idx = self
            .line_starts
            .binary_search(&pos)
            .unwrap_or_else(|i| i - 1);
        let line_start = self.line_starts[line_idx];
        LineCol {
            line: line_idx + 1,
            col: (pos - line_start) as usize + 1,
        }
    }

    pub fn line_count(&self) -> usize {
        self.line_starts.len()
    }
}

/// Global compilation session.
pub struct Session {
    pub source_file: SourceFile,
    pub source_map: SourceMap,
    pub diagnostics: crate::diagnostics::DiagnosticBuffer,
}

impl Session {
    pub fn new(source_file: SourceFile) -> Self {
        let source_map = SourceMap::new(&source_file.src);
        let diagnostics = crate::diagnostics::DiagnosticBuffer::new();
        Self {
            source_file,
            source_map,
            diagnostics,
        }
    }

    pub fn from_source(name: impl Into<String>, src: impl Into<Arc<str>>) -> Self {
        Self::new(SourceFile::new(name, src))
    }
}
