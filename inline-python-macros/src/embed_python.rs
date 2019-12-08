use proc_macro2::{Delimiter, LineColumn, Spacing, Span, TokenStream, TokenTree};
use quote::quote;
use std::collections::BTreeSet;
use std::fmt::Write;

pub struct EmbedPython {
	pub python: String,
	pub variables: TokenStream,
	pub variable_names: BTreeSet<String>,
	pub first_indent: Option<usize>,
	pub loc: LineColumn,
}

impl EmbedPython {
	pub fn new() -> Self {
		Self {
			python: String::new(),
			variables: TokenStream::new(),
			variable_names: BTreeSet::new(),
			loc: LineColumn { line: 1, column: 0 },
			first_indent: None,
		}
	}

	fn add_whitespace(&mut self, span: Span, loc: LineColumn) -> syn::Result<()> {
		if loc.line > self.loc.line {
			while loc.line > self.loc.line {
				self.python.push('\n');
				self.loc.line += 1;
			}
			let first_indent = *self.first_indent.get_or_insert(loc.column);
			let indent = loc.column.checked_sub(first_indent);
			let indent = indent.ok_or_else(|| error!(span, "Invalid indentation on line {}", loc.line))?;
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

		Ok(())
	}

	pub fn add(&mut self, input: TokenStream) -> syn::Result<()> {
		let mut tokens = input.into_iter();

		while let Some(token) = tokens.next() {
			let span = token.span();
			self.add_whitespace(span, token.span().start())?;

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
					self.add(x.stream())?;
					let mut end_loc = token.span().end();
					end_loc.column = end_loc.column.saturating_sub(end.len());
					self.add_whitespace(span, end_loc)?;
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
						let name_str = format!("_RUST_{}", name);
						self.python.push_str(&name_str);
						self.loc.column += name_str.chars().count() - 6 + 1;
						if self.variable_names.insert(name_str.clone()) {
							self.variables.extend(quote! {
								globals.set_item(#name_str, #name)
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

		Ok(())
	}
}
