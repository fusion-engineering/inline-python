use inline_python::{python, PyVarError};

#[test]
fn continue_context() {
	let c = inline_python::Context::new();
	c.run(python! {
		foo = 5
	});
	c.run(python! {
		assert foo == 5
	});
}

#[test]
fn extract_global() {
	let c = inline_python::Context::new();

	c.run(python! {
		foo = 5
	});

	assert_eq!(c.get::<i32>("foo").unwrap(), 5);
}

#[test]
fn wrong_type() {
	let c = inline_python::Context::new();

	c.run(python! {
		foo = 5
	});

	assert!(matches!(c.get::<String>("foo").unwrap_err(), PyVarError::WrongType(_)));
}

#[test]
fn not_found() {
	let c = inline_python::Context::new();

	c.run(python! {
		foo = 5
	});

	assert!(matches!(c.get::<i32>("bar").unwrap_err(), PyVarError::NotFound(_, _)));
}
