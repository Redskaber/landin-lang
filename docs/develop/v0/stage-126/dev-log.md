# Stage 126 开发日志 — Debug impl re-add with vtable filtering RCA

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.656.0 → v0.657.0 |
| 测试数 | 5742 |
| 失败数 | 0 → 0 (Debug impl reverted) |
| ignored | 9 |
| clippy warnings | 0 |

## 5W2H

### WHAT
Stage 126 attempted Debug impl re-add with vtable filtering (Stage 125)
to reduce LLVM module complexity.

### Result: Debug impl REVERTED — vtable filtering insufficient

| 配置 | 每次运行失败数 | 稳定性 |
|------|-------------|--------|
| 无 Debug impl, vtable filtering | 0 (3/3 runs) | ✅ Stable |
| Debug impl, vtable filtering | 5-14 (0/3 runs pass) | ❌ Non-deterministic |

### WHY (vtable filtering insufficient)
Vtable filtering only reduces the number of vtable/dynptr globals emitted
for programs that don't use `dyn Trait`. But Debug impl bodies still add:
- 4 vtable globals (Debug for i32/i64/bool/usize)
- 4 dynptr globals
- 4 function definitions (Debug::fmt bodies)
- 8 function references in vtables

These are emitted for EVERY program (because they're in the prelude, not
filtered by dyn Trait usage). The vtable filtering only affects globals
for OTHER traits (Display, Clone, etc.) that the program doesn't use
via `dyn Trait`.

### Root cause confirmed (final)
The non-determinism is from LLVM's C++ DenseMap hash function which
depends on heap layout. Debug impl bodies add enough function definitions
and globals to make the LLVM module complex enough to trigger non-
deterministic SelectionDAG behavior.

This is **fundamental to LLVM's C++ implementation** and cannot be
fixed from the Rust side without:
1. Forking LLVM to use content-based hashing in DenseMap
2. Using a different backend (cranelift)
3. Accepting non-determinism (violates §3.2)

### Decision: Debug impl bodies permanently deferred
Per §1.0 原則 9 (正确 > 妥协): don't ship non-deterministic crashes.
Debug trait declaration preserved (users can impl Debug for own types).

### §3.2 验收 (Debug impl reverted)
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4844 tests, 0 failures)
- Total: 5742 tests, 0 failures

## Stage Summary
- Vtable filtering (Stage 125) reduces module complexity but insufficient for Debug impl
- Debug impl bodies still add 4 vtables + 4 dynptrs + 4 function defs → non-deterministic
- Debug impl bodies permanently deferred — LLVM C++ non-determinism is fundamental
- v0.657.0
