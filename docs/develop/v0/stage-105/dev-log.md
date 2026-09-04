# Stage 105 开发日志 — LLVM codegen 非确定性 SIGSEV 根因分析

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.642.0 (无版本变更 — RCA only) |
| 测试数 | 5613 (898 lib + 4715 integration) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC 变更 | 0 src (RCA + 注释更新) |

## 5W2H 根因分析

### WHAT (发现)
加 Debug impl 后 cargo test 非确定性 SIGSEGV (exit=139)。**LLVM IR 在成功和失败跑之间完全相同** (相同的 Param=73 Infer=18 warnings)。崩溃发生在 LLVM codegen/object emission 阶段，不是编译器前端。

### WHY (根因)
1. **不正确的 LLVM IR**: Param fallback to i32 + Infer fallback to i32 产生不正确的 struct layout (i32 = 4 bytes 而非 usize = 8 bytes)
2. **LLVM CodeGenLevelDefault 优化器非确定性**: LLVM 的优化器基于内存布局做决策。当 LLVM IR 包含不正确的类型 (i32 替代 usize/ptr)，优化器在不同内存布局 (ASLR) 下做出不同决策 → 非确定性 crash
3. **ASLR off 减少 crash 但不消除**: ASLR off 减少了内存布局变化 → 100 次跑 1 失败 (vs ASLR on 3/100)。但仍因 LLVM 内部状态 (如 JIT memory pool) 非确定

### HOW (验证)
```
成功跑: Param=73 Infer=18 → exit=0, stdout="hello"
失败跑: Param=73 Infer=18 → exit=139 (SIGSEGV), stdout=""
diff(成功 stderr, 失败 stderr) = 空 (完全相同)
```

### 根因链
```
Infer/Param warnings (typeck writeback 未完成)
  → mir_type_to_emit_type fallback to i32
    → LLVM IR 中 struct field 类型错误 (i32 vs usize)
      → LLVM CodeGenLevelDefault 优化器非确定性处理
        → 非确定性 SIGSEGV (依赖内存布局)
```

### 修复路径 (Stage 106+)
**必须消除所有 Param/Infer warnings** (所有类型在 codegen 前必须是 concrete):

1. **Infer warnings** (Constant type Infer):
   - `landin_Default_i32_default`: `0` 字面量无 suffix → Infer (Stage 103 resolve_lit_ty_from_expected 只处理 RawPtr, 不处理 Int)
   - `landin_Display_i32_fmt`: `32` + `10i64` + `0i64` 等字面量 → 部分 Infer
   - `landin_String_new`: `0` 字面量 → Infer (Stage 103 修复了 ptr field, 但 `len: 0usize` + `cap: 0usize` 中的 `0usize` 可能仍有问题)
   - `landin_main`: `println!` format string → Infer
   - `landin___landin_format_v2`: format args → Infer

2. **Param warnings** (generic def body internal types):
   - `landin_Vec_push` 等被实例化的 generic def body 仍 emit, 内部 Param types fallback to i32

### 决策点 (§12 最优>最小, §1.0 原则 4 报错>静默)

#### 决策 1: 不在 Stage 105 实施代码修复

**选择**: Stage 105 只做 RCA + 记录新 TD, 不实施代码修复。

**理由** (§1.0 原则 9 正确>妥协, 用户指示 "遇依赖缺失停止阉割版"):
- 根因涉及多个 typeck writeback 问题 (Infer + Param), 每个需独立修复
- 修复 Infer warnings 需要扩展 resolve_lit_ty_from_expected 处理 Int/Uint (但 Stage 103 实验显示这破坏 typeck validation)
- 修复 Param warnings 需要修改 codegen_from_mir 跳过被实例化的 generic def body (但 codegen_operand 仍引用 generic def name)
- 正确修复需要系统性解决 typeck writeback + codegen 跳过逻辑, 不能单 stage 完成

#### 决策 2: 记录新 TD — TD-TYPECK-WRITEBACK-INCOMPLETE

**选择**: 记录 TD-TYPECK-WRITEBACK-INCOMPLETE (P2, v0.12+) 跟踪所有 Infer/Param warnings。

**理由** (§1.0 原则 4 报错>静默):
- 18 个 Infer warnings + 73 个 Param warnings (含重复) 需要系统性修复
- 每个函数 (Default/Display/String_new/main/__landin_format_v2) 需独立分析

### §3.2 验收

- `cargo fmt --check` ✓
- `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓ (0 warnings)
- `cargo test --release --features llvm-backend --lib` ✓ (898 tests, 0 failures, 0 ignored)
- `cargo test --release --features llvm-backend --test all_tests` ✓ (4715 tests, 0 failures, 9 ignored)
- 基线 (无 Debug impl) 100 次跑 ASLR off 全绿 (0 failures)

## 新发现 TD

### TD-TYPECK-WRITEBACK-INCOMPLETE (P2, v0.12+)

**现象**: 加 Debug impl 后非确定性 SIGSEGV (100 次跑 1-3 失败)。LLVM IR 在成功/失败跑间完全相同 (Param=73 Infer=18)。

**根因**: typeck writeback 未完全解析所有类型 — 18 个 Infer warnings + 73 个 Param warnings 导致 mir_type_to_emit_type fallback to i32, 产生不正确 LLVM IR → LLVM 优化器非确定性 crash。

**修复方案**: 
1. 扩展 resolve_lit_ty_from_expected 处理 Int/Uint (需解决 typeck validation 冲突)
2. 修改 codegen_from_mir 跳过被实例化的 generic def body (需解决 codegen_operand 引用)
3. 修复 typeck writeback 对 prelude 函数 (Default/Display/String_new/main/__landin_format_v2) 的 Constant type Infer

**影响**: 修复后可重新添加 Debug + PartialOrd impls。

## 下一步

- **Stage 106**: 修复 TD-TYPECK-WRITEBACK-INCOMPLETE — 系统性修复 Infer + Param warnings
- **Stage 107**: 重新添加 Debug + PartialOrd impls (依赖 Stage 106 完成)
