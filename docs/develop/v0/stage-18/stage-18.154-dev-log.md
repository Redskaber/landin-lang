# Stage 18.154 — v0.2 P0 mini-cargo Phase 3: landinc CLI

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.422.0 (Stage 18.154 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §13.4 (重构即架构设计) + §10 (API 命名) + §11 (接口隔离)
> **Complexity**: L2 (新增 binary + manifest 路径解析修复)
> **Task ID**: stage18.154

## 1. 阶段目标

按 v0.2 P0 计划推进 TD-SINGLE-FILE 修复。本 stage 实现 Phase 3: `landinc` CLI subcommands。

| Phase | 范围 | 状态 |
|-------|------|------|
| Phase 1 | 模块加载器 (`ModuleLoader` + `compile_project`) | ✅ Stage 18.152 |
| Phase 2 | `use` 跨文件 + path 跨模块 name resolution | ✅ Stage 18.153 |
| Phase 3 | `landinc` CLI subcommands (`build`/`run`/`new`/`check`/`clean`) | ✅ 本 stage |
| Phase 4 | `landin.toml` manifest 完整集成 (dependencies, profiles) | 后续 stage |

## 2. 设计对齐 (§13.1)

### 2.1 对应设计文档

`docs/lang-design/10-toolchain.md` §3:
- §3.1 命令行接口: `landinc new/build/run/test/clean/check`
- §3.2 manifest: `landin.toml`
- §3.3 项目布局

### 2.2 设计决策

**分离二进制** (§13.4 J2 单一职责):
- `landin-stage0` = 编译器 (单文件: `landin-stage0 <file> --compile`)
- `landinc` = 构建工具 (多文件项目: `landinc build`)

Per `10-toolchain.md` §1: `landin` 和 `landinc` 是独立的工具。本 stage 创建 `landinc` 二进制，保持 `landin-stage0` 不变。

## 3. 实现

### 3.1 新增: `src/bin/landinc.rs` (300 LOC)

使用 clap `Subcommand` enum 实现 5 个子命令:

| 子命令 | 功能 | 依赖 |
|--------|------|------|
| `landinc new <name>` | 创建项目骨架 (landin.toml + src/main.lin + .gitignore) | 无 |
| `landinc new --lib <name>` | 创建库项目 (src/lib.lin) | 无 |
| `landinc build [--release] [--emit-llvm]` | 编译项目 | `compile_project` |
| `landinc run [-- args]` | 编译+链接+运行 | `compile_project` + llvm-backend |
| `landinc check` | 类型检查 (无 codegen) | `compile_project` |
| `landinc clean` | 删除 target/ | 无 |

### 3.2 修复: `ProjectManifest::load_manifest` 路径解析

**Before**: manifest 中的 `entry_point = "src/main.lin"` 存储为相对路径 (相对 CWD)，导致 `landinc build` 必须在项目根目录运行。

**After**: `load_manifest` 解析 `entry_point`/`src_dir`/`target_dir` 相对于 manifest 文件所在目录 (不是 CWD)。

Per §2 原則 9 (正确>妥协): manifest 中的路径应相对于 manifest 文件，匹配 Cargo 语义。

### 3.3 Cargo.toml 注册

```toml
[[bin]]
name = "landinc"
path = "src/bin/landinc.rs"
```

## 4. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 二进制 | `landinc` | 设计文档命名 | ✅ |
| Subcommand | `Build`/`Run`/`Check`/`New`/`Clean` | `<verb>` | ✅ |
| 函数 | `cmd_build`/`cmd_run`/`cmd_check`/`cmd_new`/`cmd_clean` | `<verb>_<noun>` | ✅ |
| 函数 | `resolve_manifest_path`/`load_manifest` | `<verb>_<noun>` | ✅ |

## 5. 接口设计 (§11)

- `landinc` 仅使用公共 API: `compile_project`, `ProjectManifest`, `codegen_crate`
- 不访问 HIR/MIR/typeck 内部
- `landinc` 与 `landin-stage0` 独立编译，互不影响

## 6. 测试 (§9)

### 6.1 手动验证 (CLI 二进制)

```
$ landinc new myapp
Created binary project `myapp`
  cd myapp && landinc build

$ cd myapp && landinc check
Check passed (1 MIR bodies)

$ landinc build --emit-llvm
LLVM IR written to target/myapp.ll

$ landinc clean
Removed target
```

### 6.2 集成测试 (7 个: 5 positive + 2 negative)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_154_new_creates_valid_skeleton` | 正向 | `landinc new` 创建有效 manifest |
| `stage18_154_build_compiles_new_project` | 正向 | `landinc build` 编译新项目 |
| `stage18_154_build_multi_file_project` | 正向 | 多文件项目编译 |
| `stage18_154_check_type_checks` | 正向 | `landinc check` 类型检查 |
| `stage18_154_new_lib_creates_valid_skeleton` | 正向 | `landinc new --lib` 库项目 |
| `stage18_154_build_missing_manifest` | 负向 | 缺少 manifest 报错 |
| `stage18_154_build_missing_entry_point` | 负向 | 缺少入口文件报错 |

### 6.3 测试结果

- ✅ 7/7 通过 (5 positive + 2 negative)
- ✅ 0 回归 (629 lib + 2688 integration = 3317 total, 0 failures)

## 7. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 629 passed, 0 failed
- ✅ cargo test --tests --all-features: 2688 passed (2681 + 7 new), 0 failed
- ✅ 0 TODO/FIXME/HACK
- ✅ `landinc new/build/check/clean` 手动验证通过

## 8. 简写和缺陷记录

### 8.1 当前简写

**简写 1**: `landinc run` 需要 `--features llvm-backend`，无 feature 时打印错误。
- **原因**: LLVM 后端是可选 feature (CI/测试环境约束)。
- **修订计划**: 保持现状 (feature gate 是合理的)。

**简写 2**: `landinc build --release` 的 `--release` flag 当前不生效 (MIR 优化总是开启)。
- **原因**: `compile_project` 内部调用 `compile_inner(src, true, ...)` (optimize=true)。
- **修订计划**: Phase 4 添加 `compile_project_opt(entry_path, optimize: bool)` 让 `--release` 控制 optimize 参数。

**简写 3**: `landinc new` 创建的 `landin.toml` 不含 `[dependencies]` / `[features]` 等高级字段。
- **原因**: 依赖解析和 features 系统是 v0.2 P1+ 范围。
- **修订计划**: Phase 4+ 扩展 manifest 格式。

### 8.2 缺陷记录

**缺陷 1**: `landinc build` 不链接可执行文件 (仅编译到 MIR + 可选 LLVM IR)。
- **原因**: 链接需要 LLVM 后端 + `cc` 调用，当前仅在 `landinc run` 中实现。
- **修订计划**: 添加 `landinc build --bin` 选项链接可执行文件。

**缺陷 2**: 错误输出不使用彩色诊断 (`landin-stage0` 用 `format_via_diagnostics_colored`)。
- **原因**: `landinc` 直接遍历 `result.errors` 打印，未调用诊断格式化器。
- **修订计划**: 集成 `format_via_diagnostics_colored` 到 `landinc`。

**缺陷 3**: `landinc new` 不验证项目名合法性 (如含空格、特殊字符)。
- **原因**: MVP 阶段简化。
- **修订计划**: 添加项目名校验 (合法标识符 + 不与关键字冲突)。

## 9. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | 对齐 `10-toolchain.md` §3 (landinc CLI) | ✅ |
| J2 单一职责 | `landinc` = 构建工具; `landin-stage0` = 编译器 (分离) | ✅ |
| J3 单向流动 | landinc → compile_project → pipeline (无环) | ✅ |
| J4 编译相关表达完整 | CLI 逻辑完整在 landinc.rs | ✅ |
| J5 阶段划分清晰 | landinc 是 driver 层消费者, 不跨阶段 | ✅ |
| J6 科学合理粒度 | landinc.rs ~300 LOC, 合理 | ✅ |

## 10. Stage Summary

- **Stage 18.154 PASSED** — v0.2 P0 mini-cargo Phase 3: landinc CLI
- **新增**: `src/bin/landinc.rs` (300 LOC) — 5 subcommands (build/run/new/check/clean)
- **修复**: `ProjectManifest::load_manifest` 路径解析 (相对 manifest 目录, 非 CWD)
- **注册**: Cargo.toml `[[bin]] landinc`
- **测试**: 629 lib + 2688 integration (新增 7), 0 failures
- **TD-SINGLE-FILE**: 🟡 Phase 1-3 Resolved (phase 4 remains)
- **v0.422.0**: minor bump (新二进制 `landinc`)
- **下一步**: Phase 4 — `landin.toml` manifest 完整集成 (dependencies, profiles)
