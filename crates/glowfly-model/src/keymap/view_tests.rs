//! The View group's keys that differ by system: full screen is F11, except on macOS, where F11
//! shows the desktop and Ctrl+Cmd+F is the system's key (and stays apart from Cmd+F, Search).

use super::*;

use BindingId as I;

#[test]
fn full_screen_key_follows_the_platform() {
    let want = if cfg!(target_os = "macos") { Chord::cmd_ctrl(KeyName::Letter('F')) } else { Chord::plain(KeyName::F(11)) };
    let b = binding(I::ToggleFullscreen);
    assert_eq!(b.defaults, [want]);
    assert_eq!((b.group, b.when, b.rebindable), (Group::View, When::Anywhere, true));
    let keys = Keymap::default();
    assert_eq!(keys.lookup(want, fold_ctrl()), Some(I::ToggleFullscreen));
    assert_eq!(keys.lookup(Chord::cmd(KeyName::Letter('F')), fold_ctrl()), Some(I::Search), "Search keeps its key");
    assert_eq!(keys.hint(I::ToggleFullscreen), Some(want.text().as_str()));
}

#[test]
fn full_screen_comes_after_the_panel_keys() {
    let view: Vec<BindingId> = BINDINGS.iter().filter(|b| b.group == Group::View).map(|b| b.id).collect();
    let at = |id| view.iter().position(|&v| v == id).expect("in the View group");
    assert_eq!(at(I::ToggleFullscreen), at(I::ToggleToolbar) + 1);
}
