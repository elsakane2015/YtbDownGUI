// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args_os().any(|arg| arg == "--verify-license-config") {
        match ytbdowngui_lib::verify_embedded_license_config() {
            Ok(()) => {
                println!("embedded license public key: valid");
                return;
            }
            Err(error) => {
                eprintln!("embedded license public key: {error}");
                std::process::exit(1);
            }
        }
    }

    // The portable Windows build ships Microsoft's Evergreen WebView2
    // standalone installer next to the app. Install it on first launch when
    // the machine does not already have a usable runtime. The regular NSIS
    // installer handles the same dependency during setup.
    #[cfg(target_os = "windows")]
    ensure_portable_webview2_runtime();

    ytbdowngui_lib::run()
}

#[cfg(target_os = "windows")]
fn ensure_portable_webview2_runtime() {
    if webview2_runtime_installed() {
        return;
    }

    let Ok(executable) = std::env::current_exe() else {
        return;
    };
    let Some(app_dir) = executable.parent() else {
        return;
    };
    let installer = app_dir.join("MicrosoftEdgeWebView2RuntimeInstallerX64.exe");
    if !installer.is_file() {
        return;
    }

    // Microsoft supports an unelevated per-user installation. If a
    // per-machine Edge updater is present it may promote this automatically.
    let _ = std::process::Command::new(installer)
        .args(["/silent", "/install"])
        .status();
}

#[cfg(target_os = "windows")]
fn webview2_runtime_installed() -> bool {
    const CLIENT_ID: &str = "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}";
    let registry_keys = [
        format!(r"HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{CLIENT_ID}"),
        format!(r"HKLM\SOFTWARE\Microsoft\EdgeUpdate\Clients\{CLIENT_ID}"),
        format!(r"HKCU\Software\Microsoft\EdgeUpdate\Clients\{CLIENT_ID}"),
    ];

    registry_keys.iter().any(|key| {
        let Ok(output) = std::process::Command::new("reg.exe")
            .args(["query", key, "/v", "pv"])
            .output()
        else {
            return false;
        };
        if !output.status.success() {
            return false;
        }

        String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| line.split_whitespace().last())
            .any(|version| version != "0.0.0.0" && version.contains('.'))
    })
}
