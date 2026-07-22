# Stage 5.10 开发计划：builtin Clone/Drop 激活 + 通用 builtin trait 检查

> **阶段**: Stage 5.10
> **版本**: v0.11.8 → v0.11.9
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

扩展 Stage 5.9 的 builtin Copy 激活模式到 Clone/Drop，并添加通用的
`implements_builtin_trait()` 方法支持任意 builtin trait by name。同时更新
流程 spec 至 v3.20（§0.2 任务路由 + §1.1 环境检查 + §1.2 验收检查 +
§1.3 spec 演进）。

## 2. 设计

### 2.1 `is_clone_builtin()` + `is_drop_builtin()`

与 `is_copy_builtin()` 相同模式——自动从 interner 查找 trait Spur，无需
调用者传参。防御性 fallback `false`。

### 2.2 `implements_builtin_trait()` 通用方法

```rust
pub fn implements_builtin_trait(
    &self,
    def_id: DefId,
    trait_name: &str,
    interner: &Rodeo,
) -> bool
```

接受 trait 名字符串（如 "Send", "Sync", "Sized"），适用于任何 builtin
trait。这是 `is_copy_builtin` / `is_clone_builtin` / `is_drop_builtin` 的
通用形式。

### 2.3 命名标准化（API-naming-standard §3）

| 新增 API | 命名规则 | 备注 |
|----------|----------|------|
| `is_clone_builtin` | `is_` + `_builtin` 后缀 | 与 `is_copy_builtin` 一致 |
| `is_drop_builtin` | `is_` + `_builtin` 后缀 | 同上 |
| `implements_builtin_trait` | `implements_` 前缀 | 与 `implements` / `implements_by_def_id` 一致 |

### 2.4 流程 spec v3.20 更新

- §0.2 任务类型精确路由（8 种任务 → 必读章节）
- §1.1 环境工具检查与准备（工具缺失时查找+安装）
- §1.2 交付前验收检查（cargo clean+test+fmt+clippy 全绿）
- §1.3 Spec 持续演进原则（精要化，反臃肿）

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（936 → 943, +7 ✅）
4. §17.3 三阶段文档协议执行 ✅
5. §16 合规 ✅
6. API 命名遵循 api-naming-standard §3 ✅
7. §1.2 交付前验收：cargo clean+test+fmt+clippy 全绿 ✅

---

**创建日期**: 2026-07-22
