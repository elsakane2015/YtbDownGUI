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

    let build_channel = std::env::var("YTBDOWN_BUILD_CHANNEL_LABEL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(infer_git_build_channel)
        .unwrap_or_default();
    println!("cargo:rustc-env=APP_BUILD_CHANNEL={build_channel}");
    println!("cargo:rerun-if-env-changed=YTBDOWN_BUILD_CHANNEL_LABEL");

    let license_server_url = std::env::var("YTBDOWN_LICENSE_SERVER_URL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "https://license.ytbdown.litotime.com".to_string());
    println!("cargo:rustc-env=LICENSE_SERVER_URL={license_server_url}");
    println!("cargo:rerun-if-env-changed=YTBDOWN_LICENSE_SERVER_URL");

    // The verification key is public information and ships inside every app
    // binary. Keep the production copy in source control so release builds
    // cannot accidentally inherit a local License Server's development key.
    let production_public_key = include_str!("license-public-key.pem");
    let configured_public_key = std::env::var("YTBDOWN_LICENSE_PUBLIC_KEY").unwrap_or_default();
    let is_debug_profile = std::env::var("PROFILE").is_ok_and(|profile| profile == "debug");
    let license_public_key_source = if is_debug_profile && !configured_public_key.trim().is_empty()
    {
        configured_public_key.as_str()
    } else {
        production_public_key
    };
    let license_public_key = normalize_public_key_for_embedding(license_public_key_source);
    println!("cargo:rustc-env=LICENSE_PUBLIC_KEY={license_public_key}");
    println!("cargo:rerun-if-env-changed=YTBDOWN_LICENSE_PUBLIC_KEY");
    println!("cargo:rerun-if-changed=license-public-key.pem");

    tauri_build::build()
}

/// Cargo build-script directives are line based. Keep the PEM in a single
/// directive by canonicalising real or escaped newlines to literal `\n`;
/// the runtime restores them before parsing the key.
fn normalize_public_key_for_embedding(value: &str) -> String {
    value
        .replace("\\n", "\n")
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim()
        .replace('\n', "\\n")
}

fn infer_git_build_channel() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout);
    if branch.trim() == "pro-dev" {
        Some("Pro".to_string())
    } else {
        None
    }
}
