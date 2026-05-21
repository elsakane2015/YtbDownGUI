//! Tiny file-based logger that writes to `<data-dir>/runtime.log`.
//!
//! Why: on Windows we ship a `windows_subsystem = "windows"` binary, which
//! means no console attached — `eprintln!` goes nowhere. Without runtime
//! logs, diagnosing user reports is guesswork. This writes the same
//! messages to a file the user can share.
//!
//! Usage: `crate::core::log::write(format!("..."))`. Cheap when the log
//! path hasn't been initialised yet (early-init writes are dropped).
//! Safe to call from any thread.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_FILE: OnceLock<Mutex<File>> = OnceLock::new();

/// Initialise the log file under `data_dir/runtime.log`. Idempotent.
/// Truncates the file on startup so logs from previous app runs don't
/// accumulate forever (keeps ~1 run worth of context).
pub fn init(data_dir: &std::path::Path) {
    if LOG_FILE.get().is_some() {
        return;
    }
    let _ = std::fs::create_dir_all(data_dir);
    let path = data_dir.join("runtime.log");
    if let Ok(file) = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path)
    {
        let _ = LOG_FILE.set(Mutex::new(file));
        write(format!("=== log opened at {} ===", path.display()));
        write(format!(
            "platform: {} {}",
            std::env::consts::OS,
            std::env::consts::ARCH
        ));
        write(format!("data_dir: {}", data_dir.display()));
    }
}

/// Append a line to the runtime log. Also echoes via eprintln so dev mode
/// (where stderr is wired up) still shows the message.
pub fn write(msg: impl AsRef<str>) {
    let line = format!("[{}] {}", timestamp(), msg.as_ref());
    eprintln!("{line}");
    if let Some(file) = LOG_FILE.get() {
        if let Ok(mut f) = file.lock() {
            let _ = writeln!(f, "{line}");
            let _ = f.flush();
        }
    }
}

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // Just the seconds-since-epoch — sufficient for relative ordering and
    // doesn't pull in chrono just for diagnostics.
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{h:02}:{m:02}:{s:02}")
}

/// Path to where the log lives, if init() succeeded.
#[allow(dead_code)] // exposed for later "open log file" UI affordance
pub fn path(data_dir: &std::path::Path) -> PathBuf {
    data_dir.join("runtime.log")
}
