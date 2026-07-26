# LLVM 21 Environment Setup (User Environment)

> **Context**: User environment has LLVM 21.1.8 with `llvm-config` in PATH
> **Solution**: Use system LLVM directly — no download needed

## Environment Details

| Item | Value |
|------|-------|
| LLVM version | 21.1.8 |
| Source | System installation (NixOS / package manager) |
| llvm-config | In PATH (`llvm-config --version` → `21.1.8`) |
| Linking | Dynamic (`libLLVM-21.so`) |
| Env vars | `LLVM_SYS_211_PREFIX=<prefix>`, `LLVM_LINK_SHARED=1` |

## Setup Process

1. **Verify system LLVM**:
   ```bash
   llvm-config --version
   # Expected: 21.1.8

   llvm-config --prefix
   # Expected: /nix/store/... or /usr or /usr/local
   ```

2. **Set environment**:
   ```bash
   export LLVM_SYS_211_PREFIX=$(llvm-config --prefix)
   export LLVM_LINK_SHARED=1
   ```

3. **Update Cargo.toml** (if using LLVM 21 instead of 19):
   ```toml
   [dependencies.llvm-sys]
   version = "211"
   features = ["prefer-dynamic"]
   optional = true
   ```

4. **Update .cargo/config.toml**:
   ```toml
   [env]
   LLVM_SYS_211_PREFIX = "<prefix from llvm-config --prefix>"
   LLVM_LINK_SHARED = "1"
   ```

5. **Build**:
   ```bash
   cargo build --lib --features llvm-backend
   ```

## Notes

- LLVM 21 is newer than LLVM 19 (build server). The C API is largely compatible
  between versions 19-21; any API differences will be handled in the `LLVMSysEmitter`
  implementation (Stage 13.5 MUV-2).
- The `llvm-sys` crate version must match the LLVM version:
  - LLVM 19 → `llvm-sys` v191
  - LLVM 21 → `llvm-sys` v211
- If both environments need to be supported, use feature flags or conditional
  compilation based on the LLVM version.

## Cross-Version Compatibility

The Landin compiler's LLVM integration code should use only stable C API functions
that are available in both LLVM 19 and 21. The `llvm-sys` crate provides version-
specific bindings, so the Cargo.toml dependency version determines which LLVM
version is required at build time.

For CI/CD, the build server uses LLVM 19; for local development, the user uses
LLVM 21. The `scripts/setup-llvm-env.sh` script auto-detects which is available.
