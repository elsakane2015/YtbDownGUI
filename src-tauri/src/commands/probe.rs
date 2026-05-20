use crate::core::probe::{self as core_probe, ProbeResult};
use crate::error::AppError;
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_shell::{process::CommandEvent, ShellExt};

#[derive(Debug, Serialize)]
pub struct ToolVersion {
    pub name: String,
    pub version: String,
}

#[tauri::command]
pub async fn probe(app: AppHandle, url: String) -> Result<ProbeResult, AppError> {
    core_probe::probe(&app, &url).await
}

#[tauri::command]
pub async fn probe_tool_versions(app: AppHandle) -> Result<Vec<ToolVersion>, AppError> {
    let yt_dlp = run_version(&app, "yt-dlp", &["--version"]).await;
    let ffmpeg = run_version(&app, "ffmpeg", &["-version"]).await;
    Ok(vec![
        ToolVersion {
            name: "yt-dlp".into(),
            version: pretty_ffmpeg_or_raw(&yt_dlp),
        },
        ToolVersion {
            name: "ffmpeg".into(),
            version: pretty_ffmpeg_or_raw(&ffmpeg),
        },
    ])
}

/// Extract just the version number from "ffmpeg version 7.1.1 ..." or return
/// the line as-is for yt-dlp (which prints just the version).
fn pretty_ffmpeg_or_raw(result: &Result<String, String>) -> String {
    match result {
        Ok(s) if s.starts_with("ffmpeg version ") => s
            .strip_prefix("ffmpeg version ")
            .and_then(|rest| rest.split_whitespace().next())
            .unwrap_or(s)
            .to_string(),
        Ok(s) => s.clone(),
        Err(e) => format!("error: {e}"),
    }
}

async fn run_version(
    app: &AppHandle,
    sidecar: &str,
    args: &[&str],
) -> Result<String, String> {
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
    let out = if stdout.is_empty() { stderr } else { stdout };
    Ok(out.lines().next().unwrap_or("").trim().to_string())
}
