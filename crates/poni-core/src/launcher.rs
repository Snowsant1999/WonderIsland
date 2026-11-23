//! Rabbit Hole Launcher (PRD B1): quick-launch shortcuts for games, apps, and URLs.

use serde::{Deserialize, Serialize};

pub const MAX_ITEMS: usize = 12;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherItem {
    pub label: String,
    pub target: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Launcher {
    pub items: Vec<LauncherItem>,
}

/// Why a shortcut wasn't added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddError {
    Empty,
    Full,
    Duplicate,
}

impl AddError {
    /// What the page tells the person.
    pub fn say(self) -> &'static str {
        match self {
            AddError::Empty => "Type a name and a target, like Steam | steam://open/main",
            AddError::Full => "The launcher is full (12 shortcuts)",
            AddError::Duplicate => "That shortcut is already here",
        }
    }
}

impl Launcher {
    /// Parse `"Label | target"` or `"target"` and add an item if valid and under [`MAX_ITEMS`].
    pub fn add(&mut self, raw: &str) -> Result<(), AddError> {
        if self.items.len() >= MAX_ITEMS {
            return Err(AddError::Full);
        }
        let item = parse_entry(raw).ok_or(AddError::Empty)?;
        if self.items.iter().any(|x| x.target.eq_ignore_ascii_case(&item.target)) {
            return Err(AddError::Duplicate);
        }
        self.items.push(item);
        Ok(())
    }

    pub fn add_raw(&mut self, raw: &str) -> bool {
        self.add(raw).is_ok()
    }

    /// Move shortcut `index` one place earlier (`-1`) or later (`1`).
    pub fn shift(&mut self, index: usize, delta: i32) -> bool {
        let to = index as i32 + delta;
        if index >= self.items.len() || to < 0 || to as usize >= self.items.len() {
            return false;
        }
        self.items.swap(index, to as usize);
        true
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index < self.items.len() {
            self.items.remove(index);
            true
        } else {
            false
        }
    }
}

/// Parse `"Label | target"` or a bare `"target"` into a [`LauncherItem`].
pub fn parse_entry(raw: &str) -> Option<LauncherItem> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    let (label, target) = if let Some((l, t)) = trimmed.split_once('|') {
        (l.trim().to_string(), t.trim().to_string())
    } else {
        (derive_label(trimmed), trimmed.to_string())
    };
    if target.is_empty() {
        return None;
    }
    let label = if label.is_empty() { derive_label(&target) } else { label };
    Some(LauncherItem { label, target })
}

fn derive_label(target: &str) -> String {
    if let Some((_, rest)) = target.split_once("://") {
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        if !host.is_empty() {
            return host.to_string();
        }
    }
    let clean = target.trim_end_matches(['\\', '/']);
    clean
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(target)
        .strip_suffix(".exe")
        .or_else(|| clean.rsplit(['\\', '/']).next().and_then(|s| s.strip_suffix(".lnk")))
        .unwrap_or_else(|| clean.rsplit(['\\', '/']).next().unwrap_or(target))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_entry_supports_label_pipe_and_bare_targets() {
        let a = parse_entry("CS2 | steam://rungameid/730").unwrap();
        assert_eq!(a.label, "CS2");
        assert_eq!(a.target, "steam://rungameid/730");

        let b = parse_entry(r"C:\Games\Hades\Hades.exe").unwrap();
        assert_eq!(b.label, "Hades");

        let c = parse_entry("https://github.com/trending").unwrap();
        assert_eq!(c.label, "github.com");
    }

    #[test]
    fn errors_say_why_and_shortcuts_can_move() {
        let mut l = Launcher::default();
        assert_eq!(l.add("   "), Err(AddError::Empty));
        assert_eq!(l.add("A | steam://open/a"), Ok(()));
        assert_eq!(l.add("B | steam://open/B"), Ok(()));
        assert_eq!(l.add("again | STEAM://OPEN/A"), Err(AddError::Duplicate));
        assert!(l.shift(0, 1));
        assert_eq!(l.items[0].label, "B");
        assert!(!l.shift(0, -1), "nothing before the first");
        assert!(!l.shift(1, 1), "nothing after the last");
        for i in 0..MAX_ITEMS {
            let _ = l.add(&format!("n{i} | steam://open/{i}"));
        }
        assert_eq!(l.add("x | steam://open/x"), Err(AddError::Full));
    }

    #[test]
    fn add_and_remove_respect_limits_and_dedupe() {
        let mut l = Launcher::default();
        assert!(l.add_raw("Steam | steam://open/main"));
        assert!(!l.add_raw("Steam Again | steam://open/main"));
        assert_eq!(l.items.len(), 1);
        assert!(l.remove(0));
        assert!(l.items.is_empty());
    }
}
