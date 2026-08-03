# Migration Tool: add_impl_copy.py

> **Location**: `tools/migration/add_impl_copy.py`
> **Purpose**: Automated `impl Copy` migration for v0.3 sound Copy detection

## Overview

This script scans `.lin` conformance test files for struct definitions
without `impl Copy for <Name> {}` and adds the impl block after the
struct definition.

## Usage

```bash
python3 tools/migration/add_impl_copy.py tests/conformance/
```

## What it does

1. Finds all `struct Name { ... }` or `struct Name;` definitions
2. Checks if `impl Copy for Name {}` already exists → skip
3. Checks if `impl Drop for Name {}` exists → skip (Copy+Drop conflict)
4. If neither, adds `impl Copy for Name {}` after the struct definition

## Why it's needed

The v0.2 compiler uses unsound `ty_is_copy` which treats ALL Adt types
as Copy. The v0.3 migration enables sound Copy detection via
`with_resolver_and_sigs`, which correctly checks `impl Copy for <Type>`.
Tests that use structs as Copy need `impl Copy` added.

Per §1.0 原則 9 "正确 > 妥协": sound Copy detection is the correct approach.
Per §23: tool stored in `tools/<sub_dirname>/` per project convention.
