/// Error indicating why a variable value could not be returned
#[derive(Debug)]
pub enum PyVarError {
	NotFound(String, String),
	WrongType(String),
}

impl std::error::Error for PyVarError {}

impl std::fmt::Display for PyVarError {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		match self {
			PyVarError::NotFound(name, r#type) => write!(f, "Unable to convert `{name}` to `{type}`"),
			PyVarError::WrongType(name) => write!(f, "Python context does not contain a variable named `{name}`"),
		}
	}
}
