#![feature(proc_macro_hygiene)]

use inline_python::{python, Context};

fn main() {
    let c: Context = python! {
            import oct2py
            oc = oct2py.Oct2Py()
            m = oc.magic(3)
		};

    dbg!(c.get::<Vec<Vec<f64>>>("m"));
}
