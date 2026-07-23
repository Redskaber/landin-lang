# Stage 5.30 开发计划：stdlib std layer

> **阶段**: Stage 5.30
> **版本**: v0.11.26 → v0.11.27
> **状态**: ✅ Complete
> **流程**: stage-committee-process.md v3.20 §17.3 时期 1

## 1. 目标

扩展 stdlib 到 `std` 层——添加 OS 依赖类型（File/Path/TcpStream/Thread/Mutex/
Result/Option/...）和 I/O trait（Read/Write/Seek/Error/Termination）。

## 2. 设计

### 2.1 新增常量

| 常量 | 内容 | 数量 |
|------|------|------|
| `STDLIB_STD_TYPES` | File/Dir/Path/PathBuf/OpenOptions/TcpStream/TcpListener/UdpSocket/Thread/JoinHandle/Mutex/Condvar/Command/ExitStatus/OsStr/OsString/Stdin/Stdout/Stderr/BufReader/BufWriter/Result/Option/Some/None/Ok/Err | 26 |
| `STDLIB_STD_TRAITS` | Read/Write/Seek/BufRead/Error/Termination | 6 |

### 2.2 扩展现有 API

- `StdlibLayer` 新增 `Std` variant
- `all_stdlib_type_names()` / `all_stdlib_trait_names()` / `register_stdlib()` 扩展
- `layer_for_name()` / `names_for_layer()` 支持 Std 层

## 3. 验收标准

1. `cargo fmt --check` 零 diff ✅
2. `cargo clippy --all-targets` 0 warnings ✅
3. `cargo test` 全部通过 ✅
4. §1.2 交付前验收：全绿 ✅

---

**创建日期**: 2026-07-23
