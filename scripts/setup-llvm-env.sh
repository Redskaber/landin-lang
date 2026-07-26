#!/bin/bash
# Stage 13.5: LLVM development environment setup
#
# Auto-detects and configures the LLVM development environment.
# Supports LLVM 18-22 (system install or build-server .deb extraction).
#
# Usage: source scripts/setup-llvm-env.sh
# Then:  cargo build --lib --features llvm-backend

set -e

echo "=== LLVM Environment Setup ==="

# Step 1: Detect LLVM
LLVM_MAJOR=""

if command -v llvm-config &> /dev/null; then
    # System llvm-config found (e.g., user environment with LLVM 21)
    LLVM_FULL=$(llvm-config --version)
    LLVM_MAJOR=$(echo "$LLVM_FULL" | cut -d. -f1)
    LLVM_PREFIX=$(llvm-config --prefix)
    echo "Detected system LLVM: $LLVM_FULL (major=$LLVM_MAJOR, prefix=$LLVM_PREFIX)"
elif [ -x /tmp/llvm-19-prefix/bin/llvm-config ]; then
    # Build server LLVM 19 from previous setup
    LLVM_FULL=$(/tmp/llvm-19-prefix/bin/llvm-config --version)
    LLVM_MAJOR=19
    LLVM_PREFIX="/tmp/llvm-19-prefix"
    echo "Detected build-server LLVM: $LLVM_FULL at $LLVM_PREFIX"
fi

if [ -z "$LLVM_MAJOR" ]; then
    echo "No system llvm-config found. Setting up LLVM 19 from .deb packages..."
    LLVM_MAJOR=19
    LLVM_PREFIX="/tmp/llvm-19-prefix"

    cd /tmp
    apt-get download llvm-19-dev llvm-19 2>/dev/null || true

    [ ! -d /tmp/llvm-19-dev ] && dpkg-deb -x llvm-19-dev_1%3a19.1.7-3+b1_amd64.deb llvm-19-dev 2>/dev/null || true
    [ ! -d /tmp/llvm-19 ] && dpkg-deb -x llvm-19_1%3a19.1.7-3+b1_amd64.deb llvm-19 2>/dev/null || true

    rm -rf "$LLVM_PREFIX"
    mkdir -p "$LLVM_PREFIX"
    cp -r /tmp/llvm-19-dev/usr/lib/llvm-19/* "$LLVM_PREFIX/" 2>/dev/null || true
    cp -r /tmp/llvm-19/usr/lib/llvm-19/* "$LLVM_PREFIX/" 2>/dev/null || true

    # Fix C API headers
    rm -rf "$LLVM_PREFIX/include/llvm-c"
    mkdir -p "$LLVM_PREFIX/include/llvm-c"
    cp /tmp/llvm-19-dev/usr/include/llvm-c-19/llvm-c/*.h "$LLVM_PREFIX/include/llvm-c/"

    # Fix C++ headers
    rm -f "$LLVM_PREFIX/include/llvm"
    cp -r /tmp/llvm-19-dev/usr/include/llvm-19/llvm "$LLVM_PREFIX/include/llvm"

    # Patch llvm-config for shared linking
    cp "$LLVM_PREFIX/bin/llvm-config" "$LLVM_PREFIX/bin/llvm-config.orig"
    cat > "$LLVM_PREFIX/bin/llvm-config" << 'WRAPPER'
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

    export PATH="$LLVM_PREFIX/bin:$PATH"
fi

# Step 2: Map major version → llvm-sys crate version
case "$LLVM_MAJOR" in
    18) LLVM_SYS_VER="181" ;;
    19) LLVM_SYS_VER="191" ;;
    20) LLVM_SYS_VER="201" ;;
    21) LLVM_SYS_VER="211" ;;
    22) LLVM_SYS_VER="221" ;;
    *)  echo "ERROR: Unsupported LLVM version: $LLVM_MAJOR"; exit 1 ;;
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
