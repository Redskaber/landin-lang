# Stage 99 开发计划 — TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因调查与修复

> **阶段**: v0.9 → v0.10 (Prelude Trait Coverage Wave)
> **TD**: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (P2, v0.10+)
> **复杂度**: L3 (跨模块根因调查: prelude.rs / codegen / MIR / LLVM integration tests)
> **版本基线**: v0.637.0 (Stage 98, 5589 tests)
> **目标版本**: v0.638.0+

## 一、强制启动自检 (§1.2.1)

1. **定位**: L3 跨模块根因调查 + 修复。涉及 prelude.rs / MIR lower / LLVM codegen / LLVM integration tests。**必须执行完整流程** (§1-§17)。
2. **对齐**: 已查 tech-debt-register.md (TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH P2 v0.10+), Stage 98 dev-log (mangling 修复细节), prelude.rs (Debug/PartialOrd impl bodies 移除位置), stage-93 audit-report (架构审查)。
3. **阻断**: Stage 98 已修复 mangling collision。当前阻断是 prelude impl bodies 触发 stack smashing (mangling 修复后剩余)。

## 二、5W2H 根因分析

| 维度 | 内容 |
|------|------|
| **WHAT** | prelude impl method bodies (含复杂控制流 if/match 返回 String/Option) 在 LLVM integration tests 中触发 stack smashing。User code 中同样形式的 impl method 工作正常 (`test_sret2.landin → 42`)。 |
| **WHY** | 三个候选根因: (a) LLVM module verification 对 prelude impl method bodies 的处理不完整; (b) prelude impl method body 的 MIR 结构与 user code 可能不同 (lower 路径差异); (c) codegen 对 prelude impl method 的 alloca/load 顺序有 bug |
| **WHO** | ARCH-A 根因分析; DEV-A 修复; REV-A 审查; QA-A 测试 |
| **WHEN** | Stage 99 完成后 → 重新添加 Debug + PartialOrd impls |
| **WHERE** | prelude.rs (impl body 来源), codegen/llvm/function.rs (sret handling), codegen/llvm/function_sigs.rs (impl method sig), tests/v0/stage99/plan (复现 test) |
| **HOW** | 1) 写最小复现 test (Debug::fmt 含 if 返回 String); 2) cargo test 单独运行获取 SIGSEGV 现场; 3) 用 lldb 跟踪; 4) 对比 prelude impl vs user impl 的 MIR/LLVM IR 差异; 5) 修复 codegen 路径; 6) 重新添加 Debug + PartialOrd impls |
| **HOW MUCH** | 1 个根因 + 1 个通解修复 + N 个测试更新 + Debug/PartialOrd impls 重新添加 |

## 三、最小复现 test case

```rust
trait Debug { fn fmt(&self) -> String; }
impl Debug for i32 {
    fn fmt(&self) -> String {
        if *self == 0 { String::from_str("zero") } else { String::from_str("nonzero") }
    }
}
fn main() -> i32 { 0 }
```

放在 `tests/v0/stage99/plan/prelude_impl_body_repro.rs`, 单独 `cargo test --release --features llvm-backend stage99_prelude_impl_body_repro` 触发 crash。

## 四、调查步骤

1. 写最小复现 test → 实际触发 crash (确认 TD 复现)
2. cargo test 单独运行, 获取 backtrace
3. 用 lldb 跟踪 SIGSEGV 现场, 获取 crash 调用栈
4. 对比 prelude impl method body MIR vs user impl method body MIR (`MIR_DEBUG=1` 或类似)
5. 对比 LLVM IR (`LLVM_IR_DEBUG=1`)
6. 找出差异 → 定位根因
7. 实施通解修复 (per §12 最优>最小, §1.0 原则 6 通解>特解)
8. 重新添加 Debug + PartialOrd impls
9. §3.2 验收 + worklog + tech-debt-register 更新

## 五、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (≥5589 tests, 0 failures, 9 ignored)
- [ ] Debug + PartialOrd impls 重新添加且测试全绿

## 六、关键参考

- Rust 设计: Rust prelude 的 Debug/PartialOrd impl bodies 通过 Display trait + core::fmt 模块实现, 含复杂控制流。Landin v0.9 是简化版, 但 codegen 必须能处理同样的 if/match 控制流。
- Stage 98 修复: mangling 包含 trait 名 (4 文件 + 32 测试)。Mangling 修复正确 — user code 验证通过。
