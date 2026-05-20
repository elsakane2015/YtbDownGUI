fn main() {
    // Pick up the Xcode-style build number from `.buildnumber` at the repo
    // root. Falls back to "000" if the file is missing or unreadable so
    // local cargo invocations don't break.
    let build_number = std::fs::read_to_string("../.buildnumber")
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "000".to_string());
    println!("cargo:rustc-env=APP_BUILD_NUMBER={build_number}");
    println!("cargo:rerun-if-changed=../.buildnumber");

    tauri_build::build()
}
