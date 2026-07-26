#!/bin/bash
# Stage 13.5 MUV-1: LLVM development environment setup
#
# Sets up LLVM development environment from .deb packages (no root required).
# Supports LLVM 19 (build server) and LLVM 21 (user environment).
#
# The script auto-detects available LLVM version:
# - If llvm-config is already in PATH (user env with LLVM 21), use it
# - If /tmp/llvm-19-prefix exists (build server), use LLVM 19
# - Otherwise, download and extract from .deb packages
#
# Usage: source scripts/setup-llvm-env.sh
# Then:  cargo build --lib

set -e

echo "=== LLVM Environment Setup (Stage 13.5 MUV-1) ==="

# Step 1: Detect or set up LLVM
if command -v llvm-config &> /dev/null; then
    # System llvm-config found (user environment with LLVM 21)
    LLVM_VERSION=$(llvm-config --version)
    LLVM_PREFIX=$(llvm-config --prefix)
    echo "Using system LLVM: version=$LLM_VERSION prefix=$LLVM_PREFIX"

    # Determine llvm-sys version to use
    case "$LLVM_VERSION" in
        21.*) LLVM_SYS_VERSION="211" ;;
        20.*) LLVM_SYS_VERSION="201" ;;
        19.*) LLVM_SYS_VERSION="191" ;;
        18.*) LLVM_SYS_VERSION="181" ;;
        *) echo "Unsupported LLVM version: $LLVM_VERSION"; exit 1 ;;
    esac

    export LLVM_SYS_${LLVM_SYS_VERSION}_PREFIX="$LLVM_PREFIX"
    export LLVM_LINK_SHARED=1
    echo "LLVM_SYS_${LLVM_SYS_VERSION}_PREFIX=$LLVM_PREFIX"
    echo "LLVM_LINK_SHARED=1"

else
    # No system llvm-config — set up from .deb packages (build server)
    LLVM_PREFIX="/tmp/llvm-19-prefix"
    LLVM_SYS_VERSION="191"

    echo "No system llvm-config found. Setting up LLVM 19 from .deb packages..."

    # Download if needed
    cd /tmp
    if [ ! -f "llvm-19-dev_1%3a19.1.7-3+b1_amd64.deb" ]; then
        echo "Downloading llvm-19-dev..."
        apt-get download llvm-19-dev
    fi
    if [ ! -f "llvm-19_1%3a19.1.7-3+b1_amd64.deb" ]; then
        echo "Downloading llvm-19..."
        apt-get download llvm-19
    fi

    # Extract if needed
    if [ ! -d "/tmp/llvm-19-dev" ]; then
        echo "Extracting llvm-19-dev..."
        dpkg-deb -x llvm-19-dev_1%3a19.1.7-3+b1_amd64.deb llvm-19-dev
    fi
    if [ ! -d "/tmp/llvm-19" ]; then
        echo "Extracting llvm-19..."
        dpkg-deb -x llvm-19_1%3a19.1.7-3+b1_amd64.deb llvm-19
    fi

    # Build prefix
    echo "Building LLVM prefix at $LLVM_PREFIX..."
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
    echo "Patching llvm-config for shared linking..."
    cp "$LLVM_PREFIX/bin/llvm-config" "$LLVM_PREFIX/bin/llvm-config.orig"
    cat > "$LLVM_PREFIX/bin/llvm-config" << 'LLVM_CONFIG_WRAPPER'
#!/bin/bash
REAL="/tmp/llvm-19-prefix/bin/llvm-config.orig"
if [ "$LLVM_LINK_SHARED" = "1" ]; then
    case "$1" in
        --libfiles|--libnames|--libs)
            echo "libLLVM-19.so"
            exit 0
            ;;
    esac
fi
if [ "$1" = "--system-libs" ]; then
    $REAL "$@" | sed 's|/[^ ]*lib\([^ /]*\)\.so|-l\1|g'
else
    $REAL "$@"
fi
LLVM_CONFIG_WRAPPER
    chmod +x "$LLVM_PREFIX/bin/llvm-config"

    export LLVM_SYS_191_PREFIX="$LLVM_PREFIX"
    export LLVM_LINK_SHARED=1
    export LD_LIBRARY_PATH="$LLVM_PREFIX/lib:/usr/lib/x86_64-linux-gnu:${LD_LIBRARY_PATH:-}"

    echo "LLVM_SYS_191_PREFIX=$LLVM_PREFIX"
    echo "LLVM_LINK_SHARED=1"
    echo "LD_LIBRARY_PATH includes LLVM lib dir"
fi

# Step 2: Verify
echo ""
echo "=== Verification ==="
if command -v llvm-config &> /dev/null; then
    echo "LLM version: $(llvm-config --version)"
    echo "Prefix:      $(llvm-config --prefix)"
else
    echo "LLM version: $($LLVM_PREFIX/bin/llvm-config --version)"
    echo "Prefix:      $LLVM_PREFIX"
    echo "C API:       $(ls $LLVM_PREFIX/include/llvm-c/Core.h 2>&1)"
    echo "Shared lib:  $(ls $LLVM_PREFIX/lib/libLLVM-*.so 2>&1)"
fi
echo "LLVM_SYS_VERSION: $LLVM_SYS_VERSION"
echo ""
echo "=== LLVM environment ready ==="
echo "Build: cargo build --lib"
echo "Test:  cargo test --test all_tests"
