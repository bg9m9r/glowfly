//! The effective keymap: each binding's current keys, the defaults with the user's changes
//! (`Settings::keybindings`) applied, and the hint texts built from them.

use super::chord::{Chord, KeyName};
use super::ids::BindingId;
use super::rebind::{self, fold_ctrl, same_keys};
use super::table::{BINDINGS, Binding, GROUPS, Group, binding, runs_command};
use std::collections::BTreeMap;

/// The most keys one action can have (Preferences > Keyboard hides Add at this many).
pub const MAX_CHORDS: usize = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Keymap {
    /// The current keys, indexed by `BindingId as usize`.
    chords: Vec<Vec<Chord>>,
    /// The first key of each as text; None when the action has no key.
    hints: Vec<Option<String>>,
}

impl Default for Keymap {
    fn default() -> Self {
        let chords: Vec<Vec<Chord>> = BindingId::ALL.iter().map(|&id| binding(id).defaults.to_vec()).collect();
        let hints = chords.iter().map(|c| c.first().map(Chord::text)).collect();
        Self { chords, hints }
    }
}

impl Keymap {
    /// The defaults with `map` (a binding's settings-file id -> its keys) applied. Unknown and
    /// fixed ids, keys that don't parse or that Preferences would refuse, and keys a fixed
    /// binding uses are left out. A key on two bindings stays on the changed one, or on the first
    /// in table order when both are changed. Returns one line per thing left out.
    pub fn from_overrides(map: &BTreeMap<String, Vec<String>>) -> (Self, Vec<String>) {
        let fold = fold_ctrl();
        let mut keys = Self::default();
        let mut problems = Vec::new();
        let mut changed = vec![false; BindingId::ALL.len()];
        for (name, list) in map {
            let mut ignore = |why: String| problems.push(format!("Ignored keyboard setting {name}: {why}"));
            let Some(id) = BindingId::from_key(name) else {
                ignore("there is no such action".into());
                continue;
            };
            if !binding(id).rebindable {
                ignore("its keys can't be changed".into());
                continue;
            }
            let mut chords: Vec<Chord> = Vec::new();
            for text in list {
                match Chord::parse(text) {
                    None => ignore(format!("\"{text}\" is not a key Glowfly knows")),
                    Some(c) => match rebind::refusal(id, c) {
                        Some(r) => ignore(r.text(c)),
                        None if rebind::fixed_clash(id, c, fold).is_some() => ignore(format!("{} belongs to a fixed shortcut", c.text())),
                        None if chords.iter().any(|k| same_keys(*k, c, fold)) => {}
                        None if chords.len() == MAX_CHORDS => ignore(format!("{} is more than {MAX_CHORDS} keys", c.text())),
                        None => chords.push(c),
                    },
                }
            }
            // Nothing usable left of a list that had keys: the entry is ignored as a whole.
            if chords.is_empty() && !list.is_empty() {
                continue;
            }
            keys.chords[id as usize] = chords;
            changed[id as usize] = true;
        }
        // Changed bindings claim their keys first, in table order, then the defaults.
        let order = BINDINGS
            .iter()
            .filter(|b| b.rebindable && changed[b.id as usize])
            .chain(BINDINGS.iter().filter(|b| b.rebindable && !changed[b.id as usize]));
        let mut claimed: Vec<(BindingId, Chord)> = Vec::new();
        for b in order {
            let mine = std::mem::take(&mut keys.chords[b.id as usize]);
            let mut kept = Vec::new();
            for c in mine {
                match claimed.iter().find(|(_, k)| same_keys(*k, c, fold)) {
                    Some((other, _)) if changed[b.id as usize] => {
                        problems.push(format!(
                            "Ignored keyboard setting {}: {} is already the key for {}",
                            b.id.key(),
                            c.text(),
                            binding(*other).label
                        ));
                    }
                    Some(_) => {}
                    None => {
                        claimed.push((b.id, c));
                        kept.push(c);
                    }
                }
            }
            keys.set(b.id, kept);
        }
        (keys, problems)
    }

    /// The current keys of `id`; the first is the one hints show.
    pub fn chords(&self, id: BindingId) -> &[Chord] {
        &self.chords[id as usize]
    }

    /// The first key of `id` as text, e.g. "Ctrl+Shift+S"; None when it has no key.
    pub fn hint(&self, id: BindingId) -> Option<&str> {
        self.hints[id as usize].as_deref()
    }

    /// `label` with the key in parentheses, "New tab (Ctrl+T)", or just `label` without a key.
    pub fn tip(&self, label: &str, id: BindingId) -> String {
        format!("{label}{}", self.paren(id))
    }

    /// " (Ctrl+T)", or "" when `id` has no key.
    pub fn paren(&self, id: BindingId) -> String {
        self.hint(id).map(|h| format!(" ({h})")).unwrap_or_default()
    }

    /// Every key of `id` as text, joined with " or "; "No key" when it has none.
    pub fn keys_text(&self, id: BindingId) -> String {
        let chords = self.chords(id);
        if chords.is_empty() { "No key".to_owned() } else { chords.iter().map(Chord::text).collect::<Vec<_>>().join(" or ") }
    }

    /// The default keys of `id` as text, joined with " or "; "No key" when it has none.
    pub fn defaults_text(&self, id: BindingId) -> String {
        let defaults = binding(id).defaults;
        if defaults.is_empty() { "No key".to_owned() } else { defaults.iter().map(Chord::text).collect::<Vec<_>>().join(" or ") }
    }

    pub fn is_default(&self, id: BindingId) -> bool {
        self.chords(id) == binding(id).defaults
    }

    /// Whether any key differs from its default.
    pub fn customized(&self) -> bool {
        BindingId::ALL.iter().any(|&id| !self.is_default(id))
    }

    /// The changed bindings as the settings file stores them: id -> keys in file form.
    pub fn overrides(&self) -> BTreeMap<String, Vec<String>> {
        BINDINGS
            .iter()
            .filter(|b| b.rebindable && !self.is_default(b.id))
            .map(|b| (b.id.key().to_owned(), self.chords(b.id).iter().map(Chord::config).collect()))
            .collect()
    }

    /// Gives `id` these keys (a fixed binding keeps its own).
    pub fn set(&mut self, id: BindingId, chords: Vec<Chord>) {
        if !binding(id).rebindable {
            return;
        }
        self.hints[id as usize] = chords.first().map(Chord::text);
        self.chords[id as usize] = chords;
    }

    pub fn reset(&mut self, id: BindingId) {
        self.set(id, binding(id).defaults.to_vec());
    }

    pub fn reset_all(&mut self) {
        *self = Self::default();
    }

    /// The binding a press of `c` belongs to: the globally matched ones (rebindable and Go to
    /// tab) first, then the fixed context keys. A digit range answers to each digit in it.
    /// `fold` treats Cmd/Ctrl and Ctrl as one modifier (off macOS).
    pub fn lookup(&self, c: Chord, fold: bool) -> Option<BindingId> {
        let owns = |b: &&Binding| self.chords(b.id).iter().any(|k| answers(*k, c, fold));
        let global = |b: &&Binding| b.rebindable || b.id == BindingId::GoToTab;
        let first = BINDINGS.iter().filter(global).find(owns);
        first.or_else(|| BINDINGS.iter().filter(|b| !global(b)).find(owns)).map(|b| b.id)
    }

    /// The bindings matching `query`, by group in `GROUPS` order, empty groups left out. Every
    /// whitespace-separated term must appear (ignoring case) in the label, the note, the group's
    /// title or the current key text.
    pub fn filter(&self, query: &str) -> Vec<(Group, Vec<&'static Binding>)> {
        let terms: Vec<String> = query.split_whitespace().map(str::to_lowercase).collect();
        let matches = |b: &Binding| {
            let hay = format!("{} {} {} {}", b.label, b.when.note().unwrap_or(""), b.group.title(), self.keys_text(b.id)).to_lowercase();
            terms.iter().all(|t| hay.contains(t.as_str()))
        };
        GROUPS
            .iter()
            .map(|&g| (g, BINDINGS.iter().filter(|b| b.group == g && matches(b)).collect::<Vec<_>>()))
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }

    /// `filter(query)` for the shortcut sheet: Repository commands rows without a key are left
    /// out (they have none by default), and groups left empty are dropped.
    pub fn filter_for_sheet(&self, query: &str) -> Vec<(Group, Vec<&'static Binding>)> {
        self.filter(query)
            .into_iter()
            .map(|(g, rows)| (g, rows.into_iter().filter(|b| !(runs_command(b.id) && self.chords(b.id).is_empty())).collect::<Vec<_>>()))
            .filter(|(_, rows)| !rows.is_empty())
            .collect()
    }
}

/// Whether the key `k` of a binding answers to the press `c`: the same keys, or a digit in
/// `k`'s range with the same modifiers.
pub(super) fn answers(k: Chord, c: Chord, fold: bool) -> bool {
    match (k.key, c.key) {
        (KeyName::DigitRange(lo, hi), KeyName::Digit(d)) => (lo..=hi).contains(&d) && same_keys(Chord { key: c.key, ..k }, c, fold),
        _ => same_keys(k, c, fold),
    }
}
