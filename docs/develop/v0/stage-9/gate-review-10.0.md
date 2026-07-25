# Stage 10.0 Gate Review — CLI upgrade + Runner upgrade

> **审查日期**: 2026-07-26 | **版本**: v0.17.1 → v0.17.2
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2

## CI/CD

```
cargo clean: clean
cargo test: 2255 passed (146 unit + 2109 integration, 0 failed, 2 ignored)
cargo fmt --check: clean
cargo clippy --all-targets: 0 warnings, 0 errors
python3 tests/conformance/run_all.py: 600 passed (mode=parse), 0 failed
```

## 完成内容

### CLI 升级 (GAP-03) ✅
- `--compile`: 完整编译 via `driver::compile()`
- `--emit-llvm-ir`: 输出 LLVM IR via `codegen::codegen_crate()`

### Runner 升级 (GAP-05) ✅
- `--mode parse` (default): 向后兼容 `--emit-ast`
- `--mode compile`: 使用 `--compile` 验证完整 pipeline
- 双格式支持: legacy `//!` + spec `//` (EXPECTED field)

### 格式迁移 (GAP-02) — 推迟到 Stage 10.1
Runner 双格式兼容, 无需立即迁移 600 .lin 文件

## 委员会投票

**5/5 GO → PASS**

## 下一阶段

- **Stage 10.1**: 01-typecheck conformance (1000 tests) + format migration

---

**审查完成**: 2026-07-26
