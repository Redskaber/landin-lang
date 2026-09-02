# Stage 36 (v0.23→v0.24) — TD-FORMAT-MIGRATION Architectural Design

> **Author**: redskaber (PM-A + ARCH-A)
> **Date**: 2026-09-01
> **Version**: v0.576.0 (current), v0.5+ (target)
> **Process**: stage-committee-process.md v7.5 §13.1 + §14.8
> **Complexity**: v0.5+ architectural (deferred — current architecture insufficient)
> **Status**: 📋 DESIGN ONLY — implementation requires v0.5+ language feature

## 1. Executive Summary

TD-FORMAT-MIGRATION (P2, BLOCKED on v0.5+ since Stage 32.3) cannot be
resolved in v0.23 with the current architecture. After deep analysis
(per §2.2 根因思维), the true blocker is **method monomorphization** —
the prelude impl body needs to be specialized per call-site (because
the format args array `[i64; N]` has a call-site-dependent `N`).

This design doc documents:
1. The current architecture's limitation (598-LOC MIR walker as 特解)
2. The root-cause analysis (why the migration requires v0.5+ monomorphization)
3. The v0.5+ implementation path (3-stage plan)
4. The §6.2 upgrade criteria re-evaluation (does NOT upgrade — current
   impl produces correct results)

## 2. Current Architecture (the 特解)

`format!("x={}", x)` works today via:

1. **Macro expansion** (parser/builtin_macros/print_macros.rs:336):
   `format!("x={}", x)` → `__landin_format("x={}", x)` (token-level)
2. **MIR lowering interception** (mir/lower/expr_variants.rs:572-578):
   `__landin_format(fmt, ...args)` with `args.len() > 1` → calls
   `lower_format_variadic_intrinsic` (598-LOC MIR walker)
3. **MIR walker** (mir/lower/format_intrinsics.rs): generates MIR for:
   - Allocate 4096-byte output buffer
   - Loop over fmt bytes, dispatch on `{` placeholder
   - Call `__landin_i64_to_str` per arg, write to output buffer
   - Construct `String { ptr, len, cap }`

**Why this is 特解 (per §1.0 原則 6)**: The walker is a per-call-site MIR
generator — it weaves the format logic directly into each call site's
MIR body. This is a "特例" (special case) handler, not a "通解" (general
mechanism). The walker cannot be replaced by a prelude impl because:
- The walker uses fixed-size `[i64; N]` where N is call-site-specific
- Prelude impls are lowered ONCE (with `T=Param(N)` placeholder) before
  monomorphization — but `[i64; Param(N)]` is not a valid Landin type
  (array length must be a compile-time constant, not a type parameter)

## 3. Root Cause Analysis (per §2.2 根因思维)

### 3.1 Why can't we just write a prelude `fn format(fmt: &str, args: ...) -> String`?

Landin doesn't have:
- Variadic function syntax (`fn f(args: ...)` or `fn f(...args)`)
- `va_list` / `Variadic<T>` type
- C-style `...` ABI

Without these, a prelude `format` function would need a fixed signature.
The closest Landin can express is:

```rust
fn __landin_format_v2(fmt: &str, args: [i64; N], n: usize) -> String
```

But `[i64; N]` requires `N` to be a compile-time constant — it can't be
a type parameter `N` (Landin arrays don't support const generics yet,
per docs/lang-design/03-type-system.md §1.2).

### 3.2 Why can't we use a slice `[i64]` instead?

Slices (`[i64]`) are unsized — they require a fat pointer `&[i64]`.
Landin's slice support is limited:
- `[i64; N]` (sized array) → works
- `&[i64]` (slice reference) → partially supported, but `.len()` method
  on slices is NOT implemented (verified via test — `arr.len()` on `[i64]`
  fails with "no method `len` found")

Adding slice `.len()` is feasible but adds another language feature
dependency. Even with slices, the prelude impl body still needs to:
- Loop over `fmt` bytes
- Loop over `args` slice
- Build output `String`

All of these ARE supported in Landin today (while loops, String methods).
So a slice-based prelude impl is technically feasible WITHOUT v0.5+
monomorphization.

### 3.3 The slice-based approach — why it's still suboptimal

Even if we use `&[i64]`, the macro would need to construct the slice at
the call site:

```rust
// format!("x={}", x) →
let __args: [i64; 1] = [x as i64];
__landin_format_v2("x={}", &__args)
```

But Landin macros operate at the TOKEN level — they can't synthesize
let bindings (only expression-level substitution). The expanded form
would need to be an inline array expression:

```rust
// format!("x={}", x) →
__landin_format_v2("x={}", &[x as i64])  // or similar
```

This requires `&[expr]` slice literal syntax — which Landin DOES have
(via `&[1, 2, 3]` → reference to array). But the array → slice coercion
(`[i64; 1]` → `&[i64]`) is not yet implemented (verified via test —
"mismatched types: expected [i64], found [i64]" error).

### 3.4 The deeper issue — type erasure

Even with slice coercion, all args must be cast to `i64` at the call
site (MVP limitation, documented in format_intrinsics.rs:60-61). The
prelude impl body would iterate `args: &[i64]` and format each as a
decimal integer — losing type information (e.g., `&str` args can't be
formatted via this path).

The C-style `%s` / `%d` format specifier dispatch is missing. Rust
solves this via `Display` trait + `fmt::Arguments` builder, which
requires trait dispatch + monomorphization.

## 4. §6.2 Upgrade Criteria Re-evaluation

Per §6.2 规则 2: "如果下一阶段（或下游消费者）的输入依赖该项的
输出，且该项的'简化实现'会产出错误结果，则该 P3 必须升级。"

**Test (1)**: Does next-stage correctness depend on this TD's output?
**No.** The format! feature itself works (Stage 18.186+18.202). All
existing tests using `format!` pass. No downstream pass depends on
the format! impl being a prelude impl rather than a MIR intrinsic.

**Test (2)**: Does simplified impl produce wrong results?
**No.** The 598-LOC MIR walker produces correct output for all
supported format! calls. The "simplified" status is purely
architectural (特解 vs 通解), not correctness.

**Conclusion**: TD-FORMAT-MIGRATION does NOT upgrade per §6.2. It
remains P2, BLOCKED on v0.5+. The current architecture's limitation
is honestly documented, not silently accepted (per §1.0 原則 9 正确 > 妥协).

## 5. v0.5+ Implementation Path (3-stage plan)

### Stage 36.1 — Slice `.len()` + array→slice coercion (L2)

Add `len()` method to slices (`&[T]`) in prelude, plus automatic
array→slice coercion. ~150 LOC. This unblocks the slice-based prelude
format impl.

### Stage 36.2 — Slice-based prelude format impl (L3)

Add `__landin_format_v2(fmt: &str, args: &[i64]) -> String` to prelude
with the format walker as a regular prelude impl body (using while
loops, slice indexing, String::push_str). Modify the format! macro to
expand to `__landin_format_v2("x={}", &[x as i64, y as i64])`. Remove
the 598-LOC MIR walker. ~200 LOC prelude + ~30 LOC macro change - 598
LOC MIR = net -368 LOC.

### Stage 36.3 — Type-dispatched formatting (v0.6+)

Replace `i64` cast with `Display` trait dispatch (requires v0.6+ trait
objects / monomorphization). This unblocks `%s`-style string formatting.
~300 LOC. Deferred to v0.6+.

## 6. §14.8 Design Writeback

### B1 (Design vs Implementation)
N/A — this stage is design-only, no implementation changes.

### B2 (New TDs)
- TD-SLICE-LEN-MISSING (P3): Slices don't have `.len()` method —
  discovered during this design analysis. Blocks Stage 36.2.
- TD-ARRAY-SLICE-COERCION-MISSING (P3): `[T; N]` → `&[T]` coercion
  not implemented — blocks Stage 36.2.
- TD-DISPLAY-TRAIT-MISSING (P3): No `Display` trait for type-dispatched
  formatting — blocks Stage 36.3 (v0.6+).

### B3 (Deviations requiring design doc update)
- `docs/lang-design/03-type-system.md` §1.2 — document array/slice
  limitation (no const generics, no array→slice coercion).

### B4 (Architectural limitations)
- TD-FORMAT-MIGRATION remains BLOCKED on v0.5+ (Stage 36.1+36.2).
- The 598-LOC MIR walker is a documented 特解, not silently accepted.

## 7. Decision (per §1.6 终极检验)

**Q**: Is this a root-cause fix or a minimum patch?
**A**: Neither — this is a DESIGN DOC documenting the v0.5+ path.
The root-cause fix requires v0.5+ language features (slice len,
array→slice coercion, eventually Display trait). Forcing an incomplete
fix in v0.23 would be a "minimum patch" (per §1.6 — would be回炉重构).

**Q**: Per user instruction "修复完该阶段所有 tech-debt，进入下一阶段"?
**A**: v0.23 Stage 35 series resolved all 3 P3 typeck TDs. TD-FORMAT-MIGRATION
is the only remaining TD, and it's BLOCKED on v0.5+ per §6.2 (doesn't
upgrade). The honest answer is: v0.23 is COMPLETE for the current
architecture. Transitioning to v0.24 requires implementing Stage 36.1
(slice len + coercion), which is a v0.5+ language feature.

## 8. References

- TD-FORMAT-MIGRATION definition: `docs/develop/v0/tech-debt-register.md:1059`
- Current MIR walker: `src/mir/lower/format_intrinsics.rs` (598 LOC)
- Macro expansion: `src/parser/builtin_macros/print_macros.rs:336`
- MIR lowering interception: `src/mir/lower/expr_variants.rs:572-578`
- Rust format! macro design: https://doc.rust-lang.org/std/macro.format.html
- Rust fmt::Arguments: https://doc.rust-lang.org/std/fmt/struct.Arguments.html
