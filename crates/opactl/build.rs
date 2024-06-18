fn main() {
	//Create a .VERSION file containing 'local' if it does not exist

	let version_file = std::path::Path::new("../../.VERSION");
	if !version_file.exists() {
		std::fs::write(version_file, "local").expect("Unable to write file");
	}
}
