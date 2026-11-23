//! Wonderland Sky (A2): layered time-of-day sky backdrop inside the island.
//!
//! Draws a GPU-accelerated vertical gradient, soft radial sun/moon glow,
//! layered bottom clouds, and subtle sky accents (birds, sparkles, shooting
//! star) clipped to the island outline. Zero idle CPU: only updates when a
//! frame renders (including the once-a-minute clock wake).

use crate::gfx::Canvas;
use crate::state::AppState;
use crate::ui::{wid, Ui};
use poni_core::geom::{Color, Point, Rect};
use poni_core::svgpath::{self, Seg};
use std::f32::consts::PI;
use std::sync::OnceLock;

/// Duration of the interactive sky burst animation (seconds).
pub const BURST_SECS: f64 = 1.2;

/// Returns normalized burst progress `0.0..=1.0` while an interactive sky burst is active.
pub fn burst_progress(burst_at: Option<f64>, now: f64) -> Option<f32> {
    let t0 = burst_at?;
    let elapsed = now - t0;
    if elapsed >= 0.0 && elapsed < BURST_SECS {
        Some((elapsed / BURST_SECS) as f32)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Morning,
    Noon,
    Dusk,
    Night,
}

pub fn phase(hour: u8) -> Phase {
    match hour {
        5..=9 => Phase::Morning,
        10..=16 => Phase::Noon,
        17..=19 => Phase::Dusk,
        _ => Phase::Night,
    }
}

/// Normalized progress (0.0..=1.0) through the current sky phase.
pub fn phase_progress(hour: u8, minute: u8) -> f32 {
    let m = (hour as f32) * 60.0 + (minute as f32);
    match phase(hour) {
        Phase::Morning => ((m - 5.0 * 60.0) / (5.0 * 60.0)).clamp(0.0, 1.0),
        Phase::Noon => ((m - 10.0 * 60.0) / (7.0 * 60.0)).clamp(0.0, 1.0),
        Phase::Dusk => ((m - 17.0 * 60.0) / (3.0 * 60.0)).clamp(0.0, 1.0),
        Phase::Night => {
            let shifted = if hour >= 20 { m - 20.0 * 60.0 } else { m + 4.0 * 60.0 };
            (shifted / (9.0 * 60.0)).clamp(0.0, 1.0)
        }
    }
}

const SUN_RAYS: &str = "M12 2v2.2M12 19.8V22M4.93 4.93l1.55 1.55M17.52 17.52l1.55 1.55M2 12h2.2M19.8 12H22M4.93 19.07l1.55-1.55M17.52 6.48l1.55-1.55";
const MOON: &str = "M19.77 13.9A8 8 0 1 1 10.1 4.23A7 7 0 0 0 19.77 13.9Z";
const STAR: &str = "M12 3Q13.4 10.6 21 12Q13.4 13.4 12 21Q10.6 13.4 3 12Q10.6 10.6 12 3Z";
const BIRD: &str = "M3 14Q7.5 8 12 13.5Q16.5 8 21 14";

struct Glyphs {
    rays: Vec<Seg>,
    moon: Vec<Seg>,
    star: Vec<Seg>,
    bird: Vec<Seg>,
}

fn glyphs() -> &'static Glyphs {
    static CACHE: OnceLock<Glyphs> = OnceLock::new();
    CACHE.get_or_init(|| Glyphs {
        rays: svgpath::parse(SUN_RAYS).unwrap_or_default(),
        moon: svgpath::parse(MOON).unwrap_or_default(),
        star: svgpath::parse(STAR).unwrap_or_default(),
        bird: svgpath::parse(BIRD).unwrap_or_default(),
    })
}

fn transform(segs: &[Seg], center: Point, size: f32) -> Vec<Seg> {
    let k = size / 24.0;
    let ox = center.x - 12.0 * k;
    let oy = center.y - 12.0 * k;
    let map = |x: f32, y: f32| (ox + x * k, oy + y * k);
    segs.iter()
        .map(|s| match *s {
            Seg::MoveTo(x, y) => {
                let (x, y) = map(x, y);
                Seg::MoveTo(x, y)
            }
            Seg::LineTo(x, y) => {
                let (x, y) = map(x, y);
                Seg::LineTo(x, y)
            }
            Seg::QuadTo(x1, y1, x, y) => {
                let ((x1, y1), (x, y)) = (map(x1, y1), map(x, y));
                Seg::QuadTo(x1, y1, x, y)
            }
            Seg::CubicTo(x1, y1, x2, y2, x, y) => {
                let ((x1, y1), (x2, y2), (x, y)) = (map(x1, y1), map(x2, y2), map(x, y));
                Seg::CubicTo(x1, y1, x2, y2, x, y)
            }
            Seg::ArcTo(rx, ry, rot, large, sweep, x, y) => {
                let (x, y) = map(x, y);
                Seg::ArcTo(rx * k, ry * k, rot, large, sweep, x, y)
            }
            Seg::Close => Seg::Close,
        })
        .collect()
}

fn draw_sun(c: &mut Canvas, center: Point, radius: f32, core: Color, glow: Color, rays: bool) {
    c.fill_radial_glow(center, radius * 2.5, glow, glow.with_alpha(0.0));
    c.fill_circle(center, radius, core);
    if rays {
        let segs = transform(&glyphs().rays, center, radius * 3.3);
        c.path(&segs, None, Some((core.mul_alpha(0.85), (radius / 5.5).max(1.2))));
    }
}

/// Fluffy bottom cloud bank made of overlapping soft circles along `y_base`.
fn draw_cloud_bank(c: &mut Canvas, body: Rect, color: Color, puffs: &[(f32, f32, f32)]) {
    let base_y = body.bottom();
    for &(rx, dy, r) in puffs {
        let cx = body.x + body.w * rx;
        c.fill_circle(Point::new(cx, base_y - dy), r, color);
    }
}

/// Where the sun or moon sits in the open card (the same spot `draw` paints
/// it at), so the click target follows the ornament instead of a fixed corner.
fn ornament_center(body: Rect, hour: u8, minute: u8) -> Point {
    let t = phase_progress(hour, minute);
    let drift = (t - 0.5) * 26.0;
    match phase(hour) {
        Phase::Morning => Point::new(body.right() - 48.0 + drift * 0.5, body.bottom() - 18.0 - t * 8.0),
        Phase::Noon => Point::new(body.right() - 52.0 + drift * 0.7, body.bottom() - 22.0),
        Phase::Dusk => Point::new(body.right() - 56.0 + drift * 0.5, body.bottom() - 12.0 + t * 4.0),
        Phase::Night => Point::new(body.right() - 56.0 + drift * 0.4, body.y + body.h * 0.34),
    }
}

/// Top and bottom colours of a phase's sky gradient.
fn phase_colors(p: Phase) -> (Color, Color) {
    match p {
        Phase::Morning => (Color::hex(0xCBE4FB), Color::hex(0xFFF3DB)),
        Phase::Noon => (Color::hex(0xB8DCFA), Color::hex(0xEFF7FF)),
        Phase::Dusk => (Color::hex(0xF5C299), Color::hex(0xFCE7CE)),
        Phase::Night => (Color::hex(0x1B2352), Color::hex(0x2F3A7A)),
    }
}

/// Minutes over which the sky eases into a new phase.
const FADE_MIN: f32 = 20.0;

/// The gradient to paint now. Morning→noon and noon→dusk blend from the
/// previous phase over [`FADE_MIN`] minutes. The night boundaries (20:00 and
/// 05:00) switch at once: the ink flips between dark and light there, and a
/// half-blended sky would leave the text unreadable.
fn sky_gradient(hour: u8, minute: u8) -> (Color, Color) {
    let (top, bot) = phase_colors(phase(hour));
    let since = |start: u8| (hour as f32 - start as f32) * 60.0 + minute as f32;
    let blend = match phase(hour) {
        Phase::Noon => Some((Phase::Morning, since(10))),
        Phase::Dusk => Some((Phase::Noon, since(17))),
        _ => None,
    };
    match blend {
        Some((prev, m)) if (0.0..FADE_MIN).contains(&m) => {
            let k = m / FADE_MIN;
            let (pt, pb) = phase_colors(prev);
            (Color::lerp(pt, top, k), Color::lerp(pb, bot, k))
        }
        _ => (top, bot),
    }
}

/// Register a background click target over the sky ornament in expanded mode.
/// Must be called before page widgets so foreground buttons retain click priority.
pub fn hit(ui: &mut Ui, st: &mut AppState, body: Rect) {
    if !st.settings.langit_wonderland || body.w < 120.0 || body.h < 80.0 {
        return;
    }
    let center = ornament_center(body, st.local.hour as u8, st.local.minute as u8);
    let r = Rect::from_center(center, 76.0, 76.0);
    if ui.button(wid("sky.ornament"), r).clicked {
        st.ui.sky_burst_at = Some(ui.now);
        ui.request_anim();
    } else if burst_progress(st.ui.sky_burst_at, ui.now).is_some() {
        ui.request_anim();
    } else {
        st.ui.sky_burst_at = None;
    }
}

/// Draw the full layered Wonderland Sky inside `body` (caller clips to island outline).
pub fn draw(c: &mut Canvas, body: Rect, hour: u8, minute: u8, expanded: bool, burst: Option<f32>) {
    if body.w < 40.0 || body.h < 20.0 {
        return;
    }
    let p = phase(hour);
    let t = phase_progress(hour, minute);
    let g = glyphs();

    // Keep 0.5 DIP inset so the outer notch border stays crisp.
    let sky_r = body.inset(0.5, 0.5);

    c.unthemed(|c| {
        // 1. Vertical sky gradient (eased between phases where the ink allows)
        let (top_c, bot_c) = sky_gradient(hour, minute);
        c.fill_linear_gradient(sky_r, top_c, bot_c);

        if expanded {
            let drift = (t - 0.5) * 26.0;
            let pulse = burst.map(|u| (u * PI).sin()).unwrap_or(0.0);
            match p {
                Phase::Morning => {
                    // Sparkles top-left
                    for (x, y, sz) in [(0.08, 0.22, 11.0), (0.14, 0.32, 7.0)] {
                        let dy = burst.map(|u| -8.0 * u).unwrap_or(0.0);
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y + dy);
                        c.path(&transform(&g.star, sc, sz * (1.0 + 0.35 * pulse)), Some(Color::hex(0xFFE799).with_alpha(0.75 + 0.25 * pulse)), None);
                    }
                    // Back cloud layer
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0xD3E5FA).with_alpha(0.65),
                        &[(0.06, 6.0, 26.0), (0.16, 2.0, 22.0), (0.76, 4.0, 24.0), (0.86, 10.0, 28.0), (0.96, 14.0, 26.0)],
                    );
                    // Rising sun bottom-right
                    let sun_c = Point::new(body.right() - 48.0 + drift * 0.5, body.bottom() - 18.0 - t * 8.0);
                    draw_sun(c, sun_c, 15.0 * (1.0 + 0.12 * pulse), Color::hex(0xFFD15C), Color::hex(0xFFD15C).with_alpha(0.45 + 0.25 * pulse), true);
                    // Front cloud layer
                    draw_cloud_bank(
                        c,
                        body,
                        Color::white(0.82),
                        &[(0.03, 2.0, 24.0), (0.12, -4.0, 20.0), (0.24, -6.0, 16.0), (0.70, -5.0, 18.0), (0.82, -2.0, 20.0)],
                    );
                }
                Phase::Noon => {
                    // Birds top-right
                    for (x, y, sz) in [(0.80, 0.20, 14.0), (0.86, 0.26, 11.0)] {
                        let bc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.bird, bc, sz), None, Some((Color::white(0.80), 1.4)));
                    }
                    // Interactive flying birds on click
                    if let Some(u) = burst {
                        let ease = 1.0 - (1.0 - u).powi(2);
                        let alpha = (u * PI).sin().clamp(0.0, 1.0) * 0.90;
                        for (dx0, dy0, sz) in [(0.0, 0.0, 13.0), (22.0, 10.0, 10.0)] {
                            let bc = Point::new(body.right() - 40.0 - ease * 140.0 + dx0, body.bottom() - 46.0 - ease * 42.0 + dy0);
                            c.path(&transform(&g.bird, bc, sz), None, Some((Color::white(alpha), 1.5)));
                        }
                    }
                    // Back blue-tinted clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0xC2DFFA).with_alpha(0.70),
                        &[(0.05, 10.0, 28.0), (0.16, 4.0, 24.0), (0.76, 4.0, 24.0), (0.88, 12.0, 28.0), (0.97, 8.0, 24.0)],
                    );
                    // Bright sun bottom-right
                    let sun_c = Point::new(body.right() - 52.0 + drift * 0.7, body.bottom() - 22.0);
                    draw_sun(c, sun_c, 16.0 * (1.0 + 0.12 * pulse), Color::hex(0xFFD052), Color::hex(0xFFD052).with_alpha(0.42 + 0.28 * pulse), true);
                    // Front white clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::white(0.88),
                        &[(0.02, 2.0, 22.0), (0.11, -2.0, 22.0), (0.21, -5.0, 18.0), (0.71, -4.0, 18.0), (0.94, 0.0, 22.0)],
                    );
                }
                Phase::Dusk => {
                    // Warm sparkles
                    for (x, y, sz) in [(0.09, 0.22, 10.0), (0.88, 0.25, 8.0)] {
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.star, sc, sz * (1.0 + 0.35 * pulse)), Some(Color::white(0.65 + 0.30 * pulse)), None);
                    }
                    // Setting sun low behind clouds
                    let sun_c = Point::new(body.right() - 56.0 + drift * 0.5, body.bottom() - 12.0 + t * 4.0);
                    draw_sun(c, sun_c, 18.0 * (1.0 + 0.10 * pulse), Color::hex(0xFFC247), Color::hex(0xFF9F40).with_alpha(0.45 + 0.28 * pulse), false);
                    // Back amber clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0xF0A665).with_alpha(0.55),
                        &[(0.04, 8.0, 28.0), (0.15, 2.0, 22.0), (0.76, 6.0, 26.0), (0.88, 10.0, 28.0), (0.97, 4.0, 22.0)],
                    );
                    // Front soft peach clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0xFDE5C8).with_alpha(0.75),
                        &[(0.02, 0.0, 22.0), (0.11, -4.0, 20.0), (0.68, -4.0, 20.0), (0.82, -2.0, 22.0)],
                    );
                }
                Phase::Night => {
                    // Ambient shooting star top-left
                    let s0 = Point::new(body.x + body.w * 0.16, body.y + body.h * 0.26);
                    let s1 = Point::new(body.x + body.w * 0.08, body.y + body.h * 0.38);
                    c.line(s0, s1, Color::hex(0xDCE6FF).with_alpha(0.45), 1.3);
                    c.fill_circle(s0, 1.8, Color::hex(0xFFF6D6).with_alpha(0.85));

                    // Interactive shooting star streak on click
                    if let Some(u) = burst {
                        let ease = 1.0 - (1.0 - u).powi(2);
                        let alpha = (u * PI).sin().clamp(0.0, 1.0);
                        let head = Point::new(body.right() - 72.0 - ease * 180.0, body.y + 52.0 + ease * 58.0);
                        let tail = Point::new(head.x + 34.0, head.y - 11.0);
                        c.line(head, tail, Color::hex(0xFFF6D6).with_alpha(0.80 * alpha), 1.8);
                        c.fill_circle(head, 2.5, Color::hex(0xFFF9E6).with_alpha(alpha));
                    }

                    // Twinkling stars
                    for (x, y, sz) in [(0.10, 0.20, 7.0), (0.28, 0.16, 6.0), (0.68, 0.22, 8.0), (0.90, 0.18, 6.0), (0.74, 0.44, 5.5)] {
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.star, sc, sz * (1.0 + 0.35 * pulse)), Some(Color::hex(0xFFF2B8).with_alpha(0.70 + 0.30 * pulse)), None);
                    }
                    // Crescent moon + glow
                    let moon_c = Point::new(body.right() - 56.0 + drift * 0.4, body.y + body.h * 0.34);
                    c.fill_radial_glow(moon_c, 26.0 * (1.0 + 0.25 * pulse), Color::hex(0xF5D27A).with_alpha(0.22 + 0.20 * pulse), Color::hex(0xF5D27A).with_alpha(0.0));
                    c.path(&transform(&g.moon, moon_c, 24.0), Some(Color::hex(0xF8E4A0).with_alpha(0.88)), None);

                    // Deep indigo-violet layered bottom clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0x3D498C).with_alpha(0.65),
                        &[(0.05, 8.0, 28.0), (0.16, 3.0, 24.0), (0.78, 5.0, 26.0), (0.90, 10.0, 28.0), (0.98, 4.0, 22.0)],
                    );
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0x4E5CA8).with_alpha(0.45),
                        &[(0.02, 0.0, 22.0), (0.12, -4.0, 20.0), (0.70, -4.0, 20.0), (0.84, -2.0, 22.0)],
                    );
                }
            }
            // Morning and dusk: a few sparkles float up from the sun on click.
            if let (Some(u), Phase::Morning | Phase::Dusk) = (burst, p) {
                let origin = ornament_center(body, hour, minute);
                let alpha = (u * PI).sin().clamp(0.0, 1.0);
                let tint = if p == Phase::Morning { Color::hex(0xFFE08A) } else { Color::white(1.0) };
                for (i, dx) in [-22.0f32, -8.0, 9.0, 24.0].into_iter().enumerate() {
                    let rise = u * (34.0 + 7.0 * i as f32);
                    let sc = Point::new(origin.x + dx, origin.y - 14.0 - rise);
                    c.path(&transform(&g.star, sc, 7.0 - i as f32 * 0.6), Some(tint.with_alpha(0.85 * alpha)), None);
                }
            }
        } else {
            // Collapsed idle pill: soft cloud/star texture plus a corner accent
            let drift = (t - 0.5) * 5.0;
            match p {
                Phase::Night => {
                    for (x, y, sz) in [(0.30, 0.30, 4.5), (0.62, 0.72, 4.0), (0.80, 0.28, 3.5)] {
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.star, sc, sz), Some(Color::hex(0xFFF2B8).with_alpha(0.38)), None);
                    }
                }
                _ => {
                    let puff = if p == Phase::Dusk { Color::hex(0xFDE5C8).with_alpha(0.45) } else { Color::white(0.42) };
                    draw_cloud_bank(c, body, puff, &[(0.34, -2.0, 7.0), (0.46, -3.0, 8.0), (0.58, -2.0, 7.0)]);
                }
            }
            match p {
                Phase::Morning => {
                    let sun_c = Point::new(body.right() - 15.0 + drift, body.center().y - 1.0);
                    draw_sun(c, sun_c, 5.0, Color::hex(0xFFD15C).with_alpha(0.70), Color::hex(0xFFD15C).with_alpha(0.32), true);
                    c.fill_circle(Point::new(body.right() - 10.0, body.bottom() - 2.0), 7.5, Color::white(0.65));
                }
                Phase::Noon => {
                    let sun_c = Point::new(body.right() - 14.0 + drift, body.center().y);
                    draw_sun(c, sun_c, 5.5, Color::hex(0xFFD052).with_alpha(0.68), Color::hex(0xFFD052).with_alpha(0.30), true);
                }
                Phase::Dusk => {
                    let sun_c = Point::new(body.right() - 15.0 + drift, body.bottom() - 3.0);
                    draw_sun(c, sun_c, 6.0, Color::hex(0xFFC247).with_alpha(0.75), Color::hex(0xFF9F40).with_alpha(0.35), false);
                    c.fill_circle(Point::new(body.right() - 9.0, body.bottom() - 1.0), 6.5, Color::hex(0xFDE5C8).with_alpha(0.55));
                }
                Phase::Night => {
                    let moon_c = Point::new(body.right() - 13.0 + drift, body.center().y);
                    c.fill_radial_glow(moon_c, 11.0, Color::hex(0xF5D27A).with_alpha(0.20), Color::hex(0xF5D27A).with_alpha(0.0));
                    c.path(&transform(&g.moon, moon_c, 12.0), Some(Color::hex(0xF8E4A0).with_alpha(0.85)), None);
                    let star_c = Point::new(body.right() - 24.0, body.y + 7.0);
                    c.path(&transform(&g.star, star_c, 6.0), Some(Color::hex(0xFFF2B8).with_alpha(0.75)), None);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sky_eases_in_after_morning_and_noon_but_switches_at_once_at_night() {
        let (noon_top, noon_bot) = phase_colors(Phase::Noon);
        let (morn_top, _) = phase_colors(Phase::Morning);
        // 10:00 still wears the morning's colours, 10:20 is fully noon.
        assert_eq!(sky_gradient(10, 0).0, morn_top);
        assert_eq!(sky_gradient(10, 20), (noon_top, noon_bot));
        let mid = sky_gradient(10, 10).0;
        assert_ne!(mid, morn_top);
        assert_ne!(mid, noon_top);
        // The night's boundaries never blend (the ink flips there).
        assert_eq!(sky_gradient(20, 0), phase_colors(Phase::Night));
        assert_eq!(sky_gradient(5, 0), phase_colors(Phase::Morning));
    }

    #[test]
    fn the_click_target_follows_the_ornament() {
        let body = Rect::new(0.0, 0.0, 620.0, 240.0);
        for h in [6u8, 12, 18, 22] {
            let c = ornament_center(body, h, 30);
            assert!(body.contains(c), "hour {h}: {c:?}");
        }
        // The moon sits high, the sun low.
        assert!(ornament_center(body, 22, 0).y < ornament_center(body, 12, 0).y);
    }

    #[test]
    fn sky_glyphs_parse_cleanly() {
        let g = glyphs();
        assert!(!g.rays.is_empty());
        assert!(!g.moon.is_empty());
        assert!(!g.star.is_empty());
        assert!(!g.bird.is_empty());
    }

    #[test]
    fn sky_phases_and_progress_stay_in_range() {
        assert_eq!(phase(6), Phase::Morning);
        assert_eq!(phase(12), Phase::Noon);
        assert_eq!(phase(18), Phase::Dusk);
        assert_eq!(phase(22), Phase::Night);
        assert_eq!(phase(2), Phase::Night);
        for h in 0..24 {
            for m in [0, 30, 59] {
                let p = phase_progress(h, m);
                assert!((0.0..=1.0).contains(&p), "{h}:{m} -> {p}");
            }
        }
    }
}
