# Stage 18.46 — 自举路线图设计 (Self-Hosting Roadmap)

> **Author**: redskaber + ARCH-A + REV-A
> **Date**: 2026-08-07
> **Version**: v0.316.0
> **Process**: stage-committee-process.md v5.0 §13.5 (1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 路径选择: 渐进式自举 (路径 B)

基于 Stage 18.45 的评估，选择**渐进式自举**路径。

## 2. 路线图

### Phase 0: 标准库基础 (10-15 stages)

**目标**: 让 Landin 能做实际编程工作

| Stage | 内容 | 依赖 |
|-------|------|------|
| 18.48 | Vec<T> 实现 (extern malloc + realloc + free) | extern "C" ✅ |
| 18.49 | String 实现 (基于 Vec<u8>) | Vec ✅ |
| 18.50 | Result<T, E> / Option<T> 实现 | enum ✅ |
| 18.51 | Iterator trait + 基本迭代器 | trait ✅ |
| 18.52 | HashMap<K, V> 实现 | Vec ✅ |
| 18.53 | 文件 I/O (extern fopen/fread/fwrite/fclose) | extern "C" ✅ |
| 18.54 | 字符串方法 (split/join/parse/trim) | String ✅ |
| 18.55 | ? 运算符 (语法糖 → match) | Result ✅ |
| 18.56 | 跨文件编译 (mod/use 实际文件加载) | 文件 I/O ✅ |
| 18.57 | Phase 0 集成测试 | 全部 ✅ |

### Phase 1: Lexer 自举 (5-8 stages)

**目标**: 用 Landin 重写词法分析器

| Stage | 内容 |
|-------|------|
| 18.58 | Lexer 数据结构 (Token/TokenKind/Span) in Landin |
| 18.59 | Lexer 核心逻辑 (字符 → Token) in Landin |
| 18.60 | Lexer 测试 (用 Landin 编写测试) |
| 18.61 | Lexer 集成 (Rust 调用 Landin-compiled Lexer) |
| 18.62 | Phase 1 验证 |

### Phase 2: Parser 自举 (8-12 stages)

**目标**: 用 Landin 重写语法分析器

### Phase 3: HIR/MIR 自举 (10-15 stages)

**目标**: 用 Landin 重写 HIR lowering + MIR

### Phase 4: Codegen 自举 (5-10 stages)

**目标**: 用 Landin 重写 LLVM codegen (通过 extern FFI 调用 LLVM-C)

### Phase 5: 完整自举 (3-5 stages)

**目标**: Landin 编译器完全自举

## 3. 关键设计决策

### 3.1 内存管理
- 使用 extern "C" 调用 malloc/realloc/free
- Vec<T> 内部使用 raw pointer + length + capacity
- 不使用 GC (与 Rust 类似的手动管理 + 所有权)

### 3.2 FFI 策略
- LLVM 交互通过 LLVM-C API (extern "C")
- 文件 I/O 通过 libc (extern "C")
- 不需要嵌入运行时

### 3.3 编译策略
- Stage 0: Rust 编译器 (当前) → 编译 Landin 代码
- Stage 1: Landin 编译器 (Landin 编写, Rust 编译器编译)
- Stage 2: Landin 编译器自举 (Landin 编译器编译自身)

### 3.4 测试策略
- 每个 Phase 完成后与 Rust 实现对比输出
- 确保功能等价后才替换 Rust 实现

## 4. 优先级调整

基于自举目标，v0.7 路线图调整：

| 优先级 | 任务 | 理由 |
|--------|------|------|
| **P0** | Phase 0: 标准库基础 | 自举前置条件 |
| **P0** | 跨文件编译 | 自举必需 |
| P1 | Phase 1: Lexer 自举 | 自举第一步 |
| P2 | GATs | 有用但非自举必需 |
| P2 | 增量编译 | 有用但非自举必需 |
| P3 | Println variant 移除 | 清理, 非阻断 |

## 5. 验收

- [x] 自举路线图设计完成
- [x] 路径选择: 渐进式自举 (路径 B)
- [x] Phase 划分: 0-5, 每个 Phase 有明确目标
- [x] 关键设计决策: 内存/FFI/编译/测试策略
- [x] 优先级调整: 标准库 > 自举前端

## 6. 结论

自举路线图已完成。下一步从 Phase 0 (标准库基础) 开始，
让 Landin 具备做实际编程工作的能力。
