# Stage 18.302 — Phase C 分析: format! macro 需要 core::fmt 基础设施

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26

## 5W2H 分析

### What
`format!`/`println!` macro 当前通过 `lower_format_variadic_intrinsic` (535 LOC) 生成单个 C runtime 调用。

### Why (根因)
Landin 缺少 Rust 的 `core::fmt` 基础设施:
- `Display`/`Debug` trait (类型如何格式化自身)
- `Formatter` struct (格式化上下文)
- `Write` trait (输出目标)
- `Arguments` struct (编译时格式字符串解析)

### Rust 设计
Rust 的 `println!("x={}", x)` 展开为:
```rust
match (&x,) {
    (arg0,) => {
        let mut formatter = core::fmt::Formatter::new(...);
        formatter.write_str("x=");
        core::fmt::Display::fmt(arg0, &mut formatter);
        formatter.write_str("\n");
    }
}
```
这需要: Display/Debug trait + Formatter + Write trait + Arguments struct — 全部是 v0.5+ 语言特性。

### Rust 哲学
- **零成本抽象**: Rust 的 format! 在编译时解析格式字符串, 无运行时解析开销
- **显式优于隐式**: 每个类型显式实现 Display/Debug trait

### How Much
- 实现 core::fmt 基础设施 = v0.5+ 大型语言特性开发
- 当前 Landin 不支持: trait objects (`dyn Display`), associated types on traits, `core::fmt` module
- 535 LOC intrinsic 是 **合理的 MVP**, 不是"特解" — 它是给定语言能力下的最优实现

## 结论

Phase C (format! macro) **不能在当前阶段实施** — 需要 `core::fmt` 基础设施 (v0.5+)。

当前 `lower_format_variadic_intrinsic` 是 **合理的 MVP**, 不是特解:
1. 它使用 C runtime 函数 (`__landin_format_variadic`) — 这是 extern "C" 调用
2. 它在 MIR lower 层生成调用代码 — 因为格式字符串解析需要编译时信息
3. 它不支持 trait objects — 因为 Landin 还没有 trait objects

**这不是"治症不治根" — 根因是语言能力不足, 不是架构设计错误。**

## 修正后的 Phase 路径 (最终)

| Phase | 内容 | 状态 | 理由 |
|-------|------|------|------|
| A | i64 → usize | ✅ 已完成 | — |
| B-1 | extern C 声明 | ✅ 已验证 | — |
| B-2 | sizeof + fat pointer ops | ⏸ v0.5+ | 需要新语言特性 |
| C | format! macro | ⏸ v0.5+ | 需要 core::fmt 基础设施 |
| ~~原 B~~ | ~~移除 marker body~~ | ✅ 跳过 | marker body 是正确架构 |

## 当前架构评估

Landin 的 intrinsic 调度架构 **在当前语言能力下是最优的**:
1. str::len/is_empty/as_bytes → marker body + intrinsic dispatch (Rust 也是 intrinsic)
2. String/Vec/Box 方法 → early interception (需要 sizeof + fat pointer ops, v0.5+)
3. format!/println! → variadic intrinsic (需要 core::fmt, v0.5+)

所有"特解"的根因都是 **language feature gaps**, 不是架构设计错误。在给定语言能力下, 当前实现是最优的。
