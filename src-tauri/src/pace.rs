//! Where the current burn rate leads.
//!
//! The tray icon already answers "how much is left". The question the panel
//! exists for is the next one — "will I make it to the reset" — and the app
//! has been sitting on the data to answer it all along: it re-reads the file
//! every half minute and threw every previous reading away. Each poll now
//! drops a sample into a short ring buffer, and the slope across that buffer
//! says whether a window empties before it comes back.

use crate::limits::{Snapshot, Window};
use serde::Serialize;
use std::collections::VecDeque;

/// How far back the slope is measured: long enough for one heavy prompt to
/// average out, short enough that the answer tracks what is happening now.
pub const HISTORY_SECS: i64 = 20 * 60;
/// Below this span a slope is noise. Saying nothing beats guessing.
pub const MIN_SPAN_SECS: i64 = 3 * 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample {
    pub at: i64,
    pub five: u8,
    pub seven: u8,
}

impl Sample {
    pub fn of(s: &Snapshot, at: i64) -> Self {
        Sample { at, five: s.five_hour.remaining, seven: s.seven_day.remaining }
    }
}

/// The verdict, as data — the panel turns it into a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Pace {
    /// Not enough history yet to say anything honest.
    Unknown,
    /// Nothing was spent across everything we can see.
    Idle { minutes: i64 },
    /// Every window outlasts its own reset at this rate.
    Safe,
    /// The window that empties first, and when it does.
    RunsOut { window: &'static str, at: i64, before_reset: i64 },
}

pub fn push(history: &mut VecDeque<Sample>, s: Sample) {
    // A window coming back is a refill, not negative spending — the slope
    // across that jump would be nonsense, so the history starts over.
    let refilled = history
        .back()
        .is_some_and(|last| s.five > last.five || s.seven > last.seven);
    if refilled {
        history.clear();
    }
    history.push_back(s);
    while history.front().is_some_and(|f| s.at - f.at > HISTORY_SECS) {
        history.pop_front();
    }
}

pub fn of(history: &VecDeque<Sample>, snap: &Snapshot, now: i64) -> Pace {
    let (Some(first), Some(last)) = (history.front(), history.back()) else {
        return Pace::Unknown;
    };
    let span = last.at - first.at;
    if span < MIN_SPAN_SECS {
        return Pace::Unknown;
    }

    let spent_five = first.five.saturating_sub(last.five);
    let spent_seven = first.seven.saturating_sub(last.seven);
    if spent_five == 0 && spent_seven == 0 {
        return Pace::Idle { minutes: span / 60 };
    }

    let runs_out = |name: &'static str, spent: u8, w: Window| -> Option<Pace> {
        // A window with nothing spent in it, or one that just refilled, has
        // no pace worth projecting.
        if spent == 0 || w.refilled {
            return None;
        }
        let per_sec = spent as f64 / span as f64;
        let at = now + (w.remaining as f64 / per_sec).round() as i64;
        (at < w.resets_at).then_some(Pace::RunsOut {
            window: name,
            at,
            before_reset: w.resets_at - at,
        })
    };

    [
        runs_out("five_hour", spent_five, snap.five_hour),
        runs_out("seven_day", spent_seven, snap.seven_day),
    ]
    .into_iter()
    .flatten()
    .min_by_key(|p| match p {
        Pace::RunsOut { at, .. } => *at,
        _ => i64::MAX,
    })
    .unwrap_or(Pace::Safe)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(five: u8, five_reset: i64, seven: u8, seven_reset: i64) -> Snapshot {
        Snapshot {
            five_hour: Window { remaining: five, resets_at: five_reset, refilled: false },
            seven_day: Window { remaining: seven, resets_at: seven_reset, refilled: false },
            read_at: 0,
        }
    }

    /// Ten minutes of samples, `five` falling by `drop` percent over them.
    fn history(drop: u8) -> VecDeque<Sample> {
        let mut h = VecDeque::new();
        for i in 0..=10 {
            let at = 1000 + i * 60;
            push(&mut h, Sample { at, five: 80 - (drop as i64 * i / 10) as u8, seven: 90 });
        }
        h
    }

    #[test]
    fn too_little_history_says_nothing_rather_than_guessing() {
        let mut h = VecDeque::new();
        push(&mut h, Sample { at: 1000, five: 80, seven: 90 });
        push(&mut h, Sample { at: 1060, five: 79, seven: 90 });
        assert_eq!(of(&h, &snap(79, 99999, 90, 999999), 1060), Pace::Unknown);
        assert_eq!(of(&VecDeque::new(), &snap(80, 99999, 90, 999999), 1000), Pace::Unknown);
    }

    #[test]
    fn nothing_spent_is_idle_not_a_forecast() {
        let h = history(0);
        assert_eq!(of(&h, &snap(80, 99999, 90, 999999), 1600), Pace::Idle { minutes: 10 });
    }

    #[test]
    fn a_pace_that_empties_before_the_reset_names_the_moment() {
        // 20% burned in 10 minutes = 2% per minute; 60% left is 30 minutes,
        // and the window does not reset for another two hours.
        let h = history(20);
        let now = 1600;
        let reset = now + 2 * 3600;
        match of(&h, &snap(60, reset, 90, now + 999999), now) {
            Pace::RunsOut { window, at, before_reset } => {
                assert_eq!(window, "five_hour");
                assert_eq!(at, now + 30 * 60);
                assert_eq!(before_reset, reset - at);
            }
            other => panic!("expected a forecast, got {:?}", other),
        }
    }

    #[test]
    fn a_pace_the_window_outlasts_is_good_news_not_silence() {
        // Same burn, but the reset is ten minutes away — it arrives first.
        let h = history(20);
        let now = 1600;
        assert_eq!(of(&h, &snap(60, now + 600, 90, now + 999999), now), Pace::Safe);
    }

    #[test]
    fn a_refill_starts_the_history_over_instead_of_reading_as_a_negative_burn() {
        let mut h = history(20);
        push(&mut h, Sample { at: 1660, five: 100, seven: 90 });
        assert_eq!(h.len(), 1);
        assert_eq!(of(&h, &snap(100, 99999, 90, 999999), 1660), Pace::Unknown);
    }

    #[test]
    fn samples_older_than_the_window_fall_out_the_front() {
        let mut h = VecDeque::new();
        push(&mut h, Sample { at: 0, five: 90, seven: 95 });
        push(&mut h, Sample { at: HISTORY_SECS + 1, five: 80, seven: 95 });
        assert_eq!(h.len(), 1);
    }
}
