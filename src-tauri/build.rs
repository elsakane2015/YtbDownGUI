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

    let license_server_url = std::env::var("YTBDOWN_LICENSE_SERVER_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://license.ytbdown.litotime.com".to_string());
    println!("cargo:rustc-env=LICENSE_SERVER_URL={license_server_url}");
    println!("cargo:rerun-if-env-changed=YTBDOWN_LICENSE_SERVER_URL");

    let license_public_key = std::env::var("YTBDOWN_LICENSE_PUBLIC_KEY").unwrap_or_default();
    if std::env::var("TAURI_ENV_PLATFORM").is_ok()
        && !cfg!(debug_assertions)
        && license_public_key.trim().is_empty()
    {
        panic!("YTBDOWN_LICENSE_PUBLIC_KEY is required for production builds");
    }
    println!("cargo:rustc-env=LICENSE_PUBLIC_KEY={license_public_key}");
    println!("cargo:rerun-if-env-changed=YTBDOWN_LICENSE_PUBLIC_KEY");

    tauri_build::build()
}
