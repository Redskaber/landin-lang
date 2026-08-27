# Landin Stage 0 构建指南

> **版本**：v0.498.0 (Stage 18.336)
> **平台**：Linux (x86_64 + aarch64) — Windows/macOS 待 v0.2+
> **最后更新**：Stage 18.336 (2026-08-27)
> **LLVM**：LLVM 22.1 (llvm-sys 221, 默认) / LLVM 19.x (fallback)

---

## 1. 环境要求

### 1.1 Rust 工具链

| 组件 | 最低版本 | 推荐版本 |
|---|---|---|
| rustc | 1.70.0 | stable (最新稳定通道) |
| cargo | 1.70.0 | stable |
| Rust edition | 2021 | 2021 |
| rustfmt | — | stable (代码格式化) |
| clippy | — | stable (lint) |

**安装 Rust**（如未安装）：

```bash
# Unix/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
rustup component add rustfmt clippy
```

验证：

```bash
rustc --version
cargo --version
```

### 1.2 系统依赖

- **Linux**：`gcc`（链接器）、`musl`（可选，用于静态链接）
- **macOS**：Xcode Command Line Tools（`xcode-select --install`）
- **Windows**：MSVC build tools 或 `x86_64-pc-windows-gnu` 工具链

### 1.3 Cargo.toml 依赖

```toml
[dependencies]
la-arena = "0.3"          # Arena 分配器（用于 AST 节点 ID）
lasso = "0.7"             # String interner（用于 Ident/StrLit）
unicode-xid = "0.2"       # Unicode XID_Start/XID_Continue
clap = { version = "4", features = ["derive"] }  # CLI 参数解析
llvm-sys = { version = "221", features = ["prefer-dynamic"], optional = true }  # LLVM 22 绑定

[features]
llvm-backend = ["llvm-sys"]  # LLVM 原生后端（默认关闭，需 --features llvm-backend）

[dev-dependencies]
expect-test = "1.5"       # 期望测试（用于 snapshot testing）
```

**LLVM 依赖说明**：LLVM 22.1 (llvm-sys 221) 是默认后端，通过 `--features llvm-backend` 启用。
环境配置见 `scripts/setup-llvm-env.sh`（自动检测 LLVM 22，fallback 到 LLVM 19）。
详见 [`docs/llvm/`](llvm/) 目录下的 LLVM 集成文档。

---

## 2. 如何构建

**前提**：先设置 LLVM 环境（Stage 13.5+）：

```bash
source scripts/setup-llvm-env.sh
# 或: source scripts/env.sh  (LLVM 22 PATH + LD_LIBRARY_PATH helper)
```

### 2.1 Debug 构建（默认）

```bash
cd /path/to/landin-stage0
cargo build --features llvm-backend
```

产物：
- 二进制：`target/debug/landin-stage0` + `target/debug/landinc`
- 库：`target/debug/liblandin_compiler.rlib`

### 2.2 Release 构建

```bash
cargo build --release --features llvm-backend
```

产物：
- 二进制：`target/release/landin-stage0` + `target/release/landinc`
- 库：`target/release/liblandin_compiler.rlib`

### 2.3 仅构建库（不含 CLI）

```bash
cargo build --lib --features llvm-backend
```

### 2.4 仅构建 CLI 二进制

```bash
cargo build --bin landin-stage0
```

### 2.5 清理构建产物

```bash
cargo clean
```

### 2.6 查看依赖树

```bash
cargo tree
```

### 2.7 更新依赖

```bash
cargo update              # 更新 Cargo.lock
cargo update -p lasso     # 仅更新特定 crate
```

---

## 3. 如何运行 CLI

### 3.1 基本用法

```bash
landin-stage0 <FILE> [--emit-tokens] [--emit-ast]
```

### 3.2 选项说明

| 选项 | 说明 |
|---|---|
| `<FILE>` | 必需。输入的 `.lin` 源文件路径 |
| `--emit-tokens` | 仅输出 token 流（debug 用），不继续 parse |
| `--emit-ast` | 输出 AST 摘要（item 列表），不继续到后续阶段 |
| `-h` / `--help` | 显示帮助 |
| `-V` / `--version` | 显示版本号 |

### 3.3 示例：emit-tokens

准备 `hello.lin`：

```landin
fn main() {
    let x: i32 = 42;
}
```

运行：

```bash
$ landin-stage0 --emit-tokens hello.lin
KwFn
Ident(Spur(0))
LParen
RParen
LBrace
KwLet
Ident(Spur(1))
Colon
Ident(Spur(2))   # "i32"
Eq
IntLit(42, None)
Semicolon
RBrace
Eof
```

### 3.4 示例：emit-ast

```bash
$ landin-stage0 --emit-ast hello.lin
Crate with 1 items
  - Fn(FnDecl { sig: FnSig { ... }, body: Some(...), generics: ... })
```

### 3.5 示例：错误处理

`bad.lin`：

```landin
fn f( {}
```

运行：

```bash
$ landin-stage0 bad.lin
parse error: expected `)`, found `}` at 6..7
error: aborting due to 0 lex error(s) and 1 parse error(s)
$ echo $?
1
```

退出码：
- `0`：成功（无 lex/parse 错误）
- `1`：有 lex/parse 错误
- `2`：文件读取失败

### 3.6 从 stdin 读取（不支持）

当前 CLI 仅支持文件参数，不支持 stdin。如需测试短代码：

```bash
echo 'fn main() {}' > /tmp/test.lin
landin-stage0 --emit-tokens /tmp/test.lin
```

### 3.7 帮助信息

```bash
$ landin-stage0 --help
Landin compiler (stage 0)

Usage: landin-stage0 [OPTIONS] <FILE>

Arguments:
  <FILE>  Input file

Options:
      --emit-tokens  Emit token stream only
      --emit-ast     Emit AST only (don't proceed to later stages)
  -h, --help         Print help
  -V, --version      Print version
```

---

## 4. 项目结构说明

### 4.1 顶层结构

```
landin-stage0/
├── Cargo.toml          # 项目元数据 + 依赖
├── Cargo.lock          # 依赖版本锁定
├── src/                # 库 + 二进制源码
├── tests/              # 集成测试
├── docs/               # 文档（本目录）
└── target/             # 构建产物（gitignored）
```

### 4.2 `src/` 结构

```
src/
├── lib.rs              # 库入口，re-export 所有子模块
├── bin/
│   └── main.rs         # CLI 二进制入口（landin-stage0 命令）
├── lexer/              # 词法分析器
│   ├── mod.rs          # 模块导出 + tokenize() 入口
│   ├── reader.rs       # 字符级扫描器（940 行）
│   └── token.rs        # TokenKind 定义（353 行）
├── parser/             # 语法分析器
│   ├── mod.rs          # 模块导出
│   ├── parser.rs       # recursive-descent + Pratt parser（1439 行）
│   └── error.rs        # ParseError 结构
├── ast/                # 抽象语法树
│   ├── mod.rs          # re-export
│   └── kinds.rs        # AST 节点定义（619 行）
├── session/            # 全局会话信息
│   └── mod.rs          # Span / BytePos / FileId / SourceFile（148 行）
└── diagnostics/        # 诊断系统（占位）
    └── mod.rs
```

### 4.3 `tests/` 结构

```
tests/
├── lexer.rs            # Lexer 集成测试（79 个）
├── parser.rs           # Parser 集成测试（80 个）
└── ast_structure.rs    # AST 结构断言测试（28 个）
```

### 4.4 模块依赖关系

```
bin/main.rs
    │
    ▼
lib.rs
    │
    ├──► lexer/        ← 输入 src: &str → 输出 Vec<Token>
    │       │
    │       └──► token.rs    ← TokenKind 定义
    │
    ├──► parser/       ← 输入 Vec<Token> → 输出 Crate (AST)
    │       │
    │       └──► ast/        ← AST 节点定义
    │               │
    │               └──► lexer/token.rs  ← TokenKind (re-export)
    │
    └──► session/      ← Span / SourceFile (共享)
```

### 4.5 关键数据流

```
源文件 (.lin)
    │
    ▼ SourceFile::from_path(&path)
SourceFile { src: String, ... }
    │
    ▼ lexer::tokenize(&src, &mut interner)
(Vec<Token>, Vec<LexError>)
    │
    ▼ Parser::new(tokens, &interner)
Parser
    │
    ▼ parser.parse_crate()
Crate { items: Vec<Item>, attrs: Vec<Attr> }
```

---

## 5. 开发工作流

### 5.1 修改代码后运行测试

```bash
# 1. 修改源码
# 2. 运行所有测试
cargo test

# 3. 如有失败，定位
cargo test --test lexer test_failing_test_name -- --nocapture

# 4. 修复后重新运行
cargo test
```

### 5.2 添加新功能

```bash
# 1. 在 src/ 下实现功能
# 2. 在 tests/ 下添加测试
# 3. 运行测试验证
cargo test

# 4. 手动测试 CLI
echo 'fn main() { my_new_feature; }' > /tmp/test.lin
cargo run -- --emit-ast /tmp/test.lin
```

### 5.3 调试模式运行

```bash
# 用 rust-gdb 或 rust-lldb 调试
rust-gdb --args target/debug/landin-stage0 --emit-tokens /tmp/test.lin

# 或用 println! 调试
# 在源码中加 println!("debug: {:?}", x);
cargo run -- --emit-tokens /tmp/test.lin --nocapture
```

### 5.4 检查警告

```bash
cargo build 2>&1 | grep warning
```

清理所有警告（Stage 0 目标）：

```bash
cargo clippy --all-targets -- -D warnings
```

### 5.5 格式化代码

```bash
cargo fmt        # 自动格式化
cargo fmt --check  # 仅检查不修改
```

### 5.6 查看文档

```bash
cargo doc --open    # 生成文档并在浏览器打开
```

---

## 6. 常见问题

### 6.1 编译错误：`error: linking with cc failed`

缺少 C 编译器。安装：
- Linux (Debian/Ubuntu)：`sudo apt install build-essential`
- Linux (Fedora/RHEL)：`sudo dnf install gcc`
- macOS：`xcode-select --install`

### 6.2 编译错误：`error[E0658]: ... is unstable`

使用了 nightly 特性。Stage 0 仅使用 stable Rust，请检查代码或更新 Rust：

```bash
rustup update stable
```

### 6.3 测试失败：`test test_xxx ... FAILED`

查看详情：

```bash
cargo test test_xxx -- --nocapture
```

如果是 panic，查看 backtrace：

```bash
RUST_BACKTRACE=1 cargo test test_xxx -- --nocapture
```

### 6.4 CLI 报错：`error: cannot read file ...`

文件路径错误或权限不足。检查：

```bash
ls -la /path/to/file.lin
file /path/to/file.lin  # 确认是文本文件
```

### 6.5 CLI 报错：`lex error: unexpected character: '\u{feff}'`

文件开头有 BOM。删除：

```bash
sed -i '1s/^\xEF\xBB\xBF//' /path/to/file.lin
```

### 6.6 构建产物过大

使用 release + strip：

```bash
cargo build --release
strip target/release/landin-stage0
ls -lh target/release/landin-stage0
```

### 6.7 想要查看 AST 详细结构

`--emit-ast` 仅输出摘要。如需详细 AST：

```rust
// 在 src/bin/main.rs 末尾添加
println!("{:#?}", krate);
```

重新构建运行：

```bash
cargo build
./target/debug/landin-stage0 --emit-ast /tmp/test.lin
```

---

## 7. 发布与分发

### 7.1 v0.4 当前状态 (Stage 18.318)

v0.4 已完全可交付：
- 4203 tests (676 lib + 3527 integration), 0 failures
- 类 Rust 原始类型扩展模型完成
- 全量深度审查完成 (98 src files, 12 stale items fixed)
- 文档完全同步

### 7.2 v0.5+ 路线图 (BLOCKED — 需要 language features)

- `sizeof(T)` — 泛型类型大小计算 (解锁 Box::new + Vec::push real body)
- fat pointer 操作语法 — 拆解 + 构造 (解锁 String::as_str real body)
- `core::fmt` 基础设施 — Display/Debug/Formatter/Write (解锁 format! macro real body)
- 孤儿规则 — 多 crate coherence

### 7.3 自举（v0.3+，deferred）

详见蓝图 `08-bootstrap-strategy.md`。

---

**Landin Stage 0 构建指南 v0.493.0 (Stage 18.318) — 完**
