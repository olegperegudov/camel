//! The numbers Camel lives for: how much of the Claude Code subscription is
//! left in the 5-hour and 7-day windows, and when each window resets.
//!
//! The source is the JSON that Claude Code hands to its status line, which the
//! user's statusline script mirrors next to its own config as
//! `statusline-last.json`. Camel only ever reads those files — it talks to no
//! network and holds no credentials.
//!
//! One machine can hold several logins, each with its own config directory and
//! its own subscription: `~/.claude` for the personal one, `~/.claude-work` for
//! a work account. Camel reads every one it finds rather than a fixed path, so
//! a login added later shows up without touching this code.

use serde::Serialize;
use std::path::Path;
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

/// One Claude Code login: what to call it, and what its status line last said.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Account {
    /// Short name shown above the account's rows: "personal", "work".
    pub label: String,
    pub reading: Reading,
}

pub const SOURCE_FILE: &str = "statusline-last.json";

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Every login that has ever written a status line, personal first.
///
/// Empty means no config directory carries the file — the panel then says how
/// to set the status line up, same as a single account that never wrote one.
pub fn read() -> Vec<Account> {
    match dirs::home_dir() {
        Some(home) => read_in_home(&home),
        None => Vec::new(),
    }
}

/// The disk half, taking the home directory as an argument so a test can hand
/// it a directory it built itself.
pub fn read_in_home(home: &Path) -> Vec<Account> {
    let Ok(entries) = std::fs::read_dir(home) else {
        return Vec::new();
    };
    let mut found: Vec<Account> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let source = entry.path().join(SOURCE_FILE);
            // A config dir without the file is a login that never ran a session
            // with the status line — nothing to show, and an empty row would
            // only be noise next to the accounts that do have numbers.
            label_of(&name).filter(|_| source.is_file()).map(|label| Account {
                label,
                reading: read_at_path(&source),
            })
        })
        .collect();
    // Personal first, the rest alphabetically: the order has to be the same on
    // every launch, or the two bars in the menu bar would swap meaning.
    found.sort_by(|a, b| (a.label != "personal", &a.label).cmp(&(b.label != "personal", &b.label)));
    found
}

/// What to call the login living in this directory, or None if the directory is
/// not a Claude Code config at all. `.claude` is the personal one; anything
/// after the name — `.claude-work` — is the label itself.
fn label_of(dir_name: &str) -> Option<String> {
    match dir_name.strip_prefix(".claude")? {
        "" => Some("personal".to_string()),
        rest => {
            let name = rest.trim_start_matches(['-', '_']);
            (!name.is_empty()).then(|| name.to_string())
        }
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
    fn every_login_with_a_status_line_becomes_an_account_personal_first() {
        // Two logins on one machine, plus a config that never ran a session and
        // a directory that is not Claude Code's at all.
        let home = std::env::temp_dir().join("camel-accounts-test");
        std::fs::remove_dir_all(&home).ok();
        for (dir, file) in [
            (".claude", true),
            (".claude-work", true),
            (".claude-spare", false),
            (".config", true),
        ] {
            std::fs::create_dir_all(home.join(dir)).unwrap();
            if file {
                std::fs::write(home.join(dir).join(SOURCE_FILE), RAW).unwrap();
            }
        }

        let found = read_in_home(&home);
        let labels: Vec<&str> = found.iter().map(|a| a.label.as_str()).collect();
        assert_eq!(labels, ["personal", "work"]);
        assert!(matches!(found[0].reading, Reading::Ok(_)));

        std::fs::remove_dir_all(&home).ok();
    }

    #[test]
    fn a_home_without_any_config_reads_as_no_accounts() {
        // Not an error and not a zero: the panel says how to set the status
        // line up, which is a different sentence from "you have no quota left".
        let home = std::env::temp_dir().join("camel-empty-home-test");
        std::fs::create_dir_all(&home).unwrap();
        assert!(read_in_home(&home).is_empty());
        std::fs::remove_dir_all(&home).ok();
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
