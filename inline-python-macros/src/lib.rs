#![recursion_limit = "128"]
#![feature(proc_macro_span)]

extern crate proc_macro;

use self::embed_python::EmbedPython;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Span, TokenStream};
use pyo3::{ffi, AsPyPointer, PyErr, PyObject, Python};
use quote::quote;
use std::os::raw::c_char;
use std::ptr::NonNull;

/// Create a syn::Error with an optional span, and a format string with arguments.
///
/// If no span is given, it defaults to Span::call_site().
///
/// For example:
/// ```no_compile
/// error!(token.span(), "foo: {}", "bar"); // With span.
/// error!("foo: {}", "bar"); // Without span.
/// ```
macro_rules! error {
	($format:literal $($tokens:tt)*) => ( syn::Error::new(proc_macro2::Span::call_site(), format!($format $($tokens)*)) );
	($span:expr, $format:literal $($tokens:tt)*) => ( syn::Error::new($span, format!($format $($tokens)*)) );
}

mod embed_python;

fn python_impl(input: TokenStream) -> syn::Result<TokenStream> {
	let tokens = input.clone();

	let mut filename = input.clone().into_iter().next().map_or_else(
		|| String::from("<unknown>"),
		|t| t.span().unwrap().source_file().path().to_string_lossy().into_owned(),
	);

	let mut x = EmbedPython::new();

	x.add(input)?;

	let EmbedPython { mut python, variables, .. } = x;

	python.push('\0');
	filename.push('\0');

	let compiled = unsafe {
		let gil = Python::acquire_gil();
		let py = gil.python();

		let compiled_code = match NonNull::new(ffi::Py_CompileString(as_c_str(&python), as_c_str(&filename), ffi::Py_file_input)) {
			None => return Err(compile_error_msg(py, tokens)),
			Some(x) => PyObject::from_owned_ptr(py, x.as_ptr()),
		};

		python_marshal_object_to_bytes(py, &compiled_code)
			// TODO: Use error from Pyo3.
			.map_err(|_e| error!("failed to generate python byte-code"))?
	};

	let compiled = syn::LitByteStr::new(&compiled, proc_macro2::Span::call_site());

	Ok(quote! {
		::inline_python::FromInlinePython::magic(
			#compiled,
			|globals| { #variables },
		)
	})
}

#[proc_macro]
pub fn python(input: TokenStream1) -> TokenStream1 {
	TokenStream1::from(match python_impl(TokenStream::from(input)) {
		Ok(tokens) => tokens,
		Err(error) => error.to_compile_error(),
	})
}

unsafe fn as_c_str<T: AsRef<[u8]> + ?Sized>(value: &T) -> *const c_char {
	std::ffi::CStr::from_bytes_with_nul_unchecked(value.as_ref()).as_ptr()
}

extern "C" {
	fn PyMarshal_WriteObjectToString(object: *mut ffi::PyObject, version: std::os::raw::c_int) -> *mut ffi::PyObject;
}

/// Use built-in python marshal support to turn an object into bytes.
fn python_marshal_object_to_bytes(py: Python, object: &PyObject) -> pyo3::PyResult<Vec<u8>> {
	unsafe {
		let bytes = PyMarshal_WriteObjectToString(object.as_ptr(), 2);
		if bytes.is_null() {
			return Err(PyErr::fetch(py));
		}

		let mut buffer = std::ptr::null_mut();
		let mut size = 0isize;
		ffi::PyBytes_AsStringAndSize(bytes, &mut buffer, &mut size);
		let result = Vec::from(std::slice::from_raw_parts(buffer as *const u8, size as usize));

		ffi::Py_DecRef(bytes);
		Ok(result)
	}
}

/// Convert a PyUnicode object to String.
unsafe fn py_unicode_string(object: *mut ffi::PyObject) -> String {
	let mut size = 0isize;
	let data = ffi::PyUnicode_AsUTF8AndSize(object, &mut size) as *const u8;
	let data = std::slice::from_raw_parts(data, size as usize);
	let data = std::str::from_utf8_unchecked(data);
	String::from(data)
}

/// Convert a python object to a string using the python `str()` function.
fn python_str(object: &PyObject) -> String {
	unsafe {
		let string = ffi::PyObject_Str(object.as_ptr());
		let result = py_unicode_string(string);
		ffi::Py_DecRef(string);
		result
	}
}

/// Get the object of a PyErrValue, if any.
fn err_value_object(py: Python, value: pyo3::PyErrValue) -> Option<PyObject> {
	match value {
		pyo3::PyErrValue::None => None,
		pyo3::PyErrValue::Value(x) => Some(x),
		pyo3::PyErrValue::ToArgs(x) => Some(x.arguments(py)),
		pyo3::PyErrValue::ToObject(x) => Some(x.to_object(py)),
	}
}

/// Format a nice error message for a python compilation error.
fn compile_error_msg(py: Python, tokens: TokenStream) -> syn::Error {
	use pyo3::type_object::PyTypeObject;
	use pyo3::AsPyRef;

	if !PyErr::occurred(py) {
		return error!("failed to compile python code, but no detailed error is available");
	}

	let error = PyErr::fetch(py);

	if error.matches(py, pyo3::exceptions::SyntaxError::type_object()) {
		let PyErr {
			ptype: kind,
			pvalue: value,
			..
		} = error;

		let value = match err_value_object(py, value) {
			None => return error!("python: {}", kind.as_ref(py).name()),
			Some(x) => x,
		};

		return match value.extract::<(String, (String, i32, i32, String))>(py) {
			Ok((msg, (file, line, col, _token))) => match span_for_line(tokens, line as usize, col as usize) {
				Some(span) => error!(span, "python: {}", msg),
				None => error!("python: {} at {}:{}:{}", msg, file, line, col),
			},
			Err(_) => error!("python: {}", python_str(&value)),
		};
	}

	let PyErr {
		ptype: kind,
		pvalue: value,
		..
	} = error;

	let message = match err_value_object(py, value) {
		None => kind.as_ref(py).name().into_owned(),
		Some(x) => python_str(&x),
	};

	error!("python: {}", message)
}

/// Get a span for a specific line of input from a TokenStream.
fn span_for_line(input: TokenStream, line: usize, _col: usize) -> Option<Span> {
	let mut spans = input
		.into_iter()
		.map(|x| x.span().unwrap())
		.skip_while(|span| span.start().line < line)
		.take_while(|span| span.start().line == line);

	let mut result = spans.next()?;
	for span in spans {
		result = match result.join(span) {
			None => return Some(Span::from(result)),
			Some(span) => span,
		}
	}

	Some(Span::from(result))
}
