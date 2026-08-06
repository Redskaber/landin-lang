# Stage 16.85 — Migrate expr_operand.rs Type Errors to Use Resolver

> **Author**: redskaber + ARCH-A (Design Agent, self-reviewed)
> **Date**: 2026-08-05
> **Version**: v0.271.0
> **Process**: stage-committee-process.md v5.0 §13.5 (设计-审查 Agent 循环, 1 轮自审定稿)
> **Status**: ✅ Complete

## 1. 阶段目标

迁移 MIR lower 的 "no method found" 错误使用 resolver-backed 类型名。

## 2. 设计-审查 Agent 循环 (§13.5)

1 轮自审定稿：
- Design v1: `stage-16.85-mir-lower-resolver-design.md`
- J1-J6 全部满足

## 3. 实现内容

### 3.1 MirLowerCtxt 新增 resolver 字段

```rust
pub struct MirLowerCtxt<'a> {
    // ...
    resolver: Option<&'a crate::traits::TraitResolver>,
}
```

### 3.2 新增 set_resolver + format_ty

### 3.3 更新 lower_hir_body_to_mir_full_with_dyn_trait_plan

新增 `resolver: Option<&TraitResolver>` 参数。

### 3.4 替换 expr_operand.rs type_kind_to_string

`cx.format_ty(&recv_ty)` 替代 `type_kind_to_string(&recv_ty.kind)`

### 3.5 修复 borrowck/mod.rs:830

同样替换为 `self.format_ty(&ty)`

## 4. 测试

- 无需新增测试（stage15_88 回归测试已验证改进效果）
- stage15_88 测试更新：`<adt>` → `S`（实际类型名）
- 全量回归通过

## 5. 验收 (§3.2)

| 命令 | 要求 | 实际 |
|------|------|------|
| `cargo build --features llvm-backend` | 编译成功 | ✅ |
| `cargo fmt --check` | exit 0 | ✅ |
| `cargo clippy --all-targets` | 0 warnings | ✅ |
| `cargo test` | 0 failed | ✅ 415 lib + 2529 integration = 2944 unit tests |

## 6. 结论

GO — MIR lower 类型错误消息改进完成：
- "no method found" 显示实际类型名 ✅
- MirLowerCtxt 新增 resolver 支持 ✅
- 公共 API 变更（resolver 参数）✅
- 全量回归通过 ✅

## 7. 后续工作

- Performance Optimization (P3)
- CodegenError error system (deferred from Stage 16.76)
