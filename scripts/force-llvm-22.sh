#!/bin/bash
# Stage 127+: Force LLVM 22.1 setup (bypass auto-detection)
#
# Per user directive: "llvm 22.1 (必须22.1)"
# Always installs LLVM 22.1 from Debian pool, ignoring any legacy LLVM 19.
#
# Usage: source scripts/force-llvm-22.sh
# Then:  cargo build --lib --features llvm-backend

set -e

echo "=== Force LLVM 22.1 Setup ==="

LLVM_MAJOR=22
LLVM_PREFIX="/tmp/llvm-22-prefix"

# Capture script directory BEFORE any cd
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")" && pwd)"

cd /tmp

# Download LLVM 22.1 packages from Debian pool
LLVM22_POOL="http://deb.debian.org/debian/pool/main/l/llvm-toolchain-22"
LLVM22_VER=$(curl -sL "$LLVM22_POOL/" 2>/dev/null | grep -o 'llvm-22-dev_[^"]*b2_amd64\.deb' | head -1 | sed 's/llvm-22-dev_//;s/_amd64\.deb//')

if [ -z "$LLVM22_VER" ]; then
    echo "ERROR: Could not find LLVM 22 version from Debian pool"
    exit 1
fi

echo "Downloading LLVM $LLVM22_VER..."

NEED_DOWNLOAD=0
if [ ! -f /tmp/libllvm22.deb ] || [ ! -f /tmp/llvm-22.deb ] || [ ! -f /tmp/llvm-22-dev.deb ]; then
    NEED_DOWNLOAD=1
fi

if [ "$NEED_DOWNLOAD" = "1" ]; then
    curl -sL "$LLVM22_POOL/libllvm22_${LLVM22_VER}_amd64.deb" -o /tmp/libllvm22.deb &
    curl -sL "$LLVM22_POOL/llvm-22_${LLVM22_VER}_amd64.deb" -o /tmp/llvm-22.deb &
    curl -sL "$LLVM22_POOL/llvm-22-dev_${LLVM22_VER}_amd64.deb" -o /tmp/llvm-22-dev.deb &
    wait
    echo "Download complete."
else
    echo "Using cached .deb files."
fi

# Extract all packages to a single directory
rm -rf /tmp/llvm-22-extracted
mkdir -p /tmp/llvm-22-extracted
dpkg-deb -x /tmp/libllvm22.deb /tmp/llvm-22-extracted 2>/dev/null || true
dpkg-deb -x /tmp/llvm-22.deb /tmp/llvm-22-extracted 2>/dev/null || true
dpkg-deb -x /tmp/llvm-22-dev.deb /tmp/llvm-22-extracted 2>/dev/null || true

if [ ! -d /tmp/llvm-22-extracted/usr/lib/llvm-22 ]; then
    echo "ERROR: Extraction failed"
    exit 1
fi

# Install to /tmp/llvm-22-prefix
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

# Copy the actual libLLVM.so.22.1
if [ -f /tmp/llvm-22-extracted/usr/lib/x86_64-linux-gnu/libLLVM.so.22.1 ]; then
    rm -f "$LLVM_PREFIX/lib/libLLVM.so.22.1"
    cp /tmp/llvm-22-extracted/usr/lib/x86_64-linux-gnu/libLLVM.so.22.1 "$LLVM_PREFIX/lib/"
    ln -sf libLLVM.so.22.1 "$LLVM_PREFIX/lib/libLLVM-22.so"
    ln -sf libLLVM.so.22.1 "$LLVM_PREFIX/lib/libLLVM.so"
fi

# Fix libxml2.so.16 dependency
if ! ldconfig -p 2>/dev/null | grep -q "libxml2.so.16"; then
    ln -sf /usr/lib/x86_64-linux-gnu/libxml2.so.2 "$LLVM_PREFIX/lib/libxml2.so.16" 2>/dev/null || true
fi

# Patch llvm-config for shared linking
if [ -f "$LLVM_PREFIX/bin/llvm-config" ]; then
    cp "$LLVM_PREFIX/bin/llvm-config" "$LLVM_PREFIX/bin/llvm-config.orig"
    cat > "$LLVM_PREFIX/bin/llvm-config" << 'WRAPPER'
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

# Set environment variables
export PATH="$LLVM_PREFIX/bin:$PATH"
export LLVM_SYS_221_PREFIX="$LLVM_PREFIX"
export LLVM_LINK_SHARED=1
export LD_LIBRARY_PATH="$LLVM_PREFIX/lib:${LD_LIBRARY_PATH:-}"

# Auto-update .cargo/config.toml + Cargo.toml
if [ -f "$SCRIPT_DIR/switch-llvm-version.sh" ]; then
    echo "Auto-switching config files..."
    bash "$SCRIPT_DIR/switch-llvm-version.sh" 22 2>/dev/null || true
fi

echo ""
echo "=== LLVM 22.1 environment ready ==="
echo "  LLVM_SYS_221_PREFIX = $LLVM_SYS_221_PREFIX"
echo "  LD_LIBRARY_PATH     = $LD_LIBRARY_PATH"
echo "  Version: $($LLVM_PREFIX/bin/llvm-config --version)"
