# Stage 5.4 开发计划：DefId→name 反向映射 + 完整 Copy 检测

> **阶段**: Stage 5.4
> **版本**: v0.11.2 → v0.11.3
> **状态**: 🔄 In progress
> **流程**: stage-committee-process.md v3.18 §17.3 时期 1

## 1. 目标

1. 在 TraitResolver 中添加 `def_id_to_name: HashMap<DefId, Spur>` 反向映射
2. 在 `collect()` 时填充此映射（从 struct/enum 定义中提取名称）
3. 激活 `ty_is_copy_with_resolver` 的完整 Copy 检测：对于 Adt 类型，
   使用 DefId→name 查找类型名，然后检查 `implements(Copy, type_name)`
4. 关闭 TD-016（L-COPY-ADT）

## 2. MUV 拆分

| 子任务 | 描述 | 复杂度 |
|--------|------|--------|
| 5.4-a | TraitResolver 添加 `def_id_to_name` + `name_to_def_id` 映射 | L1 |
| 5.4-b | `collect()` 时从 struct/enum/trait 收集名称映射 | L1 |
| 5.4-c | `ty_is_copy_with_resolver` Adt 分支激活完整检测 | L2 |
| 5.4-d | 添加测试 | L1 |

## 3. 验收标准

1. `cargo fmt --check` 零 diff
2. `cargo clippy --all-targets` 0 warnings
3. `cargo test` 全部通过
4. §17.3 三阶段文档协议执行

---

**创建日期**: 2026-07-22
