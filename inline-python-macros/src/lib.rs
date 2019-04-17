#![recursion_limit="128"]

extern crate proc_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::quote;
use std::fmt::Write;
use std::collections::BTreeSet;
use syn::parse_macro_input;

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
