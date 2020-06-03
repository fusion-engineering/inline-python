use crate::error::emit_compile_error_msg;
use proc_macro2::{Span, TokenStream};
use pyo3::{ffi, AsPyPointer, PyObject, PyResult, Python};
use std::str::FromStr;

fn run_and_capture(py: Python, code: PyObject) -> PyResult<String> {
	let globals = py.import("__main__")?.dict().copy()?;

	let sys = py.import("sys")?;
	let io = py.import("io")?;

	let stdout = io.call0("StringIO")?;
	let original_stdout = sys.dict().get_item("stdout");
	sys.dict().set_item("stdout", stdout)?;

	let result =
		unsafe { PyObject::from_owned_ptr_or_err(py, ffi::PyEval_EvalCode(code.as_ptr(), globals.as_ptr(), std::ptr::null_mut())) };

	sys.dict().set_item("stdout", original_stdout)?;

	result?;

	stdout.call_method0("getvalue")?.extract()
}

pub fn run_ct_python(py: Python, code: PyObject, tokens: TokenStream) -> Result<TokenStream, ()> {
	let output = run_and_capture(py, code).map_err(|err| emit_compile_error_msg(py, err, tokens))?;

	Ok(TokenStream::from_str(&output).map_err(|e| {
		Span::call_site()
			.unwrap()
			.error(format!("Unable to parse output of ct_python!{{}} script: {:?}", e))
			.emit()
	})?)
}
