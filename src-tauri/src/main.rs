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
    ytbdowngui_lib::run()
}
