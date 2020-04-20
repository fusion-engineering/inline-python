#![feature(proc_macro_hygiene)]

use inline_python::python;

fn main() {
	let data = vec![(4, 3), (2, 8), (3, 1), (4, 0)];
	python! {
		import matplotlib.pyplot as plt
		plt.plot('data)
		plt.show()
	}
}
