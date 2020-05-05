#![feature(proc_macro_span)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use self::embed_python::EmbedPython;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Literal, Span, TokenStream};
use pyo3::{ffi, AsPyPointer, PyErr, PyObject, Python};
use quote::quote;
use std::os::raw::c_char;
use std::ptr::NonNull;

mod embed_python;

fn python_impl(input: TokenStream) -> Result<TokenStream, ()> {
	let tokens = input.clone();

	check_no_attribute(input.clone())?;

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
			None => {
				emit_compile_error_msg(py, tokens);
				return Err(());
			}
			Some(x) => PyObject::from_owned_ptr(py, x.as_ptr()),
		};

		python_marshal_object_to_bytes(py, &compiled_code)
			// TODO: Use error from Pyo3.
			.map_err(|_e| Span::call_site().unwrap().error("failed to generate python byte-code").emit())?
	};

	let compiled = Literal::byte_string(&compiled);

	let varname = variables.keys();
	let var = variables.values();

	Ok(quote! {
		::inline_python::FromInlinePython::from_python_macro(
			#compiled,
			|globals| {
				#(
					globals
						.set_item(#varname, #var)
						.expect("Unable to convert variable to Python");
				)*
			},
		)
	})
}

fn check_no_attribute(input: TokenStream) -> Result<(), ()> {
	let mut input = input.into_iter();
	if let Some(token) = input.next() {
		if token.to_string() == "#"
			&& input.next().map_or(false, |t| t.to_string() == "!")
			&& input.next().map_or(false, |t| t.to_string().starts_with('['))
		{
			token
				.span()
				.unwrap()
				.error(
					"Attributes in python!{} are no longer supported. \
					Use context.run(python!{..}) to use a context.",
				)
				.emit();
			return Err(());
		}
	}
	Ok(())
}

#[proc_macro]
pub fn python(input: TokenStream1) -> TokenStream1 {
	TokenStream1::from(match python_impl(TokenStream::from(input)) {
		Ok(tokens) => tokens,
		Err(()) => TokenStream::new(),
	})
}

unsafe fn as_c_str<T: AsRef<[u8]> + ?Sized>(value: &T) -> *const c_char {
	std::ffi::CStr::from_bytes_with_nul_unchecked(value.as_ref()).as_ptr()
}

/// Use built-in python marshal support to turn an object into bytes.
fn python_marshal_object_to_bytes(py: Python, object: &PyObject) -> pyo3::PyResult<Vec<u8>> {
	unsafe {
		let bytes = ffi::PyMarshal_WriteObjectToString(object.as_ptr(), 2);
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
fn emit_compile_error_msg(py: Python, tokens: TokenStream) {
	use pyo3::type_object::PyTypeObject;
	use pyo3::AsPyRef;

	if !PyErr::occurred(py) {
		Span::call_site()
			.unwrap()
			.error("failed to compile python code, but no detailed error is available")
			.emit();
		return;
	}

	let error = PyErr::fetch(py);

	if error.matches(py, pyo3::exceptions::SyntaxError::type_object()) {
		let PyErr {
			ptype: kind,
			pvalue: value,
			..
		} = error;

		let value = match err_value_object(py, value) {
			None => {
				return Span::call_site()
					.unwrap()
					.error(format!("python: {}", kind.as_ref(py).name()))
					.emit()
			}
			Some(x) => x,
		};

		match value.extract::<(String, (String, i32, i32, String))>(py) {
			Ok((msg, (file, line, col, _token))) => match span_for_line(tokens, line as usize, col as usize) {
				Some(span) => span.unwrap().error(format!("python: {}", msg)).emit(),
				None => Span::call_site()
					.unwrap()
					.error(format!("python: {} at {}:{}:{}", msg, file, line, col))
					.emit(),
			},
			Err(_) => Span::call_site().unwrap().error(format!("python: {}", python_str(&value))).emit(),
		}

		return;
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

	Span::call_site().unwrap().error(format!("python: {}", message)).emit();
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
