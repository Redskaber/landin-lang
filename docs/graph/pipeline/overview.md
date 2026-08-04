# Compiler Pipeline Overview

> **Date**: 2026-08-04
> **Version**: v0.232.0

## End-to-End Pipeline

```
Source Code (.lin)
    │
    ▼
┌─────────────────────────────────────────────────────────────┐
│  1. Lexer (lexer/)                                            │
│     src → Token stream                                        │
│     tokenize(src, &mut interner) → Vec<Token>                │
└─────────────┬────────────────────────────────────────────────┘
              │ Vec<Token>
              ▼
┌─────────────────────────────────────────────────────────────┐
│  2. Parser (parser/)                                          │
│     Token stream → AST                                        │
│     parse_crate(tokens, &mut interner) → AstCrate            │
└─────────────┬────────────────────────────────────────────────┘
              │ AstCrate
              ▼
┌─────────────────────────────────────────────────────────────┐
│  3. HIR Lowering (hir/lower/)                                 │
│     AST → HIR                                                 │
│     lower_crate(&ast) → HirCrate                              │
│     - Name resolution (resolve/)                             │
│     - Trait collection (traits/)                             │
└─────────────┬────────────────────────────────────────────────┘
              │ HirCrate
              ▼
┌─────────────────────────────────────────────────────────────┐
│  4. MIR Lowering (mir/lower/)                                 │
│     HIR → MIR (per body)                                      │
│     lower_hir_body_to_mir_full(body, hir) →                  │
│       (MirBody, UnificationTable, TypeErrors,                │
│        SynthesizedClosureFunctions)                           │
│     - Closure synthesis (build_synthesized_closure_mir_body) │
│     - AdtLayouts population                                   │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody + UnificationTable
              ▼
┌─────────────────────────────────────────────────────────────┐
│  5. Type Check (typeck/)                                      │
│     MIR + UnifyTable → resolved types                        │
│     TypeChecker::check_mir_body_with_tables(mir, field_ty)  │
│     - Iterative typeck for nested closures (Stage 16.32)     │
│     - FnSigTable population                                   │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (types resolved)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  6. Drop Elaboration (mir/drop_elaboration/)                 │
│     MIR → MIR (with Drop terminators)                        │
│     elaborate_drops(&mut mir, resolver, interner)            │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (with Drop terminators)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  7. Borrow Check (borrowck/)                                  │
│     MIR → BorrowErrors                                       │
│     BorrowChecker::check_mir_body_with_dataflow(&mir)        │
│     - Sound Copy detection (field-level derivation)          │
│     - NLL fixpoint liveness                                   │
│     - Region inference                                        │
└─────────────┬────────────────────────────────────────────────┘
              │ MirBody (validated)
              ▼
┌─────────────────────────────────────────────────────────────┐
│  8. Codegen (codegen/)                                        │
│     MIR → LLVM IR (text or LLVM module)                      │
│     codegen_crate(&CompileResult) → String (LLVM IR text)   │
│     codegen_crate_to_module(&CompileResult) → LLVMSysEmitter │
│     - Shared pipeline: codegen_from_mir → codegen_function   │
│     - Backend: TextEmitter (text) | LLVMSysEmitter (C-API)  │
└─────────────┬────────────────────────────────────────────────┘
              │ LLVM IR (text) or LLVM Module
              ▼
┌─────────────────────────────────────────────────────────────┐
│  9. Object Code / Execution                                   │
│     LLVM IR → Object file → Executable                       │
│     LLVMSysEmitter::to_object_file(path)                     │
│     Or: text IR → llc → object → cc → executable             │
└─────────────────────────────────────────────────────────────┘
```

## Driver Orchestration (driver.rs)

```
compile(src) → CompileResult
    │
    ├─ 1. tokenize(src)
    ├─ 2. parse_crate(tokens)
    ├─ 3. lower_crate(ast) + resolve + trait_resolver.collect
    ├─ 4. For each body in hir.bodies:
    │      ├─ lower_hir_body_to_mir_full(body, hir)
    │      ├─ Build closure MIR bodies (worklist, nested)
    │      ├─ Iterative typeck (closures + main body, max 4 passes)
    │      ├─ elaborate_drops(mir)
    │      └─ borrowck(mir)
    ├─ 5. codegen_crate(CompileResult) → LLVM IR text
    └─ 6. Return CompileResult (with errors, MIR, IR, etc.)
```

## CompileResult (data carried through pipeline)

```
CompileResult {
    mir: MirBody,                        // main body MIR
    synthesized_closure_mir_bodies: Vec<MirBody>,  // closure MIR bodies
    fn_name_by_def_id: HashMap<DefId, String>,
    fn_sig_table: FnSigTable,
    trait_resolver: TraitResolver,
    interner: Rodeo,
    adt_layouts: AdtLayouts,
    field_ty_table: FieldTyTable,
    errors: CompileErrors {
        lex: Vec<LexError>,
        parse: Vec<ParseError>,
        resolve: Vec<ResolveError>,
        typeck: Vec<TypeError>,
        borrowck: Vec<BorrowError>,
    },
}
```
