# Stage 5 Gate Review Round 36 (5.36)

> **审查日期**: 2026-07-23
> **审查范围**: Stage 5.36 (stdlib trait method signatures)
> **基线版本**: v0.11.31 → v0.11.32
> **流程**: stage-committee-process.md v3.20 §17.3 时期 2

## 1. CI/CD 验证（§1.2 交付前验收 — 实际运行）

```
cargo clean: clean (918 MiB removed)
cargo test: 1130 passed, 0 failed, 2 ignored
cargo fmt --check: clean (exit 0)
cargo clippy --all-targets: 0 warnings, 0 errors
```

## 2. 新增 API（共 7 个公开符号）

| 符号 | 类型 | §23 命名合规 |
|------|------|--------------|
| `StdlibTraitMethod` | struct | `<Noun><Noun><Noun>` ✅ |
| `StdlibSelfKind` | enum | `<Noun><Noun><Noun>` ✅ |
| `stdlib_trait_methods` | free fn | `<noun>_<noun>_<noun>` ✅ |
| `stdlib_trait_method_count` | free fn | `<noun>_<noun>_<noun>_<noun>` ✅ |
| `find_stdlib_trait_method` | free fn | `find_<noun>_<noun>_<noun>` ✅ |
| `is_stdlib_trait_method` | free fn | `is_<noun>_<noun>_<noun>` ✅ |
| `stdlib_traits_with_method` | free fn | `<noun>_<noun>_with_<noun>` ✅ |

字段命名：`name` / `self_kind` / `param_count` / `return_kind` / `is_unsafe` — 全部合规。

## 3. 注册范围

- **Markers** (空方法表，区分 `Some(&[])` vs `None`): Copy/Send/Sync/Sized/Unpin/Eq
- **Core traits** (有方法): Clone(2) / Drop(1) / Default(1) / Display(1) / Debug(1) /
  PartialEq(2) / PartialOrd(1) / Ord(1) / Hash(1) / Deref(1) / DerefMut(1) /
  IntoIterator(1) / Iterator(1)
- **I/O**: Read(1) / Write(1)
- **Unary ops**: Neg(1) / Not(1)
- **Binary arithmetic** (每个独立 const 表): Add/Sub/Mul/Div/Rem/BitAnd/BitOr/BitXor/Shl/Shr — 各 1 方法
- **Assign ops** (每个独立 const 表): AddAssign/.../ShrAssign — 各 1 方法
- **未注册** (返回 `None`): Fn/FnMut/FnOnce/From/Into/AsRef/... — 后续 stage 处理

## 4. 设计要点

1. **§16 接口隔离**：`StdlibTraitMethod` 使用 `StdlibTypeKind`（stdlib 内部），
   不引用 `mir::ty`，无循环依赖。
2. **per-op const 表 vs 共享表**：每个算术 op（Add/Sub/.../Shr）有独立的 const
   方法表，确保 `StdlibTraitMethod.name` 字段始终正确（避免 "add" 占位符 + 运行时
   名称覆盖的 hack）。
3. **`ALL_REGISTERED_TRAITS` 常量**：`stdlib_traits_with_method()` 内嵌一个本地
   trait 列表常量，避免依赖 `traits::builtin::BUILTIN_TRAIT_NAMES`（保持 stdlib 自
   包含，不向后依赖 traits 模块）。
4. **markers 返回 `Some(&[])`**：与 `None` 区分（"trait 在注册表中但无方法" vs
   "trait 完全不在注册表中"）。

## 5. 新测试（24 个）

`tests/v0/stage5/plan/stdlib_trait_method_tests.rs`：

| 测试 | 描述 |
|------|------|
| `test_stdlib_trait_methods_clone` | Clone 2 方法 |
| `test_stdlib_trait_methods_drop` | Drop 1 方法 + SelfByMutRef |
| `test_stdlib_trait_methods_default` | Default::default NoSelf |
| `test_stdlib_trait_methods_display` | Display::fmt |
| `test_stdlib_trait_methods_partial_eq` | PartialEq 2 方法 |
| `test_stdlib_trait_methods_ord` | Ord::cmp |
| `test_stdlib_trait_methods_marker_empty` | 6 markers 全空 |
| `test_stdlib_trait_methods_add` | Add::add by-value self |
| `test_stdlib_trait_methods_sub` | Sub::sub 独立表 |
| `test_stdlib_trait_methods_all_arith_binary` | 10 个二进制 op 全部正确 |
| `test_stdlib_trait_methods_all_arith_assign` | 10 个 assign op 全部正确 |
| `test_stdlib_trait_methods_iterator` | Iterator::next |
| `test_stdlib_trait_methods_none` | 未知 trait 返回 None |
| `test_stdlib_trait_method_count` | method count 查询 |
| `test_find_stdlib_trait_method_hit` | find 命中 |
| `test_find_stdlib_trait_method_miss` | find 未命中 |
| `test_find_stdlib_trait_method_arith` | 算术 op 精确匹配 |
| `test_is_stdlib_trait_method_true` | is_stdlib_trait_method true |
| `test_is_stdlib_trait_method_false` | is_stdlib_trait_method false |
| `test_stdlib_traits_with_method_clone` | 反向查询 clone → Clone |
| `test_stdlib_traits_with_method_fmt` | 反向查询 fmt → Display + Debug |
| `test_stdlib_traits_with_method_bogus` | 反向查询未知 → 空 |
| `test_stdlib_trait_method_has_self` | has_self() helper |
| `test_stdlib_trait_method_partial_eq` | Eq 派生 |

## 6. 委员会投票

- Architect: GO — §16 隔离合规，stdlib 自包含
- Tech Lead: GO — 1130 tests, 0 clippy warnings
- QA: GO — 24 新测试覆盖正/负/边界
- Doc: GO — plan + gate-review + dev-log + worklog + RELEASE_NOTES + README 同步
- API Naming: GO — 全部新 API 遵循 §23

**5/5 GO → PASS**

## 7. 后续依赖

- **Stage 5.37+ (dyn Trait MIR lowering)**: 直接使用 `stdlib_trait_methods()` 生成
  vtable 函数指针类型签名。
- **Stage 5.38+ (typeck trait bound solving)**: 使用 `find_stdlib_trait_method()`
  校验方法调用是否匹配 trait 接口。

---

**审查完成**: 2026-07-23
