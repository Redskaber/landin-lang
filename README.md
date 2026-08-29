# Landin

> A work-in-progress systems programming language inspired by Rust, using
> LLVM 22 (llvm-sys 221) for code generation. The compiler is written in
> Rust (~83K LOC across 177 files) and targets x86_64 + AArch64 Linux.

| | |
|---|---|
| **Author** | redskaber |
| **Version** | v0.510.0 (Stage 18.382) |
| **License** | MIT |
| **Status** | v0.4 stable — release-signed-off. 682 lib tests + 3727 integration tests = 4409 total, 0 failures (`ulimit -s unlimited`, single-thread). fmt clean, 0 clippy warnings. All P0/P1/P2 tech-debts resolved. v0.5+ Phase 1 progress: Stage 18.380 removed Phase 3.7, Stage 18.381 removed Phase 0 (writeback phases 10 → 8), Stage 18.382 confirmed Phase 3.5 step 1 NOT redundant (codegen reads field_ty directly). §14.5 D1-D8 deep review PASSED. Architecture health: 8.2/10. |
| **LLVM** | 22.1.8 (llvm-sys 221) |
| **Rust edition** | 2021 |
| **Process doc** | `docs/stage-committee-process.md` v7.4 (11 design principles + 13 execution principles + Bug probability distribution reasoning) |

---

## Table of Contents

1. [Quick Start](#quick-start)
2. [CLI Reference](#cli-reference)
3. [Language Features](#language-features)
4. [Codegen ABI Compliance](#codegen-abi-compliance)
5. [Testing](#testing)
6. [Architecture Overview](#architecture-overview)
7. [Tech Debt & Known Limitations](#tech-debt--known-limitations)
8. [v0.5+ Refactoring Roadmap](#v05-refactoring-roadmap)
9. [Project Layout](#project-layout)
10. [Documentation](#documentation)
11. [Contributing](#contributing)

---

## Quick Start

### Prerequisites

- Rust stable (≥ 1.70.0) + cargo + rustfmt + clippy
- LLVM 22.1 development headers (auto-installed via `scripts/setup-llvm-env.sh`)
- cc/clang (for linking)
- Linux x86_64 or aarch64

### Build

```bash
# 1. Setup LLVM 22 environment
source scripts/env.sh

# 2. Build
cargo build --release --features llvm-backend

# 3. Run tests (auto-tunes --test-threads + raises ulimit -s for LLVM)
bash scripts/run_tests.sh
```

### Hello World

```bash
echo 'fn main() -> i32 { println!("hello world"); 0 }' > hello.lin
./target/release/landin-stage0 --run hello.lin
```

### Multi-File Project (`landinc`)

```bash
landinc new my_project && cd my_project
landinc build --release
landinc run
```

---

## CLI Reference

### `landin-stage0` — single-file compiler

| Flag | Description |
|------|-------------|
| `--compile` | Full pipeline (lex → parse → typeck → borrowck → codegen) |
| `--emit-llvm-ir` | Emit LLVM IR text (implies `--compile`) |
| `--emit-obj` | Emit object file `.o` (requires `llvm-backend`) |
| `--emit-bin` | Emit executable (requires `llvm-backend`) |
| `--run` | Compile, link, and run (requires `llvm-backend`) |
| `--emit-tokens` | Emit token stream only (debug) |
| `--emit-ast` | Emit AST only (debug) |
| `--color WHEN` | Color output: `auto` / `always` / `never` (default: auto) |
| `--target TRIPLE` | Cross-compile target (e.g. `aarch64-unknown-linux-gnu`) |

### `landinc` — multi-file project tool

```bash
landinc new <name>         # Create new project
landinc build [--release]  # Build all .lin files in src/
landinc run                # Build + run
landinc check              # Type-check only (no codegen)
landinc test               # Run unit tests
```

---

## Language Features

### Types
- Primitives: `i8`–`i128`, `u8`–`u128`, `usize`, `isize`, `f32`, `f64`, `bool`
- Strings: `&str` (fat pointer `{ ptr, len }`), `String` (owned `{ ptr, len, cap }`)
- Collections: `Vec<T>`, `Box<T>`, `Option<T>`, `Result<T, E>`
- Arrays: `[T; N]`
- References: `&T`, `&mut T`, raw pointers: `*const T`, `*mut T`
- Function pointers: `fn(...) -> T`
- Closures: `|args| expr`
- Trait objects: `dyn Trait`
- Generic structs: `Pair<A, B>`, `Wrapper<T>` (field access works including `*mut T` fields)

### Constructs
- `fn` — function definitions with generic parameters
- `struct` — named-field structs (including recursive via pointer)
- `enum` — tagged unions
- `impl` — inherent + trait implementations (including on primitive types)
- `trait` — trait definitions
- `let` / `let mut` — variable bindings with pattern destructuring
- `if` / `else` / `match` — control flow with nested patterns
- `while` / `for` / `loop` — loops with `break` / `continue`
- `&` / `&mut` — borrows
- `*` — dereference

### Macros
- `println!` / `print!` / `eprintln!` / `eprint!`
- `format!` (variadic, MIR intrinsic)
- `vec!`
- `stringify!`, `concat!`, `panic!`, `assert!`, `assert_eq!`

### Trait dispatch
- Static dispatch (monomorphization)
- Dynamic dispatch via `dyn Trait` (vtable indirect call)
- Trait objects with `Copy` / `Clone` auto-derivation

### Memory safety
- Ownership + borrow checking (NLL skeleton with dataflow-driven fixpoint)
- Move semantics with flow-sensitive drop elaboration
- Zero-cost abstractions (no runtime overhead for traits, generics)
- Bounds checking on array/string indexing (panics on OOB)

---

## Codegen ABI Compliance

Landin explicitly models System V AMD64 ABI requirements at the LLVM IR
level (rather than relying on LLVM's CodeGenPrepare auto-lowering).

### ABI attributes emitted

| Attribute | When | Where |
|-----------|------|-------|
| `sret(<ty>)` | Function return type > 16 bytes | Param 1 of callee + call site |
| `byval(<ty>)` | Function param type > 16 bytes | Each large param of callee + call site |
| `ptr` (opaque) | All pointer types (LLVM 17+ opaque pointer mode) | All GEP/load/store/alloca |

### ZST handling
- **Params**: ZST params elided from LLVM signature (mirrors rustc)
- **Args**: ZST args skipped at call sites
- **Fields**: ZST fields elided from LLVM struct types via `filter_void_fields`
- **Array elements**: ZST uses `{}` (LLVM empty struct) → `[N x {}]` is valid
- **Allocas**: ZST allocas use `i8` fallback (size-0 allocas produce undef pointers = UB)

### Recursive struct handling
Recursive types (`struct Node { next: *mut Node }`) use opaque `ptr` for
`Ref`/`RawPtr` to `Adt` — no pointee type recursion. Pointee layout
resolved only at dereference sites via `detect_place_storage_type`.

---

## Testing

### Test count
- 682 unit tests (lib)
- 3721 integration tests (`tests/all_tests.rs`)
- **4403 total** (100% pass rate single-thread, 0 skipped)

### Running tests
```bash
ulimit -s unlimited
cargo test --release --features llvm-backend -- --test-threads=1
# Or use the auto-tuning script:
bash scripts/run_tests.sh
```

### §14.5 D1-D8 Deep Review (v0.4 sign-off)

| Dimension | Status | Details |
|-----------|--------|---------|
| D1 Architecture | ✅ | 177 files, 83K LOC, no circular deps |
| D2 Tech Debt | ✅ | All P0/P1/P2 resolved. 10 stubs/limitations documented for v0.5+ |
| D3 Test Coverage | ✅ | 4403 tests, 1:3+ pos:neg ratio |
| D4 Next Stage Readiness | ✅ | v0.4 release-ready |
| D5 Design Soundness | ✅ | sret+byval, ZST elision, recursive struct, TextEmitter IR validated |
| D6 Performance | ✅ | ~30s build, ~27s test single-thread |
| D7 Documentation | ✅ | 13 lang-design docs + tech-debt-register + process doc v7.4 |
| D8 Pipeline Coverage | ✅ | All 10 expression contexts verified closed |

---

## Architecture Overview

### Compilation Pipeline

```
Source → Lexer → macro_expand → Parser → HIR Lower → Resolve
→ MIR Lower → TypeCheck (10 phases incl. Phase 0 + 3.7 writeback)
→ BorrowCheck → Writeback (driver-level)
→ MIR Opt (DCE → const_prop → DCE) → Monomorphization
→ Codegen (TextEmitter / LLVMSysEmitter) → Link → Execute
```

### Module sizes (LOC)

| Module | LOC | Responsibility |
|--------|-----|----------------|
| `mir/` | 24,100 | MIR data + lowering + optimization + monomorphization |
| `codegen/` | 13,993 | LLVM IR emission (TextEmitter + LLVMSysEmitter) |
| `parser/` | 10,172 | Parser + macro expansion |
| `typeck/` | 6,420 | Type checker + writeback + unify + predicates |
| `borrowck/` | 5,856 | NLL borrow checker skeleton |
| `driver/` | 5,334 | Compilation pipeline orchestration |
| `hir/` | 3,508 | HIR data structures + lowering |
| `stdlib/` | 2,749 | Landin prelude (String/Vec/Box/Option/Result) |
| `traits/` | 2,746 | Trait resolution + coherence |
| `resolve/` | 2,676 | Name resolution |
| `lexer/` | 2,252 | Tokenizer |

### Five-layer substitute chain (Stage 18.347-18.358)

The typeck writeback architecture uses five layers of `substitute()` calls
to resolve generic `Param(N)` placeholders:

1. **Phase 0** (Stage 18.353): pre-typeck writeback — resolves Param before typeck sees it
2. **Phase 3.5** (Stage 18.357): table writeback — applies substitute when overwriting field_ty
3. **Phase 3.7** (Stage 18.355): post-table re-writeback — fixes Phase 3.5 regression
4. **resolve_place_type_with_table** (Stage 18.358): recursive substitute — resolves nested projections
5. **compute_use_writeback_ty** (Stage 18.361): recursive Projection — handles nested Projection base

### Design principles (§2.2, 11 principles)

1. 长期 > 短期 | 2. 整体 > 局部 | 3. 显式 > 隐式 | 4. 报错 > 静默
5. 去除兼容思维 | 6. 通用 > 特例 | 7. API 命名标准化 | 8. 设计驱动测试
9. 正确 > 妥协 | **10. 唯一可信数据源** | **11. 确定性边界**

### Execution principles (§2.1.1, 13 principles)

1-10. (standard: plan, naming, isolation, generality, cohesion, etc.)
**11. 确定性边界先行** | **12. 临时桩识别与记录** | **13. 架构限制记录与升级**

---

## Tech Debt & Known Limitations

All P0/P1/P2 tech-debts are **resolved** (Stage 18.372 closed TD-UNWRAP-GUARDED-EXPECT
— 15 production guarded `.unwrap()` → `.expect("invariant doc")` across 9 files;
Stage 18.373 closed TD-UNREACHABLE-INVARIANT — 4 bare `unreachable!()` →
`unreachable!("invariant msg")` across 4 files;
Stage 18.374 closed TD-TY-INFER-SPAN — 3 `fresh_infer_ty(Span::DUMMY)` →
`fresh_infer_ty(real_span)` across 2 files;
Stage 18.375 closed TD-AS-CAST-TRUNCATION — 8 `*n as u32` u128→u32 silent
truncation → `u32::try_from(*n).expect(...)` across 4 files;
Stage 18.376 closed TD-ARCH-NESTED-GENERIC-FIELD-ACCESS — nested generic
field access `Outer<Inner<T>>.inner.val` now compiles; 5-layer root-cause
fix across lower + inference + writeback + mono collect;
Stage 18.377 closed TD-ALLOW-SUPPRESSION — audited 26 production `#[allow]`,
removed 6 stale, verified 20 legitimate).
Remaining items are v0.5+ architecture limitations (documented in
`docs/develop/v0/tech-debt-register.md` §2.5.1):

| ID | Description | Status | Fix Plan |
|----|-------------|--------|----------|
| TD-STUB-PRELUDE-LOOP-BODY | Prelude `loop {}` marker bodies (4 methods) | 🟡 v0.5+ | Fat pointer construction syntax |
| TD-TYPECK-LOCAL-DECL-ERROR-CHECK | Phase 4.5 disabled (47 prelude false-positives) | 🟡 v0.5+ | Prelude lazy monomorphization |
| TD-STUB-REGION-ERASED | Region inference no-op | 🟡 v0.2+ | SCC + type tests + universe |
| TD-STUB-DROP-ELABORATION-NOOP | Drop elaboration no-op | 🟡 v0.2+ | Drop::drop codegen + dropck |
| TD-STUB-LIFETIME-ELISION-NOOP | Lifetime elision no-op | 🟡 v0.2+ | 3-rule elision per `03-type-system.md` §5 |
| TD-STUB-PROJECTION-RESOLVER | Projection resolver partial | 🟡 v0.2+ | Associated type normalization |
| TD-STUB-EMIT-TYPE-I32-FALLBACK | `mir_type_to_emit_type` i32 fallback | ✅ Mitigated | param_check (Stage 18.348) catches unresolved types |
| TD-STUB-TYPECK-BEFORE-WRITEBACK | typeck before writeback | ✅ Resolved | Phase 0 + Phase 3.7 double writeback (Stage 18.353+18.355) |
| TD-STUB-DEFAULT-INT-I32 | Default int = i32 | ✅ Design choice | Not a stub — Landin design decision |
| TD-UNWRAP-GUARDED-EXPECT | 15 production guarded unwraps lack invariant docs | ✅ Resolved (Stage 18.372) | All converted to `expect("invariant doc")` with comments |
| TD-UNREACHABLE-INVARIANT | 4 production bare `unreachable!()` lack invariant msg | ✅ Resolved (Stage 18.373) | All converted to `unreachable!("invariant msg")` with comments |
| TD-TY-INFER-SPAN | 3 production `fresh_infer_ty(Span::DUMMY)` lack source span | ✅ Resolved (Stage 18.374) | All converted to `fresh_infer_ty(real_span)` (param.span / expr.span) |
| TD-AS-CAST-TRUNCATION | 8 production `*n as u32` (u128→u32) silent truncation | ✅ Resolved (Stage 18.375) | All converted to `u32::try_from(*n).expect(...)` (panic on overflow) |
| TD-ARCH-NESTED-GENERIC-FIELD-ACCESS | Nested generic field access `Outer<Inner<T>>.inner.val` | ✅ Resolved (Stage 18.376) | 5-layer fix: lower + inference + writeback + mono collect |
| TD-ALLOW-SUPPRESSION | 26 production `#[allow]` suppressions | ✅ Resolved (Stage 18.377) | 6 stale removed, 20 verified legitimate (BLOCKED infra / forward-compat / style) |

---

## v0.5+ Refactoring Roadmap

Based on deep architecture audit (Stage 18.366-18.367), referencing Rust rustc design:

| Phase | Target | Priority | Est. | Reference |
|-------|--------|----------|------|-----------|
| 1 | typeck writeback unification (10 phases → inline) | Highest | 2-3w | rustc typeck + type propagation interwoven |
| 2 | expected_ty propagation in MIR lower | High | 1-2w | rustc MIR lower expected_ty |
| 3 | FieldTyTable removal | Medium | 1w | rustc doesn't use FieldTyTable |
| 4 | mono_layouts stored in MirBody | Medium | 1w | rustc MirSource carries type info |
| 5 | mir_type_to_emit_type returns Result | Low | 1-2w | rustc CodegenCx::layout_of |

---

## Project Layout

```
landin/
├── src/                          # Compiler source (~83K LOC, 177 files)
│   ├── bin/                      # CLI entry points (landin-stage0 + landinc)
│   ├── lexer/                    # Tokenizer (2.2K LOC)
│   ├── parser/                   # AST + macro_expand (10.2K LOC)
│   ├── hir/                      # High-level IR (3.5K LOC)
│   ├── resolve/                  # Name resolution (2.7K LOC)
│   ├── mir/                      # Mid-level IR (24.1K LOC)
│   │   ├── lower/                # MIR lowering from HIR (21 modules)
│   │   ├── monomorphize/         # Generic instantiation + layouts
│   │   ├── param_check.rs        # Pre-codegen diagnostic (Stage 18.348)
│   │   ├── optimization.rs       # DCE + const_prop
│   │   └── substitute.rs         # Type parameter substitution
│   ├── typeck/                   # Type checker (6.4K LOC)
│   │   ├── checker.rs            # 10-phase check_mir_body_with_tables
│   │   ├── check.rs              # check_statement + post_check_statement
│   │   ├── infer.rs              # Type inference + infer_projection
│   │   ├── writeback.rs          # Phase 3.5 field_ty_table writeback
│   │   └── unify.rs               # Unification table
│   ├── codegen/                  # LLVM IR emission (14K LOC)
│   │   ├── llvm/                 # LLVMSysEmitter (production, C-API)
│   │   ├── text/                 # TextEmitter (debug, --emit-llvm-ir)
│   │   ├── emitter/              # Emitter trait + EmitType
│   │   ├── mir_translation/      # MIR → EmitType (49 mono_layouts callsites)
│   │   └── trait_dispatch/       # Vtable construction
│   ├── borrowck/                 # NLL borrow checker (5.9K LOC)
│   ├── driver/                   # Compilation pipeline (5.3K LOC)
│   ├── stdlib/                   # Landin prelude (String/Vec/Box/...)
│   ├── traits/                   # Trait resolver + coherence
│   ├── session/                  # Compiler session + diagnostics
│   └── diagnostics/              # Error formatting
├── tests/                        # 4403 tests (682 lib + 3721 integration)
├── docs/                         # Documentation
│   ├── stage-committee-process.md  # SOP v7.4 (3100+ LOC)
│   ├── develop/v0/               # Dev logs + tech-debt-register
│   ├── lang-design/              # 13 language design docs
│   ├── graph/                    # Pipeline graphs
│   └── ...
├── scripts/                      # env.sh + setup-llvm-env.sh + run_tests.sh
├── examples/                     # Example programs
├── benchmark/                    # Benchmarks
└── Cargo.toml
```

---

## Documentation

- **Build guide**: `docs/build-guide.md`
- **Testing guide**: `docs/testing-guide.md`
- **SOP**: `docs/stage-committee-process.md` v7.4 (11 design principles + 13 execution principles + Bug probability distribution)
- **Tech debt register**: `docs/develop/v0/tech-debt-register.md` (10 stubs/limitations + 9 structural TDs resolved Stage 18.127-18.377, including nested generic field access + allow suppression audit)
- **Architecture audit**: Stage 18.366-18.367 worklog (health: 7.8/10, v0.5+ 5-phase roadmap)
- **Per-stage dev logs**: `docs/develop/v0/stage-N/`
- **Language design**: `docs/lang-design/` (13 docs: overview, spec, grammar, type system, etc.)

---

## Contributing

### Development workflow (per `docs/stage-committee-process.md` v7.4)

1. **Self-check (§1.2.1)**: classify task as L1/L2/L3
2. **Design alignment (§13.1)**: read `docs/lang-design/` + `docs/graph/`
3. **Certainty boundaries (§2.1.1 原则 11)**: clarify capability/design/responsibility boundaries before coding
4. **MUV (§4)**: smallest verifiable unit of work
5. **Inner review (§5)**: P0/P1 cleanup loop
6. **Iterative audit (§20)**: "finding one bug means there are many similar bugs" — audit all similar paths
7. **Stub identification (§2.1.1 原则 12)**: if passing `None`/defaults, determine if stub → record in tech-debt
8. **Architecture limitation (§2.1.1 原则 13)**: if architecture limit found → record in tech-debt → plan refactor
9. **Acceptance (§3.2)**: `cargo fmt + check + clippy + test --release` all green
10. **Documentation (§8)**: worklog + tech-debt-register + plan doc
11. **Packaging (§19)**: `landin-stage0-v<X>.<Y>.<Z>-stage<N>.<M>-<desc>-r<R>.tar.gz`

### Key principles

- §2.2 原则 3: 显式 > 隐式 (explicit > implicit)
- §2.2 原则 4: 报错 > 静默 (errors > silent)
- §2.2 原则 6: 通解 > 特解 (general > special-case)
- §2.2 原则 9: 正确 > 妥协 (correct > compromise)
- §2.2 原则 10: 唯一可信数据源 (single source of truth)
- §2.2 原则 11: 确定性边界 (certainty boundaries)
- §12: 最优 > 最小 (optimal > minimal)
- §20: 迭代审计 (iterative audit — Bug probability distribution reasoning)
- 知识搜索 > 猜测 (knowledge search > guessing)
- 唯一可信数据源 (single source of truth)

### License

MIT — see [LICENSE](LICENSE).
