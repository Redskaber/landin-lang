# LLVM Codegen Design

> **Author**: redskaber
> **Date**: 2026-07-19
> **Version**: v0.7
> **Status**: Active

## Architecture

```text
MIR → [codegen translation] → Emitter trait → [backend impl]
                                              ├─ TextEmitter (.ll text)
                                              ├─ InkwellEmitter (future)
                                              └─ CraneliftEmitter (future)
```

The translation layer walks MIR and calls `Emitter` methods. The backend
implements those methods. Switching backends only requires implementing
the `Emitter` trait — no changes to the translation layer.

## Supported Features

| Feature | LLVM IR | Status |
|---------|---------|--------|
| Function definition | `define i32 @fn_N(i32 %arg0)` | ✅ |
| Parameters | `store i32 %arg0, %loc_1` | ✅ |
| Return | `ret i32 %v` | ✅ |
| Arithmetic | `add/sub/mul/div nsw i32` | ✅ |
| Comparison | `icmp eq/ne/slt/sgt` + `zext i1` | ✅ |
| Unary | `sub 0, x` (neg), `xor x, -1` (not) | ✅ |
| Variables | `alloca`, `store`, `load` | ✅ |
| Control flow | `br i1`, `br label` | ✅ |
| Match (int) | `switch i32` | ✅ |
| Borrow | `store i32* %loc_x` | ✅ |
| Deref | `load i32, %ptr` (double load) | ✅ |
| Function calls | `call i32 @fn_N(i32 %arg)` | ✅ |
| Cast | `sext/trunc/sitofp/fptosi/fpext/fptrunc` | ✅ |
| Float | `double` constants | ✅ |
| Assert | `br` (simplified, no real overflow check) | 🔄 |

## Emitter Trait

```rust
pub trait Emitter {
    fn begin_function(&mut self, name: &str, params: &[(EmitType, &str)], ret: EmitType);
    fn end_function(&mut self);
    fn emit_constant(&mut self, val: &ConstVal) -> EmitValue;
    fn emit_binary_op(&mut self, op: BinOp, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_unary_op(&mut self, op: UnOp, ty: EmitType, operand: &EmitValue) -> EmitValue;
    fn emit_return(&mut self, ty: EmitType, val: Option<&EmitValue>);
    fn emit_alloca(&mut self, ty: EmitType, name: &str) -> EmitValue;
    fn emit_store(&mut self, ty: EmitType, val: &EmitValue, ptr: &EmitValue);
    fn emit_load(&mut self, ty: EmitType, ptr: &EmitValue) -> EmitValue;
    fn emit_branch(&mut self, label: &str);
    fn emit_cond_branch(&mut self, cond: &EmitValue, then_label: &str, else_label: &str);
    fn begin_block(&mut self, label: &str);
    fn emit_call(&mut self, fn_name: &str, args: &[EmitValue], ret_ty: EmitType) -> EmitValue;
    fn emit_icmp(&mut self, op: &str, ty: EmitType, lhs: &EmitValue, rhs: &EmitValue) -> EmitValue;
    fn emit_zext_i1_to_i32(&mut self, val: &EmitValue) -> EmitValue;
    fn emit_switch(&mut self, discr: &EmitValue, cases: &[(i128, String)], default_label: &str);
    fn emit_cast(&mut self, src: EmitType, dst: EmitType, val: &EmitValue) -> EmitValue;
    // ... local tracking methods
}
```

## Future Work

- Real overflow check via `llvm.sadd.with.overflow` intrinsic
- Switch to inkwell for JIT/AOT compilation
- Struct/enum codegen (getelementptr for field access)
- Closure codegen
- Proper SSA construction (currently using alloca/load/store pattern)
