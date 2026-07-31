# LLVM Version Switching Guide

> **Date**: 2026-07-31  
> **Version**: v0.145.0+  
> **Process**: stage-committee-process.md §29.5

## Overview

Landin supports both LLVM 19 (build server) and LLVM 21 (user environment).
The `scripts/switch-llvm-version.sh` script auto-detects the system LLVM
version and updates `.cargo/config.toml` + `Cargo.toml` accordingly.

## Quick Start

```bash
# Auto-detect system LLVM
bash scripts/switch-llvm-env.sh

# Or force a specific version
bash scripts/switch-llvm-version.sh 19   # LLVM 19
bash scripts/switch-llvm-version.sh 21   # LLVM 21
```

## What the Script Does

1. **Detects LLVM** — checks `llvm-config --version` on the system
2. **Updates `.cargo/config.toml`** — sets `LLVM_SYS_XXX_PREFIX` + `LLVM_LINK_SHARED`
3. **Updates `Cargo.toml`** — changes `llvm-sys` version to match (e.g. `"191"` or `"211"`)

## Version Mapping

| LLVM Version | llvm-sys Crate Version | Env Var Suffix |
|--------------|----------------------|----------------|
| 18           | 181                  | `LLVM_SYS_181_PREFIX` |
| 19           | 191                  | `LLVM_SYS_191_PREFIX` |
| 20           | 201                  | `LLVM_SYS_201_PREFIX` |
| 21           | 211                  | `LLVM_SYS_211_PREFIX` |
| 22           | 221                  | `LLVM_SYS_221_PREFIX` |

## Exact Version Pinning

The default `Cargo.toml` uses an exact pin for LLVM 19:
```toml
version = "=191.1.0"
```

When switching to LLVM 21, the script changes it to:
```toml
version = "211"
```

If you need an exact pin for LLVM 21, manually edit to:
```toml
version = "=211.1.0"
```

## Stage 15.20 Fix

**Bug**: The regex in `switch-llvm-version.sh` didn't match version strings
with dots (e.g. `"=191.1.0"`). The script printed success but didn't actually
update `Cargo.toml`.

**Fix**: Updated the regex to handle all formats:
- `"191"` (bare)
- `"191.1.0"` (with patch)
- `"=191.1.0"` (exact pin)

## Troubleshooting

### "No suitable version of LLVM was found"

This means `cargo` can't find the LLVM library. Check:
1. `LLVM_SYS_XXX_PREFIX` is set to the correct LLVM prefix
2. `LLVM_LINK_SHARED=1` is set
3. `llvm-config` is on `PATH` or at `$LLVM_PREFIX/bin/llvm-config`
4. `libLLVM-XX.so` exists in `$LLVM_PREFIX/lib/`

### "Cargo.toml not changed (regex did not match)"

The regex didn't match the `version = "..."` line. Check:
1. The `[dependencies.llvm-sys]` section exists in `Cargo.toml`
2. The version line is on the line immediately after the section header
3. The version string uses standard TOML format (quoted string)
