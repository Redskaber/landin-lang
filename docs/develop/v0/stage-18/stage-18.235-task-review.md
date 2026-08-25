# Stage 18.235 — Architectural Audit: Intrinsic Pattern as 特解 (Special Case)

> **Date**: 2026-08-23
> **Version**: v0.482.0 (no bump — audit + documentation)
> **Task ID**: stage18.235
> **Reviewer**: Super Z (main) — Stage Committee (ARCH-A + PM-A + REV-A + ALG-C + SKL-A)
> **流程文档**: docs/stage-committee-process.md v6.4 §1.0 原則 6 + §17.7 (缺陷纳入)
> **设计文档**: docs/lang-design/06-mir.md + 09-stdlib.md

## 1. 触发场景

Per user directive: "结合原则思考当前项目对所谓 intrinsic 等内置处理是否也是
属于特解". This is a design audit triggered by the user's observation that the
"intrinsic" handling pattern may violate §1.0 原則 6 (通解 > 特解).

Per §17.7 (缺陷纳入): if a design defect is identified during any process
(audit/review/refactor), it must be recorded with a full fix plan.

## 2. Problem Identification: Intrinsic = 特解

### 2.1 The Intrinsic Pattern (Current Architecture)

The Landin compiler handles stdlib methods (String::len, Vec::push, String::push_str,
etc.) via a **hardcoded interception** pattern in MIR lower:

```rust
// src/mir/lower/expr_variants.rs — 8 hardcoded checks:
if method_name_str == "len" && args.is_empty() { ... }       // String::len
if method_name_str == "is_empty" && args.is_empty() { ... }  // String::is_empty
if method_name_str == "as_bytes" && args.is_empty() { ... }  // String::as_bytes
if method_name_str == "as_str" && args.is_empty() { ... }   // String::as_str
if method_name_str == "len" && args.is_empty() { ... }       // Vec::len
if method_name_str == "push_str" && args.len() == 1 { ... }  // String::push_str
if method_name_str == "get" && args.len() == 1 { ... }        // Vec::get
if method_name_str == "push" && args.len() == 1 { ... }      // Vec::push
// + static method checks: String::from_str, Box::new, Vec::new
```

Each check also validates the receiver type via hardcoded name comparison:
```rust
let is_string = ... hir.find_owner(*did) ... name == "String";
let is_vec = ... hir.find_owner(*did) ... name == "Vec";
```

Additionally, typeck has a **hardcoded whitelist** (Stage 18.234):
```rust
const KNOWN_INTRINSIC_METHODS: &[&str] = &[
    "len", "is_empty", "as_bytes", "as_str", "push_str", "get",
    "push", "new", "from_str", "cap", "ptr",
];
```

### 2.2 Why This Is a 特解 (Violates §1.0 原則 6)

| Criterion | 特解 (Current) | 通解 (Target) |
|-----------|---------------|---------------|
| Adding a new stdlib type | Add if-branches + type name checks + whitelist entries | Add `impl` block in prelude source |
| Adding a new method | Add if-branch + specialized lower function + whitelist entry | Add method to `impl` block in prelude source |
| Method resolution | Hardcoded name string comparison | Standard `resolve_inherent_method` / `resolve_trait_method` |
| Type dispatch | Hardcoded `name == "String"` / `name == "Vec"` | Standard Adt DefId lookup |
| typeck validation | Whitelist of known method names | Standard method resolution (no whitelist needed) |
| Maintenance | 3+ files must be kept in sync | 1 file (prelude source) |

**Specific violations**:
1. **8 hardcoded `method_name_str == "X"` checks** — each is a 特解 for a specific method
2. **7 specialized lowering functions** — each generates MIR differently
3. **11-entry `KNOWN_INTRINSIC_METHODS` whitelist** — typeck special-cases intrinsic method names
4. **8 hardcoded `name == "String"` / `name == "Vec"` checks** — type dispatch via string comparison
5. **No DRY** — adding `HashMap::insert` would require modifying 4+ locations
6. **Scattered logic** — intrinsic handling is spread across MIR lower, typeck, driver_validations, function_sigs

### 2.3 Root Cause

The prelude already defines `struct String`, `struct Vec<T>`, `struct Box<T>` and
some `impl` blocks (e.g., `impl String { fn len(&self) -> i64 { self.len } }`).
But MIR lower **intercepts** method calls BEFORE standard method resolution can
handle them, generating specialized MIR code instead of letting the `impl` block
run through the normal pipeline.

This was originally a **MVP compromise** (Stage 18.185+): the prelude `impl`
blocks couldn't express low-level operations (alloc, memcpy, GEP, Load, Store),
so specialized lowering functions were introduced. But this compromise was never
recorded as a design debt, and it has accumulated into 8+ hardcoded checks.

### 2.4 Related Issues Caused by the Intrinsic Pattern

| Issue | Root Cause | TD |
|-------|-----------|-----|
| TD-TUPLE-CTOR-TYPECK (expected type propagation) | Intrinsics create temp locals, losing expected type context | Stage 18.233 |
| TD-METHOD-RESOLVE-STRICT (Infer receiver) | Intrinsics can't resolve when recv is Infer; whitelist needed | Stage 18.234 |
| KNOWN_INTRINSIC_METHODS whitelist | Needed because typeck can't distinguish intrinsic vs user methods | Stage 18.234 |

All three are symptoms of the same root cause: **the intrinsic pattern bypasses
the standard method resolution pipeline**.

## 3. The 通解 (General Solution): Stdlib Impl Migration

### 3.1 Design

Replace all hardcoded intrinsic handling with regular `impl` blocks in the
prelude source. Low-level operations are expressed as `extern "C"` function
calls within the method bodies:

```landin
// Prelude source (通解):
extern "C" { fn __landin_alloc(size: i64) -> *mut u8; }
extern "C" { fn __landin_realloc(ptr: *mut u8, old: i64, new: i64) -> *mut u8; }
extern "C" { fn __landin_memcpy(dst: *mut u8, src: *mut u8, n: i64); }
extern "C" { fn __landin_i64_to_str(buf: *mut u8, cap: i64, val: i64) -> i64; }

struct String { ptr: *mut u8, len: i64, cap: i64 }

impl String {
    fn new() -> String {
        String { ptr: 0 as *mut u8, len: 0, cap: 0 }
    }
    fn from_str(s: &str) -> String {
        let len = s.len;
        let ptr = __landin_alloc(len);
        __landin_memcpy(ptr, s.ptr, len);
        String { ptr, len, cap: len + 1 }
    }
    fn len(&self) -> i64 { self.len }
    fn push_str(&mut self, src: &str) {
        // Growth logic + memcpy — expressed in Landin, not MIR lower
        let new_len = self.len + src.len;
        if new_len > self.cap {
            let mut new_cap = if self.cap == 0 { 4 } else { self.cap };
            while new_cap < new_len { new_cap = new_cap + new_cap; }
            self.ptr = __landin_realloc(self.ptr, self.cap, new_cap);
            self.cap = new_cap;
        }
        let dest = self.ptr + self.len;  // GEP via pointer arithmetic
        __landin_memcpy(dest, src.ptr, src.len);
        self.len = new_len;
    }
}
```

### 3.2 Migration Plan

| Phase | Task | Target | LOC Impact |
|-------|------|--------|------------|
| 1 | Add `extern "C"` declarations to prelude | v0.3 Phase 1 | +20 prelude |
| 2 | Move `String::from_str` from MIR lower to prelude impl | v0.3 Phase 2 | -100 MIR lower, +20 prelude |
| 3 | Move `String::push_str` from MIR lower to prelude impl | v0.3 Phase 2 | -370 MIR lower, +30 prelude |
| 4 | Move `Vec::push` from MIR lower to prelude impl | v0.3 Phase 2 | -370 MIR lower, +30 prelude |
| 5 | Move `Vec::get` from MIR lower to prelude impl | v0.3 Phase 2 | -150 MIR lower, +20 prelude |
| 6 | Move `Box::new` from MIR lower to prelude impl | v0.3 Phase 2 | -150 MIR lower, +10 prelude |
| 7 | Move `format!` from MIR lower to prelude impl | v0.3 Phase 3 | -400 MIR lower, +50 prelude |
| 8 | Remove all `method_name_str == "X"` checks from MIR lower | v0.3 Phase 3 | -200 MIR lower |
| 9 | Remove `KNOWN_INTRINSIC_METHODS` whitelist from typeck | v0.3 Phase 3 | -30 typeck |
| 10 | Remove `deferred_method_calls` side-table (no longer needed) | v0.3 Phase 3 | -50 typeck/body |

**Total**: ~1500 LOC removed from MIR lower + typeck, ~200 LOC added to prelude.
Net reduction: ~1300 LOC of 特解 code.

### 3.3 Prerequisites (per §17.8 依赖审查)

| Prerequisite | Status | Notes |
|-------------|--------|-------|
| Pointer arithmetic in Landin source | ❌ Missing | Need `ptr + offset` syntax or `*mut T` indexing |
| `extern "C"` declarations in prelude | ✅ Exists | Already used for `__landin_alloc`, etc. |
| While loop in Landin source | ✅ Exists | Used in prelude `impl` blocks |
| Mutable `&mut self` in prelude methods | ✅ Exists | Already used |
| Field assignment (`self.ptr = ...`) | ✅ Exists | Already used |
| `if`/`else` in Landin source | ✅ Exists | Already used |

**Blocking dependency**: Pointer arithmetic (`ptr + offset`). Currently, the
MIR lower uses `GetElementPtr` for pointer offset. In the 通解, this would
need to be expressible in Landin source (e.g., `ptr + offset` or `ptr[offset]`).
This is a **language feature** that needs to be added before the migration.

**Per user directive "依赖与基础设施完整能力审查"**: The pointer arithmetic
dependency is a blocker. Without it, the stdlib impl migration cannot proceed.
This is recorded as a prerequisite for v0.3 Phase 2.

## 4. Decision: RECORD as TD-INTRINSIC-OVERUSE, DEFER to v0.3

Per §17.7 (缺陷纳入): This design defect must be recorded with a full fix plan.

Per §17.8 (任务审查): The fix requires a language feature (pointer arithmetic)
that is not yet implemented. This is a blocking dependency. The task is
deferred to v0.3.

### 4.1 New Tech Debt: TD-INTRINSIC-OVERUSE

| Field | Value |
|-------|-------|
| ID | TD-INTRINSIC-OVERUSE |
| Description | Stdlib methods (String::len, Vec::push, etc.) implemented as hardcoded MIR lower intrinsics instead of regular `impl` blocks in prelude source |
| Impact | Adding new stdlib types/methods requires modifying 4+ files; scattered logic; violates §1.0 原則 6 (通解 > 特解); caused TD-TUPLE-CTOR-TYPECK + TD-METHOD-RESOLVE-STRICT whitelist |
| Fix Plan | v0.3: Migrate all intrinsics to prelude `impl` blocks; requires pointer arithmetic language feature first |
| Priority | P2 (important, not blocking v0.2) |
| Blocked by | Pointer arithmetic language feature (not yet implemented) |

### 4.2 Relationship to Existing TDs

| TD | Relationship | Status |
|----|-------------|--------|
| TD-C-WRAPPER-OVERUSE | Same pattern class (特解 bypassing standard pipeline) | ✅ Resolved (18.232) — C helpers migrated to MIR intrinsics |
| TD-INTRINSIC-OVERUSE | Same pattern class — intrinsics bypassing standard method resolution | 🟡 NEW — deferred to v0.3 |
| TD-TUPLE-CTOR-TYPECK | Caused by intrinsic pattern (temp locals lose expected type) | 🟡 Deferred to v0.3 |
| TD-METHOD-RESOLVE-STRICT | Partially caused by intrinsic pattern (whitelist needed) | ✅ Partially resolved (18.234) — whitelist is a temporary workaround |

**Pattern**: TD-C-WRAPPER-OVERUSE (C helpers → MIR intrinsics) was Phase 1 of
removing 特解. TD-INTRINSIC-OVERUSE (MIR intrinsics → stdlib impl blocks) is
Phase 2. Both follow the same principle: replace 特解 with 通解.

## 5. Recommendation

**Record TD-INTRINSIC-OVERUSE** as a new tech debt item.
**Defer to v0.3** — requires pointer arithmetic language feature.
**Document in design docs** (06-mir.md + 09-stdlib.md).
**No code changes** — this is an audit stage.

## 6. Conclusion

The user's observation is correct: the "intrinsic" handling IS a 特解 that
violates §1.0 原則 6 (通解 > 特解). It is the same pattern class as
TD-C-WRAPPER-OVERUSE (which was already resolved). The 通解 is to migrate
all intrinsics to regular `impl` blocks in the prelude source, using
`extern "C"` declarations for low-level operations.

This is recorded as TD-INTRINSIC-OVERUSE, deferred to v0.3 (blocked by
pointer arithmetic language feature). The KNOWN_INTRINSIC_METHODS whitelist
in typeck (Stage 18.234) is a temporary workaround that will be removed
when the 通解 is implemented.
