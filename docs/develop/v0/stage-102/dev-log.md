# Stage 102 开发日志 — LLVMSysEmitter::Drop 释放 module + context (Layer 4)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.640.0 → v0.641.0 |
| 测试数 | 5599 → 5606 (+7 stage102) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 1 src 文件 (Drop impl ~25 LOC) + 1 测试文件 (~110 LOC) |

## 修改文件

### 源文件 (1)
| 文件 | 变更 |
|------|------|
| `src/codegen/llvm/mod.rs` | `Drop for LLVMSysEmitter` 添加 `LLVMDisposeModule` + `LLVMContextDispose` (Layer 4 修复) |

### 测试文件 (1)
| 文件 | 变更 |
|------|------|
| `tests/v0/stage102/plan/emitter_drop_ownership_tests.rs` | 新建 — 7 tests (4 positive + 3 negative) |

### 其他
- `Cargo.toml`: 版本 → 0.641.0
- `tests/all_tests.rs`: 注册 stage102_emitter_drop_ownership_tests
- `src/stdlib/prelude.rs`: 更新 Debug trait 注释 (Layer 4 部分修复, Layer 3 待 Stage 103+)

## 5W2H 根因修复

### WHAT (修复)
`LLVMSysEmitter::Drop` 添加 `LLVMDisposeModule(self.module)` + `LLVMContextDispose(self.ctx)` (在 builder 之后)。

### WHY (Layer 4 根因修复)
Stage 99 RCA Layer 4: Drop 不释放 module + context → LLVM 资源累积 → cargo test 多次 compile() 后 SIGSEGV/SIGABRT.

修复后 LLVM context 正确释放, 与 rustc `LLVMContext` 释放模式一致。

### HOW (通解)
```rust
impl Drop for LLVMSysEmitter {
    fn drop(&mut self) {
        unsafe {
            if !self.builder.is_null() {
                LLVMDisposeBuilder(self.builder);
                self.builder = std::ptr::null_mut();
            }
            // Stage 102: Dispose module + context (Layer 4 fix)
            if !self.module.is_null() {
                LLVMDisposeModule(self.module);
                self.module = std::ptr::null_mut();
            }
            if !self.ctx.is_null() {
                LLVMContextDispose(self.ctx);
                self.ctx = std::ptr::null_mut();
            }
        }
    }
}
```

### 效果
- LLVM 资源不再累积: 3 次 cargo test 全绿, 无 crash
- 测试全绿: 898 lib + 4708 integration = 5606 tests, 0 failures, 9 ignored

## 决策点 (§12 最优>最小, §1.0 原则 1 内存安全决不能妥协)

### 决策 1: Drop 释放 module + context

**选择**: 在 Drop 中添加 `LLVMDisposeModule` + `LLVMContextDispose`。

**替代方案 (拒绝)**:
- 保持现状 (不释放) — 治症不治根, LLVM 资源累积
- 拆分 LLVMSysEmitter 为 Builder + Module 两类型 — 过度设计

**理由** (§1.0 原则 1, §12):
- LLVM C API 标准所有权模式 — module + context 应由创建者释放
- 验证所有 `to_module()` 调用者均在 Drop 前使用 (调用者: tests.rs, main.rs, landinc.rs)
- 与 rustc 设计一致

### 决策 2: 不拆分 LLVMSysEmitter 类型

**选择**: 保持单一 LLVMSysEmitter 类型, 仅修改 Drop impl。

**理由** (§12 最优>最小, §1.0 原则 6 通解>特解):
- 单一 Drop 修复 Layer 4 根因
- 验证调用者安全后即可实施
- 避免 over-engineering

## Stage 102 验证实验: 加 Debug impl 测试 Layer 3 残留

### 实验
在 prelude 中添加 `impl Debug for i32 { fn fmt(&self) -> String { String::from_str("debug_i32") } }`, 跑 cargo test。

### 结果
14 个 cargo test 失败 — Debug impl 加回后仍触发 crash。

### 分析
- Layer 4 (Drop 不释放 context) 已修复 ✓
- Layer 3 (LLVM module 全局变量累积) 仍未完全修复 ✗
- 14 个失败说明 prelude impl body 触发的 LLVM module 全局变量累积不仅由 context 泄漏导致, 还有 module 本身的全局状态 (vtable/dynptr globals + function defs) 在 cargo test 多次 compile() 间累积。

### 新发现 TD
- **TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION** (P2, v0.11+) — LLVM module 全局状态在 cargo test 多次 compile() 间累积, Drop 释放 context 不够, 需进一步隔离每次 compile() 的 module state.

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage102_emitter_drop_releases_resources` | 正向 | Drop 完成, 无 panic |
| `stage102_multiple_emitter_cycles_no_accumulation` | 正向 | 10 次 create/drop 循环无累积 |
| `stage102_to_module_before_drop_safe` | 正向 | to_module() 返回 non-null, Drop 后无 use-after-free |
| `stage102_to_object_file_before_drop_safe` | 正向 | to_object_file 在 Drop 前调用安全 |
| `stage102_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage102_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage102_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4708 tests, 0 failures, 9 ignored — stage102 7 tests included)
- 3 次稳定性验证全绿 (lib + all_tests)

## 新发现 TD

### TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (P2, v0.11+) — LLVM module 全局状态累积

**现象**: 加 Debug impl 后 cargo test 14 个失败 (Drop 修复 Layer 4 不够)。

**根因**: LLVM module 中的全局变量 (vtable/dynptr globals + function defs) 在 cargo test 多次 compile() 间累积。即使 Drop 释放 context, LLVM 内部全局状态 (type table, target machine registry) 仍累积。

**修复方案**:
1. 隔离每次 compile() 的 LLVM module state — 每次创建独立 LLVMContext (已实现)
2. 减少全局状态依赖 — prelude impl body 触发的 vtable/dynptr globals 在 module 间共享
3. 考虑 LLVM 22 的 `LLVMRustExecutionContext` (LLVM 19+ 的 per-thread context)

**影响**: 修复后可重新添加 Debug + PartialOrd impls (Stage 103 前置依赖)。

## 下一步

- **Stage 103**: 调查 TD-PRELUDE-IMPL-BODY-MODULE-ACCUMULATION (Layer 3 残留)
- **Stage 104**: 重新添加 Debug + PartialOrd impls (依赖 Stage 103)
- **TD-MONO-INFER** (P3, v0.11+): type inference back-propagation for FnDef substs
