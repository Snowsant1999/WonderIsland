//! How the island paints in each theme: its ground, its ink and its accents.
//!
//! Pages and activities draw in one vocabulary, the Classic island's: a black
//! ground, white content at some opacity (text, icons, washes, tracks) and a
//! few system-style accents (`geom::palette`). A [`Look`] maps that vocabulary
//! onto its own colours as the canvas paints, so every page follows the theme
//! without knowing about it, including pages written later.
//!
//! * Greys (white, black and everything between, at any opacity) are read as
//!   "white at that brightness over the black island" and repainted between
//!   the look's ground and ink: Classic's white text becomes the ink, its 8 %
//!   white wash becomes an 8 % ink wash, the black island becomes the ground.
//! * The palette's accents swap for the look's own, with a deeper shade for
//!   text so small coloured labels stay readable on a light ground.
//! * Anything else (album-art colours, Oyen's own face, `palette::ON_ACCENT`)
//!   is left alone.
//!
//! A look also says what the island is made of: Classic and Oyen paint a
//! solid ground, Glass draws liquid glass there (gfx and the glass shader)
//! and paints content in the light or dark Glass look on top. And its
//! shape: every theme hangs from the top edge like a notch, except Pill,
//! which floats below it as a capsule (`crate::settings::Theme::Wonderland`).

use crate::geom::{palette, Color};
use crate::settings::Theme;

/// An accent and what a look paints instead: `fill` for shapes, `text` for
/// coloured text and icons.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Swap {
    pub from: Color,
    pub fill: Color,
    pub text: Color,
}

#[derive(Debug, PartialEq)]
pub struct Look {
    /// The island's fill: what Classic paints black.
    pub ground: Color,
    /// Content: what Classic paints white.
    pub ink: Color,
    /// The theme's own accent; `None` follows the Windows accent colour.
    pub accent: Option<Color>,
    /// Hairline around the island (Classic: a faint white edge on black).
    pub border: Color,
    /// Light ground with dark content.
    pub light: bool,
    /// Embedded font family the look writes in; `None` = the system UI font.
    pub font: Option<&'static str>,
    /// What the island's body is made of.
    pub material: Material,
    /// The island's outline.
    pub shape: Shape,
    swaps: &'static [Swap],
}

/// The island's body: a solid fill in `ground`, or liquid glass (with
/// `ground` as its nominal tint, used where the glass can't see the screen).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Solid,
    Glass,
}

/// The island's outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape {
    /// Hangs from the top edge: a flat top flush with the screen, concave
    /// shoulders flaring into the edge, rounded bottom corners.
    Notch,
    /// Floats a little below the top edge, every corner rounded: a capsule
    /// while it rests, a card while it's open.
    Pill,
}

pub const ALICE_BLUE: Color = Color::hex(0xF0F8FF);
pub const MIDNIGHT_INK: Color = Color::hex(0x1E2A4A);
pub const POCKET_WATCH_GOLD: Color = Color::hex(0xC9993A);
pub const POCKET_WATCH_TEXT: Color = Color::hex(0x8A6416);

/// Wonderland: Alice Blue paper, midnight ink, pocket watch gold accent,
/// and accessible palette swaps.
pub const WONDERLAND: Look = Look {
    ground: ALICE_BLUE,
    ink: MIDNIGHT_INK,
    accent: Some(POCKET_WATCH_GOLD),
    border: Color::rgba(0.118, 0.165, 0.290, 0.14),
    light: true,
    font: Some("Crimson Pro"),
    material: Material::Solid,
    shape: Shape::Notch,
    swaps: &[
        Swap { from: POCKET_WATCH_GOLD, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::ORANGE, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::YELLOW, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::GREEN, fill: Color::hex(0x2E7D68), text: Color::hex(0x1C5948) },
        Swap { from: palette::RED, fill: Color::hex(0xC23A4B), text: Color::hex(0x9E2030) },
        Swap { from: palette::BLUE, fill: Color::hex(0x2A65CC), text: Color::hex(0x1A4499) },
        Swap { from: palette::CYAN, fill: Color::hex(0x0F7F9E), text: Color::hex(0x0B5C73) },
        Swap { from: palette::PURPLE, fill: Color::hex(0x8C5AA8), text: Color::hex(0x6A3785) },
        Swap { from: palette::PINK, fill: Color::hex(0xD04868), text: Color::hex(0xA82444) },
    ],
};

/// Classic is the identity look used by Canvas::unthemed() and raw drawing.
/// With swaps empty and standard black/white ground/ink, map() returns any colour unchanged.
pub const CLASSIC: Look = Look {
    ground: Color::black(1.0),
    ink: Color::white(1.0),
    accent: None,
    border: palette::BORDER,
    light: false,
    font: None,
    material: Material::Solid,
    shape: Shape::Notch,
    swaps: &[],
};

pub const OYEN: Look = WONDERLAND;
pub const OYEN_NIGHT: Look = WONDERLAND;
pub const PILL: Look = WONDERLAND;

/// Glass over a dark backdrop (and in Windows dark mode for Settings):
pub const GLASS_DARK: Look = Look {
    ground: Color::hex(0x1C1C1E),
    ink: Color::white(1.0),
    accent: Some(POCKET_WATCH_GOLD),
    border: Color::white(0.10),
    light: false,
    font: Some("Crimson Pro"),
    material: Material::Glass,
    shape: Shape::Notch,
    swaps: &[],
};

/// Glass over a light backdrop (and in Windows light mode for Settings):
pub const GLASS_LIGHT: Look = Look {
    ground: Color::hex(0xF2F2F7),
    ink: MIDNIGHT_INK,
    accent: Some(POCKET_WATCH_GOLD),
    border: Color::black(0.08),
    light: true,
    font: Some("Crimson Pro"),
    material: Material::Glass,
    shape: Shape::Notch,
    swaps: &[
        Swap { from: POCKET_WATCH_GOLD, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::ORANGE, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::YELLOW, fill: POCKET_WATCH_GOLD, text: POCKET_WATCH_TEXT },
        Swap { from: palette::GREEN, fill: Color::hex(0x2E7D68), text: Color::hex(0x1C5948) },
        Swap { from: palette::RED, fill: Color::hex(0xC23A4B), text: Color::hex(0x9E2030) },
        Swap { from: palette::BLUE, fill: Color::hex(0x2A65CC), text: Color::hex(0x1A4499) },
        Swap { from: palette::CYAN, fill: Color::hex(0x0F7F9E), text: Color::hex(0x0B5C73) },
        Swap { from: palette::PURPLE, fill: Color::hex(0x8C5AA8), text: Color::hex(0x6A3785) },
        Swap { from: palette::PINK, fill: Color::hex(0xD04868), text: Color::hex(0xA82444) },
    ],
};

/// The theme's look. In WonderIsland, Wonderland is the sole look.
pub fn of(_theme: Theme) -> &'static Look {
    &WONDERLAND
}

/// The theme's look on a screen where the island wraps a camera notch.
pub fn around_notch(_theme: Theme) -> &'static Look {
    &WONDERLAND
}

/// The Glass look used by the Settings window and glass backdrops.
pub fn glass(light: bool) -> &'static Look {
    if light {
        &GLASS_LIGHT
    } else {
        &GLASS_DARK
    }
}

impl Look {
    /// The colour to paint for a shape the page asked to fill or stroke in `c`.
    pub fn paint(&self, c: Color) -> Color {
        self.map(c, false)
    }

    /// The colour to paint for text (and icons) the page asked for in `c`.
    pub fn paint_text(&self, c: Color) -> Color {
        self.map(c, true)
    }

    fn map(&self, c: Color, text: bool) -> Color {
        if self.swaps.is_empty() && self.ground == CLASSIC.ground && self.ink == CLASSIC.ink {
            return c;
        }
        if c.r == c.g && c.g == c.b {
            let base = match c.r {
                v if v <= 0.0 => self.ground,
                v if v >= 1.0 => self.ink,
                v => Color::lerp(self.ground, self.ink, v),
            };
            return base.with_alpha(c.a);
        }
        match self.swaps.iter().find(|s| same_rgb(s.from, c)) {
            Some(s) => (if text { s.text } else { s.fill }).with_alpha(c.a),
            None => c,
        }
    }
}

fn same_rgb(a: Color, b: Color) -> bool {
    (a.r - b.r).abs() < 1e-4 && (a.g - b.g).abs() < 1e-4 && (a.b - b.b).abs() < 1e-4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wonderland_turns_greys_into_ink_on_alice_blue() {
        assert_eq!(WONDERLAND.ground, ALICE_BLUE);
        assert_eq!(WONDERLAND.ink, MIDNIGHT_INK);
        assert_eq!(WONDERLAND.paint(Color::black(1.0)), WONDERLAND.ground);
        assert_eq!(WONDERLAND.paint(Color::white(1.0)), WONDERLAND.ink);
        assert_eq!(WONDERLAND.paint(palette::TEXT_DIM), WONDERLAND.ink.with_alpha(0.62));
        let g = WONDERLAND.paint(Color::rgba(0.16, 0.16, 0.16, 1.0));
        assert_eq!(g, Color::lerp(WONDERLAND.ground, WONDERLAND.ink, 0.16));
        assert!(WONDERLAND.paint(Color::black(1.0 / 255.0)).a < 0.01);
    }

    #[test]
    fn wonderland_swaps_accents_and_deepens_them_for_text() {
        let fill = WONDERLAND.paint(palette::ORANGE);
        let text = WONDERLAND.paint_text(palette::ORANGE);
        assert_eq!(fill, POCKET_WATCH_GOLD);
        assert_eq!(text, POCKET_WATCH_TEXT);
        assert!(text.luminance() < fill.luminance());
        assert_eq!(WONDERLAND.paint(palette::GREEN.mul_alpha(0.5)).a, 0.5);
        let art = Color::hex(0x7A3EE0);
        assert_eq!(WONDERLAND.paint(art), art);
        assert_eq!(WONDERLAND.paint_text(palette::ON_ACCENT), palette::ON_ACCENT);
    }

    #[test]
    fn wonderland_is_sole_look() {
        assert_eq!(*of(Theme::Wonderland), WONDERLAND);
        assert_eq!(*around_notch(Theme::Wonderland), WONDERLAND);
        assert_eq!(glass(true).light, true);
        assert_eq!(glass(false).light, false);
        assert_eq!(WONDERLAND.shape, Shape::Notch);
        assert_eq!(WONDERLAND.material, Material::Solid);
        assert!(WONDERLAND.light);
    }
}
