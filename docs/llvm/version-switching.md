# LLVM Version Switching

> **Script**: `scripts/switch-llvm-version.sh`
> **Purpose**: Auto-detect system LLVM version and update `.cargo/config.toml` + `Cargo.toml`

## Overview

The `switch-llvm-version.sh` script eliminates manual editing when switching between
LLVM environments (e.g., build server LLVM 19 vs user environment LLVM 21).

## Usage

```bash
# Auto-detect (uses llvm-config in PATH)
bash scripts/switch-llvm-version.sh

# Force a specific version
bash scripts/switch-llvm-version.sh 19   # LLVM 19
bash scripts/switch-llvm-version.sh 21   # LLVM 21

# Then build
cargo clean
cargo build --lib --features llvm-backend
cargo test --test all_tests
```

## What It Does

1. **Detects LLVM version**:
   - Checks `llvm-config --version` in PATH (user environment)
   - Falls back to `/tmp/llvm-19-prefix` (build server)
   - Accepts forced version as argument

2. **Updates `.cargo/config.toml`**:
   - Sets `LLVM_SYS_XXX_PREFIX` to the detected prefix
   - Sets `LLVM_LINK_SHARED = "1"`

3. **Updates `Cargo.toml`**:
   - Sets `llvm-sys` version to match detected LLVM major version

## Version Mapping

| LLVM Version | llvm-sys Version | Env Var |
|-------------|-----------------|---------|
| 18.x | 181 | `LLVM_SYS_181_PREFIX` |
| 19.x | 191 | `LLVM_SYS_191_PREFIX` |
| 20.x | 201 | `LLVM_SYS_201_PREFIX` |
| 21.x | 211 | `LLVM_SYS_211_PREFIX` |
| 22.x | 221 | `LLVM_SYS_221_PREFIX` |

## Examples

### Build Server (LLVM 19)

```bash
$ source scripts/setup-llvm-env.sh    # sets up /tmp/llvm-19-prefix
$ bash scripts/switch-llvm-version.sh
Detected system LLVM: 19.1.7 (major=19, prefix=/tmp/llvm-19-prefix)
✅ LLVM version switched to 19 (llvm-sys 191)
```

### User Environment (LLVM 21)

```bash
$ bash scripts/switch-llvm-version.sh
Detected system LLVM: 21.1.8 (major=21, prefix=/nix/store/...-llvm-21.1.8-dev)
✅ LLVM version switched to 21 (llvm-sys 211)
```

### Force Version

```bash
$ bash scripts/switch-llvm-version.sh 21
Forced LLVM version: 21
✅ LLVM version switched to 21 (llvm-sys 211)
```
