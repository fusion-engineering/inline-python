#![feature(proc_macro_hygiene)]

use inline_python::python;

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

	assert_eq!(context.get_global("foo").unwrap(), Some(5));
}
