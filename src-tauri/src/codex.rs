//! Codex usage through the local Codex CLI's app-server protocol.
//!
//! Camel never reads Codex credentials. The installed `codex` process owns
//! authentication and returns the same account rate-limit snapshot its own
//! clients use. One child stays alive between polls so refreshing two numbers
//! does not repeatedly initialise the CLI.

use crate::limits::{Reading, Snapshot, Window};
use serde_json::{json, Value};
use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

const FIVE_HOURS_MINS: i64 = 300;
const SEVEN_DAYS_MINS: i64 = 10_080;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(15);

pub struct Client {
    child: Child,
    stdin: ChildStdin,
    lines: Receiver<String>,
    next_id: u64,
}

impl Client {
    pub fn connect() -> Result<Self, Reading> {
        let executable = find_executable().ok_or(Reading::Unavailable)?;
        let path = child_path(&executable, std::env::var_os("PATH"))
            .ok_or(Reading::Unavailable)?;
        let mut command = Command::new(&executable);
        command
            // Homebrew's `codex` launcher uses `/usr/bin/env node`. Apps
            // opened from Finder inherit only the system PATH, so include the
            // launcher's directory for its sibling runtime as well.
            .env("PATH", path)
            .args(["app-server", "--stdio"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // App-server diagnostics may mention local configuration. Camel's
            // own log only records the state, never subprocess output.
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|_| Reading::Unavailable)?;
        let stdin = child.stdin.take().ok_or(Reading::Failed)?;
        let stdout = child.stdout.take().ok_or(Reading::Failed)?;
        let (tx, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if tx.send(line).is_err() {
                    break;
                }
            }
        });
        let mut client = Self {
            child,
            stdin,
            lines,
            next_id: 1,
        };
        client.request(
            "initialize",
            json!({
                "clientInfo": {"name": "camel", "version": env!("CARGO_PKG_VERSION")},
                "capabilities": {"experimentalApi": true}
            }),
        )?;
        Ok(client)
    }

    pub fn read(&mut self, now: i64) -> Reading {
        match self.request("account/rateLimits/read", Value::Null) {
            Ok(result) => parse(&result, now)
                .map(Reading::Ok)
                .unwrap_or(Reading::Failed),
            Err(state) => state,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, Reading> {
        let id = self.next_id;
        self.next_id += 1;
        let message = json!({"id": id, "method": method, "params": params});
        writeln!(self.stdin, "{message}").map_err(|_| Reading::Failed)?;
        self.stdin.flush().map_err(|_| Reading::Failed)?;

        loop {
            let line = self
                .lines
                .recv_timeout(RESPONSE_TIMEOUT)
                .map_err(|_| Reading::Failed)?;
            let Ok(value) = serde_json::from_str::<Value>(&line) else {
                continue;
            };
            if value["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(result) = value.get("result") {
                return Ok(result.clone());
            }
            let message = value["error"]["message"]
                .as_str()
                .unwrap_or_default()
                .to_lowercase();
            return Err(if message.contains("login") || message.contains("auth") {
                Reading::SignedOut
            } else {
                Reading::Failed
            });
        }
    }
}

fn child_path(executable: &Path, current: Option<OsString>) -> Option<OsString> {
    let mut paths = executable
        .parent()
        .into_iter()
        .map(Path::to_path_buf)
        .collect::<Vec<_>>();
    if let Some(current) = current {
        paths.extend(std::env::split_paths(&current));
    }
    std::env::join_paths(paths).ok()
}

impl Drop for Client {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn find_executable() -> Option<PathBuf> {
    let mut candidates = Vec::new();
    #[cfg(target_os = "macos")]
    candidates.extend([
        PathBuf::from("/opt/homebrew/bin/codex"),
        PathBuf::from("/usr/local/bin/codex"),
    ]);
    #[cfg(target_os = "windows")]
    if let Some(appdata) = std::env::var_os("APPDATA") {
        candidates.push(PathBuf::from(appdata).join("npm").join("codex.cmd"));
    }
    candidates.into_iter().find(|path| path.is_file()).or_else(|| {
        std::env::var_os("PATH").and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| {
                    dir.join(if cfg!(windows) {
                        "codex.exe"
                    } else {
                        "codex"
                    })
                })
                .find(|path| path.is_file())
        })
    })
}

/// Select the bucket that actually contains both subscription windows.
/// Limit IDs are backend-owned and change with model families, so the window
/// durations are the stable contract.
pub fn parse(result: &Value, now: i64) -> Option<Snapshot> {
    let buckets = result["rateLimitsByLimitId"]
        .as_object()
        .into_iter()
        .flat_map(|map| map.values())
        .chain(std::iter::once(&result["rateLimits"]));

    for bucket in buckets {
        let (Some(primary), Some(secondary)) =
            (bucket.get("primary"), bucket.get("secondary"))
        else {
            continue;
        };
        let windows = [primary, secondary];
        let five = windows
            .iter()
            .find(|w| w["windowDurationMins"].as_i64() == Some(FIVE_HOURS_MINS));
        let week = windows
            .iter()
            .find(|w| w["windowDurationMins"].as_i64() == Some(SEVEN_DAYS_MINS));
        if let (Some(five), Some(week)) = (five, week) {
            return Some(Snapshot {
                five_hour: parse_window(five, now)?,
                seven_day: parse_window(week, now)?,
                read_at: now,
            });
        }
    }
    None
}

fn parse_window(value: &Value, now: i64) -> Option<Window> {
    let used = value["usedPercent"].as_i64()?.clamp(0, 100) as u8;
    let resets_at = value["resetsAt"].as_i64()?;
    let refilled = now >= resets_at;
    Some(Window {
        remaining: if refilled { 100 } else { 100 - used },
        resets_at,
        refilled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gui_path_includes_the_codex_launchers_sibling_runtime() {
        let executable = Path::new("/opt/homebrew/bin/codex");
        let restricted = std::env::join_paths(["/usr/bin", "/bin"]).unwrap();
        let result = child_path(executable, Some(restricted)).unwrap();
        let paths = std::env::split_paths(&result).collect::<Vec<_>>();

        assert_eq!(paths[0], Path::new("/opt/homebrew/bin"));
        assert!(paths.contains(&PathBuf::from("/usr/bin")));
        assert!(paths.contains(&PathBuf::from("/bin")));
    }

    #[test]
    fn selects_the_bucket_with_both_real_windows_without_knowing_its_id() {
        let value = json!({
            "rateLimits": {"primary": {"usedPercent": 9, "windowDurationMins": 10080, "resetsAt": 4000}},
            "rateLimitsByLimitId": {
                "a_backend_owned_name": {
                    "primary": {"usedPercent": 17, "windowDurationMins": 300, "resetsAt": 2000},
                    "secondary": {"usedPercent": 23, "windowDurationMins": 10080, "resetsAt": 3000}
                }
            }
        });
        let snapshot = parse(&value, 1000).unwrap();
        assert_eq!(snapshot.five_hour.remaining, 83);
        assert_eq!(snapshot.seven_day.remaining, 77);
        assert_eq!(snapshot.read_at, 1000);
    }

    #[test]
    fn incomplete_or_unknown_shapes_are_not_zero_usage() {
        assert_eq!(parse(&json!({"rateLimits": {}}), 1), None);
        assert_eq!(
            parse(
                &json!({"rateLimitsByLimitId": {"x": {"primary": null}}}),
                1
            ),
            None
        );
    }

    #[test]
    fn percentages_are_clamped_and_elapsed_windows_refill() {
        let value = json!({"rateLimits": {
            "primary": {"usedPercent": 130, "windowDurationMins": 300, "resetsAt": 500},
            "secondary": {"usedPercent": -5, "windowDurationMins": 10080, "resetsAt": 3000}
        }});
        let snapshot = parse(&value, 1000).unwrap();
        assert_eq!(snapshot.five_hour.remaining, 100);
        assert!(snapshot.five_hour.refilled);
        assert_eq!(snapshot.seven_day.remaining, 100);
    }

    #[test]
    #[ignore]
    fn installed_codex_returns_both_live_windows() {
        let reading = Client::connect().unwrap().read(crate::limits::now_secs());
        assert!(
            matches!(reading, Reading::Ok(_)),
            "live reading was {reading:?}"
        );
    }
}
