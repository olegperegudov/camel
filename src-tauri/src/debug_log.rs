//! Fresh-per-launch debug log in the app's data dir.
//!
//! Camel runs headless in the tray: when the bars freeze or the panel shows
//! nothing, this file is the only witness. Events only — never file contents.

use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

static LOG_PATH: Mutex<Option<PathBuf>> = Mutex::new(None);

pub fn init() {
    let dir = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("camel");
    let _ = crate::private::create_dir(&dir);
    let path = dir.join("debug.log");
    // Fresh file per launch — an unbounded log on a tray app that runs for
    // weeks is a slow disk leak, and only the current session is ever useful.
    let _ = crate::private::write(&path, b"");
    if let Ok(mut g) = LOG_PATH.lock() {
        *g = Some(path);
    }
    log("--- camel started ---");
}

pub fn log(msg: &str) {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let line = format!("[{}] {}\n", secs, msg);
    eprint!("{}", line);
    let guard = match LOG_PATH.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    if let Some(path) = guard.as_ref() {
        if let Ok(mut f) = crate::private::append(path) {
            let _ = f.write_all(line.as_bytes());
        }
    }
}
