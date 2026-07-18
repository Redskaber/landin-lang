# 10 — 工具链

> 本文定义 Landin 的工具链组成：编译器 `landin`、包管理器 `landinc`、测试 runner、文档生成器、LSP server。MVP 必需前 3 个，后 2 个推迟到 v0.2。

---

## 1. 工具链组成

```
landin-toolchain/
├── landin              // 编译器二进制
├── landinc             // 包管理器 + 构建工具
├── landin-test         // 测试 runner
├── landin-doc          // 文档生成器（v0.2）
├── landin-fmt          // 代码格式化（v0.2）
├── landin-lsp          // LSP server（v0.2）
└── landin-toolchain.toml  // 工具链版本配置
```

### MVP 必需

| 工具 | v0.1 状态 | 行数估算 |
| --- | --- | --- |
| `landin` | ✅ 必需 | ~40,000 行 Landin（stage 1） |
| `landinc` | ✅ 必需 | ~2,500 行 Rust（stage 0 用，不参与自举）+ ~2,000 行 Landin（v0.2 重写） |
| `landin-test` | ✅ 必需 | ~1,500 行 Landin |
| `landin-doc` | ❌ v0.2 | - |
| `landin-fmt` | ❌ v0.2 | - |
| `landin-lsp` | ❌ v0.2 | - |

---

## 2. landin 编译器

### 2.1 命令行接口

```bash
# 编译单个文件
landin compile hello.lin -o hello

# 编译 crate（含依赖解析）
landin compile --crate-type bin --out target/debug/myapp

# 检查类型但不生成代码
landin check src/

# 输出 MIR（debug 用）
landin compile --emit mir hello.lin

# 输出 LLVM IR
landin compile --emit llvm-ir hello.lin

# 输出汇编
landin compile --emit asm hello.lin

# 目标平台
landin compile --target x86_64-unknown-linux-gnu hello.lin

# 优化级别
landin compile -O0/-O1/-O2/-O3/-Os/-Oz hello.lin

# Debug 信息
landin compile -g hello.lin

# 启用所有 warning
landin compile -W hello.lin
```

### 2.2 退出码

| 码 | 含义 |
| --- | --- |
| 0 | 成功 |
| 1 | 编译错误 |
| 2 | 配置错误（如 manifest 解析失败） |
| 3 | 链接错误 |
| 101 | 内部编译器错误（ICE） |
| 130 | 用户中断（Ctrl+C） |

### 2.3 环境变量

| 变量 | 作用 |
| --- | --- |
| `LANDIN_LOG` | 日志级别（debug/info/warn/error） |
| `LANDIN_MIR_DUMP` | `=1` 时 dump MIR 到 `/tmp/landin-mir-*` |
| `LANDIN_LLVM_DUMP` | `=1` 时 dump LLVM IR 到 `/tmp/landin-llvm-*.ll` |
| `LANDIN_BORROWCK_DUMP` | `=1` 时 dump borrow check 中间结果 |
| `LANDIN_NO_COLOR` | `=1` 禁用彩色输出 |
| `LANDIN_SYSROOT` | 覆盖默认 sysroot 路径 |
| `LANDIN_TARGET` | 覆盖默认目标平台 |
| `LANDIN_INCREMENTAL` | `=1` 启用增量编译（v0.2） |
| `LANDIN_STAGE0_BOOTSTRAP` | 强制使用 stage-0 编译器（开发用） |

### 2.4 错误输出格式

```
error[E0308]: mismatched types
  --> src/main.lin:10:5
   |
10 |     let x: i32 = "hello";
   |            ---   -------
   |            |     |
   |            |     expected `i32`, found `&str`
   |            expected due to this

error: aborting due to 1 previous error
```

支持 `--json` 输出（IDE 集成用）：

```json
{
    "messages": [
        {
            "level": "error",
            "code": "E0308",
            "message": "mismatched types",
            "spans": [
                {
                    "file": "src/main.lin",
                    "line_start": 10,
                    "line_end": 10,
                    "column_start": 5,
                    "column_end": 19,
                    "label": "expected `i32`, found `&str`"
                }
            ]
        }
    ]
}
```

---

## 3. landinc 包管理器

### 3.1 命令行接口

```bash
# 创建新项目
landinc new myapp           # 二进制项目
landinc new --lib mylib     # 库项目

# 构建项目
landinc build               # 默认 debug
landinc build --release     # release 优化

# 运行
landinc run                 # cargo run 等价
landinc run -- arg1 arg2    # 传递参数给程序

# 测试
landinc test                # 运行所有测试
landinc test -- --nocapture # 显示 println 输出

# 添加依赖
landinc add serde --version "^1.0"

# 清理
landinc clean

# 发布（v0.2）
landinc publish

# 检查
landinc check               # cargo check 等价
```

### 3.2 manifest: landin.toml

```toml
[package]
name = "myapp"
version = "0.1.0"
edition = "2024"          # Landin edition（v0.2 启用）
authors = ["Author <author@example.com>"]
license = "MIT"
description = "My Landin app"

[dependencies]
serde = "^1.0"            # 从 registry 拉
serde_json = { version = "^1.0", optional = true }
my_lib = { path = "../my_lib" }   # 路径依赖
my_git = { git = "https://github.com/foo/bar", tag = "v0.2.0" }   # git 依赖

[dev-dependencies]
proptest = "^1.0"

[features]
default = ["serde_json"]
json = ["serde_json"]

[profile.dev]
opt-level = 0
debug = true

[profile.release]
opt-level = 2
lto = false                # v0.2

[[bin]]
name = "myapp"
path = "src/main.lin"

[lib]
name = "myapp_lib"
path = "src/lib.lin"
```

### 3.3 项目布局

```
myapp/
├── landin.toml
├── src/
│   ├── main.lin         # 二进制入口（bin crate）
│   ├── lib.lin          # 库入口（lib crate）
│   ├── module1.lin      # 子模块
│   └── module1/
│       └── submodule.lin
├── tests/
│   ├── integration.lin  # 集成测试
│   └── helpers.lin
├── benches/            # v0.2
│   └── bench.lin
├── examples/           # v0.2
│   └── example1.lin
├── build.lin            # build script（v0.2）
└── target/             # 构建产物（gitignore）
    ├── debug/
    │   ├── myapp
    │   └── deps/
    └── release/
        └── myapp
```

### 3.4 依赖解析

MVP 实现简化版 semver：

- 支持基本形式：`1.0`, `^1.0`, `~1.0`, `1.0.0`, `1.0.0-beta`, `*`
- 不支持：`1.0.0 - 2.0.0`（range）、`1.0.0+build`
- 解析算法：贪心回溯（与 cargo 一致，但 MVP 不解决复杂 diamond dependencies 的多版本共存，要求"全局唯一版本"）

MVP 限制：每个依赖在整个依赖图中只能有一个版本。若多版本冲突报错，需用户手动解决。v0.2 加多版本支持。

### 3.5 registry

MVP 阶段：

- **本地 registry**: `~/.lin/registry/` 目录，含下载的 crate 源码
- **git 依赖**: 直接 `git clone` 到 `~/.lin/git/`
- **中心 registry**: v0.2 上线 `crates.landin-lang.org`

### 3.6 build script（v0.2）

MVP 不支持 build script。若依赖系统库，通过 `landin.toml` 配置：

```toml
[dependencies]
libc = "^0.2"

[links]
m = { static = false }    # 链接 libm
z = "static"              # 静态链接 libz
```

v0.2 加 build script（`build.lin`）后，可执行任意构建逻辑。

---

## 4. landin-test 测试 runner

### 4.1 测试函数标注

```landin
#[test]
fn test_add() {
    assert_eq!(1 + 1, 2);
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn test_oob() {
    let v = vec![1, 2, 3];
    let _ = v[10];
}

#[test]
#[ignore]   // 仅 --ignored 时运行
fn test_slow() {
    // ...
}
```

### 4.2 assert 宏（v0.2 macro，v0.1 函数）

MVP 无 macro，提供函数式 API：

```landin
// core::assert
pub fn assert(cond: bool) {
    if !cond {
        panic!("assertion failed");
    }
}

pub fn assert_eq<T: PartialEq + Debug>(left: T, right: T) {
    if left != right {
        panic!("assertion `left == right` failed\n  left: {:?}\n right: {:?}", left, right);
    }
}

pub fn assert_ne<T: PartialEq + Debug>(left: T, right: T) {
    if left == right {
        panic!("assertion `left != right` failed\n  left: {:?}\n right: {:?}", left, right);
    }
}
```

v0.2 加宏后改为 `assert!(cond)`、`assert_eq!(a, b)`，旧函数保留为内部用。

### 4.3 命令行接口

```bash
# 运行所有测试
landinc test

# 运行指定测试（按名称前缀过滤）
landinc test test_add

# 仅运行 ignored 测试
landinc test -- --ignored

# 多次运行（fuzzing 简易版）
landinc test -- --test-threads=4 --count=100

# 显示 println 输出
landinc test -- --nocapture

# 输出 JUnit XML
landinc test -- --format=junit -o test-results.xml
```

### 4.4 测试输出

```
running 5 tests
test test_add ... ok
test test_sub ... ok
test test_mul ... FAILED
test test_div ... ok
test test_oob ... ok

failures:

---- test_mul stdout ----
thread 'test_mul' panicked at 'assertion `left == right` failed
  left: 6
 right: 7', src/main.lin:5:5

test result: FAILED. 4 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

### 4.5 集成测试

`tests/` 目录下的 `.lin` 文件作为独立 crate 编译，可 `extern crate` 引用 lib crate：

```landin
// tests/integration.lin
extern crate myapp_lib;

use myapp_lib::add;

#[test]
fn test_add_integration() {
    assert_eq(add(1, 2), 3);
}
```

### 4.6 基准测试（v0.2）

MVP 不支持。v0.2 加 `#[bench]` 标注 + `benches/` 目录。

---

## 5. landin-doc 文档生成器（v0.2）

### 5.1 文档注释

```landin
/// Adds two numbers.
///
/// # Examples
///
/// ```
/// use mylib::add;
/// assert_eq(add(1, 2), 3);
/// ```
pub fn add(a: i32, b: i32) -> i32 {
    a + b
}

//! Crate-level doc.
//! This is the mylib crate.
```

### 5.2 命令行

```bash
landin-doc                  # 生成到 target/doc/
landin-doc --open           # 生成并打开浏览器
landin-doc --output-dir ./docs
```

输出为静态 HTML（仿 rustdoc）。

---

## 6. landin-fmt 代码格式化（v0.2）

```bash
landin-fmt                  # 格式化所有文件
landin-fmt --check          # 仅检查，不修改（CI 用）
landin-fmt src/main.lin      # 格式化单个文件
```

格式规则类似 rustfmt，但 Landin 特定调整（如 4 空格缩进、`{` 同行）。

---

## 7. landin-lsp LSP server（v0.2）

实现 Language Server Protocol 3.17：

| 功能 | 支持等级 |
| --- | --- |
| 诊断（diagnostics） | ✅ 完整 |
| Hover | ✅ 类型/文档 |
| Goto definition | ✅ |
| Goto implementation | ✅ |
| Find references | ✅ |
| Rename | ✅ |
| 代码补全 | ⚠️ v0.2 简化 |
| 代码格式化 | ✅（用 landin-fmt） |
| Inlay hints | ✅ v0.2 |

集成方式：VS Code 扩展、Neovim（lspconfig）、Emacs（lsp-mode）。

---

## 8. 工具链版本管理（v0.2）

### 8.1 landinup

类似 rustup：

```bash
landinup install stable
landinup install nightly
landinup default stable
landinup override set nightly  # 当前目录用 nightly
landinup component add landin-fmt landin-lsp
landinup target add wasm32-unknown-unknown
```

### 8.2 landin-toolchain.toml

```toml
[toolchain]
channel = "stable"
components = ["landin-fmt", "landin-lsp"]
targets = ["wasm32-unknown-unknown"]
profile = "default"
```

放在项目根目录，`landinc` 读取并自动选择工具链。

---

## 9. CI/CD 集成

### 9.1 GitHub Actions

```yaml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Install Landin
        run: curl --proto '=https' --tlsv1.2 -sSf https://landin-lang.org/install.sh | sh
      - name: Build
        run: landinc build --release
      - name: Test
        run: landinc test
      - name: Lint
        run: landinc clippy    # v0.2
```

### 9.2 跨平台构建矩阵

```yaml
strategy:
  matrix:
    os: [ubuntu-latest, macos-latest, windows-latest]
    target: [x86_64-unknown-linux-gnu, aarch64-unknown-linux-gnu, 
             x86_64-apple-darwin, aarch64-apple-darwin, x86_64-pc-windows-msvc]
```

---

## 10. 与 Rust 工具链的对应关系

| Rust | Landin | 状态 |
| --- | --- | --- |
| `rustc` | `landin` | ✅ MVP |
| `cargo` | `landinc` | ✅ MVP（简化版） |
| `cargo test` | `landinc test` | ✅ MVP |
| `cargo build` | `landinc build` | ✅ MVP |
| `cargo run` | `landinc run` | ✅ MVP |
| `cargo add` | `landinc add` | ✅ MVP |
| `rustdoc` | `landin-doc` | ❌ v0.2 |
| `rustfmt` | `landin-fmt` | ❌ v0.2 |
| `rust-analyzer` | `landin-lsp` | ❌ v0.2 |
| `rustup` | `landinup` | ❌ v0.2 |
| `clippy` | `landin-clippy` | ❌ v0.3 |
| `miri` | `landin-miri` | ❌ 长期 |

---

**下一文档**: [`11-testing.md`](./11-testing.md) — 测试策略
