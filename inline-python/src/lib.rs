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
	AsPyPointer,
	FromPyObject,
	IntoPyObject,
	PyErr,
	PyObject,
	PyResult,
	Python,
};

#[doc(hidden)]
pub use std::ffi::CStr;

/// An executaion context for Python code.
///
/// If you pass a manually created to the [`python`] macro, you can share it across invocations.
/// This will keep all global variables and imports intact between macro invocations.
///
/// You may also use it to inspect global variables after the execution of the Python code.
pub struct Context {
	globals: PyObject,
}

impl Context {
	/// Create a new context for running python code.
	///
	/// This function temporarily acquires the GIL.
	/// If you already have the GIL, use [`new_with_gil`] instead.
	///
	/// This function panics if it fails to create the context.
	/// See [`new_checked`] for a verion that returns a result.
	pub fn new() -> Self {
		let gil = Python::acquire_gil();
		let py  = gil.python();
		match Self::new_with_gil(py) {
			Ok(x) => x,
			Err(error) => {
				error.print(py);
				panic!("failed to create python context");
			}
		}
	}

	/// Create a new context for running python code.
	///
	/// This function temporarily acquires the GIL.
	/// If you already have the GIL, use [`new_with_gil`] instead.
	pub fn new_checked() -> PyResult<Self> {
		let gil = Python::acquire_gil();
		let py  = gil.python();
		Self::new_with_gil(py)
	}

	/// Create a new context for runnin Python code.
	///
	/// You must acquire the GIL to call this function.
	pub fn new_with_gil(py: Python) -> PyResult<Self> {
		let main_mod = unsafe { ffi::PyImport_AddModule("__main__\0".as_ptr() as *const _) };
		if main_mod.is_null() {
			return Err(PyErr::fetch(py));
		};

		let globals = PyDict::new(py);
		if unsafe { ffi::PyDict_Merge(globals.as_ptr(), ffi::PyModule_GetDict(main_mod), 0) != 0 } {
			return Err(PyErr::fetch(py));
		}

		Ok(Self { globals: globals.into_object(py) })
	}

	/// Get the globals as dictionary.
	pub fn globals<'p>(&self, py: Python<'p>) -> &'p PyDict {
		unsafe { py.from_borrowed_ptr(self.globals.as_ptr()) }
	}

	/// Retrieve a global variable from the context.
	pub fn get_global<'p, T: FromPyObject<'p>>(self, py: Python<'p>, name: &str) -> PyResult<Option<T>> {
		match self.globals(py).get_item(name) {
			None => Ok(None),
			Some(value) => FromPyObject::extract(value).map(Some),
		}
	}
}

#[doc(hidden)]
pub fn run_python_code<'p>(
	py: Python<'p>,
	context: &Context,
	compiled_code: &[u8],
	rust_vars: Option<&PyDict>,
) -> PyResult<&'p PyAny> {
	unsafe {
		// Add the rust variable in a global dictionary named RUST.
		// If no rust vars are given, make the RUST global an empty dictionary.
		let rust_vars = rust_vars.unwrap_or_else(|| PyDict::new(py)).as_ptr();
		if ffi::PyDict_SetItemString(context.globals.as_ptr(), "RUST\0".as_ptr() as *const _, rust_vars) != 0 {
			return Err(PyErr::fetch(py))
		}

		let compiled_code = python_unmarshal_object_from_bytes(py, compiled_code)?;
		let result = ffi::PyEval_EvalCode(compiled_code.as_ptr(), context.globals.as_ptr(), std::ptr::null_mut());

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
