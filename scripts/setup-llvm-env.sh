#!/bin/bash
# Stage 13.5: LLVM development environment setup
#
# Auto-detects and configures the LLVM development environment.
# Supports LLVM 18-22 (system install or build-server .deb extraction).
# Default: LLVM 22.1 (221) — Stage 18.210 upgrade.
#
# Usage: source scripts/setup-llvm-env.sh
# Then:  cargo build --lib --features llvm-backend

set -e

echo "=== LLVM Environment Setup ==="

# Step 1: Detect LLVM
LLVM_MAJOR=""

if command -v llvm-config &>/dev/null; then
  # System llvm-config found (e.g., user environment with LLVM 21/22)
  LLVM_FULL=$(llvm-config --version)
  LLVM_MAJOR=$(echo "$LLVM_FULL" | cut -d. -f1)
  LLVM_PREFIX=$(llvm-config --prefix)
  echo "Detected system LLVM: $LLVM_FULL (major=$LLVM_MAJOR, prefix=$LLVM_PREFIX)"
elif [ -x /tmp/llvm-22-prefix/bin/llvm-config ]; then
  # Build server LLVM 22 from previous setup
  LLVM_FULL=$(/tmp/llvm-22-prefix/bin/llvm-config --version)
  LLVM_MAJOR=22
  LLVM_PREFIX="/tmp/llvm-22-prefix"
  echo "Detected build-server LLVM: $LLVM_FULL at $LLVM_PREFIX"
elif [ -x /tmp/llvm-19-prefix/bin/llvm-config ]; then
  # Legacy build server LLVM 19 fallback
  LLVM_FULL=$(/tmp/llvm-19-prefix/bin/llvm-config --version)
  LLVM_MAJOR=19
  LLVM_PREFIX="/tmp/llvm-19-prefix"
  echo "Detected legacy build-server LLVM: $LLVM_FULL at $LLVM_PREFIX"
fi

# Stage 18.210: Default to LLVM 22.1 if no system LLVM found.
# LLVM 22.1 is the current default target for llvm-sys 221.
if [ -z "$LLVM_MAJOR" ]; then
  echo "No system llvm-config found. Setting up LLVM 22 from .deb packages..."
  LLVM_MAJOR=22
  LLVM_PREFIX="/tmp/llvm-22-prefix"

  cd /tmp
  # Download LLVM 22 packages directly from Debian pool
  LLVM22_POOL="http://deb.debian.org/debian/pool/main/l/llvm-toolchain-22"
  # Find the latest version available
  LLVM22_VER=$(curl -sL "$LLVM22_POOL/" 2>/dev/null | grep -o 'llvm-22-dev_[^"]*b2_amd64\.deb' | head -1 | sed 's/llvm-22-dev_//;s/_amd64\.deb//')

  if [ -n "$LLVM22_VER" ]; then
    echo "Downloading LLVM 22.1 packages (version $LLVM22_VER)..."
    curl -sL "$LLVM22_POOL/libllvm22_${LLVM22_VER}_amd64.deb" -o /tmp/libllvm22.deb &
    curl -sL "$LLVM22_POOL/llvm-22_${LLVM22_VER}_amd64.deb" -o /tmp/llvm-22.deb &
    curl -sL "$LLVM22_POOL/llvm-22-dev_${LLVM22_VER}_amd64.deb" -o /tmp/llvm-22-dev.deb &
    wait

    # Extract all packages to a single directory
    rm -rf /tmp/llvm-22-extracted
    mkdir -p /tmp/llvm-22-extracted
    dpkg-deb -x /tmp/libllvm22.deb /tmp/llvm-22-extracted 2>/dev/null || true
    dpkg-deb -x /tmp/llvm-22.deb /tmp/llvm-22-extracted 2>/dev/null || true
    dpkg-deb -x /tmp/llvm-22-dev.deb /tmp/llvm-22-extracted 2>/dev/null || true
  else
    # Fallback: try apt-get download
    apt-get download llvm-22-dev llvm-22 2>/dev/null || true
    for deb in /tmp/llvm-22-dev_*.deb; do
      [ -f "$deb" ] && dpkg-deb -x "$deb" /tmp/llvm-22-extracted 2>/dev/null || true
    done
    for deb in /tmp/llvm-22_*.deb; do
      [ -f "$deb" ] && dpkg-deb -x "$deb" /tmp/llvm-22-extracted 2>/dev/null || true
    done
  fi

  # If LLVM 22 packages not available, fall back to LLVM 19 (build server default)
  if [ ! -d /tmp/llvm-22-extracted ]; then
    echo "LLVM 22 packages not available. Falling back to LLVM 19..."
    LLVM_MAJOR=19
    LLVM_PREFIX="/tmp/llvm-19-prefix"

    apt-get download llvm-19-dev llvm-19 2>/dev/null || true

    [ ! -d /tmp/llvm-19-dev ] && dpkg-deb -x /tmp/llvm-19-dev_*.deb /tmp/llvm-19-dev 2>/dev/null || true
    [ ! -d /tmp/llvm-19 ] && dpkg-deb -x /tmp/llvm-19_*.deb /tmp/llvm-19 2>/dev/null || true
  fi

  if [ "$LLVM_MAJOR" = "22" ]; then
    rm -rf "$LLVM_PREFIX"
    mkdir -p "$LLVM_PREFIX"
    cp -r /tmp/llvm-22-extracted/usr/lib/llvm-22/* "$LLVM_PREFIX/" 2>/dev/null || true

    # Fix C API headers
    rm -rf "$LLVM_PREFIX/include/llvm-c"
    mkdir -p "$LLVM_PREFIX/include/llvm-c"
    cp /tmp/llvm-22-extracted/usr/include/llvm-c-22/llvm-c/*.h "$LLVM_PREFIX/include/llvm-c/" 2>/dev/null || true

    # Fix C++ headers
    rm -f "$LLVM_PREFIX/include/llvm"
    cp -r /tmp/llvm-22-extracted/usr/include/llvm-22/llvm "$LLVM_PREFIX/include/llvm" 2>/dev/null || true

    # Copy the actual libLLVM.so.22.1 (not the broken symlink)
    if [ -f /tmp/llvm-22-extracted/usr/lib/x86_64-linux-gnu/libLLVM.so.22.1 ]; then
      rm -f "$LLVM_PREFIX/lib/libLLVM.so.22.1"
      cp /tmp/llvm-22-extracted/usr/lib/x86_64-linux-gnu/libLLVM.so.22.1 "$LLVM_PREFIX/lib/"
      # Fix symlinks to point to the real file
      ln -sf libLLVM.so.22.1 "$LLVM_PREFIX/lib/libLLVM-22.so"
      ln -sf libLLVM.so.22.1 "$LLVM_PREFIX/lib/libLLVM.so"
    fi

    # Fix libxml2.so.16 dependency (LLVM 22 needs libxml2 v2.15+,
    # but Debian trixie only has v2.9.14 with soname .so.2)
    if ! ldconfig -p 2>/dev/null | grep -q "libxml2.so.16"; then
      ln -sf /usr/lib/x86_64-linux-gnu/libxml2.so.2 "$LLVM_PREFIX/lib/libxml2.so.16" 2>/dev/null || true
    fi

    # Patch llvm-config for shared linking
    if [ -f "$LLVM_PREFIX/bin/llvm-config" ]; then
      cp "$LLVM_PREFIX/bin/llvm-config" "$LLVM_PREFIX/bin/llvm-config.orig"
      cat >"$LLVM_PREFIX/bin/llvm-config" <<'WRAPPER'
#!/bin/bash
REAL="/tmp/llvm-22-prefix/bin/llvm-config.orig"
if [ "$LLVM_LINK_SHARED" = "1" ]; then
    case "$1" in
        --libfiles|--libnames|--libs) echo "libLLVM-22.so"; exit 0 ;;
    esac
fi
if [ "$1" = "--system-libs" ]; then
    $REAL "$@" | sed 's|/[^ ]*lib\([^ /]*\)\.so|-l\1|g'
else
    $REAL "$@"
fi
WRAPPER
      chmod +x "$LLVM_PREFIX/bin/llvm-config"
    fi
  else
    # LLVM 19 fallback setup (same as before)
    rm -rf "$LLVM_PREFIX"
    mkdir -p "$LLVM_PREFIX"
    cp -r /tmp/llvm-19-dev/usr/lib/llvm-19/* "$LLVM_PREFIX/" 2>/dev/null || true
    cp -r /tmp/llvm-19/usr/lib/llvm-19/* "$LLVM_PREFIX/" 2>/dev/null || true

    rm -rf "$LLVM_PREFIX/include/llvm-c"
    mkdir -p "$LLVM_PREFIX/include/llvm-c"
    cp /tmp/llvm-19-dev/usr/include/llvm-c-19/llvm-c/*.h "$LLVM_PREFIX/include/llvm-c/"

    rm -f "$LLVM_PREFIX/include/llvm"
    cp -r /tmp/llvm-19-dev/usr/include/llvm-19/llvm "$LLVM_PREFIX/include/llvm"

    cp "$LLVM_PREFIX/bin/llvm-config" "$LLVM_PREFIX/bin/llvm-config.orig"
    cat >"$LLVM_PREFIX/bin/llvm-config" <<'WRAPPER'
#!/bin/bash
REAL="/tmp/llvm-19-prefix/bin/llvm-config.orig"
if [ "$LLVM_LINK_SHARED" = "1" ]; then
    case "$1" in
        --libfiles|--libnames|--libs) echo "libLLVM-19.so"; exit 0 ;;
    esac
fi
if [ "$1" = "--system-libs" ]; then
    $REAL "$@" | sed 's|/[^ ]*lib\([^ /]*\)\.so|-l\1|g'
else
    $REAL "$@"
fi
WRAPPER
    chmod +x "$LLVM_PREFIX/bin/llvm-config"
  fi

  export PATH="$LLVM_PREFIX/bin:$PATH"
fi

# Step 2: Map major version → llvm-sys crate version
case "$LLVM_MAJOR" in
18) LLVM_SYS_VER="181" ;;
19) LLVM_SYS_VER="191" ;;
20) LLVM_SYS_VER="201" ;;
21) LLVM_SYS_VER="211" ;;
22) LLVM_SYS_VER="221" ;;
*)
  echo "ERROR: Unsupported LLVM version: $LLVM_MAJOR"
  exit 1
  ;;
esac

# Step 3: Set environment variables
export LLVM_SYS_${LLVM_SYS_VER}_PREFIX="$LLVM_PREFIX"
export LLVM_LINK_SHARED=1
export LD_LIBRARY_PATH="$LLVM_PREFIX/lib:${LD_LIBRARY_PATH:-}"

echo "LLVM major     : $LLVM_MAJOR"
echo "llvm-sys ver   : $LLVM_SYS_VER"
echo "LLVM prefix    : $LLVM_PREFIX"
echo "LLVM_LINK_SHARED: 1"

# Step 4: Auto-update .cargo/config.toml + Cargo.toml
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"
if [ -f "$SCRIPT_DIR/switch-llvm-version.sh" ]; then
  echo "Auto-switching config files..."
  bash "$SCRIPT_DIR/switch-llvm-version.sh" "$LLVM_MAJOR" 2>/dev/null || true
fi

echo ""
echo "=== LLVM $LLVM_MAJOR environment ready ==="
echo "Build: cargo build --lib --features llvm-backend"
echo "Test:  cargo test --test all_tests"
