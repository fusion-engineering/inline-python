# inline-python

Inline Python code directly in your Rust code.

## Example

```rust
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

### Using Rust variables

To reference Rust variables, use `'var`, as shown in the example above.
`var` needs to implement `pyo3::ToPyObject`.

### Re-using a Python context

It is possible to create a `Context` object ahead of time and use it for running the Python code.
The context can be re-used for multiple invocations to share global variables across macro calls.

```rust
let c = Context::new();

c.run(python! {
  foo = 5
});

c.run(python! {
  assert foo == 5
});
```

As a shortcut, you can assign a `python!{}` invocation directly to a
variable of type `Context` to create a new context and run the Python code
in it.

```rust
let c: Context = python! {
  foo = 5
};

c.run(python! {
  assert foo == 5
});
```

### Getting information back

A `Context` object could also be used to pass information back to Rust,
as you can retrieve the global Python variables from the context through
`Context::get`.

```rust
let c: Context = python! {
  foo = 5
};

assert_eq!(c.get::<i32>("foo"), 5);
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
