# 19 — 项目元信息（SSOT）

> 本文是 Landin 语言项目元信息的**单一来源**（Single Source of Truth）。所有文档中关于编译器名称、文件后缀、CLI、目录约定、工具链命名等元信息以此为准。v1.3.0 新增（N1/N2 审查建议），v1.3.1 重命名为 Fuller（N3/N4 双重验证），v1.3.2 重命名为 Landin（N5/N6 调研：放弃语义链思维定势，采用 PL 学术人名）。

---

## 1. 项目身份

### 1.1 语言名称

| 项 | 值 |
| --- | --- |
| **语言名** | **Landin** |
| **二进制名** | `landin` |
| **CLI 别名** | `lnd`（短缩写，可选） |
| **含义** | Landin——6 字母短词，易记、CLI 友好。名字恰好与 1966 年论文《The Next 700 Programming Languages》相关，该论文是关于"未来语言应该是什么样"的奠基性思考 |
| **文化意象** | 面向未来的系统语言（非语义链命名，非人名致敬；名字本身有独立成立的好故事） |
| **Tagline** | "A systems language for the next 700."（建议，可在月 1 确定） |

### 1.2 命名决策依据（v1.3.2 最终）

**命名哲学**（N5/N7 调研结论）：

- N5 调研 15 门成功语言命名模式，发现无一采用父语言语义链命名（Rust≠"Safer-C"，Haskell≠"Lambda-ML"）
- 用户指出"命名不必为了语义放弃更好选择，也不必为了致敬使用人名"
- N7 进一步探索纯粹命名（不语义、不人名），但 8 轮命名决策已到收敛临界点
- **最终决策**：维持 Landin——核心优势不是"致敬人名"，而是**零冲突 + 已故无需同意 + 故事可独立成立**

**Landin 的故事可独立框架**（不依赖"致敬"）：

- "Landin" 作为名字本身：6 字母、2 音节、易记、CLI 友好
- 故事层：1966 年论文《The Next 700 Programming Languages》提出"未来语言应基于 lambda calculus"——这篇论文本身就是关于"未来语言应该是什么样"的奠基性思考，与本语言"设计一门面向未来的系统语言"定位契合
- 不是"为了致敬 Peter Landin 而命名"，而是"这个名字恰好有一个独立成立的好故事"

**v1.2.3 之前使用 "Forge"**，经 N1 报告（37 轮调研）发现 17+ 冲突：

- 4 个同名编程语言（zesterer/Forge、humancto/forge-lang、Bill Cox/CodeRhapsody Forge、Treechcer/FORGE）
- Foundry `forge` CLI（Solidity 标准工具，Web3 高频命令）
- Rust 官方 `forge.rust-lang.org`（contributor 文档站）
- Atlassian Forge / Autodesk Forge®（USPTO 注册商标 #6231989）
- crates.io / PyPI / npm 全部被占用
- Minecraft Forge / Forgejo / Eclipse Forge 等强势项目

**v1.3.0 曾采用 "Quench"**，但 N3/N4 双重独立验证发现 Quench 也存在致命冲突：

- crates.io/crates/quench v0.3.0 是同名编程语言（虽已改名 Moss，但 crates.io 名称永久锁定）
- GitHub quench-lang 组织永久占用
- QUENCH 商标由 Quench.ai Ltd（伦敦 AI 公司）在 USPTO Class 42 + EUIPO Class 9/35/36/38/41/42/45 + UK 多国注册，活跃持有人
- 至少 12 个独立 Quench 品牌在运营

**Fuller 评分 29/40**（N4 报告 Top 1），核心优势：

- 无同名编程语言冲突
- GitHub fuller-lang 组织可用（HTTP 404）
- landin-lang.org / fuller-lang.com 域名可用（NXDOMAIN）
- crates.io/PyPI/npm 顶级被占但 fuller-lang / fullerc / fullerup 后缀全可用
- 无软件类商标冲突（Fuller 作为姓氏在 Lanham Act §2(e)(4) 下天然难注册——双向防御）
- 与 Rust→Forge→Fuller 金属工艺链语义连贯（锻造工具：拔长/起槽/整形）
- 与 14 门可比语言零混淆

### 1.3 备选候选（已否决）

| 候选 | 否决理由 |
| --- | --- |
| Forge | 17+ 冲突（4 同名语言 + Foundry CLI + Atlassian/Autodesk 商标 + crates.io/PyPI/npm 占用） |
| Quench | QUENCH 商标由 Quench.ai Ltd 多国注册 + crates.io 同名语言 + GitHub org 占用 |
| Fuller | 无 fatal conflict 但"锻造工具"故事陌生 + 仍陷语义链思维定势 + 品牌个性弱 |
| Wadler | 39/50 总分最高，但 Philip Wadler 在世，需征求同意 |
| Naur | NAUR Inc. 医疗软件公司商标冲突 |
| Kahan | Kahan Technologies cloud ERP 软件公司商标冲突 |
| Steele | STEELE GROUP + STEELE INDUSTRIES 双重商标冲突 |
| Anvil | Foundry `anvil` 二进制硬冲突（Ethereum 本地节点） |
| Crucible | Galois Crucible + CMU SEI Crucible 多重冲突 |

### 1.4 版本号体系

| 版本 | 含义 |
| --- | --- |
| v0.1.x | stage 0 完成（仅 Rust 实现的编译器），不自举 |
| v0.2.x | 标准库扩展 + 工具链完善 |
| v0.3.x | 自举完成（stage 1 + stage 2 验证） |
| v1.0.0 | 第一个稳定版本 |
| v1.x.y | 向后兼容的特性添加 |
| v2.0.0 | 破坏性变更 |

### 1.5 发布通道

| 通道 | 频率 | 特性 |
| --- | --- | --- |
| nightly | 每日构建 | 含 unstable 特性 |
| beta | 6 周一次 | 从 nightly 拣选 |
| stable | 6 周一次 | 从 beta 拣选 |

MVP 阶段仅 nightly，v0.5 后启用 beta/stable。

---

## 2. 文件后缀权威清单

| 后缀 | 用途 | 二进制/文本 |
| --- | --- | --- |
| `.lin` | Landin 源文件（首选） | UTF-8 文本 |
| `.lnd` | Landin 源文件（短缩写，可选） | UTF-8 文本 |
| `.linrs` | Landin MIR 文本表示（debug dump） | 文本 |
| `.lino` | Landin 对象文件（编译产物） | 二进制 |
| `.linlib` | Landin 静态库（rlib） | 二进制 |
| `landin.toml` | crate manifest | TOML 文本 |

**注意**：v1.2.3 之前使用 `.fg` / `.fgrs` / `.fgo` / `.fglib` / `forge.toml`，v1.3.0 改为 `.quench` / `.qnrs` / `.qno` / `.qnlib` / `quench.toml`，v1.3.1 改为 `.ful` / `.fulrs` / `.fulo` / `.fullib` / `fuller.toml`，v1.3.2 最终改为 `.lin` / `.linrs` / `.lino` / `.linlib` / `landin.toml`。

### 2.1 源文件编码

- UTF-8 编码
- LF 或 CRLF 均可，编译器内部归一化为 LF
- 不允许 BOM

---

## 3. 项目目录约定

```
myapp/
├── landin.toml              # crate manifest
├── src/
│   ├── main.lin           # 二进制入口（bin crate）
│   ├── lib.lin            # 库入口（lib crate）
│   ├── module1.lin        # 子模块
│   └── module1/
│       └── submodule.lin
├── tests/
│   ├── integration.lin    # 集成测试
│   └── helpers.lin
├── benches/                  # v0.2 基准测试
│   └── bench.lin
├── examples/                 # v0.2 示例
│   └── example1.lin
└── target/                   # 构建产物（gitignore）
    ├── debug/
    │   ├── myapp
    │   └── deps/
    └── release/
        └── myapp
```

---

## 4. CLI 命令权威清单

### 4.1 编译器（`landin`）

```bash
landin compile <file>           # 编译单个文件
landin compile --crate-type bin --out target/debug/myapp
landin check src/                # 仅类型检查
landin compile --emit mir <file>     # 输出 MIR
landin compile --emit llvm-ir <file> # 输出 LLVM IR
landin compile --emit asm <file>     # 输出汇编
landin compile --target x86_64-unknown-linux-gnu <file>
landin compile -O0/-O1/-O2/-O3/-Os/-Oz <file>
landin compile -g <file>             # debug 信息
landin compile --json                # JSON 输出（IDE 集成）
landin --explain E0308               # 错误代码解释
```

### 4.2 包管理器（`landinc`）

```bash
landinc new <name>              # 创建新项目
landinc new --lib <name>        # 创建库项目
landinc build                   # 构建（默认 debug）
landinc build --release         # release 构建
landinc run                     # 运行
landinc run -- arg1 arg2        # 传递参数
landinc test                    # 运行所有测试
landinc test -- --nocapture     # 显示 println 输出
landinc add <dep>               # 添加依赖
landinc clean                   # 清理
landinc check                   # 仅检查
landinc publish                 # v0.2 发布
```

**注意**：v1.2.3 之前 `forgec` 既是包管理器又是测试 runner，v1.3.0 统一为 `quenchc`（v1.3.1 改为 `fullerc`，v1.3.2 改为 `landinc`），测试通过 `landinc test` 调用。

### 4.3 工具链管理器（`landinup`，v0.2）

```bash
landinup install stable
landinup install nightly
landinup default stable
landinup override set nightly
landinup component add landin-fmt landin-lsp
landinup target add wasm32-unknown-unknown
```

### 4.4 其他工具

| 工具 | 命令 | 版本 |
| --- | --- | --- |
| 文档生成器 | `landin-doc` | v0.2 |
| 代码格式化 | `landin-fmt` | v0.2 |
| LSP server | `landin-lsp` | v0.2 |
| Lints | `landin-clippy` | v0.3 |

---

## 5. 环境变量

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

**注意**：v1.2.3 之前 `FORGE_RUSTC_BOOTSTRAP` 命名不当（暗示 Rust），v1.3.0 改为 `QUENCH_STAGE0_BOOTSTRAP`（v1.3.1 改为 `FULLER_STAGE0_BOOTSTRAP`，v1.3.2 改为 `LANDIN_STAGE0_BOOTSTRAP`）。

---

## 6. 退出码

| 码 | 含义 |
| --- | --- |
| 0 | 成功 |
| 1 | 编译错误 |
| 2 | 配置错误（如 manifest 解析失败） |
| 3 | 链接错误 |
| 101 | 内部编译器错误（ICE） |
| 130 | 用户中断（Ctrl+C） |

---

## 7. 命令行选项权威清单

### 7.1 编译器选项

| 选项 | 含义 |
| --- | --- |
| `--target <triple>` | 目标平台 |
| `-O0/-O1/-O2/-O3/-Os/-Oz` | 优化级别 |
| `-g` | 启用 debug 信息 |
| `-W` | 启用所有 warning |
| `--emit <mir\|llvm-ir\|asm\|obj>` | 输出类型 |
| `--json` | JSON 诊断输出 |
| `--no-color` | 禁用彩色 |
| `--cap-lints <level>` | 限制依赖 crate 的 lint 等级 |
| `--explain <E0xxx>` | 错误代码解释 |
| `--crate-type <bin\|lib\|rlib>` | crate 类型 |
| `--crate-name <name>` | crate 名 |
| `--out <path>` | 输出路径 |
| `--sysroot <path>` | sysroot 路径 |

### 7.2 包管理器选项

| 选项 | 含义 |
| --- | --- |
| `--release` | release 构建 |
| `--target <triple>` | 目标平台 |
| `--features <list>` | 启用 features（v0.2） |
| `--no-default-features` | 禁用默认 features（v0.2） |
| `--manifest-path <path>` | manifest 路径 |
| `--offline` | 离线模式 |
| `--frozen` | 冻结模式（要求 Cargo.lock 与 manifest 一致） |
| `-v / -vv / -vvv` | 详细输出 |

---

## 8. 工具链命名规范

### 8.1 Stage 命名

| 名称 | 含义 |
| --- | --- |
| `landin-stage0` | Rust 实现的编译器（stage 0） |
| `landin-stage1` | Landin 自身重写的编译器（stage 1） |
| `landin-stage2` | stage 1 自编译产物（stage 2，验证用） |
| `landin` | 生产编译器（stage 2 通过验证后重命名） |

**重要**：stage 0 的 Cargo.toml `[[bin]] name` 应为 `landin-stage0`，而非 `landin`，避免与最终生产二进制混淆。

### 8.2 二进制命名

| 二进制 | 用途 |
| --- | --- |
| `landin` | 编译器（生产） |
| `landinc` | 包管理器 |
| `landin-test` | 测试 runner（被 `landinc test` 调用） |
| `landin-doc` | 文档生成器（v0.2） |
| `landin-fmt` | 代码格式化（v0.2） |
| `landin-lsp` | LSP server（v0.2） |
| `landinup` | 工具链管理器（v0.2） |
| `landin-clippy` | Lints（v0.3） |

---

## 9. 仓库组织

```
github.com/landin-lang/
├── landin/                    # 主仓库（编译器 + 标准库 + 工具链）
├── landin-stage0/             # stage 0 Rust 源码（也可在主仓库内）
├── landin-compiler/           # Landin 写的编译器源码（stage 1）
├── rfcs/                      # RFC 仓库
├── landin-lang.org/           # 官网
（已废弃）              # （已废弃，因名称冲突）
└── landin-core-rs/            # core 库的 Rust 参考实现（可选）
```

**域名**：`landin-lang.org`（需 WHOIS 确认可用）

---

## 10. lang items 清单

Lang items 是编译器内部识别的特殊 trait/类型，用 `#[lang = "..."]` 标注：

| lang item | 类型 | 对应 |
| --- | --- | --- |
| `owned_box` | type | `Box<T>` |
| `freeze` | trait | `Freeze`（v0.2） |
| `sized` | trait | `Sized` |
| `unpin` | trait | `Unpin`（v0.2） |
| `drop` | trait | `Drop` |
| `clone` | trait | `Clone` |
| `copy` | trait | `Copy` |
| `sync` | trait | `Sync`（v0.2） |
| `send` | trait | `Send`（v0.2） |
| `panic` | fn | `panic!` 展开目标 |
| `panic_bounds_check` | fn | 数组越界 panic |
| `panic_overflow` | fn | 整数溢出 panic |
| `panic_div_by_zero` | fn | 除零 panic |
| `exchange_malloc` | fn | 全局 allocator 入口 |
| `start` | fn | 程序入口（main 调用前） |
| `eh_personality` | fn | 异常处理人格（v0.2 unwind） |

MVP 必需 12 个（不含 v0.2 标记）。

---

## 11. intrinsic 函数清单

Intrinsic 是编译器内建函数，通过 `landin::intrinsics` 模块访问：

| intrinsic | 用途 |
| --- | --- |
| `size_of<T>() -> usize` | 类型大小 |
| `align_of<T>() -> usize` | 类型对齐 |
| `transmute<T, U>(T) -> U` | 类型重解释 |
| `forget<T>(T)` | 跳过 drop |
| `drop_in_place<T>(*mut T)` | 原地 drop |
| `copy_nonoverlapping<T>(*const T, *mut T, usize)` | 内存复制 |
| `write_bytes<T>(*mut T, u8, usize)` | 内存填充 |
| `wrapping_add<T>(T, T) -> T` | wrapping 算术 |
| `overflowing_add<T>(T, T) -> (T, bool)` | overflowing 算术 |
| `unchecked_add<T>(T, T) -> T` | 不检查溢出（unsafe） |
| `abort() -> !` | 立即终止 |

---

## 12. ABI 名称权威清单

| ABI | 用途 | MVP |
| --- | --- | --- |
| `"Landin"` | 默认 ABI（MVP 阶段与 C 一致） | ✅ |
| `"C"` | C ABI | ✅ |
| `"System"` | 系统默认 ABI（Windows 上等同 C） | ✅ |
| `"Rust"` | Rust ABI（v0.2 互操作） | ❌ v0.2 |
| `"stdcall"` | Windows stdcall（v0.2） | ❌ v0.2 |
| `"fastcall"` | Windows fastcall（v0.2） | ❌ v0.2 |
| `"vectorcall"` | Windows vectorcall（v0.2） | ❌ v0.2 |
| `"aapcs"` | ARM AAPCS（v0.2） | ❌ v0.2 |
| `"win64"` | Windows x86-64（v0.2） | ❌ v0.2 |
| `"sysv64"` | System V x86-64（v0.2） | ❌ v0.2 |

**大小写规范**：ABI 名称首字母大写（`"Landin"` / `"C"` / `"System"`），与 05/07 文档一致。

---

## 12.1 Name Mangling 规范

Landin 的 name mangling 前缀为 `_LND`，用于 linker 区分泛型实例化：

```
Landin mangling: _LND <path> E <type_args> E <lifetime_args> E
示例: _LND3vec3mapE2Ti3u32E1aE
       → Vec::<T, u32>::map with lifetime 'a
```

Mangling 规则：

- 前缀 `_LND`
- Path: `<len><name>` 递归（Itanium 风格）
- 类型参数与生命周期参数分别编码
- 可 demangle（调试用）

---

## 12.2 Runtime 函数命名

Landin runtime 函数命名规范：

- `__landin_panic_bounds_check(index, len)` — 数组越界 panic
- `__landin_panic_overflow(op, lhs, rhs)` — 整数溢出 panic
- `__landin_panic_div_by_zero()` — 除零 panic
- `__landin_oom_abort()` — OOM abort
- `__landin_alloc(size, align)` — 全局 allocator 入口
- `__landin_dealloc(ptr, size, align)` — 全局 allocator 入口
- `liblandin_runtime` — runtime 库名

---

## 13. 标准库 crate 完整清单

| crate | 用途 | MVP |
| --- | --- | --- |
| `core` | 基本类型 + trait，无 alloc | ✅ |
| `alloc` | Box/Vec/String/Rc，依赖全局 allocator | ✅ |
| `std` | facade，re-export core + alloc + OS | ✅ |
| `landin_runtime` | panic runtime + allocator fallback | ✅ |
| `landin_intrinsics` | 编译器内建函数 | ✅（内部） |
| `libc` | libc FFI 绑定 | ✅ |
| `landin_test` | 测试 runner（libtest 等价） | ✅ |
| `landin_panic_abort` | panic = abort 实现 | ✅ |

---

## 14. 跨平台 target triple 完整清单

| Target triple | 平台 | MVP |
| --- | --- | --- |
| `x86_64-unknown-linux-gnu` | Linux x86-64 (glibc) | ✅ |
| `aarch64-unknown-linux-gnu` | Linux ARM64 | ✅ |
| `x86_64-apple-darwin` | macOS Intel | ✅ |
| `aarch64-apple-darwin` | macOS Apple Silicon | ✅ |
| `x86_64-pc-windows-msvc` | Windows MSVC | ✅ |
| `x86_64-unknown-linux-musl` | Linux musl（静态链接） | ❌ v0.2 |
| `wasm32-unknown-unknown` | WebAssembly | ❌ v0.2 |
| `wasm32-wasi` | WebAssembly + WASI | ❌ v0.2 |
| `riscv64gc-unknown-linux-gnu` | RISC-V 64 | ❌ v0.3+ |

---

## 15. 版本历史

| 版本 | 日期 | 状态 | 主要变化 |
| --- | --- | --- | --- |
| v1.0 | 2026-07-18 | 设计初版 | 13 文档，使用 "Forge" |
| v1.1 | 2026-07-18 | 修正初版 | 17 文档 |
| v1.2 / v1.2.1 / v1.2.2 | 2026-07-18 | 迭代修正 | 22 文档 |
| v1.2.3 | 2026-07-18 | 冻结 | 22 文档，0 P0 |
| v1.3.0 | 2026-07-18 | 撤销 | 23 文档（新增 19-project-meta.md），Forge → Quench 重命名，但 N3 发现 Quench 致命冲突 |
| v1.3.1 | 2026-07-18 | 撤销 | 23 文档，Quench → Fuller 重命名，但 N5 指出语义链思维定势 |
| **v1.3.2** | **2026-07-18** | **真正正式冻结** | **23 文档，Fuller → Landin 重命名（N5 推荐，PL 学术人名），元信息 SSOT 完善，0 P0 残留** |

---

## 16. 元信息一致性承诺

v1.3.2 起，所有文档的元信息以此文档为准。若发现不一致：

1. 以本文档为权威
2. 在本文档更新后，同步更新其他文档
3. 不一致报告记录到 worklog

---

**下一文档**: [`README.md`](./README.md) — 文档集入口
