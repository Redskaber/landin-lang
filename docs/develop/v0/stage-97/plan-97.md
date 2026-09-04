# Stage 97 开发计划 — TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 根因分析

> **阶段**: v0.9 (Prelude Trait Coverage Wave)
> **TD**: TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH (新发现, P2)
> **复杂度**: L2 (根因分析 + PartialOrd trait 声明)
> **版本基线**: v0.635.0 (Stage 96, 5576 tests)
> **目标版本**: v0.636.0

## 一、5W2H 启动分析

| 维度 | 内容 |
|------|------|
| **WHAT** | 调查 TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 根因。PartialOrd trait 声明 (无 impls) |
| **WHY** | Stage 96 发现 prelude impl method 返回 String (struct, sret) 触发 SIGSEGV。按用户指示"遇依赖缺失停止阉割版，转而分析根因" |
| **WHO** | ARCH-A 根因分析；DEV-A 实施 trait 声明 |
| **WHEN** | Stage 97 完成 → 进入 Stage 98 (修复) |
| **WHERE** | `src/stdlib/prelude.rs` (PartialOrd 声明) + 根因分析文档 |
| **HOW** | 1) PartialOrd trait 声明 (no impls); 2) 移除 Debug impls (避免 crash); 3) 分析根因 → 升级 TD |
| **HOW MUCH** | ~15 LOC + 4 测试。零回归 (5576→5580) |

## 二、根因初判 (5W2H WHAT/WHY/WHERE/HOW)

### WHAT
prelude impl methods 返回 String (24 bytes, 需要 sret 间接返回) → codegen SIGSEGV

### WHY
codegen sret 路径对 impl method body 的处理与 free fn 不同:
- free fn returning String/struct: sret 参数为第一个 implicit arg, codegen 已正确处理 (Stage 14.63)
- impl method returning String/struct: sret + self 参数共存, codegen 可能参数顺序错位

### WHERE
- codegen/llvm/function.rs (sret handling for impl methods)
- codegen/llvm/function_sigs.rs (impl method signature 包含 sret)

### HOW 复现
```rust
trait Debug { fn fmt(&self) -> String; }
impl Debug for i32 {
    fn fmt(&self) -> String {
        // 任何含 if/match 的 body 触发 crash
        if *self == 0 { String::from_str("zero") } else { String::from_str("nonzero") }
    }
}
```

### 新 TD
- **TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH** (P2, v0.10+) — prelude impl method returning struct/enum 触发 SIGSEGV

## 三、决策点 (§12 最优>最小, §1.0 原则 4 报错>静默)

### 决策 1: 停止阉割版推进，分析根因

**选择**: 不再添加更多 marker traits, 转向根因分析。

**理由**:
- 用户指示: 遇依赖缺失停止阉割版，转而分析根因。
- TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 是阻断项 — 不修复无法添加 Debug/PartialOrd impls。
- 继续添加 marker traits 不会推进 trait system 完整性, 只会增加 marker-only trait 数量。

### 决策 2: 只声明 PartialOrd (无 impls)

**选择**: `trait PartialOrd<Rhs> { fn partial_cmp(&self, other: &Rhs) -> Option<i32>; }` (declared, no impls).

**理由** (§1.0 原则 4 报错>静默, §12 最优>最小):
- 声明 trait 让用户可以自定义 impl — 而非完全不可用。
- 根因在 codegen, 不在 trait 声明。

## 四、§3.2 验收清单

- [ ] `cargo fmt --check` ✓
- [ ] `cargo clippy --all-targets --features llvm-backend -- -D warnings` ✓
- [ ] `cargo test --release --features llvm-backend` ✓ (5580 tests, 0 failures, 9 ignored)

## 五、Prelude traits 当前状态

Clone, Copy, Display, Fn, FnMut, FnOnce, Drop, Default, PartialEq, Eq, PartialOrd (declared), Ord

## 六、下一步

- Stage 98: 修复 TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH (mangling 修复)
- Stage 99: TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH 根因 (新发现, mangling 修复后剩余的 stack smashing)
