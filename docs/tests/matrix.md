# Global Test Matrix

> **Author**: redskaber
> **Date**: 2026-07-26 (last updated Stage 12.2)
> **Process**: v3.21 (§17 三阶段文档协议 + §18 轮次文档同步 + §21 跨阶段审查 + §23 命名标准 + §25 深度审查 + §25.8 设计回写)

## Current Status

| Stage | Tests | Coverage | Status |
|-------|-------|----------|--------|
| Stage 0 (lexer/parser/AST) | 344 | ~100% | ✅ Complete |
| Stage 1 (HIR/resolve) | 99 | ~100% | ✅ Complete |
| Stage 2 (MIR/typeck/borrowck) | 141 | ~100% | ✅ Complete |
| Stage 3 (codegen) | 309 (incl. 5 §21 audit) | ~99% | ✅ Complete |
| Stage 4 (modules + closures + macros + benches) | 13 (incl. 5 bench) | ~100% | ✅ Complete |
| Stage 5 (TraitResolver + vtable + dyn Trait + stdlib) | 977 | ~100% | ✅ Complete (99 sub-stages) |
| Stage 6 (architectural splits — 47 modules) | — (refactor, behavior-equivalent) | — | ✅ Complete (18 sub-stages, 1881 tests unchanged) |
| Stage 7 (region inference + user-defined trait dyn) | 35 (+28 unit) | ~98% | ✅ Complete (9 sub-stages) |
| Stage 8 (v0.2 features + docs standardization) | 38 (+9 unit) | ~98% | ✅ Complete (7 sub-stages) |
| Stage 9 (v0.1 conformance suite expansion) | +145 rust + +600 conformance (00-parse) | ~100% | ✅ Complete (12/12 sub-stages, 600/600 parse conformance) |
| Stage 10 (CLI upgrade + 8 conformance categories created) | +44 rust + +539 conformance | ~100% | ✅ Complete (8/8 sub-stages, 1139 conformance) |
| Stage 11 (conformance expansion 1139→5026) | +30 rust + +3887 conformance | ~100% | ✅ Complete (10/10 sub-stages, 5026/5000 v0.1 gate reached!) |
| Stage 12.1 (v0.1 release + v0.3 bootstrap prep) | +6 rust | — | ✅ Complete (v0.1 release ratified) |
| Stage 12.2 (cross-stage audit r216 + Stage 13 plan + §25.8 write-back + D7 backfill) | +10 rust | — | ✅ Complete (5/5 GO-WITH-CONDITIONS, r216 first-pass) |
| Stage 12.3 (r217 second-pass audit — 3 reports, 2055 lines, stage-round revisions) | +12 rust | — | ✅ Complete (5/5 GO-WITH-CONDITIONS, r217 second-pass) |
| Stage 12.4 (§25.8 retroactive backfill for Stage 5 + Stage 8 — 3 design-doc edits) | 0 | — | ✅ Complete (DynTraitMIRSummary + StdlibTypeKind + async/await MVP) |
| Stage 12.5 (plan-13.1.md reframe — Planned → Draft, Stage 12 output) | 0 | — | ✅ Complete |
| Stage 12.6 (version revert v0.22.0 → v0.21.2 — patch bump per r217) | 0 | — | ✅ Complete |
| Stage 12.7 (Stage 0-4 README per-module attribution corrections) | 0 | — | ✅ Complete (5 READMEs corrected) |
| Stage 12.8 (Stage 12 final gate review — §25 deep review of Stage 12) | +13 rust | — | ✅ Complete (5/5 GO-WITH-CONDITIONS-or-GO → PASS) |
| Stage 12.9 (Polish backfill — Stage 5 develop README + plan-6.{4,5,6}.md retroactive + v2.36 correction) | +13 rust | — | ✅ Complete (3/3 deferred P2/P3 items closed) |
| Stage 13.1 (Architecture baseline — TD-028 §16 violation CLOSED, 7 emit_* relocated mir→codegen) | +10 rust | — | ✅ Complete (5/5 GO → PASS, §16 compliant) |
| Stage 13.2 (if-let / while-let — TD-031 P0 CLOSED, Strategy B desugar to Match) | +11 rust | +11 PASS (5015→5026) | ✅ Complete (5/5 GO → PASS, v0.22.0 minor bump) |
| Stage 13.3 (Closure call lowering TD-030 P0 — preparation phase, §13.4 design alignment + blueprint) | +9 rust | 0 | ✅ Preparation complete (5/5 GO-WITH-CONDITIONS → PASS; 13.3a implementation pending) |
| Stage 13.3a (TD-030 P0 CLOSED — closures callable via inline approach, 30+ conformance compile_error→compile_ok) | +9 rust | +30 compile_ok | ✅ Complete (5/5 GO → PASS, v0.23.0 minor bump) |
| Stage 13.1b (TD-029 TyKind::Dynamic refactor — deferred per design alignment §15) | TBD | — | ⏳ Deferred (P2, non-blocking for P0) |
| Stage 13.4 (Built-in macros TD-032 P0 — preparation, §13.4 design alignment + TD-032 reframe) | +7 rust | 0 | ✅ Preparation complete (5/5 GO-WITH-CONDITIONS → PASS; 13.4a pending) |
| Stage 13.4a (19 missing built-in macros — TD-032 P0 CLOSED, all 26 built-in macros) | +8 rust | 0 | ✅ Complete (5/5 GO → PASS, v0.24.0 minor bump, ALL P0 CLOSED) |
| Stage 13.5 MUV-1 (LLVM library integration — `llvm-sys` v191/v211 linked) | +6 rust | 0 | ✅ Complete (LLVM 19/21 environment setup + version switching) |
| Stage 13.5 MUV-2 (LLVMSysEmitter — 36/36 Emitter trait methods, 1360 LOC) | +9 rust | 0 | ✅ Complete (real LLVM module building via C API) |
| Stage 13.5 MUV-3 (End-to-end LLVM module → object file verification) | +N rust | 0 | ✅ Complete (LLVMSysEmitter C API + to_object_file) |
| Stage 13.6 (`--emit-obj` flag — LLVM Module → TargetMachine → .o file) | +N rust | 0 | ✅ Complete |
| Stage 13.7-13.10 (`--emit-bin` + auto C wrapper + `--run` flag + runtime stubs) | +N rust | 0 | ✅ Complete (compile → link → execute pipeline) |
| Stage 13.11-13.12 (println! capture + side-table emission, with known limitation) | +N rust | 0 | ✅ Complete (helper-function approach; ordering bug identified) |
| Stage 13.13 (Inline println! emission via `StatementKind::Println` — fixes Stage 13.12 ordering bug) | +10 rust | 0 | ✅ Complete (7/7 GO → PASS, v0.24.1 patch bump) |
| Stage 13.14 (`eprintln!`/`eprint!` stderr emission via `__landin_eprint` helper — closes Stage 13.13 deferral) | +7 rust | 0 | ✅ Complete (7/7 GO → PASS, v0.24.2 patch bump) |
| Stage 13.15 (Fix `landin_main` double-prefix symbol bug — P0 linker fix; both `fn main()` and `fn landin_main()` now work) | +7 rust | 0 | ✅ Complete (7/7 GO → PASS, v0.24.3 patch bump) |
| Stage 13.16 (Format args — `println!("{}", x)` now works; P0 v0.1 blocker closed; first real I/O feature) | +9 rust | 0 | ✅ Complete (7/7 GO → PASS, v0.25.0 minor bump) |
| Stage 13.5+ (TD-033 P1 sub-items + full Strategy A + Fn/FnMut/FnOnce + v0.1 release announcement) | TBD | TBD | ⏳ Pending (P1) |
| **Total** | **2333** rust + **5026** conformance | ~100% | 🎉 v0.1 GATE REACHED + RATIFIED by r216+r217+r219 audits (5026/5000 = 100.5%); Stage 12 ✅ COMPLETE; Stage 13 🔄 IN PROGRESS (13.1 ✅ TD-028, 13.2 ✅ TD-031 P0, 13.3a ✅ TD-030 P0, 13.4a ✅ TD-032 P0; 3/3 P0 CLOSED 🎉; 13.5-13.17 LLVM execution pipeline + inline println + stderr routing + landin_main fix + format args + self binding + method call codegen ✅) |

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
| 4.3 | Visibility enforcement activation: check_visibility implements pub/private/pub-restricted checks (same-crate access allowed; cross-module deferred) | 0 | ✅ |
| 4.4 | L3 closure lowering: AggregateKind::Closure + TyKind::Closure → empty struct; capture analysis deferred to Stage 4.5 | +2 | ✅ |
| 4.5 | Complete dev-logs for all stages: Stage 1 + Stage 2 + Stage 4 dev-logs created; Stage 0 + Stage 3 dev-logs updated with retroactive entries | 0 | ✅ |
| 4.6 | Process v3.17: §17 测试目录标准化与三阶段文档协议 + tests/ 标准化目录结构 + Stage 4 plan/test-plan/gate-review 文档补齐 | 0 | ✅ |
| 4.7 | L3 closure capture analysis: collect_captured_locals + collect_pat_hir_ids + collect_block_captured; captures populate closure struct fields + Aggregate operands; codegen emits struct with capture fields | +4 | ✅ |
| 4.8 | tests/ directory restructure: 13 flat tests/*.rs → standardized tests/v0/stage{N}/plan/ per v3.17 §17.1; 14 [[test]] targets in Cargo.toml; 13 test plan docs created | 0 | ✅ |
| 4.9 | L3 closure call lowering: detect TyKind::Closure in Call lowering; simplified placeholder (full call deferred to Stage 4.10) | +2 | ✅ |
| 4.10 | Macro system: built-in macro expansion (println!/stringify!/assert!) in MIR lowering; MacroCall no longer produces Error | +3 | ✅ |
| 4.11 | Performance benchmark suite (benches/compile_bench.rs, 5 benchmarks) + ADR docs (ADR-001 to ADR-007) — closes deep review R37 conditions | +5 (bench) | ✅ |
| 4.12 | Process v3.18 (worklog mirror to docs/worklog.md) + current_module tracking for visibility enforcement + 1000 tests milestone | +2 | ✅ |
| 4.13 | Full closure call lowering: extract captures from closure struct + inferred-type result (inline body deferred to Stage 5) | +2 | ✅ |
| 4.14 | Stage 4 deep review (§25): 7-dimension analysis, GO for Stage 5. Stage 4 COMPLETE. | 0 | ✅ |
| R49 | Cross-stage deep review (§21+§25): Stage 0-4 pipeline 7-point verified, 16 tech debt cataloged, GO for Stage 5 | 0 | ✅ |
| 5.1 | TraitResolver: collect trait definitions + impl blocks + build dispatch tables (ImplMap + MethodMap); src/traits/mod.rs created | +3 | ✅ |
| **Total** | | **294 + 5 §21 audit + 3 trait** | ✅ |
| Gate audits R1-R36 + Deep reviews R37/R48/R49 + Stage 4.1-4.14 + Stage 5.1 | Audit cases | 716+ cumulative + 3 deep reviews | ✅ |
| 13.1-13.16 | LLVM integration + execution pipeline + I/O (println!/eprintln!/format args) | 1951 rust tests + 5026 conformance | ✅ |
| 13.17-13.29 | Self binding + method call codegen + compound assign + NLL flip + codegen refactoring | (no new tests; conformance flip) | ✅ |
| 13.30-13.34 | conformance fn main fix + meaningful main generation (worklog backfilled in Stage 14.2) | (no new tests) | ✅ |
| 14.1 | v0.1 capability assessment + gap analysis (research only — no code change) | 0 | ✅ |
| 14.2 | Process hygiene: worklog backfill + version sync (no code change) | 0 | ✅ |
| 14.3 | Architecture cleanup: `trait_dispatch.rs` split per §14.4 (962→4 files; zero behavior change) | 1951 (zero regression) | ✅ |
| 14.4 | API naming audit (§23): fix glob re-exports in `stdlib/mod.rs` (zero behavior change) | 1951 (zero regression) | ✅ |
| 14.5 | examples/ standardization (§17.4): 4 `[[example]]` declarations + new `trait_dispatch_emission` example | 4 examples compile | ✅ |
| 14.6-14.9 | Documentation sync + README rewrite + RELEASE_NOTES + final verification + package | (docs only) | ✅ |
| 14.10 | GAP-5 reclassified CLOSED (self.x fixed in Stage 13.18) + format_for_user trait_errors fix + GAP-17 reclassified CLOSED (print! works) | 1951 (zero regression) | ✅ |
| 14.11 | GAP-8 CLOSED: run_ok conformance runner rewrite + 6 run_ok test cases (run_ok/run_panic dispatch + EXPECTED_STDOUT/EXIT_CODE) | +6 run_ok (5032 total) | ✅ |
| 14.12 | GAP-18 CLOSED: bool prints as "true"/"false" via emit_select (added to Emitter trait + both backends) | 1951 (zero regression) | ✅ |
| 14.13 | GAP-30 PARTIAL: emit_dyn_trait_method_call implemented (was unimplemented! panic) + vtable/dynptr global content fixed (was NULL) + codegen reorder + 3 new run_ok tests | +3 run_ok (5035 total) | ✅ |
| 14.14 | Architecture cleanup investigation: >1000 LOC files analyzed (expr_operand 2039, llvm/mod 1686) — all deferred as L3 high-risk | 0 (investigation only) | ✅ |
| 14.16 | GAP-20 reclassified CLOSED (void main is NOT UB — codegen always emits ret i32 0) + 9 new run_ok tests (match, while, string, tuple, enum, recursion, struct method, if-else, void main) | +9 run_ok (5044 total) | ✅ |
| 14.17 | run_ok expansion (+5 tests: nested if, arithmetic, shadowing, iterative fib, fn composition) + discovered GAP-31 (&mut self field mutation broken) | +5 run_ok (5049 total) | ✅ |
| 14.18 | GAP-31 investigation: MIR lowering infrastructure added (query_method_self_kind + auto_deref_if_ref) but reverted — codegen Deref+Field projection issue blocks full fix | 0 (investigation) | ✅ |
| 14.19 | GAP-31 CLOSED: &mut self field mutation now propagates — codegen Deref+Field fix (3 sites) + MIR Ref-wrapping + call site Rvalue::Ref + auto_deref_if_ref + 2 run_ok tests | +2 run_ok (5051 total) | ✅ |
| 14.20 | Array repeat [val; N] fix — was 1-element array, now N elements with proper [T; N] type + 2 run_ok tests | +2 run_ok (5053 total) | ✅ |
| 14.21 | &self/&mut self + array field + index fix — codegen Deref+Index + find_receiver_struct_def_id auto-deref Ref + 2 run_ok tests | +2 run_ok (5055 total) | ✅ |
| 14.22 | Nested struct fix (mir_type_to_emit_type_with_layouts) + early return typeck (block diverges → Never) + struct type cache + 1 run_ok test | +1 run_ok (5056 total) | ✅ |
| 14.23 | Return value fix (is_terminated guard prevents overwriting return local) + return; assigns unit () + 1 run_ok test | +1 run_ok (5057 total) | ✅ |
| 14.24 | Loop break value fix (was returning 0) + test path coverage matrix + 4 run_ok tests (logical, bitwise, negative arith, loop break) | +4 run_ok (5061 total) | ✅ |
| 14.25 | Coverage matrix completion: *= /=, enum unit, i64, comparison all branches + 4 run_ok tests | +4 run_ok (5065 total) | ✅ |
| 14.26 | Pipeline test coverage matrix (603 paths, 99.7% coverage — per-stage + inter-stage + E2E) | 0 (documentation) | ✅ |
| 14.27 | *ptr = val fix (Deref store path loads pointer type, not value) + 3 run_ok tests (mut ref deref, ref read, ref param+return) | +3 run_ok (5068 total) | ✅ |
| 14.28 | Pipeline coverage expansion: closure capture, type cast, match or-pattern, string eq + 3 run_ok tests | +3 run_ok (5071 total) | ✅ |
| 14.29 | Method return type propagation (query_method_return_type) — chained calls work with annotations + 1 run_ok test | +1 run_ok (5072 total) | ✅ |
| 14.30 | Error reporting for unknown methods on concrete types (报错 > 静默) + lower_type_errors infrastructure | 0 (error reporting) | ✅ |
| 14.31 | Silent default audit: missing field / field on non-struct / index on non-array — TODO documented (blocked by immutable cx) | 0 (audit) | ✅ |
| 14.32 | Field error reporting attempted + reverted (MIR lower runs before typeck → false positives on valid fields) | 0 (reverted) | ✅ |
| 14.33 | Control flow coverage: while+continue, nested loop+break, while+break + 3 run_ok tests | +3 run_ok (5075 total) | ✅ |
| 14.34 | Match arm + return fix (is_terminated guard) + enum multi-variant, tuple struct, const, static, unit struct + 4 run_ok tests | +4 run_ok (5079 total) | ✅ |
| 14.35 | Call return type from fn_sigs (threading fn_sigs through codegen) — struct-returning calls fixed with annotations | 0 (infrastructure) | ✅ |
| 14.36 | Alloca type override for Call dest locals (get_call_dest_type) — alloca correct, field access still needs fn_sigs | 0 (partial fix) | ✅ |
| 14.37 | Call dest type writeback from fn_sigs + Assign type propagation (fixpoint) — struct-returning calls work WITHOUT annotations + 1 run_ok test | +1 run_ok (5080 total) | ✅ |
| 14.38 | Method chain resolution infrastructure (find_local_init_expr + resolve_method_by_name + query_method_return_type) — partial, two-step chains need debug | 0 (infrastructure) | ✅ |
| 14.39 | Self return type resolution in query_method_return_type + resolver bug discovered (impl method return type V has res=Unknown) | 0 (partial fix) | ✅ |
| 14.40 | Resolver fix: process impl_block.items + trait.items signatures inline — method chains now work end-to-end + 2 run_ok tests (multi-step chain, inline chain) | +2 run_ok (5082 total) | ✅ |
| 14.41 | Static method call correctness (Type::method path resolution via impl_method_index + adt_layouts re-populate after writeback) + 2 run_ok tests (static method side effect, Vec pattern) | +2 run_ok (5084 total) | ✅ |
| 14.42 | Method chain on MethodCall receivers + auto-deref for Ref + impl method namespace fix (impl_method_def_ids) + 6 run_ok tests + 2 conformance tests updated compile_error→compile_ok | +6 run_ok (5090 total) | ✅ |
| 14.43 | Nested struct mutation (2-level + 3-level) via compute_place_address + fn_sig_table self_kind-first check + recursive adt_layouts registration + 2 run_ok tests | +2 run_ok (5092 total) | ✅ |
| 14.44 | Array of structs + LLVMVerifyModule (catches silent IR errors) + 7 fixes (array aggregate, GEP, void call, branch i1, Index receiver, Index writeback) + 2 run_ok tests | +2 run_ok (5094 total) | ✅ |
| 14.45 | Or-pattern fix in match (was matching all values via wildcard) + audit (closures/strings/math — no bugs) + 3 run_ok tests | +3 run_ok (5097 total) | ✅ |
| 14.46 | Tuple destructuring in let bindings (was 0 0 0 — only created one local) + field extraction via Projection + 2 run_ok tests | +2 run_ok (5099 total) | ✅ |
| 14.47 | Match arm tuple destructure (was garbage values) + field extraction for Tuple in pattern_bindings + skip SwitchInt on non-int scrutinee + 2 conformance updated compile_error→compile_ok + 2 run_ok tests | +2 run_ok (5101 total) | ✅ |
| 14.48 | Struct destructuring in let + match (was 0 0 / garbage) + field-name→index lookup from HIR + 3 run_ok tests | +3 run_ok (5104 total) | ✅ |
| 14.49 | Nested tuple destructure (was 0 0 3) + recursive helper + 3 writeback steps (tuple literal, field projection, detect_place_type) + 2 run_ok tests | +2 run_ok (5106 total) | ✅ |
| 14.50 | Nested struct + mixed pattern destructure (was 0 0 3 / 0 0 99) + unified `lower_nested_pattern_destructure` recursive helper (handles Struct/Tuple/Ident) + 3 run_ok tests | +3 run_ok (5109 total) | ✅ |

## Stage 15 (v0.2 Phase 1 + Phase 2) Test Coverage

| Sub-stage | Feature | Tests | Status |
|-----------|---------|-------|--------|
| 15.6 | Method return type cache (perf — avoid re-querying TraitResolver) | +N rust | ✅ |
| 15.7 | Writeback consolidation (8 driver writeback passes → 2 functions, 650 LOC → 25 LOC) | +N rust | ✅ |
| 15.8 | Crate-shared AdtLayouts (Arc<AdtLayouts> — ~500KB saved per crate) | +N rust | ✅ |
| 15.9 | VtableEntry interning (fn_name: String → Spur + typed TraitError) | +N rust | ✅ |
| 15.10 | SubstsRef Vec<Ty> → Rc<[Ty]> (eliminate per-generic-app heap alloc) | +7 rust | ✅ |
| 15.11 | Const.ty Box<Ty> → Ty (eliminate per-Const heap alloc) | +N rust | ✅ |
| 15.12 | Error system cleanup (remove MirBody.lower_type_errors; lowering returns 3-tuple) | +N rust | ✅ |
| 15.13-15.18 | DiagnosticBuilder + ErrorCode catalog + Spanned trait + colored output + CLI migration | +N rust | ✅ |
| 15.27 | TypeInterner wired into CompileResult | +N rust | ✅ |
| 15.28 | Thread-local TypeInterner activated (automatic Ty dedup) | +N rust | ✅ |
| 15.29 | Ty interner integration tests + inference var from_kind_raw bypass | +7 rust | ✅ |
| 15.30 | HP-22: dyn_trait_call field moved into TerminatorKind::Call | +N rust | ✅ |
| 15.31 | HP-22 doc cleanup | 0 | ✅ |
| 15.32 | region_inference.rs dead code documentation cleanup | 0 | ✅ |
| 15.33 | v0.159 milestone review (Phase 1 core complete, Phase 2 plan) | 0 | ✅ |
| 15.34 | NLL fixpoint design doc (Phase 2 Task 7 start) | 0 | ✅ |
| 15.35 | NLL fixpoint `compute_liveness` (Phase 2 Task 7 step 1 of 4) — backwards dataflow + `successors` helper + 21 unit + 13 integration tests | +34 rust (21 unit + 13 integration) | ✅ |
| 15.36 | `kill_expired_borrows_dataflow` + `check_mir_body_with_dataflow` (Phase 2 Task 7 step 2 of 4) — `compute_live_after_point` + `active_ref_locals` + 9 unit + 13 integration tests | +22 rust (9 unit + 13 integration) | ✅ |
| 15.37 | Legacy `check_mir_body` deprecated (§23.1 rule 6); driver switch DEFERRED due to GAP-1 semantic conflict (112 conformance tests would regress); dataflow path retained for future migration; 9 integration tests document the conflict | +9 rust (integration) | ⚠️ PARTIAL |
| 15.38 | Borrow-check comparison diagnostic tool — compares legacy vs dataflow on all 5216 conformance files; found 112 LEGACY-STRICTER (GAP-1) + 1 DATAFLOW-STRICTER (false positive); reconciliation design doc recommends Option B (lexical lifetimes + "was ever read" check); 4 new tests | +4 rust (3 unit + 1 integration) | ✅ |
| 15.39 | Option B implementation — `compute_ever_read` + modified `kill_expired_borrows_dataflow` preserves GAP-1 in dataflow path; LEGACY-STRICTER dropped 112 → 0; 1 known limitation (`&mut self` false positive in loops) documented; 14 new tests | +14 rust (5 unit + 9 integration) | ✅ |
| 15.40 | Kill-on-redefinition + last-use-based kill + driver switch — false positive FIXED (DATAFLOW-STRICTER 1 → 0); driver now uses `check_mir_body_with_dataflow`; both paths agree on all 5028 comparable tests; 8 new tests | +8 rust (integration) | ✅ |
| **15.41** | **Legacy delegation cleanup — `check_mir_body` now delegates to `check_mir_body_with_dataflow`; `kill_expired_borrows` (legacy walk) removed as dead code; `compute_last_use_map` retained (part of dataflow path); 7 new tests** | **+7 rust (integration)** | ✅ |
| **15.42** | **Drop elaboration design doc (Task 8, HP-12) — design alignment per §13.4; covers `needs_drop` analysis, drop insertion, drop glue codegen; 6-stage implementation plan (15.42-15.47)** | **0 (doc only)** | ✅ |
| **15.43** | **`ty_needs_drop` analysis — new `src/mir/drop_elaboration.rs` module; recursive type traversal with cycle detection; 16 unit + 3 integration tests** | **+19 rust (16 unit + 3 integration)** | ✅ |
| **15.44** | **`elaborate_drops` pass — MIR-to-MIR transformation inserting `Drop` terminators before `StorageDead` for needs-drop locals; block splitting; 2 unit + 3 integration tests (currently no-op — no Drop impls exist yet)** | **+5 rust (2 unit + 3 integration)** | ✅ |
| **15.45** | **Drop glue codegen — `TerminatorKind::Drop` no longer no-op; emits `call void @drop_adt_<N>(...)` for ADT types, `drop_generic` for others; code path not yet exercised (no Drop terminators generated until Stage 15.46)** | **0 (code change, no new tests — path not exercised)** | ✅ |
| **15.46** | **Drop elaboration integration — `elaborate_drops` wired into driver pipeline (after typeck, before borrowck); 3 integration tests verify no regression; pass is currently no-op (no Drop impls exist yet)** | **+3 rust (integration)** | ✅ |
| **15.47** | **Drop elaboration gate review + deep review (Task 8 closure) — §25 8-dimension review; Task 8 PARTIALLY COMPLETE (infrastructure ready, `impl Drop` parser support deferred); 0 new tests (review only)** | **0 (review only)** | ✅ |
| **15.48** | **Region allocation design doc (Task 9, HP-5) — design alignment per §13.4; covers lifetime elision, MIR region assignment, constraint collection, error reporting; 5-stage implementation plan (15.48-15.52)** | **0 (doc only)** | ✅ |
| **15.49** | **Lifetime elision + MIR region assignment — new `lower_hir_ty_to_mir_ty_with_regions` function; each `&T` gets a fresh `Region::Var(vid)` instead of `Region::Erased`; zero regression** | **0 (code change, no new tests — infrastructure)** | ✅ |
| **15.50** | **Constraint collection from MIR — new `collect_mir_constraints` method on `RegionInferenceContext`; collects outlives constraints from `r = &x`, `r = Copy(x)`, `call f(&x)`; wired into `run_region_inference`; zero regression** | **0 (code change, no new tests — infrastructure)** | ✅ |
| **15.51** | **Error reporting + integration — `LifetimeError` variant added to `BorrowErrorKind`; `run_region_inference` converts `RegionInferenceError` to `BorrowError`; zero regression (no false positives)** | **0 (code change, no new tests — infrastructure)** | ✅ |
| **15.52** | **Region allocation gate review (Task 9 closure) — 6 integration tests verify no false positives on ref patterns (simple, multiple, fn params, fn return, loop, struct); Task 9 PARTIALLY COMPLETE (infrastructure integrated, simplified constraints)** | **+6 rust (integration)** | ✅ |
| **15.53** | **Closure redesign design doc (Task 10, HP-3) — design alignment per §13.4; covers Strategy A (synthesized `call` function), fat pointer representation, Fn/FnMut/FnOnce; 6-stage implementation plan (15.53-15.58)** | **0 (doc only)** | ✅ |
| **15.54** | **v0.2 Phase 2 milestone review — §25 8-dimension review; Phase 2 SUBSTANTIALLY COMPLETE; 131 new tests, 5 design docs, 1 diagnostic tool; remaining work (closure impl, impl Drop, region precision) deferred to v0.3** | **0 (review only)** | ✅ |
| **15.55** | **Phase 3 design alignment — task readiness assessment; Task 13 (impl Drop + RAII) selected as first Phase 3 task; 5-stage implementation plan (15.55-15.59)** | **0 (doc only)** | ✅ |
| **15.56** | **impl Drop parser investigation — parser already supports `impl Drop for T` (Stage 5.5); TraitResolver already collects Drop impls; crash in codegen because `drop_adt_<N>` function not emitted; remaining work is drop glue function emission (Stage 15.57)** | **0 (investigation only)** | ✅ |
| **15.57** | **Drop glue function emission — new `emit_drop_glue_functions` in `src/codegen/mod.rs`; iterates TraitResolver for Drop impls, emits `drop_adt_<DefId>` calling `landin_<Type>_drop`; zero regression** | **0 (code change, no new tests — path not yet exercised by conformance)** | ✅ |
| **15.58** | **impl Drop conformance + integration tests — 3 integration tests verify no-Drop programs compile cleanly; known limitation: impl Drop programs crash in codegen (DefId mismatch — deferred)** | **+3 rust (integration)** | ✅ |
| **15.59** | **impl Drop gate review (Task 13 closure) — §25 8-dimension review; Task 13 PARTIALLY COMPLETE (infrastructure done, DefId mismatch fix deferred); known limitation documented with root cause + 1-line fix** | **0 (review only)** | ✅ |
| **15.60** | **DefId mismatch fix — fixed `emit_drop_glue_functions` to use type's DefId (not impl's); crash persists (additional root cause in elaborate_drops or codegen); fix retained (correct); crash investigation deferred** | **0 (code change, no new tests — crash not yet fixed)** | ✅ |
| **15.61** | **impl Drop end-to-end fix — FOUR root causes resolved: (1) elaborate_drops infinite loop (OOM 137) — StorageDead no longer carried into new block; (2) Drop codegen type mismatch — pass OpaquePtr not value type; (3) LLVM backend missing drop glue — added emit_drop_glue_functions call; (4) borrowck treated Drop as read — now treats as destructor (no-op for moved, consuming for live). Task 13 COMPLETE — impl Drop programs compile, link, and run correctly (verified with 3 runtime tests)** | **+8 rust (integration e2e)** | ✅ |
| **15.62** | **Drop order + double-drop prevention — (1) StorageDead emission reversed to reverse declaration order, matching Rust RFC 1327 dropck semantics; (2) collect_moved_locals flow-insensitive analysis scans MIR for Operand::Move and skips moved locals in elaborate_drops, preventing double-drop of temporaries. Runtime verified: `let a,b,c` produces "dropping 3, 2, 1" (no duplicates). Task 13 fully complete with correct Rust-matching semantics** | **+8 rust (integration)** | ✅ |
| **15.63** | **Recursive drop (fields with Drop) — emit_drop_glue_functions rewritten to iterate ALL types needing drop (not just types with impl Drop). For types WITH impl Drop: calls user's drop + recursively drops fields. For types WITHOUT impl Drop but with Drop fields: recursively drops fields via GEP + call. Fixes link error for structs like `Outer { inner: Inner }` where only Inner has Drop. 8 integration tests, 3-level nesting verified** | **+8 rust (integration)** | ✅ |
| **15.64** | **Struct literal Copy→Move + field-copy drop prevention — (1) Struct literal now uses Operand::Move for non-Copy field types (was always Copy), preventing double-drop of non-Copy field temporaries; (2) collect_field_copy_locals function finds temps assigned from Copy(Projection(...)) and excludes them from drop (field-copy temps are views, not owned values); (3) Added shared is_mir_ty_copy_conservative helper in mir::ty (DRY — replaces inline checks in let bindings + struct literals). Runtime verified: 4 drops → 2 drops (correct). 8 integration tests** | **+8 rust (integration)** | ✅ |
| **15.65** | **HP-22 cleanup — removed legacy dyn_trait_calls side-table from MirBody, removed legacy codegen_dyn_trait_call function, removed legacy codegen dispatch path (magic Error+Int(index) marker). dyn Trait call info now solely on TerminatorKind::Call's dyn_trait_call field (Stage 15.30). 6 test files updated to use codegen_dyn_trait_call_direct + verify via terminator field. Task 16 COMPLETE** | **-1 rust (merged test, net 0 new)** | ✅ |
| **15.66** | **Recursive drop for enums (SwitchInt in drop glue) — emit_drop_glue_functions now handles AdtLayout::Enum. For enums with drop-variant payloads: loads discriminant, emits SwitchInt to dispatch to active variant's block, GEPs to payload fields, calls drop_adt_<fieldDefId>. Runtime verified: enum with impl Drop + Drop variant produces "enum dropped" then "inner dropped" (correct order). 8 integration tests, Task 13 drop semantics fully complete for structs AND enums** | **+8 rust (integration)** | ✅ |
| **15.67** | **True Rust NLL (reject GAP-1 compromise) — Per §1.0 原則 9 "正确 > 妥协" (v3.24): removed the `ever_read` guard from kill_expired_borrows_dataflow (true liveness-based NLL, not last-use+ever_read). Fixed `&mut self` false positive via kill-after-call semantics (temps consumed by calls have their borrows killed). Added block-entry kill + StorageLive/StorageDead handling in liveness. Flipped 108 conformance tests from compile_error to compile_ok (valid NLL programs now accepted). 7 integration + 2 lib tests updated. GAP-1 Option B REJECTED as design decision** | **0 (108 conformance flipped, 9 rust updated)** | ✅ |
| **15.68** | **Remove dead NLL code — removed `compute_last_use_map` function + `LastUseMap` type alias + `compute_ever_read` function + 5 unit tests + 3 integration tests. Per §1.0 原則 5 "去除兼容思维" and §15 "最优 > 最小": dead code from the GAP-1 compromise removed. True Rust NLL uses liveness-based kill exclusively** | **-8 rust (5 lib + 3 integration, removed dead tests)** | ✅ |
| **15.69** | **v0.2 milestone gate review — comprehensive review of all 68 stages (15.1-15.68) across Phase 1-4. Assessed 20 tasks: 8 COMPLETE, 2 PARTIAL, 3 READY, 5 BLOCKED/DEFERRED, 2 DESIGN-ONLY. 5/8 success criteria met. v0.2 SUBSTANTIALLY COMPLETE — remaining: Task 12 (Lifetime elision) or Task 20 (Box<T>) for release. No code change (review-only stage)** | **0 (review only)** | ✅ |
| **15.70** | **Box<T> in prelude (Task 20) — registered Box as builtin prelude type (DefId sentinel) in build_module_tree. User-defined struct Box shadows builtin (no conflict). Changed resolve_crate signature &Rodeo → &mut Rodeo for interner.get_or_intern. Updated 7 test files. Box type annotations now resolve without user definition. Full Box<T> support (heap alloc, Deref, Drop) deferred to v0.3 (needs monomorphization)** | **0 (signature change, 7 test files updated)** | ✅ |
| **15.71** | **fn_sigs integration for region inference — added fn_sigs field to BorrowChecker + with_fn_sigs constructor (without resolver, backward compat). Added collect_mir_constraints_with_sigs to RegionInferenceContext. Driver now passes fn_sigs for proper call-argument region constraints (instead of simplified 'static fallback). Sound Copy detection (HP-1) still deferred to v0.3** | **0 (no new tests — infrastructure improvement)** | ✅ |
| **15.72** | **Remove deprecated borrowck code — removed deprecated `check_mir_body` method alias, `check_mir_body` free function, `check_crate` free function (§16-violating HIR re-lowering). Updated 14 test files to use `check_mir_body_with_dataflow` directly. Removed `#[allow(deprecated)]` attributes. Per §1.0 原則 5 "去除兼容思维"** | **0 (no new tests — cleanup)** | ✅ |
| **15.73** | **Type propagation for let bindings + Move-of-Copy fix — (1) let bindings without annotation now use init expression's type (fixes struct/enum move errors); (2) borrow checker skips recording moves for Copy types (Move of Copy = no-op). 4 conformance tests flipped compile_ok → compile_error (method-not-found now correctly caught). 1 lib test updated** | **0 (4 conformance flipped, 1 lib updated)** | ✅ |
| **15.74** | **Remove duplicate Copy detection (DRY) — removed `is_capture_ty_copy` from `src/mir/lower/expr_operand.rs`. Closure capture code now uses shared `is_mir_ty_copy_conservative` from `mir::ty` (Stage 15.64). Per §23 rule 5 (DRY) and §1.0 原則 5 "去除兼容思维"** | **0 (no new tests — cleanup)** | ✅ |
| **15.75** | **Deref expression type resolution — `lower_deref_expr` now resolves result type from inner local's reference type (`&T` → `T`), instead of creating fresh Infer type. Same pattern as Stage 15.73 (let binding type propagation). Per §1.0 原則 3 "显式 > 隐式". v0.200.0 milestone (200 versions!)** | **0 (no new tests — improvement)** | ✅ |
| **15.76** | **Binary/Unary op type resolution — comparison ops (`==`, `!=`, `<`, `>`, `<=`, `>=`) result type is `Bool`; arithmetic ops result type is lhs operand's type; unary ops result type is inner operand's type. Avoids fresh Infer types that stay unresolved at borrowck time. Same pattern as Stages 15.73/15.75** | **0 (no new tests — improvement)** | ✅ |
| **15.77** | **AddrOf + Tuple type resolution — `&expr`/`&mut expr` result type is `Ref(Erased, inner.ty, mut)`; `(a, b, c)` result type is `Tuple([a.ty, b.ty, c.ty])`. Avoids fresh Infer types at borrowck time. 7 conformance tests flipped compile_ok → compile_error (soundness fix — tuple element types now correctly checked against declared element types, exposing int-literal-can't-unify-with-f64/bool/char/&str and tuple arity mismatches). Same pattern as Stages 15.73/15.75/15.76. Per §1.0 原則 3, 9** | **0 (7 conformance flipped)** | ✅ |
| **15.78** | **Array length unify fix + conformance error test audit — `unify_resolved` Array arm now compares length Const values (was: silently ignored). Audited 416 compile_error tests per user directive. Soundness fix: `let x: [i32; 3] = [1, 2];` now correctly caught. 4 conformance tests flipped compile_ok → compile_error (2 array-length-mismatch + 2 empty-array-as-field-initializer). 3 new unit tests added. Per §1.0 原則 4 "报错 > 静默" + §1.0 原則 9 "正确 > 妥协"** | **+3 rust (224 lib), 4 conformance flipped** | ✅ |
| **15.79** | **Parser `mut name: Type` mis-parse fix + param mutability propagation — (1) `is_self_param` check in `parse_params` previously matched ANY param starting with `KwMut`, silently renaming `mut n: i32` to `mut self: i32`. Fixed: now requires `KwMut` followed by `KwSelf_`. (2) MIR lowerer param locals always used `new_local` (Immutable); now uses `new_local_with_mut` with `pat_mutability` (symmetric with `let mut` lowering). 4 conformance tests flipped compile_error → compile_ok (fib e2e: count_digits, reverse_num, gcd, collatz). 2 new regression tests. Per §1.0 原則 4 "报错 > 静默" + §1.0 原則 3 "显式 > 隐式" + §1.0 原則 6 "通用 > 特例"** | **+2 rust (2132 integration), 4 conformance flipped** | ✅ |
| **15.80** | **Error system cleanup: human-readable type names + remove Debug enum leaks — (1) Added `type_to_string` / `type_kind_to_string` helpers in `src/mir/ty.rs` that format `Ty` / `TyKind` as human-readable strings (e.g., `i32`, `&mut bool`, `[i32; 10]`, `(i32, bool)`, `{integer}`, `_`) instead of Debug format (e.g., `Int(I32)`, `Infer(IntVar(IntVid(0)))`). (2) Replaced `{:?}` Debug formatting in 6 user-facing error message sites (typeck::error::mismatch, typeck::checker x3, driver.rs::to_diagnostics typeck notes). (3) Removed `({:?})` enum variant name leak from borrowck errors (driver.rs::format_for_user + to_diagnostics). 8 new unit tests for `type_to_string`. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+8 rust (232 lib), 0 conformance changes** | ✅ |
| **15.81** | **Typeck error span accuracy fix — (1) Added `operand_span` helper in `src/typeck/checker.rs` that extracts source span from `Operand` (via `Place.span`). (2) Fixed 7 typeck error sites that used `Span::DUMMY` (file start "1:1") to use actual source spans: SwitchInt discriminant mismatch, SwitchInt "expected integer or bool", Assert "assert condition must be bool", Call arity error (term.span), Call arg/dest unify errors (4 sites, term.span), Call "expected function" (operand_span(func), 2 sites). (3) Fixed last `{:?}` Debug leak in SwitchInt error message. 3 new integration tests verify exact byte offsets. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+3 rust (2135 integration), 0 conformance changes** | ✅ |
| **15.82** | **infer_rvalue span accuracy + remaining Debug leaks — (1) Added `stmt_span: Span` parameter to `infer_rvalue` (was: no span access, all errors used Span::DUMMY). (2) Used stmt_span in 8 error sites inside infer_rvalue: BinaryOp comparison/bitwise/shift/arithmetic unify+type errors, BinaryOp2 range error, UnaryOp Not/Neg type errors. (3) Used stmt.span in check_statement Assign coercion unify error. (4) Replaced 5 `{:?}` Debug leaks with `type_kind_to_string` (shift count, arithmetic x2, not, neg). Completes the 3-stage error system cleanup (15.80-15.82): 18 Span::DUMMY sites + 14 {:?} leaks fixed. 3 new integration tests. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+3 rust (2138 integration), 0 conformance changes** | ✅ |
| **15.83** | **AggregateKind (Array + Adt) span accuracy fix — Fixed last 2 Span::DUMMY error sites in infer_rvalue: AggregateKind::Array (array element type mismatch, e.g. `[1, true, 3]`) and AggregateKind::Adt (struct field type mismatch, e.g. `S { x: true }` where x is i32). Uses stmt_span override pattern. 2 new integration tests. Completes the 4-stage error system cleanup (15.80-15.83): 20 Span::DUMMY sites + 14 {:?} leaks fixed. Per §1.0 原則 4 "报错 > 静默"** | **+2 rust (2140 integration), 0 conformance changes** | ✅ |
| **15.84** | **Borrowck Debug format leak fix + region_vid_to_string — (1) Added `region_vid_to_string(vid) -> String` helper in `src/mir/ty.rs` that formats `RegionVid(N)` as `'rN` (matches Rust convention). (2) Fixed 3 {:?} Debug leaks in borrowck error messages: lifetime error RegionEscapesUniversal (`region {:?}` → `region 'rN`), lifetime error TypeTestFailed (`type {:?}` → `type T`), NotCopy error (`use of moved value: {:?}` → `use of moved value: T`). 1 new unit test. Completes the 5-stage error system cleanup (15.80-15.84): 20 Span::DUMMY + 17 {:?} fixed across typeck AND borrowck. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+1 rust (233 lib), 0 conformance changes** | ✅ |
| **15.85** | **Borrowck check_terminator span accuracy fix — (1) Added `operand_span` helper to `BorrowChecker` (mirrors typeck Stage 15.81 pattern). (2) Fixed 4 Span::DUMMY sites in check_terminator: Call func + args, SwitchInt discr, Assert cond — all now use `Self::operand_span(op)` for accurate error spans. 1 new unit test. Completes the 6-stage error system cleanup (15.80-15.85): 24 Span::DUMMY + 17 {:?} fixed. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+1 rust (234 lib), 0 conformance changes** | ✅ |
| **15.86** | **DRY refactor: unify operand_span into mir::place — Moved the 2 duplicate `operand_span` private methods (on TypeChecker + BorrowChecker) into a single shared `pub fn operand_span` in `mir::place` (the module that defines `Operand`). Updated all 8 callers to use `crate::mir::place::operand_span`. Pure refactor — no behavior change. 1 new unit test in `mir::place`. Per §1.0 原則 5 "去除兼容思维" + §23 rule 5 (DRY) + §14.4 (重构即架构设计)** | **+1 rust (235 lib), 0 conformance changes** | ✅ |
| **15.87** | **Resolve error span accuracy fix — Fixed `scan_ty_for_unresolved` in `src/driver.rs`: "cannot find type in this scope" error now uses `p.span` (type path span) instead of `Span::DUMMY` (was: "1:1" for type resolution errors like `let x: Undefined = 42;`). 1 new integration test. Completes the 8-stage error system cleanup (15.80-15.87): 25 Span::DUMMY + 17 {:?} + 1 DRY fixed across typeck AND borrowck AND resolve. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+1 rust (2141 integration), 0 conformance changes** | ✅ |
| **15.88** | **MIR lowerer Debug leak fix + hir_expr_kind_to_string — (1) Added `hir_expr_kind_to_string(kind) -> &'static str` helper in `src/hir/kinds.rs` that formats `HirExprKind` as human-readable labels (e.g., "literal", "function call", "range expression") instead of Debug format. (2) Fixed 3 {:?} Debug leaks in MIR lowerer error messages: "no method found" (recv_ty.kind → type_kind_to_string), "for-loop only supports Range" (iter.kind → hir_expr_kind_to_string), "array repeat count" (count.kind → hir_expr_kind_to_string). 1 new unit test + 1 new integration test. Completes the 9-stage error system cleanup (15.80-15.88): 25 Span::DUMMY + 20 {:?} + 1 DRY fixed across typeck+borrowck+resolve+MIR lowerer. Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+2 rust (236 lib + 2142 integration), 0 conformance changes** | ✅ |
| **15.89** | **Trait error span accuracy fix — Added `span: Span` field to ImplInfo/CoherenceError/IncompleteImpl structs, populated from HirImpl.span during collect. Updated to_diagnostics to use trait error span instead of Span::DUMMY. Fixed last Span::DUMMY error category: trait coherence errors ("conflicting implementations") and incomplete impl errors ("missing method") now point to the actual impl block (was: "1:1"). Updated 16 test ImplInfo constructions across 4 test files. 2 new integration tests. Completes the 10-stage error system cleanup (15.80-15.89): 27 Span::DUMMY + 20 {:?} + 1 DRY fixed across ALL error categories (typeck+borrowck+resolve+MIR lowerer+trait). Per §1.0 原則 3 "显式 > 隐式" + §1.0 原則 4 "报错 > 静默"** | **+2 rust (2144 integration), 0 conformance changes** | ✅ |
| **15.90** | **Lifetime elision rule 2 (Task 12 start) — Implemented RFC 141 elision rule 2: if a function has exactly one input lifetime, it's assigned to all elided output lifetimes. Added `collect_region_vids` + `apply_elision_rule_2` helpers in `src/mir/lower/mod.rs`. Reordered param-before-return lowering so elision can apply. Param types lowered once and reused (ensures region vids match). 5 new unit tests. First stage of Task 12 (Lifetime elision) — the last remaining P1 task for v0.2. Per §1.0 原則 3 "显式 > 隐式" + §23** | **+5 rust (241 lib), 0 conformance changes** | ✅ |
| **15.91** | **Lifetime elision rule 3 (self param) — Implemented RFC 141 elision rule 3: if there are multiple input lifetimes but one is &self/&mut self, that lifetime is assigned to all elided output lifetimes. Unified `apply_elision_rule_2` into `apply_elision_rules` (handles both rules 2+3). Added `self_region_vid` tracking: when &self/&mut self param is encountered, its region vid is collected and passed for rule 3. 1 new unit test + 3 updated tests. Completes elision rules 1-3 (RFC 141). Per §1.0 原則 3 "显式 > 隐式" + §23** | **+1 rust (242 lib), 0 conformance changes** | ✅ |
| **15.92** | **Explicit lifetime tracking — Added `lower_hir_ty_to_mir_ty_with_lifetimes` function that deduplicates explicit lifetimes: references with the same lifetime name (e.g., `&'a`) share the same RegionVid, instead of each getting a fresh vid. Uses a `HashMap<Symbol, RegionVid>` (lifetime_map) created per body. Non-Ref types delegate to existing `lower_hir_ty_to_mir_ty_with_regions`. 2 new unit tests (dedup + no-dedup). Foundation for region inference activation. Per §1.0 原則 3 "显式 > 隐式" + §23** | **+2 rust (244 lib), 0 conformance changes** | ✅ |
| **15.93** | **Region inference return value constraints — Added return value region constraint collection in `collect_mir_constraints_with_sigs`: when a call `dest = f()` returns `&T`, the destination's region gets an outlives constraint from the callee's return type's region (`ret_r: dest_r`). Closes the last missing constraint category (borrow + copy + call args + call return all collected now). Task 12 SUBSTANTIALLY COMPLETE. Per §1.0 原則 4 "报错 > 静默"** | **0 (no new tests), 0 conformance changes** | ✅ |
| **15.94** | **Lifetime elision + region inference conformance tests — Added 8 conformance tests verifying lifetime elision rules 2/3, explicit lifetime dedup, and various lifetime patterns end-to-end. Closes the test gap for Stages 15.90-15.93 implementations. Per user directive: simplified implementations must have complete test coverage.** | **0 rust, +8 conformance (5224 total)** | ✅ |
| **15.95** | **v0.2 FINAL GATE REVIEW — Comprehensive review of all 94 stages (15.1-15.94). 10/20 tasks COMPLETE, 8 deferred to v0.3. 6/8 success criteria met (was 5/8). Criterion 2 (Lifetimes enforced) upgraded to ✅ Met. Committee Vote: GO — v0.2 RELEASE APPROVED.** | **0 (review only)** | ✅ |
| **15.96** | **Deep audit: trait error Debug fallback fix — Added `TraitError::format_without_interner()` method for human-readable error messages when interner is None (test contexts). Fixed 2 remaining {:?} Debug fallback sites in `format_for_user` + `to_diagnostics` (driver.rs). Per user directive: audit simplified implementations for completeness. Per §1.0 原則 4 "报错 > 静默"** | **0 (no new tests), 0 conformance changes** | ✅ |
| **15.97** | **Pipeline coverage audit — Comprehensive audit of ALL MIR IR enum variants (Rvalue 7/7, TerminatorKind 7/7, Operand 3/3, StatementKind 6/6, AggregateKind 4/4, PlaceKind 3/3, ProjectionElem 5/5, BinOp 16/16). All variants covered by codegen. User misuse: 409 compile_error tests across 5 categories. Error system: all Debug leaks fixed, all spans accurate. Pipeline coverage: COMPLETE.** | **0 (review only)** | ✅ |
| **15.98** | **Region inference all-pairs matching — Replaced 3 "first-to-first" region matching sites with all-pairs matching in `collect_mir_constraints_with_sigs` (Copy/Move propagation, call args, call return). Fixes systematic simplification: types with multiple references (e.g., `&(&a T, &b U)`) now get complete constraints. Per user directive: audit simplified implementations for completeness. Per §1.0 原則 9 "正确 > 妥协".** | **0 (no new tests), 0 conformance changes** | ✅ |
| **15.99** | **Sound Copy detection infrastructure — Added `BorrowChecker::with_resolver_and_sigs()` constructor combining resolver (sound Copy) + fn_sigs (region inference). Tested enabling: 199 test failures (expected — tests need `impl Copy` migration). Reverted to `with_fn_sigs` for v0.2 compat. Sound path ready for v0.3. Per §1.0 原則 9 "正确 > 妥协".** | **0 (infrastructure only), 0 conformance changes** | ✅ |
| **Total Stage 15** | **v0.2 COMPLETE + systematic audit: 10/20 tasks COMPLETE, v0.2 RELEASE APPROVED. Pipeline coverage COMPLETE (51/51 enum variants). Error system COMPLETE (50 sites). Region inference all-pairs matching. Sound Copy detection infrastructure ready (v0.3 migration). 7612 tests, 0 failures, 0 warnings. v0.224.0!** | **+202 rust in 15.35-15.99, +8 conformance** | ✅ |

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
