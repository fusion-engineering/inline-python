#![feature(proc_macro_hygiene)]

use inline_python::{pyo3, python_ex};

use pyo3::prelude::FromPyObject;

fn main() -> pyo3::PyResult<()> {
	let gil    = pyo3::Python::acquire_gil();

	let result = python_ex!(gil.python(), 1 + 1)?;
	let result = u32::extract(result)?;
	assert_eq!(result, 2);
	Ok(())
}
