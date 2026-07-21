# Global Test Matrix

> **Author**: redskaber
> **Date**: 2026-07-22
> **Process**: v3.16 (§17 + §18 + §21 + §23 + §25)

## Current Status

| Stage | Tests | Coverage | Status |
|-------|-------|----------|--------|
| Stage 0 (lexer/parser/AST) | 344 (+1 unsafe impl/trait in Stage 3.65) | ~100% | ✅ Complete |
| Stage 1 (HIR/resolve) | 117 (+5 use resolution; +1 visibility; +3 nested modules in Stage 4.1) | ~100% | ✅ Complete |
| Stage 2 (MIR/typeck/borrowck) | 168 | ~100% | ✅ Complete |
| Stage 3 (codegen) | 294 + 5 §21 audit | ~99% | ✅ Complete |
| **Total** | **987** | ~99% | ✅ Stage 0-3 complete + Stage 4 started |

## Stage 3 Test Breakdown

| Sub-stage | Feature | Tests | Status |
|-----------|---------|-------|--------|
| 3.1-3.4 | Basic codegen (return, arith, variables, control flow) | 36 | ✅ |
| 3.5-3.7 | Params, match, float, cast | 15 | ✅ |
| 3.21 | Typed aggregates | 10 | ✅ |
| 3.22 | Block-scoped cache | 6 | ✅ |
| 3.24 | Overflow checks | 8 | ✅ |
| 3.25 | Div-by-zero checks | 6 | ✅ |
| 3.27 | String literals | 13 | ✅ |
| 3.28 | Byte strings + u8/i8 | 9 | ✅ |
| 3.30 | ADT/struct codegen | 13 | ✅ |
| 3.32 | Field type resolution | 6 | ✅ |
| 3.34 | Field mutation | 8 | ✅ |
| 3.36 | Field type propagation | 8 | ✅ |
| 3.38 | Enum variant codegen | 10 | ✅ |
| 3.40 | Enum match | 8 | ✅ |
| 3.42 | &str type fix | 6 | ✅ |
| 3.43 | Shift overflow check | 8 | ✅ |
| 3.44 | Const/Static value resolution | 8 | ✅ |
| 3.45 | L10 float bitwise ops via cast | 6 | ✅ |
| 3.46 | L14 + L9 full integer types (i8/i16/i32/i64/i128/usize/isize) | 13 | ✅ |
| 3.47 | L-PIPE-1 closure via AdtLayout side-table on MirBody (per §16) | 14 | ✅ |
| 3.48 | L-ENUM-UNION + L-ENUM-BINDING closure: flat enum storage + pattern binding extraction | 12 | ✅ |
| 3.49 | L13 fat pointer closure: &str/&[T] now { ptr, len } struct, not thin pointer | 12 | ✅ |
| 3.50 | Byte string fat pointer fix + comparison pointee type fix (Stage 3.49 latent bugs) | 10 | ✅ |
| 3.51 | Slice indexing fix: fat pointer data pointer dereference (Stage 3.49 latent P0) | 9 | ✅ |
| 3.52 | Slice element type propagation: load/store/arith use correct element type from fat pointer | 9 | ✅ |
| 3.53 | &str indexing element type fix: u8 element, not i32 (Stage 3.52 latent) | 9 | ✅ |
| 3.54 | Slice/array field store + detect_lvalue_storage_type Field projection fix | 9 | ✅ |
| 3.55 | Void function return type fix: void fn emits define void + ret void (P0 correctness) | 9 | ✅ |
| 3.56 | Pipeline architecture refactoring Phase A: codegen as pure MIR consumer (§16 compliance) | 6 | ✅ |
| 3.57 | Phase B-D: error path coverage + glob exports cleanup + Emitter trait tests | 12 | ✅ |
| 3.58 | Typeck implicit coercion: Bool→Int, narrower→wider integers; all gen_ll_unchecked eliminated | 0 | ✅ |
| 3.59 | Cross-stage audit: coercion fix (reject lossy Uint→Int narrowing) + f32→f64 widening | 7 | ✅ |
| 3.60 | Typeck section 16 compliance: FieldTyTable + FnSigTable eliminate typeck→HIR leak | 0 | ✅ |
| 3.61 | section 21 audit: lib.rs API surface + audit verification tests + process v3.14 | 5 | ✅ |
| 3.62 | Stage 3 收尾: dead code cleanup (~387 lines) + naming standardization + Stage 3 Complete | 0 | ✅ |
| 3.63 | Cross-stage naming standardization per §21 audit (9 P1 + 1 P2 fixes; pure refactoring) | 0 | ✅ |
| 3.64 | P2 ergonomics fixes (6 Error trait impls, Emitter re-export, emit_output rename, orphaned doc cleanup) + use declaration resolution (Stage 1.3 Phase C — previously stub) | +5 | ✅ |
| 3.65 | P2 architectural fixes: unsafe impl/trait AST+HIR+parser + Res::SelfTy HirSelfKind discrimination + lower_body aliases + mir_type_to_emit_type docs | +1 | ✅ |
| 3.66 | Lvalue→Place rename (167+ refs across 7+ files) + resolver owner context threading for accurate HirSelfKind (Trait vs Impl) | 0 | ✅ |
| 3.67 | P2 cleanup: body owner context threading (body-level HirSelfKind accurate) + &mut Rodeo→&Rodeo in resolve_crate (lexer interns keywords) + Span::DUMMY placeholders fixed (11 in parser.rs) | 0 | ✅ |
| 3.68 | Visibility checking infrastructure: def_visibility map + check_visibility hook (stub, ready for Stage 4 nested modules) + visibility metadata collection | +1 | ✅ |
| 3.69 | Process v3.16 (§25 阶段末尾深度审查协议) + Stage 0-3 deep review (GO-WITH-CONDITIONS for Stage 4) | 0 | ✅ |
| 4.1 | Nested module support: recursive build_module_tree + child ModuleNode + collect_item_registration + build_child_module + item_def_id | +3 | ✅ |
| 4.2 | L1 PHI optimization CLOSED: design decision to rely on LLVM mem2reg (standard approach, documented in codegen/mod.rs) | 0 | ✅ |
| **Total codegen** | | **294 + 5 §21 audit** | ✅ |
| Gate audits R1-R36 + Deep review R37 + Stage 4.1-4.2 R38 | Audit cases | 716+ cumulative + 7-dimension deep review | ✅ |

## Deferred Items (≤5% allowed per §17.3)

| ID | Feature | Reason | Plan |
|----|---------|--------|------|
| ~~L1~~ | ~~PHI node optimization~~ | CLOSED in Stage 4.2 (design decision: rely on LLVM mem2reg) ✅ |
| L3 | Closure codegen | New feature | Stage 4 |
| L5 | Trait dispatch | New feature | Stage 5 |
| L8 | lli execution verification | Env lacks LLVM tools | When available |
| ~~L9~~ | ~~i128/u128~~ | CLOSED in Stage 3.46 ✅ |
| ~~L10~~ | ~~Float bitwise ops~~ | CLOSED in Stage 3.45 ✅ |
| ~~L11~~ | ~~Shift-count overflow~~ | CLOSED in Stage 3.43 ✅ |
| ~~L13~~ | ~~Fat pointers~~ | CLOSED in Stage 3.49 ✅ |
| ~~L14~~ | ~~i16/u16 → i32~~ | CLOSED in Stage 3.46 ✅ |
| ~~L15~~ | ~~String-as-function-arg~~ | CLOSED in Stage 3.42 ✅ |
| ~~L-ENUM-UNION~~ | ~~Enum union payload~~ | CLOSED in Stage 3.48 ✅ |
| L-COPY-ADT | Proper Copy trait | Needs TraitResolver | Stage 5 |
| ~~L-PIPE-1~~ | ~~HIR lookup for Adt storage~~ | CLOSED in Stage 3.47 ✅ |
