# Stage 18.301 — Phase B (修正): extern "C" in prelude impl → 移除 6 early interception

> **Author**: Super Z (main) — PM-A + ARCH-A
> **Date**: 2026-08-26
> **Version**: v0.493.0

## 5W2H 分析

### What
将 6 个 early interception hardcoded intrinsics 转换为 prelude 中的 real body 方法:
1. Box::new(x) → alloc + store
2. String::from_str(s) → alloc + memcpy
3. String::as_str() → fat pointer construction
4. String::push_str(s) → realloc + memcpy
5. Vec::push(v) → realloc + store
6. Vec::get(i) → bounds check + GEP

### Why
- **根因**: prelude impl body 不能调用 C 运行时函数 → 每个 intrinsic 有独立 `if method_name == "xxx"` 检查 + 专用 lower 函数
- **验证**: extern "C" + impl body 已可用 (测试通过)
- **影响**: 移除 1957 LOC (intrinsic_lower.rs) + 6 个 early interception 检查

### Rust 设计依据
- Rust core/std 中 Box::new, String::from_str, Vec::push 等都有 real body
- Rust 通过 extern "C" FFI 调用 alloc/memcpy/realloc
- Landin 已支持 extern "C" block + impl body 调用 extern C 函数

### Rust 哲学
- **显式优于隐式**: prelude impl body 显式调用 alloc/memcpy, 不需要 hidden dispatch
- **让非法状态不可表示**: 移除 `if method_name == "xxx"` hardcoded dispatch — 不再有 "magic method names"
- **实用性优先**: extern "C" + impl body 已可用, 不需要新语言特性

### How Much
- 6 个 intrinsic 函数 = 1720 LOC (不含 format_variadic 535 LOC)
- format_variadic 是宏展开, 不是 impl method — deferred to Phase C
- Phase B 范围: 5 个 impl methods (不含 format!)

## 实施方案

### Step 1: 添加 extern "C" 声明到 prelude
```landin
extern "C" {
    fn __landin_alloc(size: usize) -> *mut u8;
    fn __landin_dealloc(ptr: *mut u8);
    fn __landin_memcpy(dst: *mut u8, src: *mut u8, n: usize);
    fn __landin_realloc(ptr: *mut u8, old_size: usize, new_size: usize) -> *mut u8;
}
```

### Step 2: 将 5 个 early interception 改为 real body

#### Box::new
```landin
impl<T> Box<T> {
    fn new(val: T) -> Box<T> {
        let size = /* sizeof(T) */;
        let ptr = __landin_alloc(size);
        /* store val at ptr */;
        Box { ptr: ptr as *mut T }
    }
}
```

#### String::from_str
```landin
impl String {
    fn from_str(s: &str) -> String {
        let len = s.len();
        let ptr = __landin_alloc(len);
        __landin_memcpy(ptr as *mut u8, /* str ptr */, len);
        String { ptr: ptr, len: len, cap: len }
    }
}
```

#### String::as_str
```landin
impl String {
    fn as_str(&self) -> &str {
        /* construct &str fat pointer from self.ptr + self.len */
    }
}
```

#### String::push_str
```landin
impl String {
    fn push_str(&mut self, s: &str) {
        let new_len = self.len + s.len();
        if new_len > self.cap {
            let new_cap = /* max(new_len, self.cap * 2) */;
            let new_ptr = __landin_realloc(self.ptr as *mut u8, self.cap, new_cap);
            self.ptr = new_ptr as *mut u8;
            self.cap = new_cap;
        }
        __landin_memcpy(/* dest: self.ptr + self.len */, /* src: s.ptr */, s.len());
        self.len = new_len;
    }
}
```

#### Vec::push
```landin
impl<T> Vec<T> {
    fn push(&mut self, val: T) {
        if self.len >= self.cap {
            let new_cap = /* max(self.cap + 1, self.cap * 2) */;
            let new_ptr = __landin_realloc(self.ptr as *mut u8, self.cap * /* sizeof(T) */, new_cap * /* sizeof(T) */);
            self.ptr = new_ptr as *mut T;
            self.cap = new_cap;
        }
        /* store val at self.ptr + self.len */;
        self.len = self.len + 1usize;
    }
}
```

### Step 3: 移除 early interception 代码
- 移除 `src/mir/lower/expr_variants.rs` 中 5 个 `if method_name == "xxx"` 检查
- 移除 `src/mir/lower/intrinsic_lower.rs` 中 5 个 `lower_*_intrinsic` 函数
- 保留 `lower_format_variadic_intrinsic` (Phase C)

### Step 4: 测试 + §3.2 + 文档 + 打包

## 复杂度评估

- Box::new: 中等 (需要 sizeof — 但 Landin 不支持 sizeof 泛型)
- String::from_str: 中等 (需要 fat pointer 拆解)
- String::as_str: 复杂 (需要 fat pointer 构造 — 目前用 intrinsic)
- String::push_str: 复杂 (需要 realloc + memcpy + pointer arithmetic)
- Vec::push: 复杂 (需要 realloc + store + sizeof)

## 阻塞分析

- **sizeof(T)**: Landin 不支持泛型 sizeof。Box::new 和 Vec::push 需要 sizeof(T) 来 alloc/realloc。
- **fat pointer 拆解**: &str 的 ptr 和 len 需要从 fat pointer 中提取 — 这是 str::len 的 intrinsic, 在 prelude 中不可用 (str::len 本身是 intrinsic)。
- **fat pointer 构造**: String::as_str 需要构造 &str fat pointer — 同样需要 compiler support。

## 结论

Phase B 的 5 个 early interception **不能完全转为 real body**, 因为:
1. 泛型 sizeof 不支持 (Box::new, Vec::push)
2. fat pointer 拆解/构造不可在源码中表达 (String::from_str, String::as_str, String::push_str)

这些是 **language feature gaps**, 不是简单的 "添加 extern C 声明" 就能解决的。

### 修正方案

Phase B 分为两个子阶段:
- **Phase B-1**: 添加 extern "C" 声明到 prelude (已可用) + 移除 format_variadic (Phase C)
- **Phase B-2**: 添加 sizeof(T) + fat pointer 操作语法 (v0.5+ lang feature)

当前阶段: **Phase B-1** — 添加 extern "C" 声明 + 记录 language feature gaps
