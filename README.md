# inline-python

Inline Python code directly in your Rust code.

## Example

```rust
#![feature(proc_macro_hygiene)]
use inline_python::python;

fn main() {
    let who = "world";
    let n = 5;
    python! {
        for i in range('n):
            print(i, "Hello", 'who)
        print("Goodbye")
    }
}
```

## How to use

Use the `python!{..}` macro to write Python code directly in your Rust code.
You'll need to add `#![feature(proc_macro_hygiene)]`, and use a nightly
version of the compiler that supports this feature.

### Using Rust variables

To reference Rust variables, use `'var`, as shown in the example above.
`var` needs to implement [`pyo3::ToPyObject`].

### Re-using a Python context
It is possible to create a [`Context`] object ahead of time,
to be used for running the python code.
That way, the context can be shared by multiple invocations of the macro.
Doing so will preserve global variables across macro calls:

```rust
let context = inline_python::Context::new();
python! {
  #![context = &context]
  foo = 5
}
python! {
  #![context = &context]
  assert foo == 5
}
```

### Getting information back

A [`Context`] object can also be used to pass information back to Rust.
You can retrieve global Python variables from the context.
Note that you need to acquire the GIL in order to access those globals:

```rust
use inline_python::{pyo3, python};
let context = inline_python::Context::new();
python! {
  #![context = &context]
  foo = 5
}

let gil = pyo3::Python::acquire_gil();
let py  = gil.python();
let foo: Option<i32> = context.get_global(py, "foo").unwrap();
assert_eq!(foo, Some(5));
```

### Syntax issues

Since the Rust tokenizer will tokenize the Python code, some valid Python
code is rejected. The two main things to remember are:

- Use double quoted strings (`""`) instead of single quoted strings (`''`).

  (Single quoted strings only work if they contain a single character, since
  in Rust, `'a'` is a character literal.)

- Use `//`-comments instead of `#`-comments.

  (If you use `#` comments, the Rust tokenizer will try to tokenize your
  comment, and complain if your comment doesn't tokenize properly.)

Other minor things that don't work are:

- Certain escape codes in string literals.
  (Specifically: `\a`, `\b`, `\f`, `\v`, `\N{..}`, `\123` (octal escape
  codes), `\u`, and `\U`.)

  These, however, are accepted just fine: `\\`, `\n`, `\t`, `\r`, `\xAB`
  (hex escape codes), and `\0`

- Raw string literals with escaped double quotes. (E.g. `r"...\"..."`.)

- Triple-quoted byte- and raw-strings with content that would not be valid
  as a regular string. And the same for raw-byte and raw-format strings.
  (E.g. `b"""\xFF"""`, `r"""\z"""`, `fr"\z"`, `br"\xFF"`.)

- The `//` and `//=` operators are unusable, as they start a comment.

  Workaround: you can write `##` instead, which is automatically converted
  to `//`.

Everything else should work fine.

License: BSD-2-Clause
