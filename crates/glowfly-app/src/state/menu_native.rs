//! The plan of the macOS system menu bar, built from the menu model (`menu_model.rs`) and kept
//! free of AppKit so every OS tests it: the app menu (About, Preferences, Services, Hide, Hide
//! Others, Show All, Quit), then File … Help with the app menu's items taken out, each item's
//! key equivalent, and the tags clicks come back with (`platform/macos_menu.rs` builds the
//! menus, `platform/native_menu.rs` queues the clicks).

use super::menu_model::{self, Entry, Menu, MenuAction, Role};
use crate::keymap::{Chord, KeyName};
use glowfly_core::i18n::Lang;

/// A key equivalent as AppKit takes it: the key's character and the modifier mask.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyEquivalent {
    pub key: String,
    pub command: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

/// An app menu item AppKit answers itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Standard {
    Hide,
    HideOthers,
    ShowAll,
}

/// A row of a planned menu.
#[derive(Clone, Debug, PartialEq)]
pub enum NEntry {
    /// One of Glowfly's items: `tag` names it in the model (`tag`).
    Item {
        tag: i64,
        title: String,
        key: Option<KeyEquivalent>,
        enabled: bool,
        on: bool,
        tip: Option<String>,
    },
    Standard {
        title: String,
        key: Option<KeyEquivalent>,
        action: Standard,
    },
    /// The Services submenu, which AppKit fills.
    Services(String),
    Separator,
    /// A disabled title over a group.
    Heading(String),
    Submenu {
        title: String,
        enabled: bool,
        tip: Option<String>,
        entries: Vec<NEntry>,
    },
}

/// A top-level menu after the app menu.
#[derive(Clone, Debug, PartialEq)]
pub struct NTop {
    pub title: String,
    pub entries: Vec<NEntry>,
}

/// The whole bar: the app menu, then File … Help.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The tags' generation: bumped whenever the menus are built anew.
    pub generation: u32,
    pub app: Vec<NEntry>,
    pub menus: Vec<NTop>,
}

/// `c` as a key equivalent; None for a key AppKit can't show (a digit range).
pub fn key_equivalent(c: Chord) -> Option<KeyEquivalent> {
    let fkey = |n: u32| char::from_u32(n).map(String::from);
    let key = match c.key {
        KeyName::Letter(l) => l.to_ascii_lowercase().to_string(),
        KeyName::Digit(d) => char::from_digit(u32::from(d), 10)?.to_string(),
        KeyName::DigitRange(..) => return None,
        KeyName::F(n) if (1..=12).contains(&n) => fkey(0xF704 + u32::from(n) - 1)?,
        KeyName::F(_) => return None,
        KeyName::Enter => "\r".into(),
        KeyName::Escape => "\u{1b}".into(),
        KeyName::Tab => "\t".into(),
        KeyName::Backspace => "\u{8}".into(),
        KeyName::Space => " ".into(),
        KeyName::Up => fkey(0xF700)?,
        KeyName::Down => fkey(0xF701)?,
        KeyName::Left => fkey(0xF702)?,
        KeyName::Right => fkey(0xF703)?,
        KeyName::Home => fkey(0xF729)?,
        KeyName::End => fkey(0xF72B)?,
        KeyName::PageUp => fkey(0xF72C)?,
        KeyName::PageDown => fkey(0xF72D)?,
        KeyName::Insert => fkey(0xF727)?,
        KeyName::Delete => fkey(0xF728)?,
        KeyName::Comma => ",".into(),
        KeyName::Period => ".".into(),
        KeyName::Slash => "/".into(),
        KeyName::Backslash => "\\".into(),
        KeyName::Semicolon => ";".into(),
        KeyName::Quote => "'".into(),
        KeyName::Backtick => "`".into(),
        KeyName::Minus => "-".into(),
        KeyName::Equals => "=".into(),
        KeyName::Plus => "+".into(),
        KeyName::OpenBracket => "[".into(),
        KeyName::CloseBracket => "]".into(),
    };
    let m = c.mods;
    Some(KeyEquivalent { key, command: m.command, shift: m.shift, alt: m.alt, ctrl: m.ctrl })
}

/// The tag of item `index` in generation `generation`.
pub fn tag(generation: u32, index: usize) -> i64 {
    (i64::from(generation) << 32) | (index as i64 & 0xffff_ffff)
}

/// A tag's generation and item index.
pub fn untag(tag: i64) -> (u32, usize) {
    ((tag >> 32) as u32, (tag & 0xffff_ffff) as usize)
}

/// The bar for `menus` in language `t`, its tags of `generation`.
pub fn plan(menus: &[Menu; 5], t: Lang, generation: u32) -> Plan {
    let mut next = 0;
    let mut app = AppItems::default();
    let tops = menus
        .iter()
        .map(|m| NTop { title: m.title.to_owned(), entries: tidy(planned(&m.entries, generation, &mut next, &mut app)) })
        .collect();
    let hide = |command, alt| Some(KeyEquivalent { key: "h".into(), command, shift: false, alt, ctrl: false });
    let mut out = Vec::new();
    let titled = |e: Option<NEntry>, title: &str| {
        e.map(|e| match e {
            NEntry::Item { tag, key, enabled, on, tip, .. } => NEntry::Item { tag, title: title.to_owned(), key, enabled, on, tip },
            other => other,
        })
    };
    out.extend(titled(app.about, t.tr("menu.app.about")));
    out.push(NEntry::Separator);
    out.extend(titled(app.prefs, t.tr("menu.file.prefs")));
    out.push(NEntry::Separator);
    out.push(NEntry::Services(t.tr("menu.app.services").to_owned()));
    out.push(NEntry::Separator);
    out.push(NEntry::Standard { title: t.tr("menu.app.hide").to_owned(), key: hide(true, false), action: Standard::Hide });
    out.push(NEntry::Standard { title: t.tr("menu.app.hide_others").to_owned(), key: hide(true, true), action: Standard::HideOthers });
    out.push(NEntry::Standard { title: t.tr("menu.app.show_all").to_owned(), key: None, action: Standard::ShowAll });
    out.push(NEntry::Separator);
    out.extend(titled(app.quit, t.tr("menu.app.quit")));
    Plan { generation, app: tidy(out), menus: tops }
}

/// The items the app menu takes from the others.
#[derive(Default)]
struct AppItems {
    about: Option<NEntry>,
    prefs: Option<NEntry>,
    quit: Option<NEntry>,
}

/// `entries` planned, numbering their items from `next` on (the order of `menu_model::items`).
fn planned(entries: &[Entry], generation: u32, next: &mut usize, app: &mut AppItems) -> Vec<NEntry> {
    let mut out = Vec::new();
    for e in entries {
        match e {
            Entry::Item(it) => {
                let row = NEntry::Item {
                    tag: tag(generation, *next),
                    title: it.label.clone(),
                    key: it.chord.and_then(key_equivalent),
                    enabled: it.blocked.is_none(),
                    on: it.mark.on(),
                    tip: it.blocked.clone(),
                };
                *next += 1;
                match it.role {
                    Role::Plain => out.push(row),
                    Role::About => app.about = Some(row),
                    Role::Prefs => app.prefs = Some(row),
                    Role::Quit => app.quit = Some(row),
                }
            }
            // Still numbered, so the later tags match the model.
            Entry::Taken => *next += 1,
            Entry::Separator => out.push(NEntry::Separator),
            Entry::Heading(h) => out.push(NEntry::Heading(h.clone())),
            Entry::Submenu(s) => {
                let inner = tidy(planned(&s.entries, generation, next, app));
                let inner = if s.blocked.is_some() { Vec::new() } else { inner };
                out.push(NEntry::Submenu { title: s.label.clone(), enabled: s.blocked.is_none(), tip: s.blocked.clone(), entries: inner });
            }
        }
    }
    out
}

/// `entries` without separators at either end or next to another.
pub fn tidy(entries: Vec<NEntry>) -> Vec<NEntry> {
    let mut out: Vec<NEntry> = Vec::with_capacity(entries.len());
    for e in entries {
        if e == NEntry::Separator && out.last().is_none_or(|l| *l == NEntry::Separator) {
            continue;
        }
        out.push(e);
    }
    if out.last() == Some(&NEntry::Separator) {
        out.pop();
    }
    out
}

/// Whether `a` and `b` have the same rows, kinds and nesting, whatever their titles, states,
/// keys and generation: then the same index means the same item, and the menus can be updated
/// in place.
pub fn same_shape(a: &Plan, b: &Plan) -> bool {
    fn rows(a: &[NEntry], b: &[NEntry]) -> bool {
        a.len() == b.len()
            && a.iter().zip(b).all(|(x, y)| match (x, y) {
                (NEntry::Submenu { entries: x, .. }, NEntry::Submenu { entries: y, .. }) => rows(x, y),
                (NEntry::Standard { action: x, .. }, NEntry::Standard { action: y, .. }) => x == y,
                _ => std::mem::discriminant(x) == std::mem::discriminant(y),
            })
    }
    rows(&a.app, &b.app) && a.menus.len() == b.menus.len() && a.menus.iter().zip(&b.menus).all(|(x, y)| rows(&x.entries, &y.entries))
}

/// The app menu item AppKit handles for a key press: Cmd+H hides Glowfly, Option+Cmd+H the
/// other apps. Every other key goes to the window.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn standard_key(chars: &str, command: bool, shift: bool, alt: bool, ctrl: bool) -> Option<Standard> {
    if !chars.eq_ignore_ascii_case("h") || !command || shift || ctrl {
        return None;
    }
    Some(if alt { Standard::HideOthers } else { Standard::Hide })
}

/// The actions of the picked `tags` that belong to `generation`, each item once; stale, unknown
/// and disabled items' tags are dropped.
pub fn take_picked(menus: &mut [Menu; 5], generation: u32, tags: &[i64]) -> Vec<MenuAction> {
    tags.iter()
        .filter_map(|&t| {
            let (g, index) = untag(t);
            if g != generation {
                return None;
            }
            let it = menu_model::take_item(menus, index)?;
            it.blocked.is_none().then_some(it.action)
        })
        .collect()
}

#[cfg(test)]
#[path = "menu_native_tests.rs"]
mod tests;
