# 02 — 语法文法

> 本文用 **EBNF** 定义 Landin 的完整词法与语法结构。Parser 实现采用 **手写 recursive descent + Pratt parser**（R3 推荐），不使用任何 parser generator。文法描述与实现可能存在细节差异，以实现为准。

---

## 1. 词法结构

### 1.1 字符集

- **Source file**: UTF-8 编码
- **换行**: `\n`（LF）或 `\r\n`（CRLF，归一化为 LF）
- **空白**: space, tab, 换行
- **注释**:
  - `// line comment` — 至行尾
  - `/* block comment */` — 块注释，**可嵌套**（与 Rust 一致，与 C 不同）
- **BOM**: 文件首字节不允许 BOM

### 1.2 Token 分类

```
token := keyword | identifier | literal | operator | punctuation | whitespace | comment
```

### 1.3 关键字

```
// 严格保留（不可作 identifier）
as break const continue crate dyn else enum extern false fn for if impl in let
loop match mod move mut pub ref return self Self static struct super trait true
type unsafe use where while

// 弱保留（可作字段名/方法名，不可作 item 名）
abstract become box do final macro override priv typeof unsized virtual yield
try union   // 保留为未来使用

// MVP 保留但未实现的关键字（v0.2 启用）
// v1.2.2 修正：move 已在严格保留列表（line 30），v0.2 启用 move closure
async await   // async/await（v0.2 启用）
```

### 1.4 Identifier

```
identifier := XID_Start (XID_Continue)*
raw_identifier := "r#" identifier

XID_Start := Unicode XID_Start 类（字母 + 下划线）
XID_Continue := Unicode XID_Continue 类（字母 + 数字 + 下划线 + 部分标点）
```

`r#` 前缀允许使用关键字作 identifier（参考 Rust raw identifier）。

### 1.5 整数字面量

```
integer_lit := dec_lit | hex_lit | oct_lit | bin_lit
bool_lit := "true" | "false"

dec_lit := "0" | ([1-9] (0-9 | _)*)    // 不允许前导零（042 非法）
hex_lit := "0x" (0-9 | a-f | A-F | _)*
oct_lit := "0o" (0-7 | _)*
bin_lit := "0b" (0 | 1 | _)*

integer_suffix := 
    "i8" | "i16" | "i32" | "i64" | "i128" | "isize" |
    "u8" | "u16" | "u32" | "u64" | "u128" | "usize"
```

无后缀时由类型推导决定；推导不出时默认 `i32`（参考 Rust 1.0 规则，R3 报告支持）。

### 1.6 浮点字面量

```
float_lit := 
    [0-9] (0-9 | _)* "." [0-9] (0-9 | _)* float_suffix? |
    [0-9] (0-9 | _)* float_suffix |
    [0-9] (0-9 | _)* ("e" | "E") ("+" | "-")? [0-9]+ float_suffix?

float_suffix := "f32" | "f64"
```

### 1.7 字符与字符串

```
char_lit := "'" (char_escape | unicode_char) "'"
byte_lit := "b" "'" (byte_escape | ascii_char) "'"
byte_escape := "\\n" | "\\r" | "\\t" | "\\\\" | "\\0" | "\\'" | "\\\"" | "\\x" hex_digit hex_digit   // 仅 ASCII byte escape，不允许 \\u{}

string_lit := '"' (string_escape | unicode_char | '"')* '"'
raw_string_lit := 'r' raw_hash_string
byte_string_lit := 'b' '"' (string_escape | ascii_char)* '"'
raw_byte_string_lit := 'br' raw_hash_string

raw_hash_string := '#'* '"' ... '"' '#'*    // 同等数量的 # 包围

char_escape :=
    "\n" | "\r" | "\t" | "\\" | "\0" | "\'" | "\"" |
    "\x" hex_digit hex_digit |           // ASCII byte
    "\u{" hex_digit+ "}"                  // Unicode scalar value
```

### 1.8 运算符与标点

```
operator :=
    "+" | "-" | "*" | "/" | "%" |
    "=" | "==" | "!=" | "<" | ">" | "<=" | ">=" |
    "&&" | "||" | "!" |
    "&" | "|" | "^" | "<<" | ">>" |
    "+=" | "-=" | "*=" | "/=" | "%=" |
    "&=" | "|=" | "^=" | "<<=" | ">>=" |
    "?" | ".." | "..=" |
    "->" | "=>" | "::"

punctuation :=
    "(" | ")" | "{" | "}" | "[" | "]" |
    "," | ";" | ":" | "." | ".."
```

### 1.9 长度规则（Maximal Munch）

词法分析遵循 **最长匹配** 原则：从当前位置开始，匹配能形成合法 token 的最长字符串。

- `>>` 在类型上下文中由 parser 拆分为两个 `>`（lexer hack；R3 报告支持）
- `..=` 是单 token，不能写为 `.. =`
- `1.0.0` 不是合法浮点，会 lexer 报错（应使用 `"1.0.0"` 字符串）

---

## 2. Parser 概览

Landin parser 是 **手写 recursive descent + Pratt parser**：

- 声明与语句：recursive descent
- 表达式：Pratt parser（top-down operator precedence，参考 Pratt 1973）

每个 parse 函数返回 `Result<T, ParseError>`，错误恢复通过 `synthetic node` 实现：遇到错误时插入一个虚拟节点 + 跳过至下一个 `;` 或 `}` 继续 parse。

### Pratt 优先级表

| 优先级 | 运算符 | 结合性 |
| --- | --- | --- |
| 1 (最低) | `\|\|` | 左 |
| 2 | `&&` | 左 |
| 3 | `==` `!=` `<` `>` `<=` `>=` | 需要 |
| 4 | `\|` | 左 |
| 5 | `^` | 左 |
| 6 | `&` | 左 |
| 7 | `<<` `>>` | 左 |
| 8 | `+` `-` | 左 |
| 9 | `*` `/` `%` | 左 |
| 10 | `as` | 左（一元后缀） |
| 11 | `-` `!` `*` `&` `&mut` | 一元前缀 |
| 12 | `(` `.` `[` `?` `!` | 后缀调用/字段/索引 |
| 13 (最高) | 字面量、路径、`(expr)`、`{block}` | 原子 |

---

## 3. 语法产生式

### 3.1 Crate 与 module

```ebnf
crate := item*

item :=
    vis? "fn" ident generic_params? "(" fn_params? ")" ("->" type)? where_clause? block |
    vis? "const" ident ":" type "=" expr ";" |
    vis? "static" "mut"? ident ":" type "=" expr ";" |
    vis? "struct" ident generic_params? struct_body |
    vis? "enum" ident generic_params? enum_body |
    vis? "trait" ident generic_params? (":" type_bounds)? trait_body |
    vis? "impl" generic_params? type "for" type where_clause? "{" impl_item* "}" |
    vis? "impl" generic_params? type where_clause? "{" impl_item* "}" |
    vis? "type" ident generic_params? "=" type ";" |
    "extern" string? "{" extern_item* "}" |
    "mod" ident "{" item* "}" |
    "mod" ident ";" |
    "use" use_tree ";" |
    attr* item

vis := "pub" ("(" ("crate" | "super" | "self" | "in" path) ")")?

attr := "#" "[" meta "]"
meta := ident ("=" expr | "(" meta_args? ")")?

struct_body := "{" struct_field* "}" | ";" | "(" struct_field* ")"
struct_field := vis? ident ":" type ","?

enum_body := "{" enum_variant* "}"
enum_variant := attr? vis? ident ("(" tuple_fields? ")" | "{" struct_field* "}")? ","?
tuple_fields := vis? type ("," vis? type)*

trait_body := "{" trait_item* "}"
trait_item :=
    "fn" ident generic_params? "(" fn_params? ")" ("->" type)? where_clause? (";" | block) |
    "type" ident (":" type_bounds)? ";" |
    "const" ident ":" type (":" expr)? ";"

impl_item := "fn" ident generic_params? "(" fn_params? ")" ("->" type)? where_clause? block

extern_item := vis? "fn" ident "(" fn_params? ")" ("->" type)? ";" |
               vis? "static" "mut"? ident ":" type ";"
```

### 3.2 Generic 与 bound

```ebnf
generic_params := "<" (lifetime_param | type_param)* ">"

lifetime_param := "'" ident (":" lifetime_bounds)?
type_param := ident (":" type_bounds)? ("=" type)?

type_bounds := type_bound ("+" type_bound)*
type_bound :=
    lifetime |
    "?" type_path |                        // v0.2: ?Sized
    type_path |
    "for" generic_params type_path          // higher-rank

lifetime := "'" ident | "'static"
lifetime_bounds := lifetime (":" lifetime) ("," lifetime)*

where_clause := "where" where_pred ("," where_pred)*
where_pred := (lifetime ":")? type ":" type_bounds
```

### 3.3 Type

```ebnf
type :=
    "(" type? "," type? ")" |
    "!" |                                    // never
    "[" type ";" expr "]" |
    "[" type "]" |                           // slice
    "&" lifetime? "mut"? type |
    "*const" type | "*mut" type |
    "fn" "(" fn_params? ")" ("->" type)? |
    "impl" type_bounds |
    "dyn" type_bounds |
    type_path |
    qualified_path

type_path :=
    (path "::")? ident ("::" type_segment)*
type_segment := ident generic_args?

qualified_path :=
    "<" type "as" type_path ">" "::" type_segment
```

### 3.4 表达式

```ebnf
expr :=
    "if" expr block ("else" (if_expr | block))? |
    "if" "let" pat "=" expr block ("else" (if_let_expr | block))? |
    "match" expr "{" match_arm* "}" |
    "loop" block |
    "while" expr block |
    "while" "let" pat "=" expr block |
    "for" pat "in" expr block |
    "unsafe" block |
    block |
    "return" expr? |
    "break" expr? |
    range_expr |
    "continue" |
    assign_expr

range_expr := or_expr (".." or_expr? | "..=" or_expr)?    // a..b, a.., ..b, a..=b（定义见此行）

assign_expr := or_expr (("=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<=" | ">>=") or_expr)?

or_expr := and_expr ("||" and_expr)*
and_expr := cmp_expr ("&&" cmp_expr)*
cmp_expr := bit_or_expr (("==" | "!=" | "<" | ">" | "<=" | ">=") bit_or_expr)*
bit_or_expr := bit_xor_expr ("|" bit_xor_expr)*
bit_xor_expr := bit_and_expr ("^" bit_and_expr)*
bit_and_expr := shift_expr ("&" shift_expr)*
shift_expr := add_expr (("<<" | ">>") add_expr)*
add_expr := mul_expr (("+" | "-") mul_expr)*
mul_expr := cast_expr (("*" | "/" | "%") cast_expr)*
cast_expr := unary_expr ("as" type)*

unary_expr :=
    ("-" | "!" | "*") unary_expr |
    "&" lifetime? "mut"? expr |
    primary_expr postfix*

primary_expr :=
    literal |
    path_expr |
    "(" expr? ")" |
    "(" expr ("," expr)+ ","? ")" |           // tuple
    "[" expr? "]" |                            // array literal / vec macro v0.2
    "[" expr ";" expr "]" |                    // repeat array
    "move" closure |                           // v0.2
    closure |
    struct_expr |
    "self" | "Self" |
    "_"

closure := "||" fn_params? expr  |  "||" block  |  "|" fn_params "|" expr_or_block

postfix :=
    "." ident ("::" generic_args)?            // method call / field
    "." integer_lit                              // tuple field access: t.0, t.1
    "(" expr_list? ")"                         // function call
    "[" expr "]"                                // index
    "?"                                          // error propagation
    "!" ( "(" token_tree* ")" | "{" token_tree* "}" | "[" token_tree* "]" )  // built-in macro call

token_tree := token  // for macro arguments (v0.2: full macro system)

path_expr := path_prefix? ident ("::" ident)* ("::" generic_args)?
path_prefix := "::" | "crate" "::" | "super" "::" | "self" "::"
qualified_path := "<" type ("as" type_path)? ">" "::" path_segment

struct_expr := type_path "{" struct_expr_field ("," struct_expr_field)* ","? "}"
struct_expr_field := ident ":" expr | ident | (integer_lit) ":" expr

match_arm := pat ("if" expr)? "=>" (expr "," | block)
```

### 3.5 模式

```ebnf
pat :=
    "_" |
    literal_pat |
    (path "::")? ident ("::" ident)* ("(" pat_list? ")" | "{" pat_fields? "}")? |
    "&" pat |
    "&mut" pat |
    "(" pat_list? ")" |
    "[" pat ("," pat)* ("," ".." ("," pat)*)? "]" |
    pat "|" pat |
    ident "@" pat |
    ".." |
    range_pat

literal_pat := integer_lit | float_lit | char_lit | string_lit | bool_lit | "-"? integer_lit
range_pat := pat "..=" pat | pat ".." pat
pat_list := pat ("," pat)* ","?
pat_fields := pat_field ("," pat_field)* ","?
pat_field := ident ":" pat | ident | ".."
```

### 3.6 语句

```ebnf
stmt :=
    "let" pat (":" type)? "=" expr ";" |
    expr ";" |
    expr |
    ";"

block := "{" stmt* expr? "}"
fn_params := fn_param ("," fn_param)* ","?
fn_param := self_param | (pat ":" type)

// self_param: 仅 impl 块内方法 / trait 方法的第一个参数
self_param := ("&" lifetime? "mut"? "self") | ("mut"? "self" (":" type)?)
// 例: &self, &mut self, self, self: Box<Self>, mut self
```

### 3.7 use 声明

```ebnf
use_decl := "use" use_tree ";"
use_tree :=
    path (":" ":")? "{" use_tree_list "}" |
    path "as" ident |
    path "*"
use_tree_list := use_tree ("," use_tree)*
```

---

## 4. 关键歧义与解决方案

### 4.1 `<<` 的歧义

在类型上下文中，`Vec<Vec<T>>` 的 `>>` 必须被 parser 拆分为两个 `>`。Landin 采用 **lexer hack**：

- lexer 维护一个 "type context" 标志（由 parser 通过回调设置）
- 在 type context 中，`>>` 不作为一个 token 输出，而是输出两个 `>`
- 类似处理 `>=`、`>>=` 等

R3 报告指出这是 Rust 也采用的做法，工程上稳定。

### 4.2 closure 与 binary OR 的歧义

`|x| x | y` 可能被解析为：

- (a) 闭包 `|x|` body = `x | y`
- (b) OR 模式 `|x|` 在 `match` 中

Landin 规则：

- 在表达式上下文，`|` 后面跟随 pat 时识别为 closure
- 在模式上下文，`|` 始终是 or-pattern

### 4.3 attribute 与 outer/inner

```landin
#[derive(Clone)]    // outer attribute
pub struct Foo;

#![no_std]          // inner attribute（! 前缀），作用于包含它的 item
mod bar {
    ...
}
```

`#!` 是 inner attribute，必须出现在 module/crate 顶部。`#` 是 outer attribute，作用于其后跟随的 item。

### 4.4 内建宏调用（v1.2.2 修正）

MVP **不支持** `macro_rules!` 自定义宏（推迟 v0.2），但 **支持** 26 个内建宏（编译器硬编码展开，含 matches!）。完整清单见 `13-stage1-feature-whitelist.md §2.6`。

内建宏清单（按用途分组，共 26 个，含 matches!）：

- I/O：`println!` `print!` `eprintln!` `eprint!`
- 字符串：`format!` `write!` `writeln!`
- 构造：`vec!`
- 断言：`assert!` `assert_eq!` `assert_ne!` `debug_assert!` `debug_assert_eq!` `debug_assert_ne!`
- 控制：`panic!` `unreachable!` `todo!` `unimplemented!`
- 调试：`dbg!`
- 编译期信息：`concat!` `stringify!` `file!` `line!` `column!` `module_path!`
- 模式匹配判断：`matches!`

调用形式：`ident!(args)` / `ident!{args}` / `ident![args]`。

用户使用未在上述清单中的 `ident!` 形式时，报错"unknown macro or not yet supported"。

---

## 5. Token 流与 parser 实现要点

### 5.1 Span 信息

每个 AST 节点必须带 **span**（起止字节 offset + 文件 ID），用于错误信息与 diagnostics。Span 设计：

```rust
struct Span {
    lo: BytePos,    // u32
    hi: BytePos,
    file_id: u32,   // source file id
}
```

总大小 12 字节，可压缩为 8 字节（file_id 与 lo/hi 共享 u32 的高位）。

### 5.2 错误恢复

Parser 遇到错误时不立即停止，而是：

1. 报告当前错误
2. 跳过至下一个 **synchronization token**（v1.2 统一与 05 §11.3 一致）：`;`、`}`、`fn`、`struct`、`impl`、`trait`、`pub`、`use`
3. 插入一个 `Err` AST 节点占位
4. 继续 parse

这样能在一个文件内一次性报多个错误（参考 TypeScript、Roslyn 的做法）。

### 5.3 Token 树（macro 用，v0.2）

为支持未来宏系统，parser 内部保留 token tree 概念：每个 `(...)` / `{...}` / `[...]` 在 token 流中作为一个 "delimiter group"。这是 rustc `TokenStream` 的简化版，MVP 不暴露给用户。

---

## 6. 文件后缀与编码

| 后缀 | 用途 |
| --- | --- |
| `.lin` | Landin 源文件 |
| `.linrs` | Landin 内部 IR（MIR 文本表示） |
| `.lino` | Landin 对象文件（编译产物） |
| `.linlib` | Landin 静态库（rlib） |
| `landin.toml` | crate manifest |

源文件必须 UTF-8 编码；LF 或 CRLF 均可，但编译器内部归一化为 LF。

---

## 7. 与 Rust 文法的具体差异

| 维度 | Rust | Landin | 理由 |
| --- | --- | --- | --- |
| macro_rules! | `macro_rules!` 关键字 | 无（v0.2） | R1 教训 |
| async closures | `async \|\| ...` | 无 | MVP 单线程 |
| label | `'label:` | **无** | 简化 parser |
| `try` blocks | nightly | **无** | 简化 |
| raw string | `r#"..."#` | 一致 | 兼容 |
| attribute | `#[...]` | 一致 | 兼容 |
| lifetime token | `'a` | 一致 | 兼容 |
| `self` parameter | `self` / `&self` / `&mut self` / `self: Type` | 一致 | 兼容 |

---

**下一文档**: [`03-type-system.md`](./03-type-system.md) — 类型系统
