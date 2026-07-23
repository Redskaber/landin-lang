# Stage 5.24 开发计划：mini-cargo MVP

> **阶段**: Stage 5.24
> **版本**: v0.11.21 → v0.11.22
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

实现 `landinc`（Landin 包管理器 + 构建工具）的 MVP——项目 manifest 解析 +
构建编排。

## 2. 设计

### 2.1 新增 `src/cargo.rs` 模块

| 类型/函数 | 用途 |
|----------|------|
| `ProjectManifest` | 解析 `landin.toml` 项目 manifest |
| `BuildConfig` | 构建配置（优化级别、emit LLVM IR 等） |
| `BuildResult` | 构建结果（成功/失败、错误数、LLVM IR） |
| `parse_manifest(content) -> Self` | 从字符串解析 manifest |
| `load_manifest(path) -> Result<Self>` | 从文件加载 manifest |
| `build_project(manifest, config) -> BuildResult` | 编译入口文件并返回结果 |

### 2.2 §16 合规

`build_project()` 只调用公共 API `compile()` + `codegen_crate()`，不访问
HIR/MIR/typeck 内部。

### 2.3 命名标准化

| API | 命名规则 |
|-----|---------|
| `ProjectManifest` | `<Noun><Noun>` |
| `BuildConfig` | `<Noun><Noun>` |
| `BuildResult` | `<Noun><Noun>` |
| `parse_manifest` | `parse_<noun>` |
| `load_manifest` | `load_<noun>` |
| `build_project` | `build_<noun>` |

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过（1023 → 1031, +8 ✅）
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
