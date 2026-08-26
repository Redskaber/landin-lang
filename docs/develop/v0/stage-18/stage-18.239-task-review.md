# Stage 18.239 — Task Review: TD-INTRINSIC-OVERUSE Phase 2 — String::as_str Migration

> **Date**: 2026-08-23
> **Version**: v0.485.0 → v0.486.0 (planned)
> **Task ID**: stage18.239
> **Reviewer**: Super Z (main) — ARCH-A + PM-A + REV-A + DEV-A + QA-A
> **流程文档**: docs/stage-committee-process.md v6.4 §13.1 + §17.6 + §17.8

## 1. 触发场景

Per Stage 18.238: Phase 1 removed Vec::len/new hardcodes. Per user directive
"同类型错误或者存在依赖关系（情况）的应该考虑整体性完整修复", proceed with Phase 2.

## 2. 依赖与基础设施完整能力审查

### 2.1 str::len/is_empty/as_bytes — BLOCKED

`str` is a built-in primitive type (`TyKind::Str`), NOT an Adt struct.
Built-in types **cannot have `impl` blocks** in Landin source — there's
no way to write `impl str { fn len(&self) -> i64 { ... } }` because `str`
is not a user-defined type.

**Per §17.8 (任务审查)**: str intrinsics CANNOT be migrated to prelude impl
because str is a primitive type. These hardcoded checks must remain until
the language gains the ability to impl methods on primitive types (v0.4+
feature, same as Rust's `impl str`).

**Decision**: Keep str::len/is_empty/as_bytes hardcoded. Record as MVP
(§17.6) — blocked by primitive type impl support.

### 2.2 String::as_str — CAN MIGRATE

`String` IS an Adt struct defined in prelude. We can add `as_str()` to
the existing `impl String { ... }` block. The method needs to construct
a `&str` fat pointer from String's fields (ptr, len).

**Challenge**: The current hardcoded version uses:
1. Field projection to extract ptr (field 0) and len (field 1)
2. Aggregate(Tuple, [ptr, len]) to build the fat pointer
3. Cast(Unsize, tuple, &str) to convert to &str type

In prelude source, this would be:
```landin
impl String {
    fn as_str(&self) -> &str {
        // Need to construct { ptr, len } fat pointer from self.ptr and self.len
        // This requires fat pointer construction syntax in Landin source
    }
}
```

**BLOCKER**: Landin doesn't have fat pointer construction syntax in user
source. The `&str` type is a fat pointer `{ ptr, i64 }`, but there's no
way to construct one from two separate values in Landin source code.
The hardcoded version uses MIR's `Aggregate(Tuple, [ptr, len])` + `Cast(Unsize)`
which is not expressible in Landin source.

**Per §17.8 (任务审查)**: String::as_str migration is BLOCKED by fat pointer
construction syntax. This is a language feature gap.

### 2.3 Remaining Hardcodes Analysis

| Method | Type | Can Migrate? | Blocker |
|--------|------|-------------|---------|
| str::len | primitive str | ❌ | No impl on primitive types |
| str::is_empty | primitive str | ❌ | No impl on primitive types |
| str::as_bytes | primitive str | ❌ | No impl on primitive types |
| String::as_str | Adt String | ❌ | No fat ptr construction syntax |
| String::from_str | Adt String | ❌ | Needs extern C in prelude (alloc/memcpy) |
| String::push_str | Adt String | ❌ | Needs extern C + growth logic |
| Vec::push | Adt Vec | ❌ | Needs extern C + growth logic |
| Vec::get | Adt Vec | ❌ | Needs bounds check + GEP |
| Box::new | Adt Box | ❌ | Needs extern C (alloc) |
| format! | macro | ❌ | Needs i64_to_str + format walker |

### 2.4 Decision: DEFER Phase 2 to v0.3

**Per user directive "如果当前设计和实现存在简写或缺陷或MVP（时机： 此条例触发时机）,
则需要将简写、缺陷的原因及描述等必要信息记录在开发、设计文档中并规划修订完整计划"**:

All remaining intrinsic migrations are blocked by language feature gaps:
1. **Primitive type impl** (impl str) — needs language support (v0.4+)
2. **Fat pointer construction** in source — needs language support (v0.3+)
3. **extern "C" in prelude impl** — already works, but the methods that
   use it (from_str, push_str, push, get, Box::new, format!) need fat ptr
   construction or complex MIR patterns not expressible in source

**Conclusion**: Phase 2 is deferred to v0.3 (when fat pointer construction
and primitive type impls are available). Phase 1 (Vec::len/new) was the
only migration possible without language feature changes.

## 3. Documentation Updates

Update tech-debt-register with the detailed blocking analysis.

## 4. Recommendation

**RECORD Phase 2 blockers and DEFER to v0.3**. No code changes needed —
Phase 1 (Stage 18.238) was the complete set of intrinsics that could be
migrated without language feature changes.

Per §17.7 (缺陷纳入): record the blocking analysis with full plan.
