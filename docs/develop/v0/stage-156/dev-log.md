# Stage 156 开发日志 — TD-OPTION-NONE-GENERIC-SUBSTS-MISSING 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.679.0 → v0.680.0 |
| 测试数 | 6028 → 6038 (+10 new stage156 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~70 (expr_variants.rs) + ~250 (10 tests) |

## 5W2H

### WHAT
修复 trait 方法体内部 `Option::Some(v)`/`Option::None` 构造使用空 substs → Param fallback → `{i32,i32}` 而非 `{i32,i64}` → i64 payload 截断为 i32. 导致 Iterator sum 返回 garbage value.

### WHY
`lower_path_expr` 调用 `lower_path_generic_args` 从 path 提取 turbofish args. 当 `Option::None` 或 `Option::Some(v)` 无 turbofish 构造时 (常见情况), `lower_path_generic_args` 返回空 substs → `Adt(Option, [])` → codegen 使用 crate-level AdtLayout 的 `Param(0)` → I32 fallback → `{i32, i32}`.

这只在 **trait 方法体** 中显现:
- 在 `main`/普通函数中, let 绑定类型注解 (`let opt: Option<i64> = ...`) 触发 writeback 解析类型.
- 在 trait 方法体中, 返回类型 (`Option<i64>`) 未被用于推断 variant 构造的 substs.

### HOW (修复方案)
在 `expr_variants.rs` 添加 `infer_substs_from_return_type` helper:
1. 当 `lower_path_generic_args` 返回空 substs 时, 调用 helper
2. helper 从 `fn_sigs[owner_def_id].output` 读取当前函数的返回类型
3. 如果返回类型是 `Adt(enum_def_id, concrete_substs)` 且 def_id 匹配, 使用 concrete substs
4. 如果返回类型不匹配 (如 `main` 返回 `()`), 回退到空 substs (保留旧行为 — writeback 的 let-binding 路径处理)

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **从返回类型推断不选 Param(N) 占位符** (§1.0 原則 9 正确 > 妥协) — Param(N) 需要 writeback 新规则解析 Aggregate 构造的 local, 更复杂. 直接从返回类型推断 concrete substs 更简单且正确.
2. **只处理返回类型匹配的情况** (§1.0 原則 6 通解 > 特解) — 不修改非匹配情况 (main 中的 let 绑定), 避免破坏 writeback 的 let-binding 路径.
3. **不修改 writeback 逻辑** (§1.0 原則 10 唯一可信数据源) — `fn_sigs[owner_def_id].output` 是返回类型的权威来源.

### 裁剪点 (§1.2.1)
L2 任务 (~70 LOC + 10 tests), 单轮收敛. 跳过 §14.6 跨阶段验证 (单文件修改).

### 验证
- **修复前**: Iterator sum (1+2+3=6) 返回 garbage value (422199709209888)
- **修复后**: 返回 6 (正确)
- **修复前**: trait 方法 `make()` 返回 `Some(42)` 但 match arm 打印 `some 0`
- **修复后**: 打印 `some 42` (正确)

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success (35s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings |
| `cargo test --release --features llvm-backend` | 898 lib + 5140 integration = **6038 tests, 0 failures, 12 ignored** |
