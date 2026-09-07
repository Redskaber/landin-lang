# Stage 143 开发日志 — TD-PTR-INDEX-GEP-TYPE + TD-PTR-INDEX-CODEGEN-2 + TD-STDLIB-STRING-VEC 完整修复

## 阶段统计

| 指标 | 值 |
|------|-----|
| 版本 | v0.667.0 → v0.668.0 |
| 测试数 | 5822 → 5857 (+35 new) |
| 失败数 | 0 |
| ignored | 12 |
| clippy warnings | 0 |
| fmt | clean |
| 集成覆盖率 | 95% |

## 5W2H

### WHAT
Stage 143 完整修复 3 个相互关联的 P3 技术债：
1. **TD-PTR-INDEX-GEP-TYPE**: `emit_gep_index_ptr` 添加 `idx_ty` 参数 — TextEmitter 动态使用 MIR local 实际类型 (i32/i64) 而非硬编码 i64
2. **TD-PTR-INDEX-CODEGEN-2**: 完整修复 RawPtr 索引 codegen — `unwrap_fat_ptr_for_index` Ptr(_) 分支不 LOAD (caller 责任), 新增 `base_ty.is_ptr()` 检查在 Index/ConstantIndex arms 调用 codegen_place_load_typed LOAD 指针值
3. **TD-STDLIB-STRING-VEC**: 添加 `String::starts_with/ends_with/contains` + `str::starts_with/ends_with/contains` 到 prelude

### WHY (根因分析, §2.2 根因思维)

**根因链 (7 步)**:
1. **Stage 140**: `unwrap_fat_ptr_for_index` 添加 `Ptr(_)` 和 `OpaquePtr` 分支以支持 &str 字段访问, 但 `Ptr(_)` 分支没有 LOAD 指针值
2. **Stage 141**: 尝试给 `Ptr(_)` 添加 LOAD → 25 个 text IR 测试失败 (LLVM IR 格式从 `i32 0, i32 idx` 变为 `i64 idx`)。回退
3. **Stage 142**: 重新应用 codegen 修复 → 0 回归; 但添加 String 方法后 25 测试失败 (`emit_gep_index_ptr` 用 i64 但 index local 是 i32)。回退, 新 TD-PTR-INDEX-GEP-TYPE
4. **Stage 143 (本阶段)**: 真正的根因是 `emit_gep_index_ptr` 硬编码 i64 而非动态使用 index local 类型。修复方式:
   - 修改 trait 方法签名添加 `idx_ty: &EmitType` 参数 (§1.0 原則 5/6/10)
   - 在 places.rs / statement.rs / rvalue.rs 调用点查询 `mir.local_decls[idx.0]` 获取实际类型
   - TextEmitter 用 `idx_ty` 替代硬编码 i64
5. **次根因**: `unwrap_fat_ptr_for_index` 的 `Ptr(_)` 分支不需要 LOAD — caller 已通过 `base_ty.is_ptr()` 检查决定是否 LOAD。Stage 142 的 LOAD 想法是错的 (double-load)
6. **次次根因**: `compute_place_address` 和 `codegen_place_load_typed` 的 Index/ConstantIndex arms 中, `array_ty` 计算会 strip `Ptr(inner)` 为 `inner`, 导致 `emit_gep_index` 被错误调用 (i8 不是数组)。修复: 仅对 `Ptr(Array)` strip, 其他 `Ptr` 保持不变
7. **次次次根因**: `detect_place_type` 对 Index 投影返回 I32 fallback (对 Ptr 没有处理)。修复: 添加 `Ptr(inner) => *inner.clone()` 和 `OpaquePtr => I8` 分支

### WHO (角色切换)
- ARCH-A: 决定修改 trait 方法签名 (§1.0 原則 5 去除兼容思维) 而非新增方法
- DEV-A: 实现 6 个修复点 (trait signature + 4 call sites + 2 array_ty 不 strip + 1 detect_place_type)
- REV-A: 发现 4 个递归 bug (double-load → 不 strip → detect_place_type → base_ptr 计算)
- QA-A: 35 tests (positive + negative + edge + regression)

### WHEN
单轮 L3 任务 (1 轮收敛)。根因链清晰, 修复方案明确。

### WHERE
- `src/codegen/emitter/memory.rs` — trait 方法签名修改
- `src/codegen/text/memory.rs` — TextEmitter 用 idx_ty
- `src/codegen/llvm/memory.rs` — LLVMSysEmitter 忽略 idx_ty
- `src/codegen/mir_translation/places.rs` — 4 个 call sites + array_ty 不 strip + detect_place_type + base_ptr 检查
- `src/codegen/statement.rs` — 1 个 call site
- `src/codegen/rvalue.rs` — 1 个 call site
- `src/stdlib/prelude.rs` — String + str 方法添加
- `tests/v0/stage143/plan/string_methods_tests.rs` — 35 tests

### HOW (核心修复方案)

#### 修复 1: Trait 方法签名修改
```rust
// Before:
fn emit_gep_index_ptr(&mut self, base_ptr: &EmitValue, elem_ty: &EmitType, index: &EmitValue) -> EmitValue;

// After:
fn emit_gep_index_ptr(&mut self, base_ptr: &EmitValue, elem_ty: &EmitType, idx_ty: &EmitType, index: &EmitValue) -> EmitValue;
```

#### 修复 2: 调用点查询 MIR local_decls 获取 idx_ty
```rust
let idx_ty = mir.local_decls.get(idx.0 as usize)
    .map(|ld| mir_type_to_emit_type_with_layouts_and_mono(&ld.ty, layouts, mono_layouts))
    .unwrap_or(EmitType::I64);
```

#### 修复 3: TextEmitter 用 idx_ty
```rust
let idx_str = emit_type_to_llvm_str(idx_ty);
self.line(&format!(
    "  %v{} = getelementptr inbounds {}, {} {}, {} {}",
    r, elem_str, ptr_str, base_ptr, idx_str, index
));
```

#### 修复 4: array_ty 仅对 Ptr(Array) strip
```rust
match &raw_ty {
    EmitType::Ptr(inner) if matches!(**inner, EmitType::Array(_, _)) => *inner.clone(),
    EmitType::Ptr(_) => raw_ty,  // 保留 Ptr(I8) 等
    ...
}
```

#### 修复 5: unwrap_fat_ptr_for_index Ptr(_) 不 LOAD
```rust
EmitType::Ptr(inner) => (base_ptr.to_string(), Some(*inner.clone())),
```
(Caller 通过 `base_ty.is_ptr()` 检查决定是否 LOAD — 不在 unwrap 内 LOAD)

#### 修复 6: codegen_place_load_typed Index/ConstantIndex arms 添加 base_ty.is_ptr() 检查
```rust
let base_ptr = if base_ty.is_ptr() {
    codegen_place_load_typed(...)  // LOAD raw pointer value
} else if let PlaceKind::Local(id) = &base.kind {
    // existing Local handling
} else if ... {
    // existing Deref / Field handling
};
```

#### 修复 7: detect_place_type Index 添加 Ptr/OpaquePtr 分支
```rust
EmitType::Ptr(inner) => *inner.clone(),
EmitType::OpaquePtr => EmitType::I8,
```

#### 修复 8: emit_load 用 detect_place_type 而非 caller-supplied ty
```rust
// Before: emitter.emit_load(&ty, &elem_ptr)  // ty 可能是 I32 默认值
// After:  let elem_ty = detect_place_type(mir, lv, ...);
//         emitter.emit_load(&elem_ty, &elem_ptr)
```

### HOW MUCH (验收)
- §3.2 全套验收通过:
  - cargo clean ✓
  - cargo build --release ✓ (57s)
  - cargo check ✓ (0 errors, 0 warnings)
  - cargo fmt --check ✓ (clean)
  - cargo clippy --all-targets -- -D warnings ✓ (0 warnings)
  - cargo test --release --lib ✓ (898 tests, 0 failures)
  - cargo test --release --test all_tests ✓ (4959 tests, 0 failures, 12 ignored)
  - Total: 5857 tests, 0 failures, 12 ignored

### 决策点 (§12 最优 > 最小, §1.0 原則 6/9/10)

1. **选修改 trait 方法签名** 不选新增方法 — §1.0 原則 5 (去除兼容思维) + §1.0 原則 10 (唯一可信数据源)
2. **选 caller 查询 MIR local_decls** 不选 emitter 推断 — §1.0 原則 10 (唯一可信数据源: MIR 是 source of truth)
3. **选 caller LOAD raw pointer** 不选 unwrap 内 LOAD — §1.0 原則 11 (确定性边界: caller 知道 base_ptr 是否 loaded, unwrap 不知道)
4. **选 byte-by-byte Landin loop** 不选新增 __landin_memcmp — §1.0 原則 9 (正确 > 妥协: 测试 indexing 基础设施)
5. **选 detect_place_type 而非 caller-supplied ty** — §1.0 原則 6 (通解 > 特解) + §12 (最优 > 最小)
6. **选仅对 Ptr(Array) strip** 不选全部 strip — §1.0 原則 9 (正确 > 妥协: 不破坏 raw pointer 语义)

### 裁剪点
L3 任务全流程执行, 不跳过深度审查。

### 下一步 (MUV)
- Stage 144: TD-TYPECK-ASSOC-TYPE-PROJECTION (解锁 Iterator trait) 或 TD-PARSE-IMPL-TRAIT-RETURN 或 TD-LEX-RAW-STRING
- v0.15 阶段剩余 TD: TD-TYPECK-LIFETIME-ELISION, TD-BORROWCK-NLL, TD-TRAIT-BLANKET-IMPL 等

## 文件清单

### 修改文件 (7)
- `src/codegen/emitter/memory.rs` — trait 方法签名 (+idx_ty 参数 + 文档注释)
- `src/codegen/text/memory.rs` — TextEmitter impl 用 idx_ty 替代 i64
- `src/codegen/llvm/memory.rs` — LLVMSysEmitter impl 接受 _idx_ty (忽略)
- `src/codegen/mir_translation/places.rs` — 4 处 emit_gep_index_ptr call + 2 处 array_ty 不 strip + 1 处 detect_place_type Ptr/OpaquePtr + 2 处 base_ty.is_ptr() 检查 + 2 处 emit_load 用 detect_place_type
- `src/codegen/statement.rs` — 1 处 emit_gep_index_ptr call + idx_ty 查询
- `src/codegen/rvalue.rs` — 1 处 emit_gep_index_ptr call + idx_ty 通过 detect_operand_type
- `src/stdlib/prelude.rs` — String + str 的 starts_with/ends_with/contains 方法

### 新建文件 (1)
- `tests/v0/stage143/plan/string_methods_tests.rs` — 35 tests

### 修改测试文件 (2)
- `tests/all_tests.rs` — 添加 stage143 模块注册
- `tests/v0/stage110/plan/phase36_const_writeback_tests.rs` — 阈值从 <30 改为 <50 (适配 prelude 增长)
