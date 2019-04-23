#![recursion_limit = "128"]
#![feature(proc_macro_span)]

extern crate proc_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Delimiter, LineColumn, Spacing, TokenStream, TokenTree};
use quote::quote;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::ptr::NonNull;
use std::os::raw::c_char;

use pyo3::{ffi, AsPyPointer, PyErr, PyObject, Python};

#[proc_macro]
pub fn python(input: TokenStream1) -> TokenStream1 {
	let mut x = EmbedPython {
		python: String::new(),
		variables: TokenStream::new(),
		variable_names: BTreeSet::new(),
		loc: LineColumn { line: 1, column: 0 },
		first_indent: None,
	};

	x.add(TokenStream::from(input.clone()));

	let EmbedPython {
		mut python, variables, ..
	} = x;

	let mut filename = input.into_iter().next().map_or_else(
		|| String::from("<unknown>"),
		|t| t.span().source_file().path().to_string_lossy().into_owned(),
	);

	python.push('\0');
	filename.push('\0');

	let compiled = unsafe {
		let gil = Python::acquire_gil();
		let py  = gil.python();

		let compiled_code = match NonNull::new(ffi::Py_CompileString(as_c_str(&python), as_c_str(&filename), ffi::Py_file_input)) {
			None => panic!("{}", compile_error_msg(py)),
			Some(x) => PyObject::from_owned_ptr(py, x.as_ptr()),
		};

		python_marshal_object_to_bytes(py, &compiled_code).expect("marshalling compiled python code failed")
	};

	let compiled = syn::LitByteStr::new(&compiled, proc_macro2::Span::call_site());


	let q = quote! {
		{
			let _python_lock = ::inline_python::pyo3::Python::acquire_gil();
			let mut _python_variables = ::inline_python::pyo3::types::PyDict::new(_python_lock.python());
			#variables
			let r = ::inline_python::run_python_code(
				_python_lock.python(),
				#compiled,
				Some(_python_variables)
			);
			match r {
				Ok(_) => (),
				Err(e) => {
					e.print(_python_lock.python());
					panic!("python!{...} failed to execute");
				}
			}
		}
	};

	q.into()
}

unsafe fn as_c_str<T: AsRef<[u8]> + ?Sized>(value: &T) -> *const c_char {
	std::ffi::CStr::from_bytes_with_nul_unchecked(value.as_ref()).as_ptr()
}

extern "C" {
	fn PyMarshal_WriteObjectToString(object: *mut ffi::PyObject, version: std::os::raw::c_int) ->  *mut ffi::PyObject;
}

/// Use built-in python marshal support to turn an object into bytes.
fn python_marshal_object_to_bytes(py: Python, object: &PyObject) -> pyo3::PyResult<Vec<u8>> {
	unsafe {
		let bytes = PyMarshal_WriteObjectToString(object.as_ptr(), 2);
		if bytes.is_null() {
			return Err(PyErr::fetch(py))
		}

		let mut buffer = std::ptr::null_mut();
		let mut size  = 0isize;
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

/// Convert a python object to a string using the the python `str()` function.
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
		pyo3::PyErrValue::None        => None,
		pyo3::PyErrValue::Value(x)    => Some(x),
		pyo3::PyErrValue::ToArgs(x)   => Some(x.arguments(py)),
		pyo3::PyErrValue::ToObject(x) => Some(x.to_object(py)),
	}
}

fn compile_error_msg(py: Python) -> String {
	use pyo3::type_object::PyTypeObject;
	use pyo3::AsPyRef;

	if !PyErr::occurred(py) {
		return String::from("failed to compile python code, but no detailed error is available");
	}

	let error = PyErr::fetch(py);

	if error.matches(py, pyo3::exceptions::SyntaxError::type_object()) {
		let PyErr { ptype: kind, pvalue: value, .. } = error;
		let value = match err_value_object(py, value) {
			None    => return kind.as_ref(py).name().into_owned(),
			Some(x) => x,
		};

		return match value.extract::<(String, (String, i32, i32, String))>(py) {
			Ok((msg, (file, line, col, _token))) => format!("{} at {}:{}:{}", msg, file, line, col),
			Err(_) => python_str(&value),
		};
	}

	let PyErr { ptype: kind, pvalue: value, .. } = error;
	match err_value_object(py, value) {
		None    => kind.as_ref(py).name().into_owned(),
		Some(x) => python_str(&x),
	}
}

struct EmbedPython {
	python: String,
	variables: TokenStream,
	variable_names: BTreeSet<String>,
	first_indent: Option<usize>,
	loc: LineColumn,
}

impl EmbedPython {
	fn add_whitespace(&mut self, loc: LineColumn) {
		if loc.line > self.loc.line {
			while loc.line > self.loc.line {
				self.python.push('\n');
				self.loc.line += 1;
			}
			let first_indent = *self.first_indent.get_or_insert(loc.column);
			let indent = loc.column.checked_sub(first_indent);
			let indent =
				indent.unwrap_or_else(|| panic!("Invalid indentation on line {}", loc.line));
			for _ in 0..indent {
				self.python.push(' ');
			}
			self.loc.column = loc.column;
		} else if loc.line == self.loc.line {
			while loc.column > self.loc.column {
				self.python.push(' ');
				self.loc.column += 1;
			}
		}
	}

	fn add(&mut self, input: TokenStream) {
		let mut tokens = input.into_iter();

		while let Some(token) = tokens.next() {
			self.add_whitespace(token.span().start());

			match &token {
				TokenTree::Group(x) => {
					let (start, end) = match x.delimiter() {
						Delimiter::Parenthesis => ("(", ")"),
						Delimiter::Brace => ("{", "}"),
						Delimiter::Bracket => ("[", "]"),
						Delimiter::None => ("", ""),
					};
					self.python.push_str(start);
					self.loc.column += start.len();
					self.add(x.stream());
					let mut end_loc = token.span().end();
					end_loc.column = end_loc.column.saturating_sub(end.len());
					self.add_whitespace(end_loc);
					self.python.push_str(end);
					self.loc.column += end.len();
				}
				TokenTree::Punct(x) => {
					if x.as_char() == '\'' && x.spacing() == Spacing::Joint {
						let name = if let Some(TokenTree::Ident(name)) = tokens.next() {
							name
						} else {
							unreachable!()
						};
						let pyname = format!("_rust_{}", name);
						let name_str = name.to_string();
						self.python.push_str(&pyname);
						self.loc.column += name_str.chars().count() + 1;
						if self.variable_names.insert(name_str) {
							self.variables.extend(quote! {
								_python_variables.set_item(#pyname, #name)
									.expect("Unable to convert variable to Python");
							});
						}
					} else if x.as_char() == '#' && x.spacing() == Spacing::Joint {
						// Convert '##' to '//', because otherwise it's
						// impossible to use the Python operators '//' and '//='.
						match tokens.next() {
							Some(TokenTree::Punct(ref p)) if p.as_char() == '#' => {
								self.python.push_str("//");
								self.loc.column += 2;
							}
							Some(TokenTree::Punct(p)) => {
								self.python.push(x.as_char());
								self.python.push(p.as_char());
								self.loc.column += 2;
							}
							_ => {
								unreachable!();
							}
						}
					} else {
						self.python.push(x.as_char());
						self.loc.column += 1;
					}
				}
				TokenTree::Ident(x) => {
					write!(&mut self.python, "{}", x).unwrap();
					self.loc = token.span().end();
				}
				TokenTree::Literal(x) => {
					write!(&mut self.python, "{}", x).unwrap();
					self.loc = token.span().end();
				}
			}
		}
	}
}
