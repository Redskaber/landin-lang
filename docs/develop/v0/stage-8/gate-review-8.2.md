# Stage 8 Gate Review Round 2 (8.2) — Object safety rules (§2.3)

> **审查日期**: 2026-07-25 | **版本**: v0.15.0 → v0.15.1
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 134 unit + 1928 integration = 2062 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `03-type-system.md` §2.3 (Trait object / Object safety, RFC #255)。

## 新增内容

### `src/traits/object_safety.rs` (新模块, ~220 LOC)

| 类型/方法 | 用途 |
|---------|------|
| `check_object_safety(trait_def)` | 检查 trait 是否 object-safe |
| `ObjectSafetyError` (enum) | 违规类型 (InvalidReceiver/ReturnsSelf/GenericMethod/AssociatedConst) |
| `is_object_safe_receiver(sig)` | 检查 receiver 是否 &self/&mut self |
| `returns_self(sig)` | 检查方法是否返回 Self |
| `has_generic_params(sig)` | 检查方法是否有泛型参数 |

### Object safety 规则 (§2.3)

1. 所有 method receiver 是 `&self` 或 `&mut self`
2. 所有 method 不返回 `Self`
3. 所有 method 不含泛型参数
4. trait 不含 associated const

### 测试

- 5 个单元测试 (object_safety.rs 内联)
- 5 个集成测试 (tests/v0/stage8/plan/object_safety_tests.rs)

## v0.2 路线图

| 优先级 | 行动 | 状态 |
|--------|------|------|
| P1 | Lifetime elision (§3.2) | ✅ Stage 8.1 |
| P2 | Object safety (§2.3) | ✅ Stage 8.2 |
| P2 | extern "C" ABI (§13.2) | pending |
| P2 | Drop elaboration (§5) | pending |
| P3 | async/await (§10) | pending |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
