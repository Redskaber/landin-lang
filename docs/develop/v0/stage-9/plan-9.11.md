# Stage 9.11 开发计划: Realistic programs conformance 扩展

> **阶段**: Stage 9.11 (Stage 9 第 11 个子阶段)
> **版本**: v0.16.9 → v0.16.10
> **状态**: 🟡 In Progress
> **流程**: stage-committee-process.md v3.21 §13.4 + §17.1/§17.2/§17.3 + §1.2 验收

## 1. 背景

Stage 9.10 完成 conformance 497 → 547 (error recovery category, 91.2% — approaching
v0.1 release!). Stage 9.11 继续扩展 **realistic programs** 类别 (per
`17-conformance-suite.md` §2 §10-realistic — "Full programs (fib, iterators, traits)").

## 2. §13.4 设计对齐

查阅:
- `docs/lang-design/17-conformance-suite.md` §2 (10-realistic category description)
- `docs/lang-design/02-grammar.md` §2-§3 (all grammar — realistic programs combine all features)
- 现有 2 个 realistic tests (fibonacci.lin + trait_impl.lin)

## 3. 测试设计 (52 个 .lin tests)

### 3.1 Classic algorithms (12 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_fib_iterative.lin | iterative fibonacci |
| realistic_factorial.lin | recursive factorial |
| realistic_gcd.lin | Euclidean GCD |
| realistic_bubble_sort.lin | bubble sort on array |
| realistic_binary_search.lin | binary search |
| realistic_linear_search.lin | linear search |
| realistic_power.lin | recursive power |
| realistic_is_prime.lin | primality test |
| realistic_sum_array.lin | sum array elements |
| realistic_max_array.lin | find max in array |
| realistic_reverse_array.lin | reverse array in place |
| realistic_countdown.lin | countdown loop |

### 3.2 Data structures (10 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_linked_list.lin | singly linked list node + push |
| realistic_stack.lin | stack with push/pop |
| realistic_queue.lin | queue with enqueue/dequeue |
| realistic_tree_node.lin | binary tree node |
| realistic_tree_insert.lin | BST insert |
| realistic_hash_map_entry.lin | hash map entry struct |
| realistic_vec_wrapper.lin | Vec wrapper struct |
| realistic_option.lin | Option enum |
| realistic_result.lin | Result enum |
| realistic_point.lin | 2D point struct |

### 3.3 Trait patterns (10 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_trait_display.lin | Display trait |
| realistic_trait_default.lin | trait with default method |
| realistic_trait_iterator.lin | Iterator trait |
| realistic_trait_clone.lin | Clone trait |
| realistic_trait_eq.lin | PartialEq trait |
| realistic_trait_ord.lin | PartialOrd trait |
| realistic_trait_supertrait.lin | trait with supertrait |
| realistic_trait_multi_impl.lin | impl multiple traits |
| realistic_trait_associated_type.lin | trait with associated type |
| realistic_trait_static_method.lin | trait with static method |

### 3.4 Closures & iterators (8 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_closure_map.lin | map closure |
| realistic_closure_filter.lin | filter closure |
| realistic_closure_reduce.lin | reduce closure |
| realistic_closure_compose.lin | compose two closures |
| realistic_closure_capture.lin | closure capturing multiple vars |
| realistic_closure_move_capture.lin | move closure capturing |
| realistic_closure_recursive.lin | recursive via closure |
| realistic_closure_callback.lin | callback pattern |

### 3.5 Pattern matching (6 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_match_option.lin | match Option |
| realistic_match_result.lin | match Result |
| realistic_match_enum.lin | match enum with data |
| realistic_match_nested.lin | nested match |
| realistic_match_guard.lin | match with guard |
| realistic_match_or_pat.lin | match with or-pattern |

### 3.6 Real-world snippets (6 tests)

| 测试文件 | 描述 |
|---------|------|
| realistic_calculator.lin | simple calculator |
| realistic_string_ops.lin | string operations |
| realistic_counter.lin | counter struct with methods |
| realistic_config.lin | config struct |
| realistic_state_machine.lin | state machine |
| realistic_error_handling.lin | error handling with Result |

**累计**: 12 + 10 + 10 + 8 + 6 + 6 = **52 tests**

## 4. 验收标准

- ✅ `cargo clean && cargo test`: 2215+ tests pass (期望 +10 verification tests = 2225)
- ✅ `cargo fmt --check`: clean
- ✅ `cargo clippy --all-targets`: 0 warnings
- ✅ `python3 tests/conformance/run_all.py`: 599 passed (547 + 52 new)
- ✅ §17.3 三阶段文档协议: plan + gate-review + test plan
- ✅ 0 regressions

## 5. 版本

- Cargo.toml: 0.16.9 → 0.16.10
- api-naming-standard.md: v2.13 → v2.14

---

**创建日期**: 2026-07-26
