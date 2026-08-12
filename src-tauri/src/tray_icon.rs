//! The menu-bar face of the app: a pair of vertical pills per account, drawn
//! at runtime.
//!
//! Within a pair, the left pill is the 5-hour window and the right one the
//! week. Fill height is the remaining share; each pill wears its own level
//! colour. Pairs sit apart from each other by a wider gap, in the same order
//! the panel lists them — personal first — so a glance says both which account
//! is running out and which of its windows.
//!
//! When an update is waiting, a green badge sits in the top-right corner — same
//! signal as Ribbit/Quill/Iago, whose badge lives on a static PNG; here the
//! icon is redrawn on every data change, so the badge is composed in.

use crate::limits::{level, Level};

/// Canvas height in pixels. 32 px, same as the neighbours (Iago/Ribbit ship
/// 32x32.png tray icons): NSImage takes pixel size as points, and a 44 pt
/// image did not fit the menu bar - the status item rendered nothing at all.
/// Width is free, though, and grows with the number of accounts.
pub const HEIGHT: usize = 32;

/// A single account keeps the shape Camel has always had: two fat pills on a
/// square. Two or more, and the pills slim down to make room.
const SOLO_BAR_W: usize = 10;
const SOLO_GAP: usize = 4;
const SOLO_SIDE: usize = 32;

const BAR_W: usize = 8;
/// Between the two windows of one account.
const GAP: usize = 3;
/// Between accounts — wide enough that the pairs read as pairs.
const ACCOUNT_GAP: usize = 8;
/// Pills sit slightly in from the edges so the badge corner stays clear.
const MARGIN_Y: usize = 3;
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

/// A drawn icon: pixels plus the width they were drawn at, since the canvas
/// grows with the number of accounts while the menu bar fixes the height.
pub struct Icon {
    pub pixels: Vec<u8>,
    pub width: usize,
}

/// Where each account's pair of pills starts, and how wide a pill is.
///
/// The menu bar fixes the height and leaves the width alone, so more accounts
/// widen the icon instead of squeezing the pills into mush.
fn layout(accounts: usize) -> (usize, usize, Vec<usize>) {
    if accounts <= 1 {
        let left = (SOLO_SIDE - (2 * SOLO_BAR_W + SOLO_GAP)) / 2;
        return (SOLO_SIDE, SOLO_BAR_W, vec![left]);
    }
    let pair = 2 * BAR_W + GAP;
    let width = accounts * pair + (accounts - 1) * ACCOUNT_GAP;
    let starts = (0..accounts).map(|i| i * (pair + ACCOUNT_GAP)).collect();
    (width, BAR_W, starts)
}

/// One pair of pills per account: (five_hour, seven_day) remaining percent.
/// An empty list means no data at all — one grey pair, so the icon still reads
/// as Camel rather than as a hole in the menu bar.
pub fn render(accounts: &[(u8, u8)], update_badge: bool) -> Icon {
    let (width, bar_w, starts) = layout(accounts.len());
    let radius = bar_w as f32 / 2.0;
    let mut px = vec![0u8; width * HEIGHT * 4];
    let heights = |remaining: u8| -> usize {
        // Never less than the round end: a sliver clipped by the bottom cap
        // reads as a rendering fault, not as an empty tank, and a bar that
        // vanishes outright reads as a broken icon.
        let usable = HEIGHT - 2 * MARGIN_Y;
        (usable * remaining.min(100) as usize / 100).max(radius as usize)
    };
    let bar = |px: &mut Vec<u8>, x0: usize, remaining: Option<u8>| {
        let fill_h = heights(remaining.unwrap_or(100));
        let fill_colour = remaining.map(colour).unwrap_or(UNKNOWN);
        // Where the fill's flat top sits. Below it the pill is level colour,
        // above it the dim track — both clipped to the same rounded outline.
        let level_y = (HEIGHT - MARGIN_Y - fill_h) as f32;
        for y in MARGIN_Y..HEIGHT - MARGIN_Y {
            for x in x0..x0 + bar_w {
                // Averaging premultiplied colour over the subsamples gives the
                // rounded ends their soft edge and, on the fill line, blends
                // the two colours in whatever ratio the pixel actually holds.
                let mut acc = [0f32; 4];
                for sy in 0..SS {
                    for sx in 0..SS {
                        let px_x = x as f32 + (sx as f32 + 0.5) / SS as f32;
                        let px_y = y as f32 + (sy as f32 + 0.5) / SS as f32;
                        if !in_pill(px_x, px_y, x0 as f32, radius) {
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
                put(px, width, x, y, c);
            }
        }
    };
    for (i, x0) in starts.iter().enumerate() {
        let pair = accounts.get(i).copied();
        bar(&mut px, *x0, pair.map(|p| p.0));
        bar(&mut px, x0 + bar_w + if accounts.len() <= 1 { SOLO_GAP } else { GAP }, pair.map(|p| p.1));
    }

    if update_badge {
        // Top-right corner, ringed so it separates from whatever is under it.
        let cx = width as f32 - BADGE_R - 1.0;
        let cy = BADGE_R + 1.0;
        for y in 0..HEIGHT {
            for x in 0..width {
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
                    width,
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
    Icon { pixels: px, width }
}

/// Is this point inside the pill whose left edge is `x0`? Distance to the
/// rounded rectangle, measured from the nearest point of its straight core.
fn in_pill(x: f32, y: f32, x0: f32, radius: f32) -> bool {
    let top = MARGIN_Y as f32;
    let bottom = (HEIGHT - MARGIN_Y) as f32;
    let cx = x0 + radius;
    let cy = y.clamp(top + radius, bottom - radius);
    let dx = x - cx;
    let dy = y - cy;
    (dx * dx + dy * dy).sqrt() <= radius
}

fn put(px: &mut [u8], width: usize, x: usize, y: usize, c: [u8; 4]) {
    let i = (y * width + x) * 4;
    px[i..i + 4].copy_from_slice(&c);
}

/// Source-over: `c` laid on whatever the canvas already holds there.
fn blend(px: &mut [u8], width: usize, x: usize, y: usize, c: [u8; 4]) {
    let i = (y * width + x) * 4;
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

    fn at(icon: &Icon, x: usize, y: usize) -> [u8; 4] {
        let i = (y * icon.width + x) * 4;
        [icon.pixels[i], icon.pixels[i + 1], icon.pixels[i + 2], icon.pixels[i + 3]]
    }

    /// Column inside the left bar / right bar of a lone account.
    const LX: usize = 8;
    const RX: usize = 24;

    #[test]
    fn bar_heights_follow_the_remaining_share() {
        let icon = render(&[(100, 10)], false);
        // Left bar full: coloured right below the top margin.
        assert_eq!(at(&icon, LX, MARGIN_Y + 1), GREEN);
        // Right bar nearly empty: track at the top, red near the bottom.
        assert_eq!(at(&icon, RX, MARGIN_Y + 1), TRACK);
        assert_eq!(at(&icon, RX, HEIGHT - MARGIN_Y - 2), RED);
    }

    #[test]
    fn each_bar_wears_its_own_level_colour() {
        let icon = render(&[(73, 35)], false);
        assert_eq!(at(&icon, LX, HEIGHT - MARGIN_Y - 2), GREEN);
        assert_eq!(at(&icon, RX, HEIGHT - MARGIN_Y - 2), YELLOW);
    }

    #[test]
    fn the_bars_are_pills_the_corners_stay_empty() {
        let icon = render(&[(100, 100)], false);
        // Left bar spans x 4..14: its top-left corner falls outside the round
        // end, while the same column at mid height is solidly inside.
        assert_eq!(at(&icon, 4, MARGIN_Y)[3], 0);
        assert_eq!(at(&icon, 4, HEIGHT - MARGIN_Y - 1)[3], 0);
        assert_eq!(at(&icon, 4, HEIGHT / 2), GREEN);
    }

    #[test]
    fn no_data_renders_grey_bars_not_an_empty_square() {
        let icon = render(&[], false);
        assert_eq!(icon.width, SOLO_SIDE);
        assert_eq!(at(&icon, LX, HEIGHT / 2), UNKNOWN);
        assert_eq!(at(&icon, RX, HEIGHT / 2), UNKNOWN);
    }

    #[test]
    fn a_lone_account_keeps_the_square_icon() {
        // The shape Camel shipped with: one account must not look different
        // just because the renderer learned to draw several.
        assert_eq!(render(&[(50, 50)], false).width, 32);
        assert_eq!(render(&[(50, 50)], false).pixels.len(), 32 * HEIGHT * 4);
    }

    #[test]
    fn a_second_account_widens_the_icon_and_keeps_its_height() {
        let icon = render(&[(90, 90), (10, 10)], false);
        // Two pairs of 8 px pills: 19 px each, 8 px between them.
        assert_eq!(icon.width, 46);
        assert_eq!(icon.pixels.len(), 46 * HEIGHT * 4);
    }

    #[test]
    fn each_account_wears_its_own_colours_in_its_own_pair() {
        // Personal is fine, work is nearly out: the icon has to say which one.
        let icon = render(&[(90, 90), (10, 10)], false);
        let low = HEIGHT - MARGIN_Y - 2;
        for x in [4, 15] {
            assert_eq!(at(&icon, x, low), GREEN, "left pair at x={}", x);
        }
        for x in [31, 42] {
            assert_eq!(at(&icon, x, low), RED, "right pair at x={}", x);
        }
    }

    #[test]
    fn the_gap_between_accounts_is_wider_than_the_one_inside() {
        // The pairs have to read as pairs, and that rests entirely on the empty
        // space: the run of blank columns between two accounts must be longer
        // than the one between an account's own two windows.
        let icon = render(&[(100, 100), (100, 100)], false);
        let mid = HEIGHT / 2;
        let blank: Vec<usize> = (0..icon.width).filter(|&x| at(&icon, x, mid)[3] == 0).collect();
        let mut runs = vec![1];
        for pair in blank.windows(2) {
            if pair[1] == pair[0] + 1 {
                *runs.last_mut().unwrap() += 1;
            } else {
                runs.push(1);
            }
        }
        runs.sort();
        // Three empty runs: two inside the pairs, one between the accounts.
        assert_eq!(runs, vec![GAP, GAP, ACCOUNT_GAP], "empty runs across the icon");
    }

    /// Not a test — a faucet. `cargo test -- --ignored dump_icons` writes the
    /// real rendered icons as raw RGBA for the README compositor, so the
    /// screenshots show pixels this code actually draws.
    #[test]
    #[ignore]
    fn dump_icons_for_readme() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target/icon-dump");
        std::fs::create_dir_all(&dir).unwrap();
        for (name, accounts, badge) in [
            ("ok", vec![(73u8, 95u8)], false),
            ("low", vec![(9, 38)], false),
            ("update", vec![(73, 95)], true),
            ("two-accounts", vec![(41, 74), (88, 19)], false),
        ] {
            let icon = render(&accounts, badge);
            std::fs::write(dir.join(format!("{}-{}.rgba", name, icon.width)), icon.pixels).unwrap();
        }
    }

    #[test]
    fn the_update_icon_carries_the_green_badge() {
        let plain = render(&[(73, 95)], false);
        let lit = render(&[(73, 95)], true);
        // Badge centre, top-right corner.
        let cx = lit.width - 6;
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
