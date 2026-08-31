#!/bin/bash
# Stage 18.311 — environment setup for LLVM 22 (llvm-sys 221) + Rust toolchain
# After Stage 18.210, default LLVM is 22.1 (was 19.x).
# Source this file before any cargo command:
#   source scripts/env.sh
#
# Stage 30.11: Now also sets up Rust toolchain (via setup-rust-env.sh).
# The Rust setup is idempotent — if Rust is already installed, it just
# ensures cargo/rustc are in PATH.

# Step 1: Set up Rust toolchain (idempotent)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -f "$SCRIPT_DIR/setup-rust-env.sh" ]; then
    source "$SCRIPT_DIR/setup-rust-env.sh"
fi

# Step 2: Set up LLVM 22 environment
export PATH="/tmp/llvm-22-prefix/bin:$PATH"
export LLVM_SYS_221_PREFIX=/tmp/llvm-22-prefix
export LLVM_LINK_SHARED=1
export LD_LIBRARY_PATH="/tmp/llvm-22-prefix/lib:${LD_LIBRARY_PATH:-}"

echo "LLVM 22 (llvm-sys 221) environment ready."
echo "  LLVM_SYS_221_PREFIX = $LLVM_SYS_221_PREFIX"
echo "  LD_LIBRARY_PATH      = $LD_LIBRARY_PATH"

