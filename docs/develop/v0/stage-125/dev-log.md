# Stage 125 开发日志 — Vtable filtering fix: only emit used vtable/dynptr globals

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.653.0 → v0.656.0 |
| 测试数 | 5742 |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | +80 src + test fixes |

## 5W2H

### WHAT
Stage 125 implemented vtable/dynptr filtering: only emit vtable and dynptr
globals for (trait, type) pairs that are actually used in `dyn Trait` method
calls in the program's MIR. This mirrors rustc's approach.

### Key Insight (from rustc research)
Rust only emits vtable globals for impls that are actually referenced.
Landin was emitting ALL prelude impl vtables (~20 globals) for every
program, regardless of whether `dyn Trait` was used. This made the LLVM
module unnecessarily complex → DenseMap hash collisions → non-deterministic
SIGSEGV.

### Implementation
1. `collect_used_dyn_trait_pairs()` — walks MIR bodies, finds
   `TerminatorKind::Call { dyn_trait_call: Some(info), .. }` terminators,
   collects (trait_name, type_name) pairs.
2. `emit_vtables_filtered()` — only emits vtable globals for used pairs.
3. `emit_dyn_trait_ptrs_filtered()` — only emits dynptr globals for used pairs.
4. `Emitter::is_llvm_emitter()` — new trait method (default: false).
   LLVMSysEmitter overrides to return true.
5. Pipeline: if `is_llvm_emitter() && !used_dyn_traits.is_empty()`,
   use filtered emission. Otherwise (TextEmitter or no dyn Trait),
   emit all vtables (preserves test compatibility).

### §3.2 验收
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests ✓ (4844 tests, 0 failures, 3/3 stable)
- Total: 5742 tests, 0 failures

## Stage Summary
- Vtable/dynptr filtering implemented (only emit used globals for LLVM path)
- TextEmitter path preserved (all vtables emitted for IR structure tests)
- 3/3 stable runs (0 failures)
- v0.656.0
