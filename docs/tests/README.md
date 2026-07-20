# Test Documentation

> **Author**: redskaber
> **Date**: 2026-07-20
> **Process**: v3.12 (§17 测试矩阵全覆盖原则)

## Structure

```
docs/tests/
├── README.md           # This file
├── v0/                 # Major version
│   └── stage3/         # Stage
│       └── plan/       # Test plan documents
│           ├── codegen_basic.md
│           ├── codegen_overflow.md
│           ├── codegen_struct.md
│           └── codegen_enum.md
└── matrix.md           # Global test matrix
```

Each test document corresponds to a test code file under `tests/` and
is cross-referenced with `docs/develop/v0/stage-N/`.

See §17 of `docs/stage-committee-process.md` for the full specification.
