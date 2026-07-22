# Examples Index

> **Process**: stage-committee-process.md v3.19 §17.4
> **Structure**: `usage/` (API demos, maintained) + `audit/` (historical gate review scripts, archived)

## Directory Structure

```
examples/
├── README.md                          ← this file (index)
├── usage/                             ← API usage demos (MUST compile with current API)
│   ├── struct_call_codegen.rs         ← compile() + codegen_crate() — struct + fn call
│   ├── struct_variants_codegen.rs     ← codegen_crate() — named/tuple structs
│   └── struct_compile_check.rs        ← compile() only — error count check
└── audit/                             ← historical stage gate audit scripts (archived)
    ├── cross_stage_audit.rs           ← cross-stage integration audit
    ├── round3_audit.rs … round6_audit.rs  ← round-by-round negative-case audits
    ├── stage2_4d_audit.rs             ← Stage 2.4d integration audit
    └── stage3_gate_audit.rs … stage3_gate_audit_r23.rs  ← Stage 3 gate reviews (R1-R23)
```

## Usage Examples (`usage/`)

These demos showcase the **current public API** and MUST compile. If an API
change breaks them, fix in the same round (per §17.4.2 rule 3).

| Example | Demonstrates | Run |
|---------|-------------|-----|
| `struct_call_codegen.rs` | `compile()` + `codegen_crate(&CompileResult)` | `cargo run --example usage/struct_call_codegen` |
| `struct_variants_codegen.rs` | `codegen_crate()` on named/tuple structs | `cargo run --example usage/struct_variants_codegen` |
| `struct_compile_check.rs` | `compile()` + `CompileResult.errors` inspection | `cargo run --example usage/struct_compile_check` |

### API Quick Reference (§16 compliant)

```rust
use landin_compiler::driver::compile;        // src → CompileResult
use landin_compiler::codegen::codegen_crate; // &CompileResult → LLVM IR string

let result = compile("fn main() {}");
let llvm_ir = codegen_crate(&result);        // single arg (since Stage 3.56)
println!("{}", llvm_ir);
```

## Audit Scripts (`audit/`)

Historical stage gate review scripts. These are **archived** — they may not
compile with the current API (audit scripts are frozen at the round they
were written for). Per §17.4.2 rule 4, they are kept as historical reference.

| Script | Round | Stage | Notes |
|--------|-------|-------|-------|
| `stage3_gate_audit.rs` | R1 | 3.1-3.22 | First Stage 3 gate audit (38 cases) |
| `stage3_gate_audit_r2.rs` … `r23.rs` | R2-R23 | 3.x | Stage 3 iterative gate reviews |
| `stage2_4d_audit.rs` | — | 2.4d | Stage 2.4d integration audit |
| `round3_audit.rs` | R3 | 2.x | Negative-case expansion (~30 cases) |
| `round4_audit.rs` | R4 | 2.x | Round 4 audit |
| `round5_audit.rs` | R5 | 2.x | Round 5 audit |
| `round5_deep.rs` | R5 | 2.x | Round 5 deep inspection |
| `round6_audit.rs` | R6 | 2.x | Round 6 audit |
| `cross_stage_audit.rs` | — | 0-3 | Cross-stage integration audit |

### Running Audit Scripts

```bash
# Run a specific audit script (may fail to compile if API has changed)
cargo run --example audit/stage3_gate_audit_r23
```

> **Note**: Audit scripts use the API as it was at the time of the round.
> If the public API has since changed (e.g., `codegen_crate` signature),
> these scripts may not compile. They are kept for historical reference
> only. For working API demos, use `usage/` examples above.

## Maintenance

- **Adding a new usage demo**: place under `examples/usage/`, ensure it
  compiles with the current API, add a row to the table above.
- **Adding a new audit script**: place under `examples/audit/`, named
  `stage<N>_gate_audit_r<R>.rs` per §17.4.3.
- **API change**: check all `usage/` examples compile; fix any breakage
  in the same round (per §17.4.2 rule 3).
- **Stage closure**: move that stage's audit scripts to `audit/` (already
  done for Stage 0-4); no further maintenance needed.

---

**Last updated**: 2026-07-22 (Stage 5.5 audit — examples/ restructured per v3.19 §17.4)
