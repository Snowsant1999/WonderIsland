//! Rabbit Hole Launcher page (PRD B1): quick-launch shortcuts for games, apps, and URLs.
//!
//! Type `Name | target` (or just the target) and press Enter. A tile opens
//! its target on click. On hover a tile shows buttons to move it earlier or
//! later and to remove it; a right-click puts it back in the input to edit.

use crate::commands::Command;
use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, Ui};
use poni_core::geom::{palette, Color, Point, Rect};
use poni_core::launcher::{parse_entry, MAX_ITEMS};

const HEIGHT: f32 = 240.0;
const INPUT_H: f32 = 34.0;
const GRID_TOP: f32 = 50.0;
const COLS: usize = 3;
const TILE_H: f32 = 40.0;
const GAP: f32 = 6.0;
const MAX_INPUT_LEN: usize = 240;
/// How long a "why it wasn't added" line stays under the input (seconds).
const NOTE_S: f64 = 3.5;
const REFUSED: &str = "Not allowed: use a path, an http(s) link or a launcher link like steam://";

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

/// The typed shortcut: refused with a reason, or added and saved.
fn submit(st: &mut AppState, now: f64) -> bool {
    let note = match parse_entry(&st.ui.launcher_input) {
        None => Some(poni_core::launcher::AddError::Empty.say()),
        Some(item) if !crate::shell::launcher_target_ok(&item.target) => Some(REFUSED),
        Some(_) => match st.store.launcher.add(&st.ui.launcher_input) {
            Ok(()) => None,
            Err(e) => Some(e.say()),
        },
    };
    match note {
        Some(text) => {
            st.ui.launcher_note = Some((text.to_string(), now + NOTE_S));
            false
        }
        None => {
            st.ui.launcher_note = None;
            st.cmd(Command::SaveStore);
            true
        }
    }
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let accent = st.accent;
    let now = ui.now;
    let full = st.store.launcher.items.len() >= MAX_ITEMS;
    let input_r = Rect::new(area.x, area.y, area.w, INPUT_H);
    let placeholder = if full {
        "Launcher is full (12 shortcuts max)"
    } else {
        "Add shortcut: Name | steam://... or path or https://..."
    };
    let resp = ui.text_input_row(wid("launcher.input"), input_r, &mut st.ui.launcher_input, placeholder, MAX_INPUT_LEN, accent, Some(Icon::Plus));
    if resp.submitted && submit(st, now) {
        st.ui.launcher_input.clear();
        ui.input.caret = 0;
    }

    // Why the last shortcut wasn't added, for a few seconds.
    if st.ui.launcher_note.as_ref().is_some_and(|(_, until)| now >= *until) {
        st.ui.launcher_note = None;
    }
    if let Some((text, _)) = &st.ui.launcher_note {
        ui.request_anim();
        ui.c.text(text, Rect::new(area.x + 4.0, area.y + INPUT_H + 2.0, area.w - 8.0, 14.0), &TextStyle::footnote().color(palette::ORANGE));
    }

    let grid_r = Rect::new(area.x, area.y + GRID_TOP, area.w, (HEIGHT - GRID_TOP).max(0.0));
    if st.store.launcher.items.is_empty() {
        super::empty_state(
            ui,
            grid_r,
            Icon::App,
            "Down the Rabbit Hole",
            "Add Steam URIs, apps, folders, or links above",
        );
        return;
    }

    let card = super::card::pill_card(st, area);
    let tile_w = ((grid_r.w - GAP * (COLS as f32 - 1.0)) / COLS as f32).max(40.0);
    let items: Vec<(String, String)> = st.store.launcher.items.iter().map(|i| (i.label.clone(), i.target.clone())).collect();
    let count = items.len();
    let mut launch: Option<String> = None;
    let mut remove: Option<usize> = None;
    let mut shift: Option<(usize, i32)> = None;
    let mut edit: Option<usize> = None;

    for (idx, (label, target)) in items.iter().enumerate().take(MAX_ITEMS) {
        let col = idx % COLS;
        let row = idx / COLS;
        let r = Rect::new(
            grid_r.x + col as f32 * (tile_w + GAP),
            grid_r.y + row as f32 * (TILE_H + GAP),
            tile_w,
            TILE_H,
        );
        let tile_resp = ui.button(wid_n("launcher.tile", idx as u64), r);
        let bg = if tile_resp.pressed {
            Color::white(0.14)
        } else if tile_resp.hovered {
            Color::white(0.10)
        } else {
            Color::white(0.05)
        };
        super::card::fill_box(ui, card, r, 10.0, bg);

        let lower = target.to_ascii_lowercase();
        let icon = if lower.starts_with("http") {
            Icon::Open
        } else if lower.contains("://") {
            Icon::App
        } else {
            Icon::Folder
        };
        ui.c.icon(icon, Rect::from_center(Point::new(r.x + 16.0, r.center().y), 15.0, 15.0), accent);

        // The row of small buttons: earlier, later, remove.
        let btn = 16.0;
        let rail = Rect::new(r.right() - 3.0 * btn - 8.0, r.y + 3.0, 3.0 * btn + 4.0, btn);
        let show_tools = tile_resp.hovered || ui.hovered(rail);
        let text_w = (r.w - 34.0 - if show_tools { rail.w + 2.0 } else { 8.0 }).max(0.0);
        ui.c.text(label, Rect::new(r.x + 30.0, r.y + 5.0, text_w, 16.0), &TextStyle::body().weight(Weight::Medium));
        ui.c.text(target, Rect::new(r.x + 30.0, r.y + 21.0, (r.w - 38.0).max(0.0), 14.0), &TextStyle::footnote().color(palette::TEXT_DIM));

        let mut tool_used = false;
        if show_tools {
            let tools = [
                ("launcher.prev", Icon::Prev, idx > 0),
                ("launcher.next", Icon::Next, idx + 1 < count),
                ("launcher.del", Icon::Close, true),
            ];
            for (n, (id, icon, enabled)) in tools.into_iter().enumerate() {
                let br = Rect::new(rail.x + n as f32 * (btn + 2.0), rail.y, btn, btn);
                let col = if enabled { palette::TEXT_DIM } else { palette::TEXT_FAINT };
                let b = ui.icon_button(wid_n(id, idx as u64), br, icon, col, 10.0);
                if b.clicked && enabled {
                    tool_used = true;
                    match n {
                        0 => shift = Some((idx, -1)),
                        1 => shift = Some((idx, 1)),
                        _ => remove = Some(idx),
                    }
                }
            }
        }
        if tile_resp.rclicked {
            edit = Some(idx);
        } else if tile_resp.clicked && !tool_used && !ui.hovered(rail) {
            launch = Some(target.clone());
        }
    }

    if let Some((idx, delta)) = shift {
        if st.store.launcher.shift(idx, delta) {
            st.cmd(Command::SaveStore);
        }
    } else if let Some(idx) = remove {
        if st.store.launcher.remove(idx) {
            st.cmd(Command::SaveStore);
        }
    } else if let Some(idx) = edit {
        // Back into the input as `Name | target`; Enter adds it again.
        if let Some(item) = st.store.launcher.items.get(idx).cloned() {
            st.store.launcher.remove(idx);
            st.ui.launcher_input = format!("{} | {}", item.label, item.target);
            ui.input.caret = st.ui.launcher_input.chars().count();
            st.cmd(Command::SaveStore);
        }
    } else if let Some(target) = launch {
        st.cmd(Command::OpenLauncher(target));
    }
}
