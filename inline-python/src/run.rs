use crate::Context;
use pyo3::{ffi, types::PyAny, AsPyPointer, PyErr, PyObject, PyResult, Python};
use std::os::raw::c_char;

extern "C" {
	fn PyMarshal_ReadObjectFromString(data: *const c_char, len: isize) -> *mut ffi::PyObject;
}

pub fn run_python_code<'p>(py: Python<'p>, context: &Context, bytecode: &[u8]) -> PyResult<&'p PyAny> {
	unsafe {
		let object = PyMarshal_ReadObjectFromString(bytecode.as_ptr() as *const c_char, bytecode.len() as isize);
		if object.is_null() {
			return Err(PyErr::fetch(py));
		}
		let compiled_code = PyObject::from_owned_ptr(py, object);
		let result = ffi::PyEval_EvalCode(compiled_code.as_ptr(), context.globals.as_ptr(), std::ptr::null_mut());
		py.from_owned_ptr_or_err(result)
	}
}
