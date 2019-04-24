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
//!   (E.g. `b"""\xFF"""`, `r"""\z"""`, `fr"\z"`, `br"\xFF"`.)
//!
//! - The `//` and `//=` operators are unusable, as they start a comment.
//!
//!   Workaround: you can write `##` instead, which is automatically converted
//!   to `//`.
//!
//! Everything else should work fine.

use std::os::raw::c_char;

pub use inline_python_macros::python;
pub use pyo3;

use pyo3::{
	ffi,
	types::{PyAny, PyDict},
	AsPyPointer, PyErr, PyObject, PyResult, Python,
};

#[doc(hidden)]
pub use std::ffi::CStr;

#[doc(hidden)]
pub fn run_python_code<'p>(
	py: Python<'p>,
	compiled_code: &[u8],
	locals: Option<&PyDict>,
) -> PyResult<&'p PyAny> {
	unsafe {
		let main_mod = ffi::PyImport_AddModule("__main__\0".as_ptr() as *const _);
		if main_mod.is_null() {
			return Err(PyErr::fetch(py));
		}

		let globals = PyDict::new(py);
		if ffi::PyDict_Merge(globals.as_ptr(), ffi::PyModule_GetDict(main_mod), 0) != 0 {
			return Err(PyErr::fetch(py));
		}

		let rust_vars = locals.map(|x| x.as_ptr()).unwrap_or_else(|| py.None().as_ptr());
		if ffi::PyDict_SetItemString(globals.as_ptr(), "RUST\0".as_ptr() as *const _, rust_vars) != 0 {
			return Err(PyErr::fetch(py))
		}

		let compiled_code = python_unmarshal_object_from_bytes(py, compiled_code)?;
		let result = ffi::PyEval_EvalCode(compiled_code.as_ptr(), globals.as_ptr(), std::ptr::null_mut());

		py.from_owned_ptr_or_err(result)
	}
}

extern "C" {
	fn PyMarshal_ReadObjectFromString(data: *const c_char, len: isize) -> *mut ffi::PyObject;
}

/// Use built-in python marshal support to read an object from bytes.
fn python_unmarshal_object_from_bytes(py: Python, data: &[u8]) -> pyo3::PyResult<PyObject> {
	unsafe {
		let object = PyMarshal_ReadObjectFromString(data.as_ptr() as *const c_char, data.len() as isize);
		if object.is_null() {
			return Err(PyErr::fetch(py))
		}

		Ok(PyObject::from_owned_ptr(py, object))
	}
}
