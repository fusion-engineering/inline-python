#![recursion_limit = "128"]
#![feature(proc_macro_span)]

extern crate proc_macro;

use proc_macro::TokenStream as TokenStream1;
use proc_macro2::{Delimiter, LineColumn, Spacing, TokenStream, TokenTree};
use quote::quote;
use std::collections::BTreeSet;
use std::fmt::Write;

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

	let q = quote! {
		{
			let _python_lock = ::inline_python::pyo3::Python::acquire_gil();
			let mut _python_variables = ::inline_python::pyo3::types::PyDict::new(_python_lock.python());
			#variables
			let r = ::inline_python::run_python_code(
				_python_lock.python(),
				unsafe { ::inline_python::CStr::from_bytes_with_nul_unchecked(#python.as_bytes()) },
				unsafe { ::inline_python::CStr::from_bytes_with_nul_unchecked(#filename.as_bytes()) },
				Some(_python_variables)
			);
			match r {
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
							panic!()
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
