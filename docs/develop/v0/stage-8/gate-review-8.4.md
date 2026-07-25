# Stage 8 Gate Review Round 4 (8.4) — Drop elaboration (§5)

> **审查日期**: 2026-07-25 | **版本**: v0.15.2 → v0.15.3
> **流程**: stage-committee-process.md v3.21 §13.4 + §14.4 + §17.1 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 143 unit + 1940 integration = 2083 total (0 failed)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
```

## §13.4 设计对齐

查阅 `04-ownership-borrowing.md` §5 (Drop check + Drop 顺序)。

## 新增内容

### `src/borrowck/drop_elaboration.rs` (新模块, ~250 LOC)

| 类型/方法 | 用途 |
|---------|------|
| `DropElaborator` | Drop 析构分析器 |
| `DropSet` | 需要析构的 local 集合（逆序） |
| `register_drop_impl(def_id)` | 注册有 `impl Drop` 的类型 |
| `needs_drop(ty)` | 检查类型是否需要析构 |
| `compute_drop_set(mir, bb_id)` | 计算基本块的 drop 集合 |
| `elaborate(mir)` | 分析所有基本块的 drop 需求 |

### Drop 顺序规则 (§5.4)

1. 局部变量：按声明顺序**逆序**析构
2. Struct 字段：按声明顺序**逆序**析构
3. Match arm 绑定：在 arm 块结束时析构

### needs_drop 规则

| 类型 | needs_drop |
|------|-----------|
| Bool/Char/Int/Uint/Float | ❌ |
| Ref/RawPtr | ❌ |
| FnDef/FnPtr | ❌ |
| Str/Slice | ❌ |
| Array | 递归检查元素 |
| Tuple | 递归检查任一元素 |
| Adt | 检查是否有 `impl Drop` |
| Closure | 递归检查捕获 |
| Param/Foreign | 保守 true |

### 测试

- 9 个单元测试 (drop_elaboration.rs 内联)
- 7 个集成测试 (tests/v0/stage8/plan/drop_elaboration_tests.rs)

## v0.2 路线图

| 优先级 | 行动 | 状态 |
|--------|------|------|
| P1 | Lifetime elision (§3.2) | ✅ Stage 8.1 |
| P2 | Object safety (§2.3) | ✅ Stage 8.2 |
| P2 | extern "C" ABI (§13.2) | ✅ Stage 8.3 |
| P2 | Drop elaboration (§5) | ✅ Stage 8.4 |
| P3 | async/await (§10) | pending |

## 委员会投票

**5/5 GO → PASS**

---

**审查完成**: 2026-07-25
