# Agent Team Roles

> **Author**: redskaber
> **Date**: 2026-07-19
> **Version**: v0.1
> **Status**: Active

## Overview

The Landin compiler project uses a multi-agent team structure with 9 categories
and 25 agent roles. Each agent has specific responsibilities in the development
pipeline.

## Role Categories

| Category | Roles | Responsibility |
|----------|-------|----------------|
| Frontend | Lexer Engineer, Parser Engineer, AST Designer | Source → AST |
| HIR | HIR Architect, Name Resolution Specialist | AST → HIR + resolve |
| MIR | MIR Designer, MIR Lowering Engineer | HIR → MIR |
| Type System | Type Theorist, Type Inference Engineer | Type checking |
| Borrow Check | Ownership Analyst, NLL Specialist | Borrow checking |
| Codegen | LLVM Codegen Engineer, Backend Architect | MIR → LLVM IR |
| Testing | QA Lead, Test Engineer, Conformance Tester | Test coverage |
| Tooling | Build Engineer, CI/CD Specialist, DX Lead | Developer experience |
| Process | Stage Committee (5 roles) | Review & approval |

## Committee Roles (per §5.1)

| Role | Weight | Responsibility |
|------|--------|----------------|
| Compiler Engineer (Architect) | 2.0 | Technical direction |
| Soundness Reviewer | 1.5 | Code correctness |
| Testing & QA Lead | 1.0 | Test coverage |
| Type System Theorist | 1.0 | Type system semantics |
| Tooling & DX Lead | 1.0 | Developer experience |

Total weight: 5.5. Approval requires ≥95% (≥5.225 votes).
