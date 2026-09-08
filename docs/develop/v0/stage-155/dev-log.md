# Stage 155 开发日志 — TD-MATCH-SCRUT-RET-COPY-TYPE 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.678.0 → v0.679.0 |
| 测试数 | 6019 → 6028 (+9 new stage155 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~40 (pattern_lower.rs) + ~200 (9 tests) |

## 5W2H

### WHAT
修复函数返回泛型枚举 (如 `Option<i64>`) 后 match arm 读取 garbage value (-1 而非 42) 的 P1 bug.

### WHY
`pattern_lower.rs:238` 在 scrutinee 类型为 Infer/Error 时用**空 substs** (`Vec::new().into()`) 覆盖类型 → `Adt(Option, [])`. 空 substs 使 `needs_writeback` 返回 false (substs 中无 Param/Infer/Error) → writeback 跳过该 local → 类型未解析 → codegen 使用 crate-level AdtLayout 的 `Param(0)` → I32 fallback → `{i32, i32}` 而非 `{i32, i64}`. Call dest 的正确类型 `{i32, i64}` 被 store 到错误的 `{i32, i32}` alloca, 截断 i64→i32.

### HOW (修复方案)
在 `pattern_lower.rs:238`, 用 `Param(N)` 占位符替代空 substs:
1. 从 HIR 查找 enum 定义, 计算泛型参数个数 (仅 Type params)
2. 为每个泛型参数创建 `Param(i)` 占位符
3. 用 `Adt(enum_def_id, [Param(0), ...])` 替代 `Adt(enum_def_id, [])`

这样 `needs_writeback` 返回 true (Param 在 substs 中), writeback 的 Call dest 规则从 callee 的返回类型解析具体类型 (如 `Adt(Option, [i64])`).

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **用 Param(N) 占位符不选空 substs** (§1.0 原則 9 正确 > 妥协) — 空 substs 使 writeback 跳过, 导致 silent wrong results. Param(N) 触发 writeback 解析.
2. **从 HIR 查找泛型参数个数** (§1.0 原則 10 唯一可信数据源) — HIR enum 定义是泛型参数个数的权威来源.
3. **不修改 writeback 逻辑** (§1.0 原則 6 通解 > 特解) — writeback 已有正确的 Call dest 规则, 只需让 `needs_writeback` 返回 true.

### 裁剪点 (§1.2.1)
L2 任务 (~40 LOC + 9 tests), 单轮收敛. 跳过 §14.6 跨阶段验证 (单文件修改, 无架构变化).

### 验证
- **修复前**: `make_some()` 返回 `Option<i64>`, match arm `Some(v) => println!("{}", v)` 打印 -1 (garbage)
- **修复后**: 打印 42 (正确)

### 发现的已修复 TD (验证)
- **TD-OPTION-UNWRAP-OR-MATCH**: ✅ 已修复 (Stage 152 substitute_adt_layout 间接修复). 验证: some.unwrap_or(0) = 42, none.unwrap_or(99) = 99.
- **TD-TRAIT-METHOD-REMONO-LINK**: ✅ linker 已修复 (vtable 符号与函数 emit 名称匹配). 运行时值仍有问题 (见新 TD).

### 发现的新 TD
- **TD-OPTION-NONE-GENERIC-SUBSTS-MISSING** (P3, v0.16+): trait 方法体内部 `Option::None` 构造使用空 substs → Param fallback → `{i32,i32}` 而非 `{i32,i64}`. 影响: trait 方法返回 `Option<i64>` 的 None 分支 payload 类型错误. 根因: `lower_path_expr` 对 unit variant 使用 `lower_path_generic_args` 读取 HIR path 的 generic args, 但 path 无 turbofish 时 substs 为空. typeck 未从期望返回类型推断 substs.

### 下一步 (MUV)
Stage 156 候选:
- **TD-OPTION-NONE-GENERIC-SUBSTS-MISSING** (P3, v0.16+): 修复 trait 方法体 Option::None 构造的 substs 缺失
- **TD-VTABLE-MISSING-DEFAULT-BODY** (P3, v0.16+): 修复 vtable 包含 default body 方法
- **TD-TYPECK-GENERIC-ARG-VALIDATION** (P3, v0.16+): 修复 typeck turbofish arg 验证

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success (34s) |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings |
| `cargo test --release --features llvm-backend` | 898 lib + 5130 integration = **6028 tests, 0 failures, 12 ignored** |
