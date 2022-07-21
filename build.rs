fn main() {
	cc::Build::new()
		.cpp(true)
		.file("src/nanoflann/nanoflann_shim.cpp")
		.compile("nanoflann_shim");
}
