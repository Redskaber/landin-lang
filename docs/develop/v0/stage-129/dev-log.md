# Stage 129 开发日志 — TD-UFCS-AMBIGUITY-E1109 (歧义检测)

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.659.0 → v0.660.0 |
| 测试数 | 5795 (898 lib + 4897 integration, +10 stage129) |
| 失败数 | 0 |
| ignored | 9 |
| clippy warnings | 0 |
| fmt | clean |

## 5W2H

### WHAT
Stage 129 实现 TD-UFCS-AMBIGUITY-E1109 — 普通方法调用 `obj.method()` 当 2+ trait 提供同名方法时报错（而非静默选择第一个）。

### WHY (根因分析)
- **根因**: `resolve_trait_method` (mir/lower/method_resolution.rs) 遍历所有 trait impl 块，返回**第一个匹配** — 无候选收集，无多候选报错
- **违反原则**: §1.0 原则 4 (报错 > 静默) — 歧义被静默处理
- **通解**: 收集所有候选 trait impl 方法 DefId，>1 个来自不同 trait 时返回 None（caller 报 "no method found"）

### HOW (实施策略)
1. **`resolve_trait_method`**: 改为收集所有候选到 `Vec<DefId>`
2. **歧义检测**: 如果候选来自 >1 不同 trait，返回 None（caller 报错）
3. **非歧义返回**: 1 个候选或同 trait 多候选，返回第一个

### 决策点
- **选收集候选 + 返回 None** — 不选改返回类型为 Result（更大重构，TD-UFCS-AMBIGUITY-E1109-CLEANUP v0.14+）
- **依据**: §12 最优>最小 — 当前方案最小改动满足报错需求，精确 E1109 错误信息留给 v0.14+

### §3.2 验收
- cargo fmt --check ✓
- cargo clippy --all-targets --features llvm-backend -- -D warnings ✓ (0 warnings)
- cargo test --release --features llvm-backend --lib ✓ (898 tests, 0 failures)
- cargo test --release --features llvm-backend --test all_tests -- --test-threads=1 ✓ (4897 tests, 0 failures, 9 ignored)
- Total: 5795 tests, 0 failures, 9 ignored

## Stage Summary
- Stage 129 PASSED — 歧义检测实现
- 10 tests: 3 positive + 2 negative + 2 workaround + 3 regression
- 0 regression (5785 → 5795 tests, +10 new)
- TD-UFCS-AMBIGUITY-E1109 ✅ 基本修复（返回 None 触发 caller 报错；精确 E1109 错误信息留给 v0.14+）
- v0.660.0
