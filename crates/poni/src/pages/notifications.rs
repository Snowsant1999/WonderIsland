//! Notifications page, the bell tab while the Notifications add-on is on
//! (Windows): other apps' notifications on the island
//! (`poni_core::notifications`, read by `services::notifications`).
//!
//! First a bell and what Windows allows, with Allow while Windows hasn't
//! been asked and Windows settings once it said no there. Under a line, two
//! columns: the apps that have sent a notification since the add-on came on,
//! each with a switch (WhatsApp, Telegram and Phone Link start on), and what
//! the island shows of a message: who it's from, and the message itself
//! (either off, it waits for Show). A change saves at once
//! (`Command::SaveSettings`). The page keeps no notifications: they come and
//! go on the closed island.

use crate::commands::{Command, NotifyCmd};
use crate::gfx::{TextStyle, Weight};
use crate::icons::Icon;
use crate::pages::settings::switch_row;
use crate::state::{AppState, InlineAction, Notice, NoticeAction, NotifyAccess};
use poni_core::notifications::{self, NotificationSettings, Seen};
use crate::ui::{wid, wid_n, PillStyle, Ui, PILL_GAP, PILL_SMALL};
use poni_core::geom::{palette, Color, Point, Rect};

const HEAD_H: f32 = 44.0;
/// The bell's disc, and the text column right of it.
const DISC: f32 = 36.0;
const TEXT_X: f32 = 48.0;
const RULE_Y: f32 = 54.0;
const LABELS_Y: f32 = 62.0;
const ROWS_Y: f32 = 80.0;
const ROW_H: f32 = 28.0;
const COL_GAP: f32 = 24.0;
/// The apps listed at most: with the rows above, the page stays shorter than
/// the System page (256), so the window keeps its size.
pub const MAX_ROWS: usize = 6;
/// Rows the right column takes (who it's from, the message), and the note
/// that stands in for an empty list.
const MIN_ROWS: usize = 2;

/// The island's notice for one notification (None: it has no words): who
/// it's from and the message, or Show for what the person keeps hidden, as
/// in a Money in notice; Open, or a click on it, opens the app.
/// Wonderland card suit mapping for notifications (PRD A3):
/// - Heart (♥): Personal messaging / chat (WhatsApp, Telegram, Phone Link, Signal, Discord, etc.)
/// - Spade (♠): System, security, updates, hardware
/// - Diamond (♦): Financial, banking, money, transactions
/// - Club (♣): General apps, web browsers, work emails
pub fn suit_for_notification(app_id: &str, app_name: &str) -> Icon {
    let id = app_id.to_lowercase();
    let name = app_name.to_lowercase();

    // Financial / Money -> Diamond
    if id.contains("bank") || id.contains("pay") || id.contains("wallet") || id.contains("finance")
        || name.contains("bank") || name.contains("bca") || name.contains("mandiri") || name.contains("dana")
        || name.contains("gopay") || name.contains("ovo") || name.contains("paypal")
    {
        return Icon::Diamond;
    }

    // System / Security / Update -> Spade
    if id.contains("security") || id.contains("system") || id.contains("update") || id.contains("defender")
        || name.contains("security") || name.contains("system") || name.contains("update") || name.contains("windows")
    {
        return Icon::Spade;
    }

    // Chat / Messages / Social -> Heart
    if id.contains("whatsapp") || id.contains("telegram") || id.contains("phonelink") || id.contains("phone")
        || id.contains("signal") || id.contains("discord") || id.contains("slack") || id.contains("messenger")
        || id.contains("teams") || name.contains("whatsapp") || name.contains("telegram") || name.contains("phone")
        || name.contains("chat") || name.contains("message")
    {
        return Icon::Heart;
    }

    // Default general / web / mail -> Club
    Icon::Club
}

pub fn message_notice(seen: &Seen, choices: &NotificationSettings, accent: Color, now: f64) -> Option<Notice> {
    let said = notifications::compose(seen, choices.who, choices.text)?;
    let icon = suit_for_notification(&seen.app_id, &seen.app);
    Some(Notice {
        icon,
        color: notifications::color(&seen.app_id, &seen.app).unwrap_or(accent),
        title: said.title,
        title_dim: said.title_dim,
        subtitle: said.subtitle,
        until: now + notifications::NOTICE_S,
        action: Some(NoticeAction::OpenApp(seen.app_id.clone())),
        inline: Some(if said.reveal.is_some() { InlineAction::ShowAmount } else { InlineAction::Open }),
        reveal: said.reveal,
    })
}

/// Height of history section: label + up to 5 rows of HIST_ROW_H each.
pub const HIST_LABEL_H: f32 = 28.0;
pub const HIST_ROW_H: f32 = 34.0;

pub fn history_h(st: &AppState) -> f32 {
    let n = st.settings.notifications.history.len();
    if n == 0 { 0.0 } else { HIST_LABEL_H + n as f32 * HIST_ROW_H }
}

pub fn height(st: &AppState) -> f32 {
    ROWS_Y + st.settings.notifications.apps.len().clamp(MIN_ROWS, MAX_ROWS) as f32 * ROW_H
        + history_h(st)
}

/// A button beside the line: its label, what it asks for, and whether it
/// is the one to press (the accent's colour).
type Button = (&'static str, NotifyCmd, bool);

const SETTINGS: Button = ("Windows settings", NotifyCmd::OpenWindowsSettings, false);

/// What the line under the title says, and the buttons beside it (right to left).
fn access_look(access: NotifyAccess) -> (&'static str, Vec<Button>) {
    match access {
        NotifyAccess::Unknown => ("Checking with Windows\u{2026}", vec![]),
        NotifyAccess::NoPackage => ("Windows lets only the Microsoft Store copy read notifications.", vec![]),
        NotifyAccess::Ask => ("Windows asks you once before the island can read notifications.", vec![("Allow", NotifyCmd::Ask, true)]),
        NotifyAccess::Asking => ("Answer Windows' question to go on.", vec![]),
        NotifyAccess::Unanswered => ("Windows got no answer. Ask again, or turn it on in Windows' settings.", vec![("Ask again", NotifyCmd::Ask, true), SETTINGS]),
        NotifyAccess::Denied => ("Turned off for WonderIsland in Windows' settings.", vec![SETTINGS]),
        NotifyAccess::Allowed => ("On. New ones from the apps you pick show on the island.", vec![]),
        NotifyAccess::Failed => ("Windows didn't let the island listen. Switch the add-on off and on to try again.", vec![]),
    }
}

pub fn draw(ui: &mut Ui, st: &mut AppState, area: Rect) {
    let accent = st.accent;
    let head = Rect::new(area.x, area.y, area.w, HEAD_H);
    let cy = head.center().y;
    let c = Point::new(head.x + DISC / 2.0, cy);
    ui.c.fill_circle(c, DISC / 2.0, accent.with_alpha(0.22));
    ui.c.icon(Icon::Bell, Rect::from_center(c, 20.0, 20.0), accent);
    let (line, buttons) = access_look(st.notify_access);
    let mut right = head.right();
    for (i, (label, cmd, primary)) in buttons.into_iter().enumerate() {
        let w = ui.pill_width(label, false, true);
        let b = Rect::new(right - w, cy - PILL_SMALL / 2.0, w, PILL_SMALL);
        let style = if primary { PillStyle::Primary(accent) } else { PillStyle::Utility };
        if ui.pill(wid_n("notify.access", i as u64), b, label, None, style).clicked {
            if cmd == NotifyCmd::Ask {
                st.notify_access = NotifyAccess::Asking;
            }
            st.cmd(Command::Notify(cmd));
        }
        right = b.x - PILL_GAP;
    }
    let right = if right < head.right() { right - 16.0 + PILL_GAP } else { right };
    let x = head.x + TEXT_X;
    let w = (right - x).max(0.0);
    ui.c.text("Notifications", Rect::new(x, cy - 19.0, w, 20.0), &TextStyle::headline());
    let line_color = if st.notify_access == NotifyAccess::Failed { palette::ORANGE } else { palette::TEXT_DIM };
    ui.c.text(line, Rect::new(x, cy + 1.0, w, 17.0), &TextStyle::footnote().color(line_color));

    let px = 1.0 / ui.c.scale().max(0.5);
    ui.c.fill_rect(Rect::new(area.x, area.y + RULE_Y, area.w, px), Color::white(0.10));
    let col_w = ((area.w - COL_GAP) / 2.0).max(0.0);
    let cols = [area.x, area.x + col_w + COL_GAP];
    let label_st = TextStyle::caption().weight(Weight::Semibold).color(palette::TEXT_FAINT);
    ui.c.text("APPS", Rect::new(cols[0], area.y + LABELS_Y, col_w, 14.0), &label_st);
    ui.c.text("THE ISLAND SHOWS", Rect::new(cols[1], area.y + LABELS_Y, col_w, 14.0), &label_st);

    // The apps, in the order they first sent one.
    let row = |i: usize, col: usize| Rect::new(cols[col], area.y + ROWS_Y + i as f32 * ROW_H, col_w, ROW_H);
    let apps: Vec<(String, String, bool)> =
        st.settings.notifications.apps.iter().take(MAX_ROWS).map(|a| (a.id.clone(), a.name.clone(), a.on)).collect();
    if apps.is_empty() {
        let note = "Apps show up here after their first notification. WhatsApp, Telegram and Phone Link start on.";
        let r = Rect::new(cols[0], area.y + ROWS_Y + 4.0, col_w, MIN_ROWS as f32 * ROW_H - 4.0);
        ui.c.text(note, r, &TextStyle::footnote().color(palette::TEXT_DIM).wrap().top());
    }
    for (i, (id, name, on)) in apps.iter().enumerate() {
        if switch_row(ui, wid_n("notify.app", i as u64), row(i, 0), name, Some(*on), accent) {
            st.settings.notifications.set(id, !on);
            st.cmd(Command::SaveSettings);
        }
    }

    // What a notice shows at once.
    let n = &st.settings.notifications;
    let (who, text) = (n.who, n.text);
    if switch_row(ui, wid("notify.who"), row(0, 1), "Who it's from", Some(who), accent) {
        st.settings.notifications.who = !who;
        st.cmd(Command::SaveSettings);
    }
    if switch_row(ui, wid("notify.text"), row(1, 1), "Message text", Some(text), accent) {
        st.settings.notifications.text = !text;
        st.cmd(Command::SaveSettings);
    }

    // ----- Recent history (last 5, session only) -----
    draw_history(ui, st, area);
}

/// Draw the recent-history section below the two columns.
/// Shows up to MAX_HISTORY entries (newest first); nothing when the list is empty.
fn draw_history(ui: &mut Ui, st: &AppState, area: Rect) {
    let history: Vec<(String, String, String, f64)> = st.settings.notifications.history
        .iter()
        .map(|e| (e.app.clone(), e.title.clone(), e.body.clone(), e.at))
        .collect();
    if history.is_empty() {
        return;
    }

    // Base Y: below the cols section
    let rows_used = st.settings.notifications.apps.len().clamp(MIN_ROWS, MAX_ROWS);
    let base_y = area.y + ROWS_Y + rows_used as f32 * ROW_H;

    // Divider
    let px = 1.0 / ui.c.scale().max(0.5);
    ui.c.fill_rect(Rect::new(area.x, base_y, area.w, px), Color::white(0.10));

    // Section label
    let label_st = TextStyle::caption().weight(Weight::Semibold).color(palette::TEXT_FAINT);
    ui.c.text("RECENT", Rect::new(area.x, base_y + 8.0, area.w, 14.0), &label_st);

    // Rows
    let now = st.now;
    let accent = st.accent;
    for (i, (app, title, body, at)) in history.iter().enumerate() {
        let ry = base_y + HIST_LABEL_H + i as f32 * HIST_ROW_H;
        let row_r = Rect::new(area.x, ry, area.w, HIST_ROW_H);

        // Suit icon (card suit for this app)
        let suit = suit_for_notification("", app);
        let icon_r = Rect::from_center(Point::new(area.x + 10.0, ry + HIST_ROW_H / 2.0), 14.0, 14.0);
        ui.c.icon(suit, icon_r, accent.with_alpha(0.70));

        // Text block
        let tx = area.x + 24.0;
        let tw = area.w - 24.0 - 38.0;
        let top_label = if title.is_empty() { app.as_str() } else { title.as_str() };
        ui.c.text(top_label, Rect::new(tx, ry + 4.0, tw, 15.0), &TextStyle::body().weight(Weight::Medium));
        if !body.is_empty() {
            ui.c.text(body, Rect::new(tx, ry + 18.0, tw, 13.0), &TextStyle::footnote().color(palette::TEXT_DIM));
        } else if !title.is_empty() {
            // show app name as subtitle when title is the sender
            ui.c.text(app, Rect::new(tx, ry + 18.0, tw, 13.0), &TextStyle::footnote().color(palette::TEXT_DIM));
        }

        // Relative time (e.g. "2m ago")
        let elapsed = (now - at).max(0.0) as u64;
        let ago = if elapsed < 60 { format!("{elapsed}s") }
                  else if elapsed < 3600 { format!("{} m", elapsed / 60) }
                  else { format!("{} h", elapsed / 3600) };
        let time_x = area.x + area.w - 36.0;
        ui.c.text(&ago, Rect::new(time_x, ry + 4.0, 36.0, 13.0), &TextStyle::footnote().color(palette::TEXT_FAINT));

        // Row separator (thin line between rows)
        if i + 1 < history.len() {
            let sep_y = ry + HIST_ROW_H - px;
            ui.c.fill_rect(Rect::new(area.x + 24.0, sep_y, area.w - 24.0, px), Color::white(0.06));
        }
        let _ = row_r;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_grows_with_its_apps_up_to_six_rows() {
        let mut st = AppState::new(Default::default(), Default::default());
        let empty = height(&st);
        assert_eq!(empty, ROWS_Y + MIN_ROWS as f32 * ROW_H, "room for the note and the two switches");
        for i in 0..10 {
            st.settings.notifications.remember(&format!("App.{i}"), &format!("App {i}"));
        }
        assert_eq!(height(&st), ROWS_Y + MAX_ROWS as f32 * ROW_H);
        assert!(height(&st) <= 256.0, "shorter than the System page");
    }

    #[test]
    fn allow_shows_only_while_windows_can_be_asked() {
        let asks = |a| access_look(a).1.into_iter().map(|b| b.1).collect::<Vec<_>>();
        assert_eq!(asks(NotifyAccess::Ask), [NotifyCmd::Ask]);
        assert_eq!(asks(NotifyAccess::Unanswered), [NotifyCmd::Ask, NotifyCmd::OpenWindowsSettings]);
        assert_eq!(asks(NotifyAccess::Denied), [NotifyCmd::OpenWindowsSettings]);
        for a in [NotifyAccess::Unknown, NotifyAccess::NoPackage, NotifyAccess::Asking, NotifyAccess::Allowed, NotifyAccess::Failed] {
            assert!(asks(a).is_empty(), "{a:?}");
        }
    }
}
