#!/bin/bash
# Stage 30.11: Rust toolchain setup script
#
# Auto-detects and configures the Rust toolchain.
# Installs Rust via rustup if not present.
#
# Usage: source scripts/setup-rust-env.sh
# Then:  cargo build --release --features llvm-backend

set -e

echo "=== Rust Toolchain Setup ==="

# Step 1: Check if cargo is already available
if command -v cargo &>/dev/null; then
  CARGO_PATH=$(which cargo)
  RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")
  CARGO_VERSION=$(cargo --version 2>/dev/null || echo "unknown")
  echo "Detected existing Rust: $RUST_VERSION"
  echo "Detected cargo: $CARGO_VERSION at $CARGO_PATH"

  # Ensure cargo is in PATH
  CARGO_BIN_DIR=$(dirname "$CARGO_PATH")
  if [[ ":$PATH:" != *":$CARGO_BIN_DIR:"* ]]; then
    export PATH="$CARGO_BIN_DIR:$PATH"
    echo "Added $CARGO_BIN_DIR to PATH"
  fi
  return 0 2>/dev/null || exit 0
fi

# Step 2: Check for rustup-installed Rust in common locations
CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
RUSTUP_HOME="${RUSTUP_HOME:-$HOME/.rustup}"

if [ -x "$CARGO_HOME/bin/cargo" ]; then
  export PATH="$CARGO_HOME/bin:$PATH"
  export CARGO_HOME
  export RUSTUP_HOME
  RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")
  echo "Found rustup-installed Rust: $RUST_VERSION at $CARGO_HOME/bin"
  return 0 2>/dev/null || exit 0
fi

# Step 3: Install Rust via rustup
echo "No Rust installation found. Installing Rust via rustup..."

# Install rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable

# Source the cargo env
if [ -f "$CARGO_HOME/env" ]; then
  source "$CARGO_HOME/env"
fi

export PATH="$CARGO_HOME/bin:$PATH"

# Verify installation
RUST_VERSION=$(rustc --version 2>/dev/null || echo "unknown")
CARGO_VERSION=$(cargo --version 2>/dev/null || echo "unknown")
echo "Installed Rust: $RUST_VERSION"
echo "Installed cargo: $CARGO_VERSION"

echo "=== Rust Toolchain Setup Complete ==="
