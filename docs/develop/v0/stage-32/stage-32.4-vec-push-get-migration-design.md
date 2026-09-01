# Stage 32.4 — Vec::push/Vec::get Intrinsic → Prelude Impl Migration

> **Author**: PM-A + ARCH-A + DEV-A (Super Z)
> **Date**: 2026-09-01
> **Version**: v0.570.0 (target)
> **Stage**: v0.20 Stage 32.4
> **Predecessor**: v0.569.0 (Stage 32.3 — TD-PRELUDE-MONO-ORDER RESOLVED)
> **Tech-Debt Target**: Complete TD-INTRINSIC-OVERUSE migration (last 2/7 intrinsics)

## §13.1 Design Alignment

Stage 32.3 unblocked `impl<T> Vec<T>` body lowering. This stage completes
the migration of the last 2 TD-INTRINSIC-OVERUSE intrinsics (Vec::push,
Vec::get) to prelude impl bodies.

## §1.2.1 Task Classification

L3 (cross-module: `src/stdlib/prelude.rs` + `src/mir/lower/method_call_lower.rs`
+ `src/mir/lower/vec_intrinsics.rs` + tests). Full L3 process applies.

## 5W2H

### WHAT
Replace `lower_vec_push_intrinsic` (440 LOC) and `lower_vec_get_intrinsic`
(140 LOC) with prelude impl bodies in `src/stdlib/prelude.rs`. The MIR
patterns (field access, GEP, Store, Load, SwitchInt, Assert) will be
expressed via Landin source.

### WHY
Per §12 (最优 > 最小): standard method resolution replaces intrinsic
dispatch — the 通解 (general mechanism) replaces the 特解 (special case).
Per §1.0 原則 6 (通解 > 特解): one method resolution path for all Vec
methods, no per-method intrinsic dispatch.

### WHO
PM-A + ARCH-A + DEV-A.

### WHEN
Stage 32.4, immediately after Stage 32.3 (which unblocked this work).

### WHERE
- `src/stdlib/prelude.rs` — add Vec::push, Vec::get impl bodies
- `src/mir/lower/method_call_lower.rs` — remove Vec::push/get dispatch
- `src/mir/lower/vec_intrinsics.rs` — delete file (after migration)
- `src/mir/lower/mod.rs` — remove `vec_intrinsics` module declaration

### HOW
All needed language features exist (Stage 32.3 verification):
- Pointer arithmetic `ptr + idx` → GEP (Stage 18.236)
- Store through Deref `*ptr = val` (Stage 14.27)
- Load from Deref `x = *ptr` (Stage 15.75)
- `extern "C"` calls (Stage 31.6b, used by String::push_str)
- `if`/`else` → SwitchInt
- `assert!(cond)` → Assert terminator
- BinaryOp (Add, Ge, Eq, Mul, Lt)
- Struct field access (`self.cap`, `self.len`, `self.ptr`)
- `sizeof TYPE` (Stage 31.6e)

### HOW MUCH
- ~200 LOC added (prelude impl bodies)
- ~647 LOC removed (vec_intrinsics.rs)
- ~10 test cases (positive + negative)

## Vec::push Prelude Impl Design

```landin
impl<T> Vec<T> {
    fn new() -> Vec<T> { Vec { ptr: 0 as *mut T, len: 0usize, cap: 0usize } }
    fn len(&self) -> usize { self.len }
    fn push(&mut self, value: T) {
        if self.len >= self.cap {
            let new_cap: usize = if self.cap == 0 { 4usize } else { self.cap + self.cap };
            let new_bytes: usize = new_cap * sizeof T;
            let old_bytes: usize = self.cap * sizeof T;
            let new_ptr: *mut T = __landin_realloc(self.ptr, old_bytes, new_bytes);
            self.ptr = new_ptr;
            self.cap = new_cap;
        }
        let elem_ptr: *mut T = self.ptr + self.len;
        *elem_ptr = value;
        self.len = self.len + 1usize;
    }
    fn get(&self, idx: usize) -> T {
        assert!(idx < self.len);
        let elem_ptr: *mut T = self.ptr + idx;
        *elem_ptr
    }
}
```

### Note on `assert!` macro
Landin doesn't have `assert!` macro syntax yet. Use the existing pattern:
- For Vec::get bounds check, we need to emit `Assert(BoundsCheck)`.
- Option A: Use `if idx >= self.len { panic_bounds_check() } else { ... }`
  — but panic!() macro isn't available yet.
- Option B: Define `assert!` as a macro (new language feature — too big).
- Option C: Skip bounds check in prelude (let codegen handle it separately).
- Option D: Use a builtin `__landin_panic_bounds_check` extern call with
  an `if idx >= self.len` guard.

**Decision**: Use Option D — `extern "C" { fn __landin_panic_bounds_check(idx: usize, len: usize); }`
and `if idx >= self.len { __landin_panic_bounds_check(idx, self.len); }`.

Per §1.0 原則 4 (报错 > 静默): bounds check is explicit in source.
Per §1.0 原則 6 (通解 > 特解): one panic pattern for all bounds checks.

## Vec::get return type challenge

Vec::get returns `T` (not `Option<T>` — Landin MVP omits Option). If idx is
out of bounds, the panic path is `unreachable`. Codegen handles this by
emitting `unreachable` after the panic call.

## Migration Steps

1. Update `src/stdlib/prelude.rs` — add Vec::push, Vec::get bodies.
2. Update `src/mir/lower/method_call_lower.rs` — remove Vec::push/get
   intrinsic dispatch (lines 577-617).
3. Delete `src/mir/lower/vec_intrinsics.rs`.
4. Update `src/mir/lower/mod.rs` — remove `vec_intrinsics` module.
5. Add tests (positive + negative).

## §14.8 Design Writeback (B1-B4)

### B1: Design vs. Implementation Match
- Vec::push prelude impl matches intrinsic MIR pattern (GEP + Store + len++).
- Vec::get prelude impl matches intrinsic MIR pattern (bounds check + GEP + Load).

### B2: New TD Items
- None expected.

### B3: Deviations
- Bounds check uses `if + extern panic call` instead of `assert!` macro
  (Landin doesn't have assert! yet). Documented.

### B4: Architectural Limitations
- None — all features now exist.
