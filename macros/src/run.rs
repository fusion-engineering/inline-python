use crate::error::emit_compile_error_msg;
use proc_macro2::{Span, TokenStream};
use pyo3::{ffi, AsPyPointer, PyObject, PyResult, Python};
use std::str::FromStr;

#[cfg(unix)]
fn ensure_libpython_symbols_loaded(py: Python) -> PyResult<()>{
	// On Unix, Rustc loads proc-macro crates with RTLD_LOCAL, which (at least
	// on Linux) means all their dependencies (in our case: libpython) don't
	// get their symbols made available globally either. This means that
	// loading modules (e.g. `import math`) will fail, as those modules refer
	// back to symbols of libpython.
	//
	// This function tries to (re)load the right version of libpython, but this
	// time with RTLD_GLOBAL enabled.

	let sysconfig = py.import("sysconfig")?;
	let libdir: String = sysconfig.call1("get_config_var", ("LIBDIR",))?.extract()?;
	let so_name: String = sysconfig.call1("get_config_var", ("INSTSONAME",))?.extract()?;
	let path = std::ffi::CString::new(format!("{}/{}", libdir, so_name)).unwrap();
	unsafe {
		libc::dlopen(path.as_ptr(), libc::RTLD_NOW | libc::RTLD_GLOBAL);
	}
	Ok(())
}

fn run_and_capture(py: Python, code: PyObject) -> PyResult<String> {
	#[cfg(unix)]
	let _ = ensure_libpython_symbols_loaded(py);

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
