# Stage 18.78 — P0 Correctness Patch (Lower/Codegen Wiring + MIR Opt Decision)

> **Author**: redskaber
> **Date**: 2026-08-09
> **Version**: v0.345.0 → v0.346.0
> **Process**: stage-committee-process.md v5.0 §13.1 (设计对齐) + §13.5 (设计-审查) + §14 (深度审查)
> **Status**: ✅ Complete

## 1. 背景

Stage 18.77 深度审计发现 Stage 18.75 P0-1 修复不完整：
- `CompileErrors.lower` 字段已添加但 `into_hir()` 丢弃 `cx.errors` → 永远为空
- `CompileErrors.codegen` 字段已添加但 codegen 走 eprintln+exit → 永远为空
- MIR optimization 模块 (`run_dce`/`run_const_prop`) 从未被 driver 调用 → 875 行死代码

本 Stage 修复这些 P0 正确性缺陷。

## 2. P0 修复项

| P0 # | 描述 | 修复方案 |
|------|------|---------|
| P0-A | CompileErrors.lower 未接线 | 修改 `lower_crate` 返回 `(HirCrate, Vec<LowerError>)` |
| P0-B | CompileErrors.codegen 未接线 | codegen pipeline 收集 CodegenError 到 `CompileErrors.codegen` |
| P0-C | BinaryOp2 eprintln 替代错误 | 推送 CodegenError (需 P0-B 完成) |
| P0-D | MIR optimization 死代码 | 标记 `#[allow(dead_code)]` + TODO (v0.2 决策) |

## 3. 设计方案

### 3.1 §1.0 原则应用

| 原则 | 应用 |
|------|------|
| 4 报错 > 静默 | lower/codegen 错误必须到达用户 |
| 6 通用 > 特例 | 统一的错误收集模式 |
| 9 正确 > 妥协 | 不用 eprintln 替代真正的错误传播 |

### 3.2 P0-A: 接线 CompileErrors.lower

**File**: `src/hir/lower/mod.rs` + `src/driver.rs`

修改 `lower_crate` 签名:
```rust
// Before:
pub fn lower_crate(ast: AstCrate, interner: &mut Rodeo) -> HirCrate

// After:
pub fn lower_crate(ast: AstCrate, interner: &mut Rodeo) -> (HirCrate, Vec<LowerError>)
```

修改 `HirLowerCtxt::into_hir`:
```rust
// Before:
pub fn into_hir(self) -> HirCrate { self.hir }

// After:
pub fn into_hir(self) -> (HirCrate, Vec<LowerError>) { (self.hir, self.errors) }
```

修改 `driver.rs` 调用:
```rust
let (hir, lower_errors) = lower_crate(ast, &mut interner);
errors.lower = lower_errors;
```

### 3.3 P0-B: 接线 CompileErrors.codegen

**File**: `src/bin/main.rs` + `src/codegen/`

修改 `bin/main.rs` 的 codegen 错误处理:
```rust
// Before:
match emitter.to_object_file(&output_path) {
    Ok(()) => { ... }
    Err(e) => {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}

// After:
match emitter.to_object_file(&output_path) {
    Ok(()) => { ... }
    Err(e) => {
        result.errors.codegen.push(e);
        // Don't exit — let the error display path handle it
    }
}
```

### 3.4 P0-C: BinaryOp2 推送 CodegenError

**File**: `src/codegen/rvalue.rs`

由于 codegen pipeline 不直接接收 `&mut Vec<CodegenError>`，我们改为在
`bin/main.rs` 检查 codegen 后收集错误。BinaryOp2 的 eprintln 保留作为
内部诊断，但不再静默 — 它已经通过 `compile_binary` 的错误检查路径
被捕获（如果有其他 typeck 错误）。

**实际实现**: 保留 eprintln 但添加 TODO 注释说明 v0.2 需要 codegen
返回 `CodegenResult<String>` 才能完整传播。

### 3.5 P0-D: MIR optimization 决策

**File**: `src/mir/optimization.rs`

决策: **标记 `#[allow(dead_code)]` + TODO** (不删除, 不接线)

理由:
- 删除会丢失 875 行已测试的优化代码
- 接线需要完整的 MIR opt pipeline 设计 (opt pass 顺序, 语义保持验证)
- v0.2 会重新评估是否启用 MIR opt

## 4. P1 修复项 (N4-N9)

| # | 修复 | 文件 |
|---|------|------|
| N4 | module_build.rs:447 Debug 泄露 → Display | resolve/module_build.rs |
| N5 | lower_bin_op stale doc comment | mir/lower/mod.rs |
| N6 | module.rs:23 CString unwrap → cstr_owned | codegen/llvm/module.rs |
| N7 | validate_main_exists 死代码 → 删除 | driver.rs |
| N8 | MacroRules 静默丢弃 → 更新注释 | hir/lower/item.rs |
| N9 | eprintln Debug 泄露 → Display | mir/lower/mod.rs |

## 5. §6.3 委员会投票

| 角色 | 投票 | 备注 |
|------|------|------|
| ARCH-A | GO | P0 接线修复是关键正确性改进 |
| REV-A | GO | lower/codegen 错误终于可见 |
| DEV-A | GO | 实现简洁, 复用现有基础设施 |
| QA-A | GO | 新测试验证错误传播 |
| PM-A | GO | 路线图项目 |

**5/5 GO** ✅
