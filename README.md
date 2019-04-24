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

Use the `python!{..}` macro to write Python code direcly in your Rust code.
You'll need to add `#![feature(proc_macro_hygiene)]`, and use a nightly
version of the compiler that supports this feature.

### Using Rust variables

To reference Rust variables, use `'var`, as shown in the example above.
`var` needs to implement `pyo3::ToPyObject`.

### Getting information back

Right now, this crate provides no easy way to get information from the
Python code back into Rust. Support for that will be added in a later
version of this crate.

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
