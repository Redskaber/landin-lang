# Stage 97 开发日志 — TD 根因分析 + PartialOrd 声明

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.635.0 → v0.636.0 |
| 测试数 | 5576 → 5580 (+4) |
| 失败数 | 0 → 0 |
| ignored | 9 |
| clippy warnings | 0 |
| LOC | +15 prelude.rs, +43 test |

## 修改文件

| 文件 | 变更 |
|------|------|
| `src/stdlib/prelude.rs` | 添加 `PartialOrd<Rhs>` trait 声明 (no impls); 移除 Debug impls (避免 crash) |
| `tests/v0/stage97/plan/partial_ord_trait_tests.rs` | 新建 — 4 tests |
| `Cargo.toml` | 版本 → 0.636.0 |

## 根因分析

### WHAT
prelude impl methods 返回 String (24 bytes, sret) → codegen SIGSEGV

### WHY (初判)
codegen sret 路径对 impl method body 的处理与 free fn 不同:
- free fn returning String/struct: sret 参数为第一个 implicit arg, codegen 已正确处理
- impl method returning String/struct: sret + self 参数共存, codegen 可能参数顺序错位

### WHERE
- `codegen/llvm/function.rs` (sret handling for impl methods)
- `codegen/llvm/function_sigs.rs` (impl method signature 包含 sret)

### HOW 复现
```rust
trait Debug { fn fmt(&self) -> String; }
impl Debug for i32 {
    fn fmt(&self) -> String {
        if *self == 0 { String::from_str("zero") } else { String::from_str("nonzero") }
    }
}
```

### 新 TD
- **TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH** (P2, v0.10+) — Stage 98 修复 (mangling collision)
- **TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH** (P2, v0.10+) — Stage 99 调查 (mangling 修复后剩余 stack smashing)

## 关键决策

### 决策 1: 停止阉割版推进，分析根因

**理由** (用户指示 + §1.0 原则 4 报错>静默):
- 用户指示: 遇依赖缺失停止阉割版，转而分析根因。
- TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH 是阻断项。

### 决策 2: 只声明 PartialOrd (无 impls)

**理由** (§1.0 原则 4 报错>静默):
- 声明 trait 让用户可以自定义 impl。
- 根因在 codegen, 不在 trait 声明。

## 测试覆盖

| 测试 | 类型 | 验证 |
|------|------|------|
| `stage97_partial_ord_trait_declared` | 正向 | trait 声明编译通过 |
| `stage97_undefined_type_errors` | 负向 | undefined type 报错 |
| `stage97_type_mismatch_errors` | 负向 | type mismatch 报错 |
| `stage97_nonexistent_method_errors` | 负向 | nonexistent method 报错 |

## Prelude traits 状态

Clone, Copy, Display, Fn, FnMut, FnOnce, Drop, Default, PartialEq, Eq, PartialOrd (declared), Ord

## 下一步

- Stage 98: 修复 mangling collision → TD-STRUCT-RETURN-FROM-PRELUDE-IMPL-CODEGEN-CRASH
- Stage 99: 调查 TD-PRELUDE-IMPL-BODY-CODEGEN-CRASH (mangling 修复后剩余的 stack smashing)
