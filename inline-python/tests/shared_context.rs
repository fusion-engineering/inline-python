#![feature(proc_macro_hygiene)]

use inline_python::python;

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

	assert_eq!(c.get::<i32>("foo"), 5);
}
