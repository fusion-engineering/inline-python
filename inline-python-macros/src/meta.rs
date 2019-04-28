use syn::{
	bracketed,
	parse::{Parse, ParseStream},
	punctuated::Punctuated,
	token,
};

pub struct NameValue {
	pub name: syn::Ident,
	pub eq: token::Eq,
	pub value: syn::Expr,
}

pub struct Meta {
	pub pound: token::Pound,
	pub bang: token::Bang,
	pub bracket: token::Bracket,
	pub args: Punctuated<NameValue, token::Comma>,
}

impl Parse for NameValue {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		Ok(Self {
			name: input.parse()?,
			eq: input.parse()?,
			value: input.parse()?,
		})
	}
}

impl Parse for Meta {
	fn parse(input: ParseStream) -> syn::Result<Self> {
		let content;
		Ok(Self {
			pound: input.parse()?,
			bang: input.parse()?,
			bracket: bracketed!(content in input),
			args: Punctuated::parse_terminated(&content)?,
		})
	}
}

impl Meta {
	pub fn peek(input: ParseStream) -> bool {
		input.peek(token::Pound) && input.peek2(token::Bang)
	}

	pub fn maybe_parse(input: ParseStream) -> syn::Result<Option<Self>> {
		if !Self::peek(input) {
			Ok(None)
		} else {
			Self::parse(input).map(Some)
		}
	}
}
