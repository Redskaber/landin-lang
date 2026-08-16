# Stage 18.155 — v0.2 P0 mini-cargo Phase 4: 简写与缺陷修复

> **Author**: redskaber (ARCH-A + DEV-A + REV-A)
> **Date**: 2026-08-16
> **Version**: v0.423.0 (Stage 18.155 dev-log)
> **Process**: docs/stage-committee-process.md v6.4 §12 (最优>最小) + §13.4 (重构即架构设计) + §2 原则 4 (报错>静默)
> **Complexity**: L2 (3 修复点 + 新公共 API)
> **Task ID**: stage18.155

## 1. 阶段目标

修复 Stage 18.154 记录的 3 项简写/缺陷:

| 编号 | 类型 | 描述 | 状态 |
|------|------|------|------|
| 缺陷2 | 缺陷 | landinc 错误输出不用彩色诊断 | ✅ Resolved |
| 简写1 | 简写 | `--release` flag 不生效 (optimize 参数未传递) | ✅ Resolved |
| 缺陷3 | 缺陷 | 项目名不校验合法性 | ✅ Resolved |

## 2. 修复实现

### 2.1 缺陷2 修复: landinc 错误输出集成彩色诊断

**Before** (Stage 18.154): `cmd_build`/`cmd_check`/`cmd_run` 各自遍历 `result.errors.lex`/`parse`/`lower`/`resolve` 用 `eprintln!` 打印——无源码上下文、无颜色、重复代码。

**After** (Stage 18.155): 提取 `print_compile_errors(result, entry)` helper:
- 调用 `CompileErrors::format_via_diagnostics_colored` (与 `landin-stage0` 一致)
- 自动检测 TTY (stderr.is_terminal) 选择 Always/Never 颜色
- 重读 entry 文件提供源码上下文 (span underline)
- 3 个 cmd 函数共用, 消除重复 (§13.4 J2 单一职责, §1.0 原則 6 通解>特解)

### 2.2 简写1 修复: `--release` 生效 → `compile_project_opt`

**Before** (Stage 18.154): `cmd_build` 的 `release` 参数被忽略 (`_release: bool`), 调用 `compile_project(entry)` (内部固定 `optimize=true`)。

**After** (Stage 18.155): 新增公共 API `compile_project_opt(entry_path, optimize)`:
- `optimize=true`: 运行 MIR DCE + const_prop
- `optimize=false`: 跳过 MIR opt (用于测试或 LLVM 级优化)
- `compile_project` 委托 `compile_project_opt(path, true)` (默认优化)
- `cmd_build` 调用 `compile_project_opt(entry, true)` (当前单一 opt level)

**简写记录**: `--release` 当前仍传 `optimize=true` (与 debug 相同), 因为 Landin 目前只有单一 MIR opt level。真正的 release 优化 (LLVM opt-level=2/3) 需要未来 stage 扩展 LLVM target machine options。Per §2 原則 9 (正确>妥协): 显式传递 `true` 而非忽略 flag, 记录当前限制。

### 2.3 缺陷3 修复: 项目名合法性校验

**Before** (Stage 18.154): `cmd_new` 接受任意字符串作为项目名, 包括 `"my-app"` (含连字符)、`"fn"` (关键字)、`"2app"` (数字开头)。

**After** (Stage 18.155): 新增公共函数 `lexer::is_valid_ident(s)`:
- 非空
- 首字符: ASCII 字母或 `_`
- 后续字符: ASCII 字母、数字或 `_`
- 不是关键字 (`keyword_from_str(s).is_none()`)

`cmd_new` 调用 `is_valid_ident(name)`, 无效时报错退出 (§2 原則 4 报错>静默)。

## 3. API 命名标准化 (§10)

| 新增 | 命名 | 模式 | 合规 |
|------|------|------|------|
| 函数 | `compile_project_opt` | `<verb>_<noun>_<adj>` | ✅ |
| 函数 | `is_valid_ident` | `<verb>_<adj>_<noun>` | ✅ |
| 函数 (private) | `print_compile_errors` | `<verb>_<noun>_<noun>` | ✅ |

## 4. 接口设计 (§11)

- `compile_project_opt` 是公共 API (lib.rs re-export)
- `is_valid_ident` 公开在 `lexer` 模块 (供 landinc + 未来工具复用)
- `print_compile_errors` 是 landinc 内部 helper (不公开)
- 不跨阶段调用 — 仅使用 driver/codegen 公共 API

## 5. 测试 (§9)

### 5.1 Lib 测试 (6 个, in `lexer/ident.rs`)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_155_valid_ident_simple` | 正向 | `myapp`/`my_app`/`app2` |
| `stage18_155_valid_ident_underscore` | 正向 | `_internal`/`_` |
| `stage18_155_invalid_ident_empty` | 负向 | 空字符串 |
| `stage18_155_invalid_ident_digit_start` | 负向 | `2app`/`123` |
| `stage18_155_invalid_ident_special_chars` | 负向 | `my-app`/`my.app`/`my app`/`my$app` |
| `stage18_155_invalid_ident_keyword` | 负向 | `fn`/`mod`/`struct`/`use` |

### 5.2 集成测试 (5 个, in `stage18_155_deficiency_fix_tests.rs`)

| 测试 | 类型 | 验证点 |
|------|------|--------|
| `stage18_155_valid_project_names` | 正向 | 有效项目名通过 |
| `stage18_155_invalid_project_names` | 负向 | 无效项目名拒绝 |
| `stage18_155_compile_project_opt_with_optimization` | 正向 | `compile_project_opt(path, true)` |
| `stage18_155_compile_project_opt_without_optimization` | 正向 | `compile_project_opt(path, false)` |
| `stage18_155_compile_project_opt_missing_file` | 负向 | 缺失文件报错 |

### 5.3 手动验证

```
$ landinc new "my-app"
error: invalid project name `my-app`
hint: name must start with a letter or underscore, contain only
      letters, digits, or underscores, and not be a keyword

$ landinc new "fn"
error: invalid project name `fn`
...

$ landinc new "myapp"
Created binary project `myapp`
  cd myapp && landinc build
```

## 6. §3.2 验收

- ✅ cargo check --all-features: 0 errors / 0 warnings
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-features --all-targets: 0 warnings
- ✅ cargo test --lib: 635 passed (629 + 6 new), 0 failed
- ✅ cargo test --tests --all-features: 2693 passed (2688 + 5 new), 0 failed
- ✅ 0 TODO/FIXME/HACK
- ✅ `landinc new` 项目名校验手动验证通过

## 7. 简写和缺陷记录 (剩余)

### 7.1 简写 (Stage 18.154 剩余)

**简写2**: `landin.toml` 不含 `[dependencies]`/`[features]` 高级字段。
- **原因**: 依赖解析和 features 系统是 v0.2 P1+ 范围。
- **修订计划**: v0.2 P1 实现 semver + registry 后扩展 manifest 格式。

### 7.2 缺陷 (Stage 18.154 剩余)

**缺陷1**: `landinc build` 不链接可执行文件 (仅编译到 MIR + 可选 LLVM IR)。
- **原因**: 链接需要 LLVM 后端 + `cc` 调用, 当前仅在 `landinc run` 中实现。
- **修订计划**: 添加 `landinc build --bin` 选项链接可执行文件 (复用 `cmd_run` 的 link 逻辑)。

### 7.3 新简写 (Stage 18.155 引入)

**简写4**: `--release` 当前传 `optimize=true` (与 debug 相同), 无 LLVM 级优化差异。
- **原因**: Landin 目前只有单一 MIR opt level (DCE + const_prop), 无 LLVM opt-level 控制。
- **修订计划**: 未来 stage 扩展 `TargetTriple` + LLVM `TargetOptions` 支持 opt-level=2/3。

## 8. §13.4 重构治理评估 (J1-J6)

| J | 评估 | 结果 |
|---|------|------|
| J1 架构设计对齐 | `compile_project_opt` 匹配 `compile`/`compile_no_opt` 模式 | ✅ |
| J2 单一职责 | `print_compile_errors` 提取消除重复; `is_valid_ident` 单一校验 | ✅ |
| J3 单向流动 | landinc → compile_project_opt → compile_inner (无环) | ✅ |
| J4 编译相关表达完整 | 校验逻辑在 lexer, 编译逻辑在 driver, CLI 在 landinc | ✅ |
| J5 阶段划分清晰 | 公共 API 边界清晰 | ✅ |
| J6 科学合理粒度 | 3 修复点分散在合适模块, 无过大文件 | ✅ |

## 9. Stage Summary

- **Stage 18.155 PASSED** — v0.2 P0 mini-cargo Phase 4: 简写与缺陷修复
- **修复 3 项**: 彩色诊断 + `compile_project_opt` + 项目名校验
- **新增公共 API**: `compile_project_opt(path, optimize)`, `lexer::is_valid_ident(s)`
- **测试**: 635 lib + 2693 integration (新增 11), 0 failures
- **TD-SINGLE-FILE**: 🟡 Phase 1-4 Resolved (核心功能完成; 依赖解析/registry 是 v0.2 P1+)
- **v0.423.0**: patch bump
- **下一步**: v0.2 P1 — stdlib facade (TD-STDLIB-FACADE) 或 format macros (TD-NO-FORMAT-MACRO)
