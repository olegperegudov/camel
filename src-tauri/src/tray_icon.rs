//! The menu-bar face of the app: two vertical pills, drawn at runtime.
//!
//! Left pill — the 5-hour window, right pill — the week. Fill height is the
//! remaining share; each pill wears its own level colour. When an update is
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
/// Pills sit slightly in from the edges so the badge corner stays clear.
const MARGIN_Y: usize = 3;
/// Fully round ends: at 10 px wide, half the width is the only radius that
/// reads as a pill rather than as a rectangle with the corners nicked off.
const RADIUS: f32 = BAR_W as f32 / 2.0;
/// Subsamples per axis when measuring how much of a pixel the pill covers.
/// 4x4 gives 17 alpha steps — enough that a 32 px icon has no visible stairs.
const SS: usize = 4;

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
        // Never less than the round end: a sliver clipped by the bottom cap
        // reads as a rendering fault, not as an empty tank, and a bar that
        // vanishes outright reads as a broken icon.
        let usable = SIDE - 2 * MARGIN_Y;
        (usable * remaining.min(100) as usize / 100).max(RADIUS as usize)
    };
    let bar = |px: &mut Vec<u8>, x0: usize, remaining: Option<u8>| {
        let fill_h = heights(remaining.unwrap_or(100));
        let fill_colour = remaining.map(colour).unwrap_or(UNKNOWN);
        // Where the fill's flat top sits. Below it the pill is level colour,
        // above it the dim track — both clipped to the same rounded outline.
        let level_y = (SIDE - MARGIN_Y - fill_h) as f32;
        for y in MARGIN_Y..SIDE - MARGIN_Y {
            for x in x0..x0 + BAR_W {
                // Averaging premultiplied colour over the subsamples gives the
                // rounded ends their soft edge and, on the fill line, blends
                // the two colours in whatever ratio the pixel actually holds.
                let mut acc = [0f32; 4];
                for sy in 0..SS {
                    for sx in 0..SS {
                        let px_x = x as f32 + (sx as f32 + 0.5) / SS as f32;
                        let px_y = y as f32 + (sy as f32 + 0.5) / SS as f32;
                        if !in_pill(px_x, px_y, x0 as f32) {
                            continue;
                        }
                        let c = if px_y >= level_y { fill_colour } else { TRACK };
                        let a = c[3] as f32 / 255.0;
                        for i in 0..3 {
                            acc[i] += c[i] as f32 * a;
                        }
                        acc[3] += a;
                    }
                }
                let n = (SS * SS) as f32;
                if acc[3] <= 0.0 {
                    continue;
                }
                let alpha = acc[3] / n;
                let c = [
                    (acc[0] / acc[3]).round() as u8,
                    (acc[1] / acc[3]).round() as u8,
                    (acc[2] / acc[3]).round() as u8,
                    (alpha * 255.0).round() as u8,
                ];
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
                // Same subsampling as the pills: a hard-edged circle next to
                // softened ends is the one shape that looks drawn by hand.
                let mut acc = [0f32; 4];
                for sy in 0..SS {
                    for sx in 0..SS {
                        let dx = x as f32 + (sx as f32 + 0.5) / SS as f32 - cx;
                        let dy = y as f32 + (sy as f32 + 0.5) / SS as f32 - cy;
                        let d = (dx * dx + dy * dy).sqrt();
                        let c = if d <= BADGE_R - BADGE_RIM_W {
                            BADGE
                        } else if d <= BADGE_R {
                            BADGE_RIM
                        } else {
                            continue;
                        };
                        for i in 0..3 {
                            acc[i] += c[i] as f32;
                        }
                        acc[3] += 1.0;
                    }
                }
                if acc[3] <= 0.0 {
                    continue;
                }
                let n = (SS * SS) as f32;
                // Over, not replace: the badge sits on top of a pill, and
                // overwriting its soft rim would punch holes in what is under.
                blend(
                    &mut px,
                    x,
                    y,
                    [
                        (acc[0] / acc[3]).round() as u8,
                        (acc[1] / acc[3]).round() as u8,
                        (acc[2] / acc[3]).round() as u8,
                        (255.0 * acc[3] / n).round() as u8,
                    ],
                );
            }
        }
    }
    px
}

/// Is this point inside the pill whose left edge is `x0`? Distance to the
/// rounded rectangle, measured from the nearest point of its straight core.
fn in_pill(x: f32, y: f32, x0: f32) -> bool {
    let top = MARGIN_Y as f32;
    let bottom = (SIDE - MARGIN_Y) as f32;
    let cx = x0 + RADIUS;
    let cy = y.clamp(top + RADIUS, bottom - RADIUS);
    let dx = x - cx;
    let dy = y - cy;
    (dx * dx + dy * dy).sqrt() <= RADIUS
}

fn put(px: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    let i = (y * SIDE + x) * 4;
    px[i..i + 4].copy_from_slice(&c);
}

/// Source-over: `c` laid on whatever the canvas already holds there.
fn blend(px: &mut [u8], x: usize, y: usize, c: [u8; 4]) {
    let i = (y * SIDE + x) * 4;
    let sa = c[3] as f32 / 255.0;
    let da = px[i + 3] as f32 / 255.0;
    let out_a = sa + da * (1.0 - sa);
    if out_a <= 0.0 {
        return;
    }
    for k in 0..3 {
        let s = c[k] as f32 * sa;
        let d = px[i + k] as f32 * da * (1.0 - sa);
        px[i + k] = ((s + d) / out_a).round() as u8;
    }
    px[i + 3] = (out_a * 255.0).round() as u8;
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
    fn the_bars_are_pills_the_corners_stay_empty() {
        let px = render(Some((100, 100)), false);
        // Left bar spans x 4..14: its top-left corner falls outside the round
        // end, while the same column at mid height is solidly inside.
        assert_eq!(at(&px, 4, MARGIN_Y)[3], 0);
        assert_eq!(at(&px, 4, SIDE - MARGIN_Y - 1)[3], 0);
        assert_eq!(at(&px, 4, SIDE / 2), GREEN);
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
        // The rim keeps the badge readable over the pill underneath. Its edge
        // is antialiased, so the check is that the ring darkens the badge
        // colour there — an exact rim value would only be testing the sampler.
        let ring = at(&lit, cx - BADGE_R as usize, cy);
        assert!(ring[1] < BADGE[1], "rim should darken the badge: {:?}", ring);
        assert!(ring[1] > BADGE_RIM[1], "and sit between rim and fill: {:?}", ring);
    }
}
