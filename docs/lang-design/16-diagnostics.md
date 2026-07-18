# 16 — 诊断系统

> 本文定义 Landin 的统一诊断架构、错误代码注册表、suggestion 引擎、错误信息格式。v1.2 新增（R12 完备性审查建议）。

---

## 1. 诊断架构

### 1.1 诊断对象

```rust
struct Diagnostic {
    level: Level,
    code: Option<DiagnosticId>,        // E0xxx
    message: String,
    span: MultiSpan,                    // 可能跨多 span
    children: Vec<SubDiagnostic>,       // 关联诊断
    suggestions: Vec<Suggestion>,
    notes: Vec<String>,                 // = note: ...
}

enum Level {
    Error,          // E0xxx
    Warning,        // W0xxx
    Note,           // 仅作为其他 diagnostic 的 child
    Help,           // 仅作为其他 diagnostic 的 child
    Fatal,          // 立即终止编译
    Bug,            // ICE：internal compiler error
}

enum DiagnosticId {
    Error(ErroredCode),       // E0001-E1599
    Warning(WarningCode),     // W0001-W0199
    Lint(LintCode),           // unused, dead_code 等
}

struct MultiSpan {
    primary_spans: Vec<Span>,
    labels: Vec<(Span, String)>,
}

struct SubDiagnostic {
    level: Level,
    message: String,
    span: MultiSpan,
}

struct Suggestion {
    span: Span,
    replacement: String,
    label: String,
    applicability: Applicability,
}

enum Applicability {
    MachineApplicable,    // 可自动应用
    MaybeIncorrect,       // 可能不正确
    HasPlaceholders,      // 含 placeholder（如 _xxx）
    Unspecified,
}
```

### 1.2 诊断收集

所有 pass 共享一个 `DiagnosticBuffer`：

```rust
struct DiagnosticBuffer {
    diagnostics: Vec<Diagnostic>,
    error_count: usize,
    warning_count: usize,
    /// 编译终止阈值（默认 128）
    error_limit: usize,
    /// 是否把 warning 当 error（-D warnings）
    deny_warnings: bool,
}

impl DiagnosticBuffer {
    fn emit(&mut self, diag: Diagnostic) {
        match diag.level {
            Level::Error | Level::Fatal => self.error_count += 1,
            Level::Warning => self.warning_count += 1,
            _ => {}
        }
        self.diagnostics.push(diag);
        
        // 超过 error_limit 时停止
        if self.error_count >= self.error_limit {
            self.emit_fatal("too many errors, stopping");
        }
    }
}
```

### 1.3 诊断输出

诊断按 span 排序后输出（同 file 内按 line 排序，跨 file 按 file_id）。

---

## 2. 错误代码注册表

### 2.1 错误代码范围（v1.2 统一）

| 范围 | 类别 | 文档位置 |
| --- | --- | --- |
| E0001-E0499 | type system errors | 03 |
| E0500-E0699 | borrow check errors | 04 |
| E0700-E0899 | lifetime errors | 04 |
| E0900-E0999 | name resolution errors | 01 |
| E1000-E1099 | parse errors | 02 |
| E1100-E1299 | trait resolution errors | 03 |
| E1300-E1399 | codegen errors | 07 |
| E1400-E1499 | unsafe check errors | 14 |
| E1500-E1599 | coherence / orphan errors | 03 |
| E1600-E1699 | attribute errors | 15 |
| E1700-E1799 | macro errors | 02 |
| E1800-E1899 | stdlib errors | 09 |

### 2.2 警告代码范围

| 范围 | 类别 |
|---|---|
| W0001-W0099 | unused warnings（unused_imports, unused_variables, dead_code） |
| W0100-W0199 | style warnings（naming, formatting） |

### 2.3 MVP 必需错误代码（30-50 个）

#### Type system (E0001-E0499)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E0308 | mismatched_types | 期望类型与实际类型不匹配 |
| E0382 | use_of_moved_value | 使用了已 move 的值 |
| E0433 | failed_to_resolve | 路径未解析 |
| E0271 | type_mismatch | trait bound 不满足 |
| E0277 | trait_not_implemented | 类型未实现 trait |
| E0282 | type_annotations_needed | 需要类型注解 |
| E0283 | ambiguous_type | 类型歧义 |
| E0284 | type_not_implemented | 类型不可实现 |
| E0384 | cannot_assign_immutable | 不能赋值不可变变量 |
| E0381 | used_uninitialized | 使用未初始化变量 |

#### Borrow check (E0500-E0699)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E0500 | closure_unique_borrow | closure 需独占但已被借用 |
| E0502 | borrowed_as_mutable | 不能作为可变借用（已被不可变借用） |
| E0503 | used_while_mutably_borrowed | 使用了被可变借用的值 |
| E0505 | cannot_move_borrowed | 不能 move 被借用的值 |
| E0507 | cannot_move_closure_capture | 不能 move 闭包捕获的值 |
| E0515 | cannot_return_reference_to_local | 不能返回局部变量引用 |
| E0516 | cannot_borrow_local_with_unknown_lifetime | 局部变量 lifetime 未知 |
| E0521 | borrowed_data_escapes | 借用的数据逃逸 |
| E0594 | cannot_borrow_as_mutable | 不能作为可变借用（不可变） |
| E0596 | cannot_borrow_as_mut_in_loop | 循环中不能可变借用 |
| E0597 | does_not_live_long_enough | lifetime 不够长 |
| E0599 | no_method_named | 没有此方法 |

#### Lifetime (E0700-E0899)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E0704 | missing_lifetime_specifier | 缺少 lifetime 标注 |
| E0708 | lifetime_mismatch | lifetime 不匹配 |
| E0716 | temporary_value_borrowed_for_too_long | 临时值借用太久 |
| E0719 | reference_lifetime_too_short | 引用 lifetime 太短 |
| E0726 | implicit_lifetime_not_allowed | 不允许隐式 lifetime |

#### Name resolution (E0900-E0999)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E0901 | unresolved_import | 未解析的 import |
| E0902 | unresolved_name | 未解析的名称 |
| E0903 | unresolved_module | 未解析的 module |
| E0904 | private_item | 私有 item 不可访问 |
| E0905 | ambiguous_glob | glob import 歧义 |
| E0906 | unused_import | 未使用的 import（W0001 同时） |

#### Parse (E1000-E1099)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1001 | unexpected_token | 不期望的 token |
| E1002 | expected_token | 期望的 token 缺失 |
| E1003 | unclosed_delimiter | 未闭合的分隔符 |
| E1004 | invalid_literal | 无效字面量 |
| E1005 | invalid_attribute | 无效属性 |
| E1006 | invalid_lifetime | 无效 lifetime |

#### Trait resolution (E1100-E1299)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1101 | trait_not_satisfied | trait bound 不满足 |
| E1102 | conflicting_impls | 冲突的 impl |
| E1103 | orphan_rule_violation | 违反 orphan rule |
| E1104 | not_object_safe | trait 不是 object safe |
| E1105 | ambiguous_implementation | 歧义 impl |
| E1106 | reached_recursion_limit | 达到递归深度限制 |
| E1107 | cannot_find_implementation | 找不到 impl |
| E1108 | ambiguous_from_implementation | 歧义 From impl（`?` 操作符） |

#### Codegen (E1300-E1399)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1301 | unsupported_target | 不支持的目标平台 |
| E1302 | link_failure | 链接失败 |
| E1303 | invalid_abi | 无效 ABI |
| E1304 | unsupported_feature | 不支持的特性 |

#### Unsafe check (E1400-E1499)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1401 | unsafe_operation_in_safe_context | safe 上下文中执行 unsafe 操作 |
| E1402 | unused_unsafe | 不必要的 unsafe |
| E1403 | deref_null_pointer | 解引用 null 指针 |
| E1404 | data_race | 数据竞争（v0.2） |

#### Coherence (E1500-E1599)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1501 | coherence_conflict | coherence 冲突 |
| E1502 | overlapping_impls | 重叠 impl |

#### Attribute (E1600-E1699)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1601 | unknown_attribute | 未知属性 |
| E1602 | malformed_attribute | 属性格式错误 |
| E1603 | unknown_derive | 未知 derive trait |
| E1604 | attribute_position_error | 属性位置错误 |

#### Macro (E1700-E1799)

| 代码 | 名称 | 描述 |
| --- | --- | --- |
| E1701 | unknown_macro | 未知宏 |
| E1702 | macro_args_error | 宏参数错误 |
| E1703 | macro_rules_not_supported | macro_rules! 不支持（v0.2） |

---

## 3. 错误信息格式

### 3.1 标准格式

```
error[E0308]: mismatched types
  --> src/main.lin:10:5
   |
10 |     let x: i32 = "hello";
   |            ---   -------
   |            |     |
   |            |     expected `i32`, found `&str`
   |            expected due to this
   |
   = note: expected type `i32`
           found reference `&'static str`

error: aborting due to 1 previous error
```

### 3.2 多 span 格式

```
error[E0502]: cannot borrow `v` as mutable because it is also borrowed as immutable
  --> src/main.lin:5:5
   |
 3 |     let r = &v;
   |              - immutable borrow occurs here
 4 |     println!("{}", r);
 5 |     v.push(4);
   |     ^^^^^^^^^ mutable borrow occurs here
 6 |     println!("{}", r);
   |                   - immutable borrow later used here
   |
help: the borrow of `v` as immutable needs to be released before borrowing it as mutable
   |
 4 |     drop(r);
 5 |     v.push(4);
   |
```

### 3.3 含建议的格式

```
error[E0425]: cannot find value `xy` in this scope
  --> src/main.lin:8:9
   |
 8 |     println!("{}", xy);
   |                    ^^ help: a local variable with a similar name exists: `xyz`
```

### 3.4 含多个 child 诊断的格式

```
error[E0277]: the trait bound `T: Display` is not satisfied
  --> src/main.lin:5:5
   |
 5 |     print(t);
   |     ^^^^^  the trait `Display` is not implemented for `T`
   |
note: required by a bound in `print`
  --> src/main.lin:1:13
   |
 1 | fn print<T: Display>(t: T) { ... }
   |             -------  required by this bound in `print`
help: consider restricting type parameter `T`
   |
 1 | fn print<T: Display>(t: T) { ... }
   |          +
```

---

## 4. Suggestion 引擎

### 4.1 拼写纠正

对未解析的名称，用编辑距离找候选：

```rust
fn suggest_similar_names(name: &str, candidates: &[&str]) -> Vec<String> {
    candidates.iter()
        .map(|c| (c, levenshtein(name, c)))
        .filter(|(_, d)| *d <= 3)  // 编辑距离 ≤3
        .filter(|(c, _)| c.len() >= 2)
        .sort_by_key(|(_, d)| *d)
        .take(3)
        .map(|(c, _)| c.to_string())
        .collect()
}
```

### 4.2 自动修复

`Applicability::MachineApplicable` 的建议可被 `landin fix` 工具自动应用（v0.2）。

### 4.3 常见建议模板

| 场景 | 建议 |
| --- | --- |
| 拼写错误 | `help: a local variable with a similar name exists: \`xyz\`` |
| 缺少 trait bound | `help: consider restricting type parameter \`T\`` |
| 缺少 import | `help: add \`use std::collections::HashMap;\`` |
| 缺少 mut | `help: consider adding \`mut\`: \`let mut x\`` |
| 缺少 ref | `help: consider adding \`ref\`: \`let ref x\`` |

---

## 5. ICE（Internal Compiler Error）处理

### 5.1 ICE 触发

编译器内部 invariant 被违反时触发 ICE：

```rust
// 用 panic! 或 unreachable! 触发 ICE
fn assume_invariant(b: bool) {
    if !b {
        panic!("internal compiler error: invariant violated at {}:{}", file!(), line!());
    }
}
```

### 5.2 ICE 输出格式

```
error: internal compiler error: unexpected type in codegen: Ty::Placeholder

thread 'main' panicked at 'internal compiler error: unexpected type in codegen: Ty::Placeholder', compiler/rustc_codegen_ssa/src/mir/mod.rs:234:17
stack backtrace:
   0: std::panicking::begin_panic
   1: landin_codegen::mir::codegen_block
   2: landin_codegen::codegen_function
   ...

note: the compiler unexpectedly panicked. this is a bug.
note: we would appreciate a bug report: https://github.com/landin-lang/landin/issues/new
note: Landin version: 0.1.0
```

### 5.3 ICE 报告

ICE 发生时：

1. 写 stack trace 到 `~/.lin/ice-<timestamp>.log`
2. 提示用户提交 bug report
3. 退出码 101

---

## 6. 诊断输出选项

### 6.1 命令行选项

| 选项 | 作用 |
| --- | --- |
| `--json` | JSON 输出（IDE 集成） |
| `--no-color` | 禁用彩色 |
| `--error-format=short` | 短格式 |
| `--error-format=human` | 人可读（默认） |
| `--error-format=json` | JSON |
| `-D warnings` | warning 升级为 error |
| `-A warning` | 关闭 warning |
| `--cap-lints=level` | 限制依赖 crate 的 lint 等级 |
| `--explain E0xxx` | 显示错误代码详细解释 |

### 6.2 JSON 输出格式

```json
{
    "messages": [
        {
            "level": "error",
            "code": {"code": "E0308", "explanation": "..."},
            "message": "mismatched types",
            "spans": [
                {
                    "file": "src/main.lin",
                    "line_start": 10,
                    "line_end": 10,
                    "column_start": 5,
                    "column_end": 19,
                    "label": "expected `i32`, found `&str`",
                    "suggested_replacement": null,
                    "suggestion_applicability": null
                }
            ],
            "children": [
                {
                    "level": "note",
                    "message": "expected type `i32`",
                    "spans": []
                }
            ]
        }
    ]
}
```

---

## 7. 错误代码文档

### 7.1 `landin --explain E0308`

```
E0308: mismatched types

Expected type did not match the received type.

Erroneous code example:

```landin
let x: i32 = "hello";
//      ---   -------
//      |     |
//      |     expected `i32`, found `&str`
//      expected due to this
```

To fix this error, ensure the type annotation matches the value type:

```landin
let x: &str = "hello";  // OK
let x: i32 = 42;        // OK
```

Learn more:

- Type inference: <https://landin-lang.org/reference/type-inference.html>

```

### 7.2 错误代码生成

错误代码列表由 build script 从 `compiler/landin_error_codes` 目录生成，每个错误代码一个 markdown 文件。

---

## 8. 与 rustc 诊断系统的差异

| 维度 | rustc | Landin | 理由 |
|---|---|---|---|
| DiagnosticBuilder | 复杂 | 简化（单一 struct） | MVP 简化 |
| Lint 系统 | 完整（100+ lints） | MVP 仅 unused/dead_code | 简化 |
| Suggestion | 完整 | MVP 仅拼写纠正 + 简单建议 | 简化 |
| JSON 输出 | 完整 | ✅ 一致 | IDE 集成必需 |
| `--explain` | ✅ | ✅ 一致 | 用户体验 |
| Structured diagnostic | ✅ | ✅（简化） | 工具集成 |

---

**下一文档**: [`17-conformance-suite.md`](./17-conformance-suite.md) — Conformance 测试套件规范
