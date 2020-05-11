use crate::Context;
use pyo3::{ffi, types::PyAny, AsPyPointer, PyObject, PyResult, Python};

pub fn run_python_code<'p>(py: Python<'p>, context: &Context, bytecode: &[u8]) -> PyResult<&'p PyAny> {
	unsafe {
		let ptr = ffi::PyMarshal_ReadObjectFromString(bytecode.as_ptr() as *const _, bytecode.len() as isize);
		let code = PyObject::from_owned_ptr_or_err(py, ptr)?;
		let result = ffi::PyEval_EvalCode(code.as_ptr(), context.globals.as_ptr(), std::ptr::null_mut());
		py.from_owned_ptr_or_err(result)
	}
}
