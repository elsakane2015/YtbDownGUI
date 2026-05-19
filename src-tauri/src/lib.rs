use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

#[derive(Debug, Serialize)]
struct ToolVersion {
    name: String,
    version: String,
}

#[tauri::command]
async fn probe_tool_versions(app: AppHandle) -> Result<Vec<ToolVersion>, String> {
    let yt_dlp = run_version(&app, "yt-dlp", &["--version"]).await;
    let ffmpeg = run_version(&app, "ffmpeg", &["-version"]).await;
    Ok(vec![
        ToolVersion {
            name: "yt-dlp".into(),
            version: yt_dlp.unwrap_or_else(|e| format!("error: {e}")),
        },
        ToolVersion {
            name: "ffmpeg".into(),
            version: ffmpeg.unwrap_or_else(|e| format!("error: {e}")),
        },
    ])
}

async fn run_version(app: &AppHandle, sidecar: &str, args: &[&str]) -> Result<String, String> {
    let cmd = app
        .shell()
        .sidecar(sidecar)
        .map_err(|e| format!("sidecar lookup failed: {e}"))?
        .args(args);
    let (mut rx, _child) = cmd.spawn().map_err(|e| format!("spawn failed: {e}"))?;
    let mut stdout = String::new();
    let mut stderr = String::new();
    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => stdout.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Stderr(bytes) => stderr.push_str(&String::from_utf8_lossy(&bytes)),
            CommandEvent::Terminated(payload) => {
                if payload.code != Some(0) {
                    return Err(format!(
                        "exit {:?}: {}",
                        payload.code,
                        stderr.lines().next().unwrap_or("").trim()
                    ));
                }
                break;
            }
            _ => {}
        }
    }
    // yt-dlp prints just the version; ffmpeg prints "ffmpeg version X.Y ..."
    let out = if stdout.is_empty() { stderr } else { stdout };
    Ok(out.lines().next().unwrap_or("").trim().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![probe_tool_versions])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
