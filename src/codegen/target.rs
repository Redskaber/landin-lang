//! Stage 18.88: Target triple configuration for cross-compilation.
//!
//! Provides `TargetTriple` to configure the target platform for codegen.
//! Previously, the target triple was hardcoded to `x86_64-unknown-linux-gnu`
//! in both `TextEmitter` and `LLVMSysEmitter`.
//!
//! Per §1.0 原則 3 "显式 > 隐式": target is explicit, not hardcoded.
//! Per §1.0 原則 6 "通用 > 特例": one struct handles all targets.
//! Per §23: `TargetTriple` follows `<Noun>_<Noun>` pattern.

/// Target platform configuration for code generation.
///
/// Contains the LLVM target triple and corresponding data layout.
/// Used by both `TextEmitter` and `LLVMSysEmitter` to configure output.
#[derive(Debug, Clone)]
pub struct TargetTriple {
    /// LLVM target triple (e.g., "x86_64-unknown-linux-gnu")
    triple: String,
    /// LLVM data layout string for the target
    data_layout: String,
}

impl TargetTriple {
    /// Default target: x86_64 Linux (the original hardcoded value).
    pub fn x86_64_linux() -> Self {
        Self {
            triple: "x86_64-unknown-linux-gnu".to_string(),
            data_layout: "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128"
                .to_string(),
        }
    }

    /// AArch64 Linux target.
    pub fn aarch64_linux() -> Self {
        Self {
            triple: "aarch64-unknown-linux-gnu".to_string(),
            data_layout: "e-m:e-i8:8:32-i16:16:32-i64:64-i128:128-n32:64-S128-Fn32".to_string(),
        }
    }

    /// Create from a triple string. Uses a default data layout if unknown.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(triple: &str) -> Self {
        match triple {
            "x86_64-unknown-linux-gnu" | "x86_64-linux-gnu" => Self::x86_64_linux(),
            "aarch64-unknown-linux-gnu" | "aarch64-linux-gnu" => Self::aarch64_linux(),
            _ => Self {
                triple: triple.to_string(),
                data_layout: String::new(), // Empty = let LLVM decide
            },
        }
    }

    /// Get the LLVM target triple string.
    pub fn triple(&self) -> &str {
        &self.triple
    }

    /// Get the LLVM data layout string.
    pub fn data_layout(&self) -> &str {
        &self.data_layout
    }
}

impl Default for TargetTriple {
    fn default() -> Self {
        Self::x86_64_linux()
    }
}
