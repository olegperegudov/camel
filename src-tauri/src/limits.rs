//! The numbers Camel lives for: how much of the Claude Code subscription is
//! left in the 5-hour and 7-day windows, and when each window resets.
//!
//! The source is the JSON that Claude Code hands to its status line, which the
//! user's statusline script mirrors to `~/.claude/statusline-last.json`. Camel
//! only ever reads that file — it talks to no network and holds no credentials.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Remaining percentage at which the bar turns yellow / red: yellow once
/// half the window is spent, red once three quarters are.
pub const LOW_AT: u8 = 50;
pub const CRITICAL_AT: u8 = 25;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Window {
    /// Percent still available, 0–100.
    pub remaining: u8,
    /// Unix seconds when this window resets.
    pub resets_at: i64,
    /// The reset moment is already behind us: the window refilled and nobody
    /// has run a session since to write the next one. `resets_at` then names
    /// the refill, not a future event, and the panel must not count down to it.
    pub refilled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    pub five_hour: Window,
    pub seven_day: Window,
    /// Unix seconds of the source file's last write — how fresh the data is.
    pub read_at: i64,
}

/// What the source file had to say. Three outcomes, not two: a file that is
/// not there needs setup instructions, a file we cannot make sense of needs a
/// different sentence, and neither of them is a zero.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "state", content = "snapshot", rename_all = "lowercase")]
pub enum Reading {
    Ok(Snapshot),
    /// No file yet — Claude Code's status line has never written one here.
    Missing,
    /// The file exists but carries no rate limits we can read.
    Unreadable,
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

/// Read and interpret the source file at its usual place.
pub fn read() -> Reading {
    match source_path() {
        Some(path) => read_at_path(&path),
        None => Reading::Missing,
    }
}

/// The disk half, taking its path as an argument so a test can hand it a real
/// file instead of the user's home directory.
pub fn read_at_path(path: &Path) -> Reading {
    let Ok(meta) = std::fs::metadata(path) else {
        return Reading::Missing;
    };
    let read_at = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or_else(now_secs);
    let Ok(raw) = std::fs::read_to_string(path) else {
        return Reading::Unreadable;
    };
    match parse(&raw, read_at, now_secs()) {
        Some(s) => Reading::Ok(s),
        None => Reading::Unreadable,
    }
}

/// Interpret the status-line JSON. Separated from the disk so it can be tested.
///
/// A window whose reset moment has already passed is shown as full: the quota
/// did refresh, only nobody has run a session since to rewrite the file.
pub fn parse(raw: &str, read_at: i64, now: i64) -> Option<Snapshot> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    let window = |key: &str| -> Option<Window> {
        let w = &v["rate_limits"][key];
        // as_f64, not as_u64: Claude Code writes whatever precision it has,
        // and "7.0" must not silently read as "no data".
        let used = w["used_percentage"].as_f64()?;
        let resets_at = w["resets_at"].as_i64()?;
        let refilled = now >= resets_at;
        let remaining = if refilled {
            100
        } else {
            (100.0 - used.clamp(0.0, 100.0)).round() as u8
        };
        Some(Window { remaining, resets_at, refilled })
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
        assert_eq!(s.five_hour, Window { remaining: 73, resets_at: 2000, refilled: false });
        assert_eq!(s.seven_day, Window { remaining: 95, resets_at: 3000, refilled: false });
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
    fn a_refilled_window_is_flagged_so_nothing_counts_down_to_the_past() {
        // Without the flag the panel printed the old reset moment as a future
        // event, with a countdown frozen at zero.
        let s = parse(RAW, 100, 2500).unwrap();
        assert!(s.five_hour.refilled);
        assert!(!s.seven_day.refilled);
    }

    #[test]
    fn missing_or_null_limits_is_no_snapshot_not_a_zero() {
        assert_eq!(parse(r#"{"rate_limits": null}"#, 1, 2), None);
        assert_eq!(parse("not json", 1, 2), None);
        assert_eq!(parse("{}", 1, 2), None);
    }

    #[test]
    fn levels_match_the_status_line_thresholds() {
        assert_eq!(level(100), Level::Ok);
        assert_eq!(level(50), Level::Ok);
        assert_eq!(level(49), Level::Low);
        assert_eq!(level(25), Level::Low);
        assert_eq!(level(24), Level::Critical);
        assert_eq!(level(0), Level::Critical);
    }

    #[test]
    fn fractional_percentages_parse_instead_of_vanishing() {
        // Seen live: the status line wrote 7.0, and an integer-only parser
        // dropped the whole snapshot on the floor.
        let raw = r#"{"rate_limits": {
            "five_hour": {"used_percentage": 7.0, "resets_at": 2000},
            "seven_day": {"used_percentage": 4.6, "resets_at": 3000}
        }}"#;
        let s = parse(raw, 1, 1000).unwrap();
        assert_eq!(s.five_hour.remaining, 93);
        assert_eq!(s.seven_day.remaining, 95);
    }

    #[test]
    fn a_file_we_cannot_read_is_not_the_same_as_no_file() {
        // The panel says different things to each: one needs setup, the other
        // needs to know its status line writes something we don't understand.
        let dir = std::env::temp_dir().join("camel-reading-test");
        std::fs::create_dir_all(&dir).unwrap();

        assert_eq!(read_at_path(&dir.join("nothing-here.json")), Reading::Missing);

        let junk = dir.join("junk.json");
        std::fs::write(&junk, "{\"rate_limits\": null}").unwrap();
        assert_eq!(read_at_path(&junk), Reading::Unreadable);

        let good = dir.join("good.json");
        std::fs::write(&good, RAW).unwrap();
        assert!(matches!(read_at_path(&good), Reading::Ok(_)));

        std::fs::remove_dir_all(&dir).ok();
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
