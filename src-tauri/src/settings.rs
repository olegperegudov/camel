//! The one choice Camel remembers: whether to ask Codex for its limits.
//!
//! Asking means keeping a `codex app-server` child alive, and every launch of
//! that child costs disk on the Codex side (see CHANGELOG v0.1.29). Someone
//! who has Codex installed but does not use it should be able to say "leave it
//! alone" once and have that survive restarts and updates.
//!
//! Stored as `settings.json` next to the debug log, owner-only via `private`.
//! Anything unreadable falls back to the default — Codex shown — because a
//! broken file must never hide a row the user expects to see.

use std::path::{Path, PathBuf};

const FILE: &str = "settings.json";

pub fn dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("camel")
}

pub fn codex_enabled(dir: &Path) -> bool {
    std::fs::read(dir.join(FILE))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|v| v.get("codex").and_then(|c| c.as_bool()))
        .unwrap_or(true)
}

pub fn set_codex_enabled(dir: &Path, enabled: bool) -> std::io::Result<()> {
    crate::private::create_dir(dir)?;
    let body = serde_json::json!({ "codex": enabled }).to_string();
    crate::private::write(&dir.join(FILE), body.as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("camel-settings-{}-{}", name, std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn codex_is_shown_until_someone_turns_it_off() {
        let dir = scratch("default");
        assert!(codex_enabled(&dir));
    }

    #[test]
    fn turning_codex_off_survives_a_restart() {
        let dir = scratch("roundtrip");
        set_codex_enabled(&dir, false).unwrap();
        assert!(!codex_enabled(&dir));
        set_codex_enabled(&dir, true).unwrap();
        assert!(codex_enabled(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_broken_file_does_not_hide_codex() {
        let dir = scratch("broken");
        crate::private::create_dir(&dir).unwrap();
        std::fs::write(dir.join(FILE), b"{not json").unwrap();
        assert!(codex_enabled(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
