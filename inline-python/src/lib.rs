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
//! ## Tokens
//!
//! The tokens need to be valid rust tokens, which means you cannot use
//! single-quoted strings, should use `//`-comments instead of `#`-comments,
//! cannot use `u""`-strings, etc.
//!
//! A later version of this crate will provide workarounds for some of these
//! things.
//!
//! ## Using Rust variables
//!
//! To reference rust variables, use `'var`, as shown in the example above.
//! `var` needs to implement [`pyo3::ToPyObject`].
//!
//! ## Getting information back
//!
//! Right now, this crate provides no easy way to get information from the
//! Python code back into Rust. Support for that will be added in a later
//! version of this crate.

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
