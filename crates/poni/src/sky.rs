//! Wonderland Sky (A2): layered time-of-day sky backdrop inside the island.
//!
//! Draws a GPU-accelerated vertical gradient, soft radial sun/moon glow,
//! layered bottom clouds, and subtle sky accents (birds, sparkles, shooting
//! star) clipped to the island outline. Zero idle CPU: only updates when a
//! frame renders (including the once-a-minute clock wake).

use crate::gfx::Canvas;
use poni_core::geom::{Color, Point, Rect};
use poni_core::svgpath::{self, Seg};
use std::sync::OnceLock;

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

/// Draw the full layered Wonderland Sky inside `body` (caller clips to island outline).
pub fn draw(c: &mut Canvas, body: Rect, hour: u8, minute: u8, expanded: bool) {
    if body.w < 40.0 || body.h < 20.0 {
        return;
    }
    let p = phase(hour);
    let t = phase_progress(hour, minute);
    let g = glyphs();

    // Keep 0.5 DIP inset so the outer notch border stays crisp.
    let sky_r = body.inset(0.5, 0.5);

    c.unthemed(|c| {
        // 1. Vertical sky gradient
        let (top_c, bot_c) = match p {
            Phase::Morning => (Color::hex(0xCBE4FB), Color::hex(0xFFF3DB)),
            Phase::Noon => (Color::hex(0xB8DCFA), Color::hex(0xEFF7FF)),
            Phase::Dusk => (Color::hex(0xF5C299), Color::hex(0xFCE7CE)),
            Phase::Night => (Color::hex(0x1B2352), Color::hex(0x2F3A7A)),
        };
        c.fill_linear_gradient(sky_r, top_c, bot_c);

        if expanded {
            let drift = (t - 0.5) * 26.0;
            match p {
                Phase::Morning => {
                    // Sparkles top-left
                    for (x, y, sz) in [(0.08, 0.22, 11.0), (0.14, 0.32, 7.0)] {
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.star, sc, sz), Some(Color::hex(0xFFE799).with_alpha(0.75)), None);
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
                    draw_sun(c, sun_c, 15.0, Color::hex(0xFFD15C), Color::hex(0xFFD15C).with_alpha(0.45), true);
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
                    // Back blue-tinted clouds
                    draw_cloud_bank(
                        c,
                        body,
                        Color::hex(0xC2DFFA).with_alpha(0.70),
                        &[(0.05, 10.0, 28.0), (0.16, 4.0, 24.0), (0.76, 4.0, 24.0), (0.88, 12.0, 28.0), (0.97, 8.0, 24.0)],
                    );
                    // Bright sun bottom-right
                    let sun_c = Point::new(body.right() - 52.0 + drift * 0.7, body.bottom() - 22.0);
                    draw_sun(c, sun_c, 16.0, Color::hex(0xFFD052), Color::hex(0xFFD052).with_alpha(0.42), true);
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
                        c.path(&transform(&g.star, sc, sz), Some(Color::white(0.65)), None);
                    }
                    // Setting sun low behind clouds
                    let sun_c = Point::new(body.right() - 56.0 + drift * 0.5, body.bottom() - 12.0 + t * 4.0);
                    draw_sun(c, sun_c, 18.0, Color::hex(0xFFC247), Color::hex(0xFF9F40).with_alpha(0.45), false);
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
                    // Shooting star top-left
                    let s0 = Point::new(body.x + body.w * 0.16, body.y + body.h * 0.26);
                    let s1 = Point::new(body.x + body.w * 0.08, body.y + body.h * 0.38);
                    c.line(s0, s1, Color::hex(0xDCE6FF).with_alpha(0.45), 1.3);
                    c.fill_circle(s0, 1.8, Color::hex(0xFFF6D6).with_alpha(0.85));

                    // Twinkling stars
                    for (x, y, sz) in [(0.10, 0.20, 7.0), (0.28, 0.16, 6.0), (0.68, 0.22, 8.0), (0.90, 0.18, 6.0), (0.74, 0.44, 5.5)] {
                        let sc = Point::new(body.x + body.w * x, body.y + body.h * y);
                        c.path(&transform(&g.star, sc, sz), Some(Color::hex(0xFFF2B8).with_alpha(0.70)), None);
                    }
                    // Crescent moon + glow
                    let moon_c = Point::new(body.right() - 56.0 + drift * 0.4, body.y + body.h * 0.34);
                    c.fill_radial_glow(moon_c, 26.0, Color::hex(0xF5D27A).with_alpha(0.22), Color::hex(0xF5D27A).with_alpha(0.0));
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
        } else {
            // Collapsed idle pill: subtle corner accent on the right edge
            let drift = (t - 0.5) * 5.0;
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
