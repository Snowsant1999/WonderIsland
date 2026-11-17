//! Rabbit Hole Launcher page (PRD B1): quick-launch shortcuts for games, apps, and URLs.

use crate::commands::Command;
use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::state::AppState;
use crate::ui::{wid, wid_n, Ui};
use poni_core::geom::{palette, Color, Point, Rect};
use poni_core::launcher::MAX_ITEMS;

const HEIGHT: f32 = 240.0;
const INPUT_H: f32 = 34.0;
const GRID_TOP: f32 = 42.0;
const COLS: usize = 3;
const TILE_H: f32 = 44.0;
const GAP: f32 = 8.0;
const MAX_INPUT_LEN: usize = 240;

pub fn height(_st: &AppState) -> f32 {
    HEIGHT
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let accent = st.accent;
    let full = st.store.launcher.items.len() >= MAX_ITEMS;
    let input_r = Rect::new(area.x, area.y, area.w, INPUT_H);
    let placeholder = if full {
        "Launcher is full (12 shortcuts max)"
    } else {
        "Add shortcut: Name | steam://... or path or https://..."
    };
    let resp = ui.text_input_row(wid("launcher.input"), input_r, &mut st.ui.launcher_input, placeholder, MAX_INPUT_LEN, accent, Some(Icon::Plus));
    if resp.submitted && st.store.launcher.add_raw(&st.ui.launcher_input) {
        st.ui.launcher_input.clear();
        ui.input.caret = 0;
        st.cmd(Command::SaveStore);
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
    let mut launch: Option<String> = None;
    let mut remove: Option<usize> = None;

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

        let icon = if target.to_ascii_lowercase().starts_with("http") {
            Icon::Open
        } else if target.contains("://") {
            Icon::App
        } else {
            Icon::Folder
        };
        ui.c.icon(icon, Rect::from_center(Point::new(r.x + 16.0, r.center().y), 15.0, 15.0), accent);

        let del_r = Rect::new(r.right() - 22.0, r.y + 4.0, 18.0, 18.0);
        let show_del = tile_resp.hovered || ui.hovered(del_r);
        let text_w = (r.w - 34.0 - if show_del { 20.0 } else { 8.0 }).max(0.0);
        ui.c.text(label, Rect::new(r.x + 30.0, r.y + 6.0, text_w, 16.0), &TextStyle::body().weight(Weight::Medium));
        ui.c.text(target, Rect::new(r.x + 30.0, r.y + 22.0, text_w, 14.0), &TextStyle::footnote().color(palette::TEXT_DIM));

        if show_del {
            let dr = ui.icon_button(wid_n("launcher.del", idx as u64), del_r, Icon::Close, palette::TEXT_FAINT, 10.0);
            if dr.clicked {
                remove = Some(idx);
            }
        }
        if tile_resp.clicked && remove.is_none() {
            launch = Some(target.clone());
        }
    }

    if let Some(idx) = remove {
        if st.store.launcher.remove(idx) {
            st.cmd(Command::SaveStore);
        }
    } else if let Some(target) = launch {
        st.cmd(Command::OpenLauncher(target));
    }
}
