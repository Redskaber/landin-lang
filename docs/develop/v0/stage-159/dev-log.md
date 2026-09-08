# Stage 159 开发日志 — TD-STDLIB-ITERATOR 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.682.0 → v0.683.0 |
| 测试数 | 6056 → 6066 (+10 new stage159 tests) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| LOC 变更 | ~30 (prelude.rs) + ~250 (10 tests) + 5 test files updated |

## 5W2H

### WHAT
将 Iterator trait 添加到 prelude (`src/stdlib/prelude.rs`), 使用户无需每次自定义 `trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }`.

### WHY
Iterator trait 是 Rust stdlib 的核心 trait, 几乎所有集合类型都实现它. 当前用户必须每次手写 Iterator trait 定义, 违反 DRY 原则. Stage 156 已解锁 Iterator 运行时行为 (trait 方法返回 Option<T> 正确推断 substs). Stage 157 修复 default body self type. Stage 158 修复 vtable default body entry.

### HOW (修复方案)
1. 在 `src/stdlib/prelude.rs` 末尾添加 `trait Iterator { type Item; fn next(&mut self) -> Option<Self::Item>; }`
2. 从 5 个测试文件 (stage148/149/150/153/156) 中移除用户定义的 `trait Iterator` 块
3. 添加 10 个 Stage 159 测试 (正/回归/边界/负)

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)
1. **添加到 prelude 不选用户自定义** (§1.0 原則 9 正确 > 妥协) — prelude 包含是 Rust 的标准做法, 避免 DRY 违反.
2. **只添加 trait 声明, 不添加 adapters** (§1.0 原則 6 通解 > 特解) — adapters (map, filter, collect) 需要闭包支持 (TD-FN-CLOSURE-COERCION), 延迟到 v0.2+.
3. **移除测试中的用户定义** (§1.0 原則 10 唯一可信数据源) — prelude 是唯一可信的 Iterator 定义源.

### 裁剪点 (§1.2.1)
L2 任务 (~30 LOC + 10 tests), 单轮收敛. 跳过 §14.6 跨阶段验证.

### 发现的新 TD
- **TD-DYN-ITERATOR-ASSOC-TYPE** (P3, v0.17+): dyn dispatch `&mut dyn Iterator<Item = i64>` codegen 失败 — 关联类型投影 + dyn dispatch 组合问题. 静态调用工作正常.

## §3.2 全套验收

| 命令 | 结果 |
|------|------|
| `cargo build --release --features llvm-backend` | success |
| `cargo check --features llvm-backend` | 0 errors, 0 warnings |
| `cargo fmt --check` | exit 0 |
| `cargo clippy --all-targets --features llvm-backend -- -D warnings` | 0 warnings |
| `cargo test --release --features llvm-backend` | 898 lib + 5168 integration = **6066 tests, 0 failures, 12 ignored** |
