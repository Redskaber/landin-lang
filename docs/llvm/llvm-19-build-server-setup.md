# LLVM 19 Environment Setup (Build Server)

> **Context**: Build server has LLVM 19.1.7 runtime but no dev packages (no root access)
> **Solution**: Download .deb packages + extract + patch llvm-config

## Environment Details

| Item | Value |
|------|-------|
| LLVM version | 19.1.7 |
| Source | Debian `trixie` repository |
| Packages | `llvm-19-dev` (43 MB) + `llvm-19` (16 MB) |
| Prefix | `/tmp/llvm-19-prefix` |
| Linking | Dynamic (`libLLVM-19.so`) |
| Env vars | `LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix`, `LLVM_LINK_SHARED=1` |

## Setup Process

1. **Download .deb packages** (no root needed):
   ```bash
   cd /tmp
   apt-get download llvm-19-dev llvm-19
   ```

2. **Extract**:
   ```bash
   dpkg-deb -x llvm-19-dev_*.deb llvm-19-dev
   dpkg-deb -x llvm-19_*.deb llvm-19
   ```

3. **Build prefix**:
   ```bash
   mkdir -p /tmp/llvm-19-prefix
   cp -r /tmp/llvm-19-dev/usr/lib/llvm-19/* /tmp/llvm-19-prefix/
   cp -r /tmp/llvm-19/usr/lib/llvm-19/* /tmp/llvm-19-prefix/
   ```

4. **Fix C API headers** (path mismatch):
   ```bash
   mkdir -p /tmp/llvm-19-prefix/include/llvm-c
   cp /tmp/llvm-19-dev/usr/include/llvm-c-19/llvm-c/*.h /tmp/llvm-19-prefix/include/llvm-c/
   ```

5. **Fix C++ headers** (broken symlink):
   ```bash
   rm -f /tmp/llvm-19-prefix/include/llvm
   cp -r /tmp/llvm-19-dev/usr/include/llvm-19/llvm /tmp/llvm-19-prefix/include/llvm
   ```

6. **Patch llvm-config** (shared linking):
   ```bash
   cp /tmp/llvm-19-prefix/bin/llvm-config /tmp/llvm-19-prefix/bin/llvm-config.orig
   # Write wrapper that returns libLLVM-19.so for --libfiles/--libnames/--libs
   # when LLVM_LINK_SHARED=1
   ```

## Patched llvm-config

The original `llvm-config` returns all static library names (e.g., `libLLVMCore.a`,
`libPolly.a`, etc.) for `--libfiles`/`--libnames`. This requires all static libs
to be present, which they are — but `libPolly.a` causes linking issues.

The patched wrapper returns `libLLVM-19.so` (the shared library) for all lib-related
queries when `LLVM_LINK_SHARED=1` is set, using dynamic linking instead.

## Reproducible Setup

```bash
source scripts/setup-llvm-env.sh
```

This script performs all steps above automatically.

## Verification

```bash
/tmp/llvm-19-prefix/bin/llvm-config --version
# Expected: 19.1.7

ls /tmp/llvm-19-prefix/include/llvm-c/Core.h
# Expected: exists

ls /tmp/llvm-19-prefix/lib/libLLVM-19.so
# Expected: exists

LLVM_SYS_191_PREFIX=/tmp/llvm-19-prefix LLVM_LINK_SHARED=1 cargo build --lib --features llvm-backend
# Expected: success
```
