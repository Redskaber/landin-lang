# Stage 8 Gate Review Round 3 (8.3) — extern "C" ABI support (§13.2)

> **审查日期**: 2026-07-25 | **版本**: v0.15.1 → v0.15.2
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 134 unit + 1933 integration = 2067 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `07-codegen.md` §13.2 (ABI 兼容性) + `01-language-specification.md` (extern)。

## 新增内容

### 1. BodyMeta 扩展

`BodyMeta` 新增 `abi: Abi` 字段，从 HIR 函数签名的 `f.sig.abi` 提取。

### 2. codegen_function 扩展

`codegen_function` 新增 `abi: Abi` 参数，传递到函数生成。
当前 MVP 中 Landin ABI 和 C ABI 使用相同的 LLVM 调用约定（C 是 LLVM 默认），
ABI 信息被跟踪但不在 IR 中区分。未来可添加自定义调用约定。

### 3. 测试文件 (§17.1)

`tests/v0/stage8/plan/extern_c_abi_tests.rs` — 5 个测试：
- extern C fn 声明 / 调用 / 回归 / void fn / 无参数 fn

## v0.2 路线图

| 优先级 | 行动 | 状态 |
|--------|------|------|
| P1 | Lifetime elision (§3.2) | ✅ Stage 8.1 |
| P2 | Object safety (§2.3) | ✅ Stage 8.2 |
| P2 | extern "C" ABI (§13.2) | ✅ Stage 8.3 |
| P2 | Drop elaboration (§5) | pending |
| P3 | async/await (§10) | pending |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
