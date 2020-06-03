//! Execute Python code at compile time to generate Rust code.
//!
//! # Example
//!
//! ```
//! #![feature(proc_macro_hygiene)]
//! use ct_python::ct_python;
//!
//! static SIN_2: f64 = ct_python! {
//!     from math import sin
//!     print(sin(2))
//! };
//!
//! ct_python! {
//!     print("type num = f64;")
//! }
//!
//! fn main() {
//!     assert_eq!(num::sin(2.0), SIN_2);
//! }
//! ```
//!
//! # How to use
//!
//! Use the `ct_python!{..}` macro to generate Rust code from an embedded
//! Python script.
//! The output of the script (`print()` and anything else through `sys.stdout`)
//! is captured, and will be parsed as Rust code.
//!
//! If you want to use the macro to generate an expression (as in the example),
//! you'll need to add `#![feature(proc_macro_hygiene)]`.
//!
//! ## Python Errors
//!
//! Any syntax errors or runtime exceptions from the Python code will be
//! reported by the Rust compiler as compiler errors.
//!
//! ## Syntax issues
//!
//! Since the Rust tokenizer will tokenize the Python code, some valid Python
//! code is rejected. See [the `inline-python` documentation][1] for details.
//!
//! [1]: https://docs.rs/inline-python/#syntax-issues

/// A block of compile-time executed Rust code generating Python code.
///
/// See [the crate's module level documentation](index.html) for examples.
pub use inline_python_macros::ct_python;
