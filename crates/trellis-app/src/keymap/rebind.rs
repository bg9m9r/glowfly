//! The rules for changing a key in Preferences > Keyboard: keys no action may take, keys only
//! some actions may take, clashes with other bindings, and what Swap and Reset do. Pure: they
//! read a `Keymap` and say what to change.

use super::active::{Keymap, answers};
use super::chord::{Chord, KeyName};
use super::ids::BindingId;
use super::table::{BINDINGS, Binding, When, binding};

/// Whether Cmd/Ctrl ("Mod") and the real Ctrl are one key here: everywhere but macOS.
pub fn fold_ctrl() -> bool {
    !cfg!(target_os = "macos")
}

/// Whether `a` and `b` are the same keys; with `fold`, Mod and Ctrl count as one modifier.
pub fn same_keys(a: Chord, b: Chord, fold: bool) -> bool {
    if a.key != b.key || a.mods.shift != b.mods.shift || a.mods.alt != b.mods.alt {
        return false;
    }
    if fold {
        (a.mods.command || a.mods.ctrl) == (b.mods.command || b.mods.ctrl)
    } else {
        a.mods.command == b.mods.command && a.mods.ctrl == b.mods.ctrl
    }
}

/// Which of an action's keys a recorded key goes to.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Slot {
    /// Replaces the key at this index.
    Replace(usize),
    /// Becomes one more key.
    Add,
}

/// Why a key can't be given to an action at all.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Refusal {
    /// Esc, and the plain typing and moving keys (Enter, Tab, the arrows…).
    Reserved,
    /// Mod+C, Mod+X, Mod+V and Mod+A (except Mod+A for selecting every commit).
    ClipboardKey,
    /// A plain key for an action that also works while typing.
    NeedsModifier,
}

impl Refusal {
    /// The sentence Preferences shows under the row.
    pub fn text(self, c: Chord) -> String {
        let key = c.text();
        match self {
            Refusal::Reserved if c.key == KeyName::Escape => format!("{key} can't be used: Esc cancels and closes things"),
            Refusal::Reserved => format!("{key} can't be used: it is kept for typing and moving around"),
            Refusal::ClipboardKey => format!("{key} can't be used: it is used for copy, cut, paste and select all"),
            Refusal::NeedsModifier => {
                let mods = if cfg!(target_os = "macos") { "Cmd, Ctrl or Alt" } else { "Ctrl or Alt" };
                format!("{key} needs {mods}: this key also works while typing")
            }
        }
    }
}

/// Why `id` can never have `c`, if it can't.
pub fn refusal(id: BindingId, c: Chord) -> Option<Refusal> {
    use KeyName as K;
    let plain = !c.has_command_mod();
    let moving = matches!(
        c.key,
        K::Enter | K::Tab | K::Backspace | K::Space | K::Up | K::Down | K::Left | K::Right | K::Home | K::End | K::PageUp | K::PageDown
    );
    if c.key == K::Escape || matches!(c.key, K::DigitRange(..)) || (plain && moving) {
        return Some(Refusal::Reserved);
    }
    let primary = c.mods.command || (fold_ctrl() && c.mods.ctrl);
    // Selecting every commit is select all, in the graph, where no text field has the keys.
    let select_all = id == BindingId::SelectAllCommits && c.key == K::Letter('A');
    if primary && !c.mods.shift && !c.mods.alt && matches!(c.key, K::Letter('C' | 'X' | 'V' | 'A')) && !select_all {
        return Some(Refusal::ClipboardKey);
    }
    // Delete edits text in every field and the terminal; F-keys and Insert don't.
    let typed = !matches!(c.key, K::F(_) | K::Insert);
    if plain && typed && binding(id).when == When::Anywhere {
        return Some(Refusal::NeedsModifier);
    }
    None
}

/// What happens to a recorded key (`check`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Check {
    /// It can be applied.
    Ok,
    /// It already is one of the action's keys.
    Same,
    Refused(Refusal),
    /// Another binding that can be reached at the same time has it.
    Clash {
        other: BindingId,
        fixed: bool,
    },
}

/// Whether `id` may get `c` in `keys`.
pub fn check(keys: &Keymap, id: BindingId, c: Chord, fold: bool) -> Check {
    if keys.chords(id).iter().any(|k| same_keys(*k, c, fold)) {
        return Check::Same;
    }
    if let Some(r) = refusal(id, c) {
        return Check::Refused(r);
    }
    if let Some(other) = fixed_clash(id, c, fold) {
        return Check::Clash { other, fixed: true };
    }
    match holder(keys, id, c, fold, &[id]) {
        Some(other) => Check::Clash { other, fixed: false },
        None => Check::Ok,
    }
}

/// The fixed binding, reachable together with `id`, whose keys include `c`.
pub(super) fn fixed_clash(id: BindingId, c: Chord, fold: bool) -> Option<BindingId> {
    let when = binding(id).when;
    let hit = |b: &&Binding| !b.rebindable && b.when.overlaps(when) && b.defaults.iter().any(|k| answers(*k, c, fold));
    BINDINGS.iter().find(hit).map(|b| b.id)
}

/// The rebindable binding other than those in `skip`, reachable together with `id`, that has `c`.
fn holder(keys: &Keymap, id: BindingId, c: Chord, fold: bool, skip: &[BindingId]) -> Option<BindingId> {
    let when = binding(id).when;
    let hit = |b: &&Binding| {
        b.rebindable && !skip.contains(&b.id) && b.when.overlaps(when) && keys.chords(b.id).iter().any(|k| same_keys(*k, c, fold))
    };
    BINDINGS.iter().find(hit).map(|b| b.id)
}

/// What `other` would get in exchange when `id` takes its key: `id`'s old key at a `Replace`
/// slot, when `other` could have it. None for `Add`, or when the old key is refused for `other`
/// or used by a third binding.
pub fn swap_chord(keys: &Keymap, id: BindingId, slot: Slot, other: BindingId, fold: bool) -> Option<Chord> {
    let Slot::Replace(n) = slot else { return None };
    let old = *keys.chords(id).get(n)?;
    let free =
        refusal(other, old).is_none() && fixed_clash(other, old, fold).is_none() && holder(keys, other, old, fold, &[id, other]).is_none();
    free.then_some(old)
}

/// `id`'s keys with `c` put at `slot`.
pub fn apply(keys: &Keymap, id: BindingId, slot: Slot, c: Chord) -> Vec<Chord> {
    let mut chords = keys.chords(id).to_vec();
    match slot {
        Slot::Replace(n) if n < chords.len() => chords[n] = c,
        _ => chords.push(c),
    }
    chords
}

/// Puts `id` back to its default keys, taking any of them another binding holds now. Returns
/// what was taken from whom.
pub fn reset_taking(keys: &mut Keymap, id: BindingId, fold: bool) -> Vec<(BindingId, Chord)> {
    keys.reset(id);
    let mut taken = Vec::new();
    for &d in binding(id).defaults {
        while let Some(other) = holder(keys, id, d, fold, &[id]) {
            let (lost, kept): (Vec<Chord>, Vec<Chord>) = keys.chords(other).iter().partition(|k| same_keys(**k, d, fold));
            taken.extend(lost.into_iter().map(|c| (other, c)));
            keys.set(other, kept);
        }
    }
    taken
}
