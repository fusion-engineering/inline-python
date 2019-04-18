#![recursion_limit = "128"]

extern crate proc_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use quote::quote;
use std::collections::BTreeSet;
use std::fmt::Write;

#[proc_macro]
pub fn python(input: TokenStream1) -> TokenStream1 {
	let mut x = EmbedPython {
		python: String::new(),
		variables: TokenStream::new(),
		variable_names: BTreeSet::new(),
		line: 0,
		indent: None,
	};

	x.add(TokenStream::from(input));

	let EmbedPython {
		python, variables, ..
	} = x;

	let q = quote! {
		{
			let _python_lock = ::inline_python::pyo3::Python::acquire_gil();
			let mut _python_variables = ::inline_python::pyo3::types::PyDict::new(_python_lock.python());
			#variables
			match _python_lock.python().run(#python, None, Some(_python_variables)) {
				Ok(_) => (),
				Err(e) => {
					e.print(_python_lock.python());
					panic!("Python code failed");
				}
			}
		}
	};

	q.into()
}

struct EmbedPython {
	python: String,
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
				self.python.push('\n');
				let indent = *self.indent.get_or_insert(loc.column);
				for _ in 0..(loc.column.saturating_sub(indent)) {
					self.python.push(' ');
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
					self.python.push(start);
					self.add(x.stream());
					self.python.push(end);
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
						self.python.push_str(&pyname);
						self.python.push(' ');
					} else {
						self.python.push(x.as_char());
						if x.spacing() == Spacing::Alone {
							self.python.push(' ');
						}
					}
				}
				TokenTree::Ident(x) => write!(&mut self.python, "{} ", x).unwrap(),
				TokenTree::Literal(x) => write!(&mut self.python, "{} ", x).unwrap(),
			}
		}
	}
}
