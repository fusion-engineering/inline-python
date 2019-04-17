#![recursion_limit="128"]

extern crate proc_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::quote;
use std::fmt::Write;
use std::collections::BTreeSet;
use syn::parse_macro_input;

/// Execute Python code directly from Rust.
///
/// The macro interprets all given tokens as Python code.
/// See the main module documentation for more details on what is allowed in the Python code.
/// The main module documentation also explains how to refer to Rust objects from the Python code.
///
/// This macro doesn't return a value.
/// If the python code raises an error, the error is printed and the program panics.
///
/// See [`python_ex`] for a version that does return a value.
///
/// An example:
/// ```no_run
/// #![feature(proc_macro_hygiene)]
/// use inline_python::python;
/// let data = vec![(4, 3), (2, 8), (3, 1), (4, 0)];
/// python! {
///   import matplotlib.pyplot as plot
///   plot.plot('data)
///   plot.show()
/// }
/// ```
#[proc_macro]
pub fn python(input: TokenStream1) -> TokenStream1 {
	let mut x = EmbedPython {
		code: String::new(),
		variables: TokenStream::new(),
		variable_names: BTreeSet::new(),
		line: 0,
		indent: None,
	};

	x.add(TokenStream::from(input));

	let EmbedPython {
		code, variables, ..
	} = x;

	TokenStream1::from(quote! {
		{
			let _python_lock = ::inline_python::pyo3::Python::acquire_gil();
			let mut _python_variables = ::inline_python::pyo3::types::PyDict::new(_python_lock.python());
			#variables
			match _python_lock.python().run(#code, None, Some(_python_variables)) {
				Ok(_) => (),
				Err(e) => {
					e.print(_python_lock.python());
					panic!("Python code failed");
				}
			}
		}
	})
}

struct PythonExArgs {
	pub python: syn::Expr,
	pub comma: syn::token::Comma,
	pub code: TokenStream,
}

impl syn::parse::Parse for PythonExArgs {
	fn parse(input: syn::parse::ParseStream) -> syn::parse::Result<Self> {
		Ok(Self {
			python: input.parse()?,
			comma: input.parse()?,
			code: input.parse()?,
		})
	}
}


/// Execute Python code directly from Rust with a return value.
///
/// The macro can be invoked as `python_ex!(python, code ...)`, where the first argument is a `pyo3::Python` object.
/// See the main module documentation for more details on what is allowed in the Python code.
/// The main module documentation also explains how to refer to Rust objects from the Python code.
///
/// This macro returns the evaluation result of the python code.
/// If the python code raises an error, it is returned as an `Err(pyo3::PyErr)`.
/// If the code runs successfully, the value is returned as `Ok(pyo3::PyAny)`.
///
/// See [`python_ex`] for a version that does return a value.
///
/// An example:
/// ```no_run
/// #![feature(proc_macro_hygiene)]
/// use inline_python::{pyo3, python_ex, pyo3::FromPyObject};
/// # fn main() -> pyo3::PyResult<()> {
/// let gil       = pyo3::Python::acquire_gil();
///
/// let result = python_ex!{gil.python(), 1 + 1}?;
/// let result = u32::extract(result)?;
/// assert_eq!(result, 2);
///
/// #  Ok(())
/// # }
/// ```
#[proc_macro]
pub fn python_ex(input: TokenStream1) -> TokenStream1 {
	let PythonExArgs {
		python, code, ..
	} =  parse_macro_input!(input as PythonExArgs);

	let mut x = EmbedPython {
		code: String::new(),
		variables: TokenStream::new(),
		variable_names: BTreeSet::new(),
		line: 0,
		indent: None,
	};

	x.add(code);

	let EmbedPython {
		code, variables, ..
	} = x;

	TokenStream1::from(quote! {
		{
			let mut _python_variables = ::inline_python::pyo3::types::PyDict::new(#python);
			#variables
			#python.run(#code, None, Some(_python_variables))
		}
	})
}

struct EmbedPython {
	code: String,
	variables: TokenStream,
	variable_names: BTreeSet<String>,
	line: usize,
	indent: Option<usize>,
}

impl EmbedPython {
	fn add(&mut self, input: TokenStream) {
		let mut tokens = input.into_iter();

		while let Some(token) = tokens.next() {
			let loc = token.span().start();

			if loc.line != self.line {
				self.code.push('\n');
				let indent = *self.indent.get_or_insert(loc.column);
				for _ in 0..(loc.column.saturating_sub(indent)) {
					self.code.push(' ');
				}
				self.line = loc.line;
			}

			match &token {
				TokenTree::Group(x) => {
					let (start, end) = match x.delimiter() {
						Delimiter::Parenthesis => ('(', ')'),
						Delimiter::Brace => ('{', '}'),
						Delimiter::Bracket => ('[', ']'),
						Delimiter::None => (' ', ' '),
					};
					self.code.push(start);
					self.add(x.stream());
					self.code.push(end);
				}
				TokenTree::Punct(x) => {
					if x.as_char() == '\'' && x.spacing() == Spacing::Joint {
						let name = if let Some(TokenTree::Ident(name)) = tokens.next() {
							name
						} else {
							panic!()
						};
						let pyname = format!("_rust_{}", name);
						if self.variable_names.insert(name.to_string()) {
							self.variables.extend(quote! {
								_python_variables.set_item(#pyname, #name)
									.expect("Unable to convert variable to Python");
							});
						}
						self.code.push_str(&pyname);
						self.code.push(' ');
					} else {
						self.code.push(x.as_char());
						if x.spacing() == Spacing::Alone {
							self.code.push(' ');
						}
					}
				}
				TokenTree::Ident(x) => write!(&mut self.code, "{} ", x).unwrap(),
				TokenTree::Literal(x) => write!(&mut self.code, "{} ", x).unwrap(),
			}
		}
	}
}
