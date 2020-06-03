#![feature(proc_macro_hygiene)]

use inline_python_macros::ct_python;

static DIRECTIONS: [(f64, f64); 32] = ct_python! {
	from math import sin, cos, tau
	n = 32
	print("[")
	for i in range(n):
		x = cos(i / n * tau)
		y = sin(i / n * tau)
		print(f"({x}, {y}),")
	print("]")
};

fn main() {
	dbg!(&DIRECTIONS);
}
