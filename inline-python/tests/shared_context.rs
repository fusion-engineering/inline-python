#![feature(proc_macro_hygiene)]

use inline_python::{pyo3, python};

#[test]
fn continue_context() {
	let context = inline_python::Context::new();
	python! {
		#![context = &context]
		foo = 5
	}
	python! {
		#![context = &context]
		assert foo == 5
	}
}

#[test]
fn extract_global() {
	let context = inline_python::Context::new();
	python! {
		#![context = &context]
		foo = 5
	}

	let gil = pyo3::Python::acquire_gil();
	let py = gil.python();

	assert_eq!(context.get_global(py, "foo").unwrap(), Some(5));
}
