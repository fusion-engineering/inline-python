#![feature(proc_macro_span)]
#![feature(proc_macro_diagnostic)]

extern crate proc_macro;

use self::embed_python::EmbedPython;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Literal, Span, TokenStream};
use pyo3::{ffi, types::PyBytes, AsPyPointer, FromPyPointer, PyErr, PyObject, Python, ToPyObject};
use quote::quote;
use std::ffi::CString;

mod embed_python;

fn python_impl(input: TokenStream) -> Result<TokenStream, ()> {
	let tokens = input.clone();

	check_no_attribute(input.clone())?;

	let filename = Span::call_site().unwrap().source_file().path().to_string_lossy().into_owned();

	let mut x = EmbedPython::new();

	x.add(input)?;

	let EmbedPython { python, variables, .. } = x;

	let python = CString::new(python).unwrap();
	let filename = CString::new(filename).unwrap();

	let bytecode = unsafe {
		let gil = Python::acquire_gil();
		let py = gil.python();

		let code = PyObject::from_owned_ptr_or_err(py, ffi::Py_CompileString(python.as_ptr(), filename.as_ptr(), ffi::Py_file_input))
			.map_err(|err| emit_compile_error_msg(py, err, tokens))?;

		Literal::byte_string(
			PyBytes::from_owned_ptr_or_err(py, ffi::PyMarshal_WriteObjectToString(code.as_ptr(), pyo3::marshal::VERSION))
				.map_err(|_e| Span::call_site().unwrap().error("failed to generate python bytecode").emit())?
				.as_bytes(),
		)
	};

	let varname = variables.keys();
	let var = variables.values();

	Ok(quote! {
		::inline_python::FromInlinePython::from_python_macro(
			#bytecode,
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

/// Format a nice error message for a python compilation error.
fn emit_compile_error_msg(py: Python, error: PyErr, tokens: TokenStream) {
	use pyo3::type_object::PyTypeObject;
	use pyo3::AsPyRef;

	let value = error.to_object(py);

	if value.is_none() {
		Span::call_site()
			.unwrap()
			.error(format!("python: {}", error.ptype.as_ref(py).name()))
			.emit();
		return;
	}

	if error.matches(py, pyo3::exceptions::SyntaxError::type_object()) {
		let line: Option<usize> = value.getattr(py, "lineno").ok().and_then(|x| x.extract(py).ok());
		let msg: Option<String> = value.getattr(py, "msg").ok().and_then(|x| x.extract(py).ok());
		if let (Some(line), Some(msg)) = (line, msg) {
			if let Some(span) = span_for_line(tokens, line) {
				span.unwrap().error(format!("python: {}", msg)).emit();
				return;
			}
		}
	}

	Span::call_site()
		.unwrap()
		.error(format!("python: {}", value.as_ref(py).str().unwrap()))
		.emit();
}

/// Get a span for a specific line of input from a TokenStream.
fn span_for_line(input: TokenStream, line: usize) -> Option<Span> {
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
