use proc_macro::Span;
use proc_macro2::{Delimiter, Ident, Spacing, TokenStream, TokenTree};
use quote::quote_spanned;
use std::collections::BTreeMap;
use std::fmt::Write;

pub struct EmbedPython {
	pub python: String,
	pub variables: BTreeMap<String, Ident>,
	pub first_indent: Option<usize>,
	pub line: usize,
	pub column: usize,
	pub compile_time: bool,
}

impl EmbedPython {
	pub fn new() -> Self {
		Self {
			python: String::new(),
			variables: BTreeMap::new(),
			line: 1,
			column: 0,
			first_indent: None,
			compile_time: false,
		}
	}

	fn add_whitespace(&mut self, span: Span, line: usize, column: usize) -> Result<(), TokenStream> {
		#[allow(clippy::comparison_chain)]
		if line > self.line {
			while line > self.line {
				self.python.push('\n');
				self.line += 1;
			}
			let first_indent = *self.first_indent.get_or_insert(column);
			let indent = column.checked_sub(first_indent);
			let indent = indent.ok_or_else(|| quote_spanned!(span.into() => compile_error!{"Invalid indentation"}))?;
			for _ in 0..indent {
				self.python.push(' ');
			}
			self.column = column;
		} else if line == self.line {
			while column > self.column {
				self.python.push(' ');
				self.column += 1;
			}
		}

		Ok(())
	}

	pub fn add(&mut self, input: TokenStream) -> Result<(), TokenStream> {
		let mut tokens = input.into_iter();

		while let Some(token) = tokens.next() {
			let span = token.span().unwrap();
			self.add_whitespace(span, span.line(), span.column())?;

			match &token {
				TokenTree::Group(x) => {
					let (start, end) = match x.delimiter() {
						Delimiter::Parenthesis => ("(", ")"),
						Delimiter::Brace => ("{", "}"),
						Delimiter::Bracket => ("[", "]"),
						Delimiter::None => ("", ""),
					};
					self.python.push_str(start);
					self.column += start.len();
					self.add(x.stream())?;
					let end_span = token.span().unwrap().end();
					self.add_whitespace(span, end_span.line(), end_span.column().saturating_sub(end.len()))?;
					self.python.push_str(end);
					self.column += end.len();
				}
				TokenTree::Punct(x) => {
					if !self.compile_time && x.as_char() == '\'' && x.spacing() == Spacing::Joint {
						let name = if let Some(TokenTree::Ident(name)) = tokens.next() {
							name
						} else {
							unreachable!()
						};
						let name_str = format!("_RUST_{}", name);
						self.python.push_str(&name_str);
						self.column += name_str.chars().count() - 6 + 1;
						self.variables.entry(name_str).or_insert(name);
					} else if x.as_char() == '#' && x.spacing() == Spacing::Joint {
						// Convert '##' to '//', because otherwise it's
						// impossible to use the Python operators '//' and '//='.
						match tokens.next() {
							Some(TokenTree::Punct(ref p)) if p.as_char() == '#' => {
								self.python.push_str("//");
								self.column += 2;
							}
							Some(TokenTree::Punct(p)) => {
								self.python.push(x.as_char());
								self.python.push(p.as_char());
								self.column += 2;
							}
							_ => {
								unreachable!();
							}
						}
					} else {
						self.python.push(x.as_char());
						self.column += 1;
					}
				}
				TokenTree::Ident(x) => {
					write!(&mut self.python, "{}", x).unwrap();
					let end_span = token.span().unwrap().end();
					self.line = end_span.line();
					self.column = end_span.column();
				}
				TokenTree::Literal(x) => {
					let s = x.to_string();
					// Remove space in prefixed strings like `f ".."`.
					// (`f".."` is not allowed in some versions+editions of Rust.)
					if s.starts_with('"')
						&& self.python.ends_with(' ')
						&& self.python[..self.python.len() - 1].ends_with(|c: char| c.is_ascii_alphabetic())
					{
						self.python.pop();
					}
					self.python += &s;
					let end_span = token.span().unwrap().end();
					self.line = end_span.line();
					self.column = end_span.column();
				}
			}
		}

		Ok(())
	}
}
