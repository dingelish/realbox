# RealBox: Make Box great again!

## Background: The hidden memory copy

It's wellknown that `Box<T>` allocates memory on **stack** first, and copies the initialized struct to heap. So on embedded devices, create a large boxed object would result in **stack overflow** instead of heap allocator OOM.

[`copyless`](https://github.com/kvark/copyless) wants to solve it by invoking allocation primitives directly, and resulted in using `ptr::write`, which defined as:

```rust
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
pub unsafe fn write<T>(dst: *mut T, src: T) {
    intrinsics::move_val_init(&mut *dst, src)
}
```

The `intrinsics` is a "Intrinsics Symbol" and the compiler backend can recognizes it. The comments said that:

```rust
if let Some(sym::move_val_init) = intrinsic {
    // `move_val_init` has "magic" semantics - the second argument is
    // always evaluated "directly" into the first one.
```

However, this is not always true. In debug build, rustc would still triggers `memcpy`:

```
448a:       48 8b 7c 24 38          mov    0x38(%rsp),%rdi
448f:       48 89 ce                mov    %rcx,%rsi
4492:       ba 94 01 00 00          mov    $0x194,%edx
4497:       48 89 44 24 30          mov    %rax,0x30(%rsp)
449c:       e8 e7 f9 ff ff          callq  3e88 <memcpy@plt>
```

My conclusion is: `move_val_init`'s guarantee depends on optimization, which might not be guaranteed by Rust.


## Solution

The key difference is that the API provided in this crate is:

```rust
impl<T> RealBox<T, Global> {
    pub fn heap_init<F>(initialize: F) -> Box<T>
    where
        F: Fn(&mut T),
    {
        unsafe {
            let mut t = Self::new_in(Global).into_box();
            initialize(t.as_mut());
            t
        }
    }
}
```

which **requires** an initializer `Fn(&mut T)`, and does not depends on `move_val_init`.

## Usage

```rust
#[derive(Debug)]
struct Obj {
    x: u32,
    y: f64,
    a: [u8; 4],
}

let stack_obj = Obj {
    x: 12,
    y: 0.9,
    a: [0xff, 0xfe, 0xfd, 0xfc],
};

let heap_obj = RealBox::<Obj>::heap_init(|mut t| {
    t.x = 12;
    t.y = 0.9;
    t.a = [0xff, 0xfe, 0xfd, 0xfc]
});
```
