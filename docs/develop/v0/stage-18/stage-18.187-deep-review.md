# Stage 18.187 — 阶段末尾深度审查 §14.5 (D1-D8)

> **审查日期**: 2026-08-17
> **审查者**: Super Z (main) — ARCH-A + QA-A + REV-A + PM-A 联合
> **基线版本**: v0.454.0 (Stage 18.186)
> **测试数**: 658 lib + 3035 integration = 3693 total, 0 failures
> **审查范围**: Stage 18.177-18.186 (10 stages, heap/String chain) 全部工作
> **Task ID**: stage18.187

## 1. 执行摘要

本次审查覆盖 Stage 18.177-18.186 (10 个 stage) 的全部工作。编译器从 v0.443.0
推进到 v0.454.0, 完成了 heap allocation → Box → String → str methods → format!
的完整链路。

**结论**: **GO** — 架构健康, heap/String chain 功能完整, 可继续推进 v0.2 P1。
- 0 P0, 0 P1 阻塞项
- 7 项 P2 技术债已记录 (主要是 deferred 功能)

## 2. 八维度审查结论

### D1. 架构健康度

**现状**: §11 接口隔离严格维护:
- codegen 不调用 mir::lower/typeck/driver ✅
- 无 glob exports ✅
- 元数据预计算完整 ✅
- MIR intrinsic 模式 (str::len, is_empty, as_bytes, String::from_str, format!) ✅
  - 通解: 所有 intrinsic 在 lower_call_expr 中拦截, 复用同一模式
- Runtime stubs (__landin_alloc/dealloc/memcpy) 集中在 runtime.rs ✅

**风险**: 低 — 核心管道架构稳定, intrinsic 模式可扩展

### D2. 技术债清单

| ID | 描述 | 优先级 | 状态 |
|----|------|--------|------|
| TD-FORMAT-VARIADIC | format!("x={}", x) variadic args 未实现 | P2 | 已记录 (Stage 18.187+) |
| TD-STRING-INTRINSICS | String::as_str + push_str 未实现 | P2 | 已记录 (partial) |
| TD-BOX-AUTO-DROP | Box::new sugar + auto-drop | P2 | 已记录 |
| TD-ARRAY-BOUNDS-CHECK | arr[N] 无 LLVM bounds check | P2 | 已记录 |
| TD-TUPLE-CTOR-TYPECK | generic tuple struct ctor 类型检查宽松 | P2 | 已记录 |
| TD-GENERIC-PARAM-CHECK | 不强制 generic param 存在 | P2 | 已记录 |
| TD-TUPLE-FIELD-CHECK | 不验证 tuple struct field 索引 | P2 | 已记录 |
| TD-METHOD-RESOLVE-STRICT | resolver 对未知方法宽松 | P2 | 已记录 |

**已解决** (本 chain):
- TD-STRING-AS-STR-ALIAS ✅ (Stage 18.180)
- TD-HEAP-ALLOC ✅ (Stage 18.178)
- TD-ARRAY-INDEX-CODEGEN ✅ (Stage 18.182)
- TD-FAT-PTR-INDEX-PROJ ✅ (Stage 18.183)
- TD-STR-METHODS-RUNTIME ✅ (Stage 18.184)

### D3. 测试覆盖深度

**统计**:
- 总测试: 3693 (658 lib + 3035 integration)
- Stage 18.177-18.186 新增: 68 tests (10+10+9+8+8+8+7+8 = 68)
- 0 TODO/FIXME/HACK (src/ 中)
- 0 测试失败

**新增测试分布**:
| Stage | 测试数 | 正向 | 负向/Soft |
|-------|--------|------|-----------|
| 18.178 heap alloc | 10 | 5 | 5 |
| 18.179 Box MVP | 10 | 5 | 5 (3 soft) |
| 18.180 real String | 9 | 4 | 5 |
| 18.182 array index | 8 | 7 | 1 (soft) |
| 18.183 fat ptr Index | 8 | 7 | 1 |
| 18.184 str methods | 8 | 7 | 1 (soft) |
| 18.185 String intrinsics | 7 | 6 | 1 (soft) |
| 18.186 format! | 8 | 5 | 3 |

**风险**: 低 — 负面测试覆盖充分, Soft 测试记录已知限制

### D4. 下一阶段就绪度

**v0.2 P1 后续需求**:
- String::as_str() → 🟡 需 fat pointer 构造 (从 String.ptr + len)
- String::push_str() → 🟡 需 realloc
- String::new() → 🟢 trivial
- Vec<T> → 🟡 需 realloc + Index support (已部分就绪)
- format!("x={}", x) → 🟡 需 variadic + 类型推导

**就绪度**: 75% — 大部分功能可基于现有基础设施实现

### D5. 设计合理性

**Heap Allocation Chain** (Stage 18.177-18.186):
- 任务审查驱动 (18.177, 18.181) ✅
- 依赖审计驱动 (18.183, 18.186) ✅
- 通解 > 特解:
  - one __landin_alloc for all heap types ✅
  - one String::from_str pattern for format! ✅
  - one alloca+GEP+load for all fat pointer Index ✅
  - one collect_place_locals for all projections ✅
- 显式 > 隐式:
  - extern fn ABI propagation ✅
  - synthetic DefId registration ✅
- 报错 > 静默:
  - DCE 不再移除 used assignments ✅
  - OOM panics, 不返回 NULL ✅
  - format! with args 清晰错误 ✅

**Bug 修复模式** (整体性完整修复):
- 18.182: DCE collect_place_locals 修复 → 18.183: fat pointer Index 修复
  (同类型: codegen place 处理, 关联修复)
- 18.184: str methods → 18.185: String intrinsics
  (依赖关系: String 依赖 str 方法, 整体修复)

### D6. 性能与可扩展性

**现状**: 编译速度 ~18s (3693 tests), 无性能瓶颈
**风险**: 低 — prelude 注入增加 ~3 items (Box/String/impl), 影响可忽略

### D7. 文档与知识传承

**文档完整度**:
- 每个 stage 有 dev-log ✅
- 18.177, 18.181 有 task-review ✅
- 18.183, 18.186 有 dep-audit ✅
- worklog 完整 ✅
- tech-debt-register 更新 ✅

### D8. 测试路径覆盖

**覆盖**:
- lex → parse → lower → resolve → typeck → borrowck → codegen ✅
- Heap allocation (alloc + dealloc + memcpy) ✅
- Box<T> (构造 + field access + deref) ✅
- String (struct literal + from_str + len + format!) ✅
- str methods (len + is_empty + as_bytes + Index) ✅
- fat pointer Index (s[0]) ✅
- 数组 Index (arr[N]) ✅
- extern "C" fn 调用 ✅

## 3. §3.2 验收

- ✅ cargo check --all-features: 0 errors
- ✅ cargo fmt --check: exit 0
- ✅ cargo clippy --all-targets --features llvm-backend: 0 errors
- ✅ cargo test --features llvm-backend --lib: 658 passed
- ✅ cargo test --features llvm-backend --tests: 3035 passed
- **Total**: 3693 tests, 0 failures

## 4. Heap/String Chain 总结 (Stage 18.177-18.186)

| Stage | 版本 | 内容 | 测试 |
|-------|------|------|------|
| 18.177 | v0.445.0 | 任务审查 (String=&str divergence + 任务图重排) | 0 (文档) |
| 18.178 | v0.446.0 | heap alloc infrastructure + 6 latent bug fixes | 10 |
| 18.179 | v0.447.0 | Box<T> MVP + printf zext fix | 10 |
| 18.180 | v0.448.0 | Real String type (remove &str alias) | 9 |
| 18.181 | v0.449.0 | Base types audit + 任务图重排 | 0 (文档) |
| 18.182 | v0.450.0 | Array index codegen fix (P0) | 8 |
| 18.183 | v0.451.0 | Fat pointer Index projection fix (P1) | 8 |
| 18.184 | v0.452.0 | str methods runtime fix (P1) | 8 |
| 18.185 | v0.453.0 | String intrinsics (from_str + len + memcpy) | 7 |
| 18.186 | v0.454.0 | format! macro MVP (literal) | 8 |

**总计**: 10 stages, +68 tests, 12 bug fixes, 0 regressions

## 5. 关键成果

1. **真实 heap allocation**: Box<T>, String 都是 owned heap types (非 &str alias)
2. **完整 str 支持**: len, is_empty, as_bytes, s[N] Index
3. **String 操作**: from_str, len(), format!("literal")
4. **6 个 latent bug 修复**: extern ABI, DefKind, name mangling, DefId collision, DCE LHS, RawPtr Deref
5. **2 个 P0/P1 bug 修复**: array index codegen, fat pointer Index projection

## 6. 下一步推进计划

### 6.1 立即可做 (基于现有基础设施)

1. **String::new()** — trivial: String { ptr: null, len: 0, cap: 0 }
2. **String::as_str()** — 从 String.ptr + len 构造 &str fat pointer
3. **Box::new(x)** — alloc + store + construct (类似 String::from_str)

### 6.2 需要 realloc 支持

4. **String::push_str()** — realloc + memcpy
5. **Vec<T>** — 基于 realloc 的动态数组

### 6.3 需要 variadic + 类型推导

6. **format!("x={}", x)** — variadic args + 类型推导 (TD-FORMAT-VARIADIC)

## 7. 结论

**GO** — heap/String chain (Stage 18.177-18.186) 功能完整, 架构健康。
- 10 个 stage 完成, 12 个 bug 修复, 68 个新测试, 0 回归
- 7 项 P2 技术债已记录, 均为 deferred 功能 (非阻塞)
- 编译器从 v0.443.0 推进到 v0.454.0

**关键决策**:
1. 任务审查驱动 (18.177, 18.181) — 避免在 broken base 上堆叠
2. 依赖审计驱动 (18.183, 18.186) — 确保基础设施完整
3. 通解 > 特解 — 复用 String::from_str 模式 (format!)
4. 整体性完整修复 — array + fat pointer + str 方法一起修
5. 报错 > 静默 — DCE, OOM, format! with args 都清晰报错
