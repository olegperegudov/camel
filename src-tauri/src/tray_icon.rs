//! The menu-bar face of the app: two vertical bars, drawn at runtime.
//!
//! Left bar — the 5-hour window, right bar — the week. Bar height is the
//! remaining share; each bar wears its own level colour. When an update is
//! waiting, a green badge sits in the top-right corner — same signal as
//! Ribbit/Quill/Iago, whose badge lives on a static PNG; here the icon is
//! redrawn on every data change, so the badge is composed in.

use crate::limits::{level, Level};

/// Canvas size in pixels. 32 px, same as the neighbours (Iago/Ribbit ship
/// 32x32.png tray icons): NSImage takes pixel size as points, and a 44 pt
/// image did not fit the menu bar - the status item rendered nothing at all.
pub const SIDE: usize = 32;

const BAR_W: usize = 10;
const GAP: usize = 4;
/// Bars sit slightly in from the edges so the badge corner stays clear.
const MARGIN_Y: usize = 3;

const GREEN: [u8; 4] = [48, 179, 80, 255];
const YELLOW: [u8; 4] = [224, 168, 0, 255];
const RED: [u8; 4] = [229, 72, 77, 255];
/// The empty part of a bar: visible on dark and light menu bars alike.
const TRACK: [u8; 4] = [128, 128, 134, 92];
/// Data missing: both bars full but plainly grey.
const UNKNOWN: [u8; 4] = [128, 128, 134, 160];

const BADGE: [u8; 4] = [46, 204, 113, 255];
const BADGE_RIM: [u8; 4] = [18, 90, 50, 255];
const BADGE_R: f32 = 5.5;
const BADGE_RIM_W: f32 = 1.5;

fn colour(remaining: u8) -> [u8; 4] {
    match level(remaining) {
        Level::Ok => GREEN,
        Level::Low => YELLOW,
        Level::Critical => RED,
    }
}

/// RGBA canvas, SIDE×SIDE. `bars` is (five_hour, seven_day) remaining percent,
/// `None` when there is no data to show.
pub fn render(bars: Option<(u8, u8)>, update_badge: bool) -> Vec<u8> {
    let mut px = vec![0u8; SIDE * SIDE * 4];
    let left_x = (SIDE - (2 * BAR_W + GAP)) / 2;
    let heights = |remaining: u8| -> usize {
        // Never zero: a vanished bar reads as a broken icon, not an empty tank.
        let usable = SIDE - 2 * MARGIN_Y;
        (usable * remaining.min(100) as usize / 100).max(2)
    };
    let bar = |px: &mut Vec<u8>, x0: usize, remaining: Option<u8>| {
        let fill_h = heights(remaining.unwrap_or(100));
        let fill_colour = remaining.map(colour).unwrap_or(UNKNOWN);
        for y in MARGIN_Y..SIDE - MARGIN_Y {
            let filled = y >= SIDE - MARGIN_Y - fill_h;
            let c = if filled { fill_colour } else { TRACK };
            for x in x0..x0 + BAR_W {
                put(px, x, y, c);
            }
        }
    };
    bar(&mut px, left_x, bars.map(|b| b.0));
    bar(&mut px, left_x + BAR_W + GAP, bars.map(|b| b.1));

    if update_badge {
        // Top-right corner, ringed so it separates from whatever is under it.
        let cx = SIDE as f32 - BADGE_R - 1.0;
        let cy = BADGE_R + 1.0;
        for y in 0..SIDE {
            for x in 0..SIDE {
                let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
                if d <= BADGE_R - BADGE_RIM_W {
                    put(&mut px, x, y, BADGE);
                } else if d <= BADGE_R {
                    put(&mut px, x, y, BADGE_RIM);
                }
            }
        }
    }
    px
}

fn put(px: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    let i = (y * SIDE + x) * 4;
    px[i..i + 4].copy_from_slice(&c);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(px: &[u8], x: usize, y: usize) -> [u8; 4] {
        let i = (y * SIDE + x) * 4;
        [px[i], px[i + 1], px[i + 2], px[i + 3]]
    }

    /// Column inside the left bar / right bar.
    const LX: usize = 8;
    const RX: usize = 24;

    #[test]
    fn bar_heights_follow_the_remaining_share() {
        let px = render(Some((100, 10)), false);
        // Left bar full: coloured right below the top margin.
        assert_eq!(at(&px, LX, MARGIN_Y + 1), GREEN);
        // Right bar nearly empty: track at the top, red near the bottom.
        assert_eq!(at(&px, RX, MARGIN_Y + 1), TRACK);
        assert_eq!(at(&px, RX, SIDE - MARGIN_Y - 2), RED);
    }

    #[test]
    fn each_bar_wears_its_own_level_colour() {
        let px = render(Some((73, 35)), false);
        assert_eq!(at(&px, LX, SIDE - MARGIN_Y - 2), GREEN);
        assert_eq!(at(&px, RX, SIDE - MARGIN_Y - 2), YELLOW);
    }

    #[test]
    fn no_data_renders_grey_bars_not_an_empty_square() {
        let px = render(None, false);
        assert_eq!(at(&px, LX, SIDE / 2), UNKNOWN);
        assert_eq!(at(&px, RX, SIDE / 2), UNKNOWN);
    }

    /// Not a test — a faucet. `cargo test -- --ignored dump_icons` writes the
    /// real rendered icons as raw RGBA for the README compositor, so the
    /// screenshots show pixels this code actually draws.
    #[test]
    #[ignore]
    fn dump_icons_for_readme() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/icon-dump");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bars, badge) in [
            ("ok", Some((73u8, 95u8)), false),
            ("low", Some((9, 38)), false),
            ("update", Some((73, 95)), true),
        ] {
            std::fs::write(dir.join(format!("{}.rgba", name)), render(bars, badge)).unwrap();
        }
    }

    #[test]
    fn the_update_icon_carries_the_green_badge() {
        let plain = render(Some((73, 95)), false);
        let lit = render(Some((73, 95)), true);
        // Badge centre, top-right corner.
        let cx = SIDE - 6;
        let cy = 6;
        assert_eq!(at(&lit, cx, cy), BADGE);
        assert_ne!(at(&plain, cx, cy), BADGE);
        // The rim keeps the badge readable over the bar underneath.
        assert_eq!(at(&lit, cx - BADGE_R as usize, cy), BADGE_RIM);
    }
}
