# Stage 5.24 测试计划：mini-cargo MVP

> **阶段**: Stage 5.24
> **对应代码**: tests/v0/stage5/plan/mini_cargo_tests.rs
> **状态**: ✅ Complete

## 1. 测试目标

验证 `ProjectManifest::parse_manifest()`、`build_project()` 正确解析 manifest
并编译入口文件。

## 2. 覆盖场景

| 场景 | 测试函数名 | 状态 |
|------|-----------|------|
| 基本解析 | test_parse_manifest_basic | ✅ |
| 默认值 | test_parse_manifest_defaults | ✅ |
| 注释跳过 | test_parse_manifest_comments | ✅ |
| 构建成功 | test_build_project_success | ✅ |
| 构建错误 | test_build_project_errors | ✅ |
| 文件不存在 | test_build_project_file_not_found | ✅ |
| emit LLVM | test_build_project_emit_llvm | ✅ |
| 默认值 | test_project_manifest_default | ✅ |

---

**创建日期**: 2026-07-23
