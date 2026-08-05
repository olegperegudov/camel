//! The numbers Camel lives for: how much of the Claude Code subscription is
//! left in the 5-hour and 7-day windows, and when each window resets.
//!
//! The source is the JSON that Claude Code hands to its status line, which the
//! user's statusline script mirrors to `~/.claude/statusline-last.json`. Camel
//! only ever reads that file — it talks to no network and holds no credentials.

use serde::Serialize;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Remaining percentage at which the bar turns yellow / red. Mirrors the
/// thresholds of the in-session status line (yellow at 50% used, red at 80%),
/// so the tray and the terminal never disagree about how bad things are.
pub const LOW_AT: u8 = 50;
pub const CRITICAL_AT: u8 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Window {
    /// Percent still available, 0–100.
    pub remaining: u8,
    /// Unix seconds when this window resets.
    pub resets_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub five_hour: Window,
    pub seven_day: Window,
    /// Unix seconds of the source file's last write — how fresh the data is.
    pub read_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Ok,
    Low,
    Critical,
}

pub fn level(remaining: u8) -> Level {
    if remaining >= LOW_AT {
        Level::Ok
    } else if remaining >= CRITICAL_AT {
        Level::Low
    } else {
        Level::Critical
    }
}

/// The window closer to exhaustion — the one number worth showing next to the
/// tray icon.
pub fn worst(s: &Snapshot) -> u8 {
    s.five_hour.remaining.min(s.seven_day.remaining)
}

/// Where Claude Code's status line data lives, on any OS. The path is derived
/// from the OS home dir — never spelled out.
pub fn source_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("statusline-last.json"))
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Read and interpret the source file. `None` when the file is missing or has
/// no rate limits yet (a machine where Claude Code has not spoken today).
pub fn read() -> Option<Snapshot> {
    let path = source_path()?;
    let read_at = std::fs::metadata(&path)
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)?;
    let raw = std::fs::read_to_string(&path).ok()?;
    parse(&raw, read_at, now_secs())
}

/// Interpret the status-line JSON. Separated from the disk so it can be tested.
///
/// A window whose reset moment has already passed is shown as full: the quota
/// did refresh, only nobody has run a session since to rewrite the file.
pub fn parse(raw: &str, read_at: i64, now: i64) -> Option<Snapshot> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let window = |key: &str| -> Option<Window> {
        let w = &v["rate_limits"][key];
        let used = w["used_percentage"].as_u64()?;
        let resets_at = w["resets_at"].as_i64()?;
        let remaining = if now >= resets_at {
            100
        } else {
            100u8.saturating_sub(used.min(100) as u8)
        };
        Some(Window { remaining, resets_at })
    };
    Some(Snapshot {
        five_hour: window("five_hour")?,
        seven_day: window("seven_day")?,
        read_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const RAW: &str = r#"{
        "model": {"display_name": "x"},
        "rate_limits": {
            "five_hour": {"used_percentage": 27, "resets_at": 2000},
            "seven_day": {"used_percentage": 5, "resets_at": 3000}
        }
    }"#;

    #[test]
    fn remaining_is_the_flip_of_used() {
        let s = parse(RAW, 100, 1000).unwrap();
        assert_eq!(s.five_hour, Window { remaining: 73, resets_at: 2000 });
        assert_eq!(s.seven_day, Window { remaining: 95, resets_at: 3000 });
        assert_eq!(s.read_at, 100);
    }

    #[test]
    fn a_window_past_its_reset_reads_as_full_again() {
        // Stale file: the 5h reset (t=2000) is behind us, the weekly is not.
        let s = parse(RAW, 100, 2500).unwrap();
        assert_eq!(s.five_hour.remaining, 100);
        assert_eq!(s.seven_day.remaining, 95);
    }

    #[test]
    fn missing_or_null_limits_is_no_snapshot_not_a_zero() {
        assert_eq!(parse(r#"{"rate_limits": null}"#, 1, 2), None);
        assert_eq!(parse("not json", 1, 2), None);
        assert_eq!(parse("{}", 1, 2), None);
    }

    #[test]
    fn the_worst_window_is_the_one_shown_in_the_bar() {
        let s = parse(RAW, 100, 1000).unwrap();
        assert_eq!(worst(&s), 73);
    }

    #[test]
    fn levels_match_the_status_line_thresholds() {
        assert_eq!(level(100), Level::Ok);
        assert_eq!(level(50), Level::Ok);
        assert_eq!(level(49), Level::Low);
        assert_eq!(level(20), Level::Low);
        assert_eq!(level(19), Level::Critical);
        assert_eq!(level(0), Level::Critical);
    }

    #[test]
    fn used_over_100_clamps_instead_of_wrapping() {
        let raw = r#"{"rate_limits": {
            "five_hour": {"used_percentage": 130, "resets_at": 2000},
            "seven_day": {"used_percentage": 0, "resets_at": 3000}
        }}"#;
        let s = parse(raw, 1, 1000).unwrap();
        assert_eq!(s.five_hour.remaining, 0);
    }
}
