# LLVM Integration Documentation

> **Stage 13.5 MUV-1**: LLVM library integration for Landin compiler
> **Date**: 2026-07-26
> **Supported versions**: LLVM 19.1.7 (build server), LLVM 21.1.8 (user environment)

## Overview

Landin compiler integrates with LLVM via the `llvm-sys` Rust crate, which provides
Rust bindings to the LLVM C API. This enables:
- Building LLVM modules (instead of text IR strings)
- Generating object files (.o)
- Target machine configuration
- Future JIT execution support

## Environment Setup

### Option 1: System LLVM (LLVM 21 — user environment)

If `llvm-config` is in PATH (e.g., LLVM 21.1.8 installed via system package manager):

```bash
# Verify
llvm-config --version
# Expected: 21.1.8

# Set environment (or add to .bashrc)
export LLVM_SYS_211_PREFIX=$(llvm-config --prefix)
export LLVM_LINK_SHARED=1

# Build
cargo build --lib --features llvm-backend
```

### Option 2: Downloaded LLVM 19 (build server — no root)

If no system LLVM is available, use `scripts/setup-llvm-env.sh`:

```bash
source scripts/setup-llvm-env.sh
# Downloads llvm-19-dev + llvm-19 .deb packages
# Extracts to /tmp/llvm-19-prefix
# Patches llvm-config for shared linking
# Sets LLVM_SYS_191_PREFIX + LLVM_LINK_SHARED

cargo build --lib --features llvm-backend
```

### Option 3: Manual LLVM installation

Install LLVM development packages:

```bash
# Debian/Ubuntu
sudo apt install llvm-21-dev

# Or build from source (https://github.com/llvm/llvm-project)
git clone https://github.com/llvm/llvm-project.git
cd llvm-project
cmake -B build -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DLLVM_ENABLE_PROJECTS=llvm \
  -DLLVM_BUILD_LLVM_DYLIB=ON \
  -DLLVM_LINK_LLVM_DYLIB=ON \
  llvm/
cmake --build build
sudo cmake --install build

# Set environment
export LLVM_SYS_211_PREFIX=/usr/local
export LLVM_LINK_SHARED=1
```

## llvm-sys Version Mapping

| LLVM Version | llvm-sys Version | Env Var |
|-------------|-----------------|---------|
| 18.x | 181 | `LLVM_SYS_181_PREFIX` |
| 19.x | 191 | `LLVM_SYS_191_PREFIX` |
| 20.x | 201 | `LLVM_SYS_201_PREFIX` |
| 21.x | 211 | `LLVM_SYS_211_PREFIX` |

## Cargo.toml Configuration

```toml
[dependencies.llvm-sys]
version = "191"  # or "211" for LLVM 21
features = ["prefer-dynamic"]
optional = true

[features]
default = []
llvm-backend = ["llvm-sys"]
```

## Build Commands

```bash
# Without LLVM (text IR only — current default)
cargo build --lib

# With LLVM (module building + object files)
source scripts/setup-llvm-env.sh
cargo build --lib --features llvm-backend

# Test
cargo test --test all_tests --features llvm-backend
```

## Verification

```bash
# Verify LLVM is linked
cargo build --lib --features llvm-backend 2>&1 | grep "llvm-sys"
# Should show: Compiling llvm-sys vXXX

# Verify llvm-config works
llvm-config --version  # or /tmp/llvm-19-prefix/bin/llvm-config --version
```

## Architecture

```
Landin source code
      ↓
  Lexer → Parser → AST → HIR → MIR → typeck → borrowck
      ↓
  Codegen (TextEmitter → String LLVM IR)     ← Current (no LLVM)
  Codegen (LLVMSysEmitter → LLVM Module)     ← Stage 13.5 MUV-2 (planned)
      ↓
  LLVM Module → TargetMachine → Object (.o)  ← Stage 13.5 MUV-3 (planned)
      ↓
  Linker (cc/clang) → Executable             ← Stage 13.6 (planned)
      ↓
  --run flag → Execute program               ← Stage 13.8 (planned)
```

## References

- LLVM Project: https://github.com/llvm/llvm-project
- LLVM Website: https://llvm.org
- llvm-sys crate: https://crates.io/crates/llvm-sys
- LLVM C API: https://llvm.org/doxygen/group_llvm_c.html

## Documentation Index

| Document | Stage | Description |
|----------|-------|-------------|
| `README.md` (this file) | 13.5+ | LLVM integration overview + environment setup |
| `version-switching.md` | 13.5 MUV-1 | Switching between LLVM 19 (build server) and LLVM 21 (user env) |
| `llvm-19-build-server-setup.md` | 13.5 MUV-1 | LLVM 19 setup on build server (no root) via `setup-llvm-env.sh` |
| `llvm-21-user-environment-setup.md` | 13.5 MUV-1 | LLVM 21 setup on user environment with system `llvm-config` |
| `stage-13.6-object-file-generation.md` | 13.6 | `--emit-obj` flag — LLVM Module → TargetMachine → .o file |
| `execution-pipeline.md` | 13.8-13.10 | End-to-end pipeline: Landin → MIR → LLVMSysEmitter → .o → cc → exe → run |
| `stage-13.13-println-inline-emission.md` | 13.13 | Inline `println!` emission via `StatementKind::Println` (fixes Stage 13.12 ordering bug) |
| `stage-13.14-eprintln-stderr-emission.md` | 13.14 | `eprintln!`/`eprint!` stderr emission via `__landin_eprint` helper (closes Stage 13.13 deferral) |
| `stage-13.16-format-args.md` | **13.16** | **Format args (`println!("{}", x)`) — extends Println variant to carry args, builds C printf format string** (first real I/O feature) |

## Known Issues

### Build Server (LLVM 19)

The build server has LLVM 19.1.7 runtime (`libLLVM-19.so`) but no dev packages
(`llvm-config`, headers). The `scripts/setup-llvm-env.sh` script downloads and
extracts the dev packages without root access.

The `llvm-config` binary is patched to return `libLLVM-19.so` for `--libfiles`/
`--libnames`/`--libs` when `LLVM_LINK_SHARED=1` is set, avoiding the need for
all static libraries (e.g., `libPolly.a`).

### User Environment (LLVM 21)

The user environment has LLVM 21.1.8 with `llvm-config` in PATH. To use LLVM 21:

1. Update `Cargo.toml`: `version = "211"` instead of `"191"`
2. Update `.cargo/config.toml`: `LLVM_SYS_211_PREFIX` instead of `LLVM_SYS_191_PREFIX`
3. Set `LLVM_LINK_SHARED=1` in shell or `.cargo/config.toml`
4. Build: `cargo build --lib --features llvm-backend`
