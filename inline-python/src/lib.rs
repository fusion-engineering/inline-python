//! Inline Python code directly in your Rust code.
//!
//! # Example
//!
//! ```
//! #![feature(proc_macro_hygiene)]
//! use inline_python::python;
//!
//! fn main() {
//!     let who = "world";
//!     let n = 5;
//!     python! {
//!         for i in range('n):
//!             print(i, "Hello", 'who)
//!         print("Goodbye")
//!     }
//! }
//! ```
//!
//! # How to use
//!
//! Use the `python!{..}` macro to write Python code direcly in your Rust code.
//! You'll need to add `#![feature(proc_macro_hygiene)]`, and use a nightly
//! version of the compiler that supports this feature.
//!
//! ## Using Rust variables
//!
//! To reference Rust variables, use `'var`, as shown in the example above.
//! `var` needs to implement [`pyo3::ToPyObject`].
//!
//! ## Getting information back
//!
//! Right now, this crate provides no easy way to get information from the
//! Python code back into Rust. Support for that will be added in a later
//! version of this crate.
//!
//! ## Syntax issues
//!
//! Since the Rust tokenizer will tokenize the Python code, some valid Python
//! code is rejected. The two main things to remember are:
//!
//! - Use double quoted strings (`""`) instead of single quoted strings (`''`).
//!
//!   (Single quoted strings only work if they contain a single character, since
//!   in Rust, `'a'` is a character literal.)
//!
//! - Use `//`-comments instead of `#`-comments.
//!
//!   (If you use `#` comments, the Rust tokenizer will try to tokenize your
//!   comment, and complain if your comment doesn't tokenize properly.)
//!
//! Other minor things that don't work are:
//!
//! - Certain escape codes in string literals.
//!   (Specifically: `\a`, `\b`, `\f`, `\v`, `\N{..}`, `\123` (octal escape
//!   codes), `\u`, and `\U`.)
//!
//!   These, however, are accepted just fine: `\\`, `\n`, `\t`, `\r`, `\xAB`
//!   (hex escape codes), and `\0`
//!
//! - Raw string literals with escaped double quotes. (E.g. `r"...\"..."`.)
//!
//! - Triple-quoted byte- and raw-strings with content that would not be valid
//!   as a regular string. And the same for raw-byte and raw-format strings.
//!   (E.g. `b"""\xFF"""` and `r"""\z"""`, `fr"\z"`, `br"\xFF"`.)
//!
//!   Other triple-quoted strings are accepted just fine though:
//!   E.g. `"""hello"""`, `b"""hello"""`, `r"""\n"""`, `fr"\n"`, `br"123"`.
//!
//! - The `//` and `//=` operators are unusable, as they start a comment.
//!
//!   Workaround: you can write `##` instead, which is automatically converted
//!   to `//`.
//!
//! Everything else should work fine.

pub use inline_python_macros::python;
pub use pyo3;

use pyo3::{
	ffi,
	types::{PyAny, PyDict},
	AsPyPointer, PyErr, PyResult, Python,
};

#[doc(hidden)]
pub use std::ffi::CStr;

#[doc(hidden)]
pub fn run_python_code<'p>(
	py: Python<'p>,
	code: &CStr,
	filename: &CStr,
	locals: Option<&PyDict>,
) -> PyResult<&'p PyAny> {
	unsafe {
		let mptr = ffi::PyImport_AddModule("__main__\0".as_ptr() as *const _);
		if mptr.is_null() {
			return Err(PyErr::fetch(py));
		}

		let globals = ffi::PyModule_GetDict(mptr);
		let locals = locals.map(AsPyPointer::as_ptr).unwrap_or(globals);

		let cptr = ffi::Py_CompileString(code.as_ptr(), filename.as_ptr(), ffi::Py_file_input);
		if cptr.is_null() {
			return Err(PyErr::fetch(py));
		}

		let res_ptr = ffi::PyEval_EvalCode(cptr, globals, locals);

		py.from_owned_ptr_or_err(res_ptr)
	}
}
