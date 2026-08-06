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
    /// The reset moments the readings belonged to. A window that starts a new
    /// period is a different window, and old samples do not describe it.
    pub five_reset: i64,
    pub seven_reset: i64,
}

impl Sample {
    pub fn of(s: &Snapshot, at: i64) -> Self {
        Sample {
            at,
            five: s.five_hour.remaining,
            seven: s.seven_day.remaining,
            five_reset: s.five_hour.resets_at,
            seven_reset: s.seven_day.resets_at,
        }
    }
}

/// The verdict, as data — the panel turns it into a sentence.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Pace {
    /// Not enough history yet to say anything honest.
    Unknown,
    /// The level is not falling across everything we can see — either nobody
    /// is working, or usage is ageing out as fast as it arrives.
    Steady { minutes: i64 },
    /// Every window outlasts its own reset at this rate.
    Safe,
    /// The window that empties first, and when it does.
    RunsOut { window: &'static str, at: i64, before_reset: i64 },
}

pub fn push(history: &mut VecDeque<Sample>, s: Sample) {
    // A *new period* invalidates the history, and only that. Remaining going
    // up inside the same period is ordinary: these windows roll, so usage
    // ages out the back and the number recovers on its own — treating every
    // rise as a refill cleared the buffer every couple of minutes and the
    // forecast never had enough history to say anything.
    let new_period = history
        .back()
        .is_some_and(|last| s.five_reset != last.five_reset || s.seven_reset != last.seven_reset);
    if new_period {
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

    // saturating_sub, so a window that recovered over the span counts as no
    // burn rather than a negative one.
    let spent_five = first.five.saturating_sub(last.five);
    let spent_seven = first.seven.saturating_sub(last.seven);
    if spent_five == 0 && spent_seven == 0 {
        return Pace::Steady { minutes: span / 60 };
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

    const FIVE_RESET: i64 = 99999;
    const SEVEN_RESET: i64 = 999999;

    fn sample(at: i64, five: u8, seven: u8) -> Sample {
        Sample { at, five, seven, five_reset: FIVE_RESET, seven_reset: SEVEN_RESET }
    }

    /// Ten minutes of samples, `five` falling by `drop` percent over them.
    fn history(drop: u8) -> VecDeque<Sample> {
        let mut h = VecDeque::new();
        for i in 0..=10 {
            push(&mut h, sample(1000 + i * 60, 80 - (drop as i64 * i / 10) as u8, 90));
        }
        h
    }

    #[test]
    fn too_little_history_says_nothing_rather_than_guessing() {
        let mut h = VecDeque::new();
        push(&mut h, sample(1000, 80, 90));
        push(&mut h, sample(1060, 79, 90));
        assert_eq!(of(&h, &snap(79, FIVE_RESET, 90, SEVEN_RESET), 1060), Pace::Unknown);
        assert_eq!(of(&VecDeque::new(), &snap(80, FIVE_RESET, 90, SEVEN_RESET), 1000), Pace::Unknown);
    }

    #[test]
    fn a_level_that_is_not_falling_is_steady_not_a_forecast() {
        let h = history(0);
        assert_eq!(of(&h, &snap(80, FIVE_RESET, 90, SEVEN_RESET), 1600), Pace::Steady { minutes: 10 });
    }

    #[test]
    fn a_rolling_window_recovering_does_not_wipe_the_history() {
        // Seen live: these windows roll, so usage ages out the back and the
        // remaining percent climbs on its own. Treating every rise as a refill
        // cleared the buffer every couple of minutes and the forecast stayed
        // Unknown forever.
        let mut h = VecDeque::new();
        for (i, five) in [82u8, 83, 82, 81, 88, 81, 80].iter().enumerate() {
            push(&mut h, sample(1000 + i as i64 * 60, *five, 90));
        }
        assert_eq!(h.len(), 7);
        assert!(matches!(
            of(&h, &snap(80, FIVE_RESET, 90, SEVEN_RESET), 1360),
            Pace::RunsOut { .. } | Pace::Safe
        ));
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
    fn a_new_period_starts_the_history_over() {
        // The reset moment moved: this is a fresh window, and samples from the
        // old one describe a budget that no longer exists.
        let mut h = history(20);
        push(
            &mut h,
            Sample { at: 1660, five: 100, seven: 90, five_reset: FIVE_RESET + 5 * 3600, seven_reset: SEVEN_RESET },
        );
        assert_eq!(h.len(), 1);
        assert_eq!(of(&h, &snap(100, FIVE_RESET + 5 * 3600, 90, SEVEN_RESET), 1660), Pace::Unknown);
    }

    #[test]
    fn samples_older_than_the_window_fall_out_the_front() {
        let mut h = VecDeque::new();
        push(&mut h, sample(0, 90, 95));
        push(&mut h, sample(HISTORY_SECS + 1, 80, 95));
        assert_eq!(h.len(), 1);
    }
}
