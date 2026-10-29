use super::*;
use eframe::egui::Modifiers;
use glowfly_core::terminal::{Modes, encode};

/// Text is selected; nothing else.
const SELECTED: Opts = Opts { has_selection: true, history_keys: false, alt_is_meta: false };
/// Alt with a character key sends ESC and the character.
const META: Opts = Opts { has_selection: false, history_keys: false, alt_is_meta: true };
/// Shift with Page Up/Down, Home and End scrolls.
const HISTORY: Opts = Opts { has_selection: false, history_keys: true, alt_is_meta: false };

/// The bytes the inputs of `events` send.
fn bytes(events: &[Event], opts: Opts) -> Vec<u8> {
    collect(events, &mut None, opts).inputs.iter().flat_map(|i| encode(i, Modes::default())).collect()
}

fn key(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key { key, physical_key: None, pressed, repeat: false, modifiers }
}

fn input(key: TermKey, ctrl: bool, alt: bool, shift: bool) -> TermInput {
    TermInput::Key { key, mods: KeyMods { ctrl, alt, shift } }
}

#[test]
fn maps_egui_events() {
    let none = Modifiers::NONE;
    let one = |e: Event| collect(&[e], &mut None, Opts::default()).inputs;
    assert_eq!(one(Event::Text("a".into())), [TermInput::Text("a".into())]);
    assert_eq!(one(key(Key::Enter, true, none)), [input(TermKey::Enter, false, false, false)]);
    assert!(one(key(Key::Enter, false, none)).is_empty());
    assert_eq!(one(key(Key::C, true, Modifiers::CTRL)), [input(TermKey::Char('c'), true, false, false)]);
    assert!(one(key(Key::Q, true, Modifiers::CTRL | Modifiers::ALT)).is_empty(), "AltGr types text");
    assert_eq!(one(Event::Paste("x".into())), [TermInput::Paste("x".into())]);
    assert_eq!(one(key(Key::ArrowUp, true, Modifiers::SHIFT)), [input(TermKey::Up, false, false, true)]);
    assert!(one(key(Key::A, true, none)).is_empty(), "plain letters come as text");
    assert_eq!(one(key(Key::OpenBracket, true, Modifiers::CTRL)), [input(TermKey::Char('['), true, false, false)]);
}

#[test]
fn real_ctrl_on_every_platform_and_never_cmd() {
    let one = |e: Event| collect(&[e], &mut None, Opts::default()).inputs;
    let ctrl_c = [input(TermKey::Char('c'), true, false, false)];
    // Windows and Linux report Ctrl as both `ctrl` and `command`; macOS only as `ctrl`.
    assert_eq!(one(key(Key::C, true, Modifiers::CTRL | Modifiers::COMMAND)), ctrl_c);
    assert_eq!(one(key(Key::C, true, Modifiers::CTRL)), ctrl_c);
    // macOS Cmd (`mac_cmd` and `command`) sends nothing to the shell.
    let cmd = Modifiers::MAC_CMD | Modifiers::COMMAND;
    assert!(one(key(Key::C, true, cmd)).is_empty());
    assert!(one(key(Key::ArrowLeft, true, cmd)).is_empty(), "not a plain Left");
    assert!(one(key(Key::Enter, true, cmd | Modifiers::SHIFT)).is_empty());
}

/// Feeds `frames` of events in turn, as the panel does, and returns all the input.
fn frames(frames: &[&[Event]]) -> Vec<TermInput> {
    let mut pending = None;
    frames.iter().flat_map(|f| collect(f, &mut pending, Opts::default()).inputs).collect()
}

#[test]
fn copy_with_a_selection_copies_instead_of_interrupting() {
    let mut pending = None;
    let got = collect(&[Event::Copy], &mut pending, SELECTED);
    assert_eq!(got, Collected { copy: true, ..Default::default() });
    assert_eq!(pending, None, "no ^C waits for the key's release");
    let up = key(Key::C, false, Modifiers::CTRL);
    assert_eq!(collect(&[up], &mut pending, Opts::default()), Collected::default(), "the release sends nothing either");
    // Ctrl+Insert on Windows arrives the same way.
    let got = collect(&[Event::Copy, key(Key::Insert, false, Modifiers::CTRL)], &mut None, SELECTED);
    assert_eq!(got, Collected { copy: true, ..Default::default() });
    // Without a selection, nothing changes: ^C waits for its key (not on macOS, where Cmd+C copies).
    let got = collect(&[Event::Copy], &mut pending, Opts::default());
    assert!(!got.copy && got.inputs.is_empty());
    assert_eq!(pending.is_some(), !cfg!(target_os = "macos"));
}

#[test]
fn copy_and_cut_wait_for_their_key() {
    if cfg!(target_os = "macos") {
        return;
    }
    let up = |k, m| key(k, false, m);
    let (ctrl, shift) = (Modifiers::CTRL, Modifiers::SHIFT);
    let ctrl_c = input(TermKey::Char('c'), true, false, false);
    assert_eq!(frames(&[&[Event::Copy], &[up(Key::C, ctrl)]]), std::slice::from_ref(&ctrl_c));
    assert_eq!(frames(&[&[Event::Copy, up(Key::C, ctrl)]]), std::slice::from_ref(&ctrl_c), "in one frame");
    assert_eq!(frames(&[&[Event::Copy], &[Event::Copy], &[up(Key::C, ctrl)]]), [ctrl_c.clone(), ctrl_c.clone()], "held");
    assert_eq!(frames(&[&[Event::Cut], &[up(Key::X, ctrl)]]), [input(TermKey::Char('x'), true, false, false)]);

    // Ctrl+Insert and Shift+Delete don't interrupt the shell.
    assert_eq!(frames(&[&[Event::Copy], &[up(Key::Insert, ctrl)]]), [input(TermKey::Insert, true, false, false)]);
    assert_eq!(frames(&[&[Event::Cut], &[up(Key::Delete, shift)]]), [input(TermKey::Delete, false, false, true)]);
    assert!(frames(&[&[Event::Copy], &[up(Key::Copy, Modifiers::NONE)]]).is_empty(), "a Copy key");

    // Typing on before the release sends the ^C first, in order.
    let typed = frames(&[&[Event::Copy], &[Event::Text("a".into())], &[up(Key::C, ctrl)]]);
    assert_eq!(typed, [ctrl_c.clone(), TermInput::Text("a".into())]);
    // An unrelated release keeps it waiting.
    assert_eq!(frames(&[&[Event::Copy], &[up(Key::A, ctrl)]]), []);
    assert_eq!(frames(&[&[Event::Copy, up(Key::A, ctrl)], &[up(Key::C, ctrl)]]), [ctrl_c]);
}

#[test]
fn alt_letters_send_escape_and_the_letter_once() {
    let (alt, shift) = (Modifiers::ALT, Modifiers::SHIFT);
    let alt_b = key(Key::B, true, alt);
    assert_eq!(collect(std::slice::from_ref(&alt_b), &mut None, META).inputs, [input(TermKey::Char('b'), false, true, false)]);
    assert_eq!(bytes(std::slice::from_ref(&alt_b), META), b"\x1bb");
    assert_eq!(bytes(&[key(Key::B, true, alt | shift)], META), b"\x1bB");
    // The letter's own text in the same frame isn't typed as well; later text is.
    let got = collect(&[alt_b.clone(), Event::Text("b".into()), Event::Text("x".into())], &mut None, META).inputs;
    assert_eq!(got, [input(TermKey::Char('b'), false, true, false), TermInput::Text("x".into())]);
    let got = collect(&[key(Key::B, true, alt | shift), Event::Text("B".into())], &mut None, META).inputs;
    assert_eq!(got, [input(TermKey::Char('B'), false, true, true)]);
    // Without it (macOS), Option composes: the key sends nothing and the text passes through.
    let got = collect(&[alt_b, Event::Text("b".into())], &mut None, Opts::default()).inputs;
    assert_eq!(got, [TermInput::Text("b".into())]);
    // AltGr (Ctrl+Alt) still types its character.
    assert!(collect(&[key(Key::Q, true, Modifiers::CTRL | alt)], &mut None, META).inputs.is_empty());
    // Alt with named keys is unchanged.
    assert_eq!(bytes(&[key(Key::Enter, true, alt)], META), b"\x1b\r");
}

#[test]
fn ctrl_digits_send_xterms_bytes() {
    let ctrl = |k| bytes(&[key(k, true, Modifiers::CTRL | Modifiers::COMMAND)], Opts::default());
    assert_eq!(ctrl(Key::Num2), [0x00]);
    assert_eq!(ctrl(Key::Num3), [0x1b]);
    assert_eq!(ctrl(Key::Num4), [0x1c]);
    assert_eq!(ctrl(Key::Num5), [0x1d]);
    assert_eq!(ctrl(Key::Num6), [0x1e]);
    assert_eq!(ctrl(Key::Num7), [0x1f]);
    assert_eq!(ctrl(Key::Num8), [0x7f]);
    assert_eq!(ctrl(Key::Num1), b"1");
    assert_eq!(ctrl(Key::Num9), b"9");
    assert!(ctrl(Key::Num0).is_empty(), "zoom reset");
}

#[test]
fn shift_paging_keys_scroll_the_history() {
    let shift = Modifiers::SHIFT;
    let steps = [
        (Key::PageUp, ScrollStep::PageUp, TermKey::PageUp),
        (Key::PageDown, ScrollStep::PageDown, TermKey::PageDown),
        (Key::Home, ScrollStep::Top, TermKey::Home),
        (Key::End, ScrollStep::Bottom, TermKey::End),
    ];
    for (k, step, term) in steps {
        let got = collect(&[key(k, true, shift)], &mut None, HISTORY);
        assert_eq!(got, Collected { scroll: vec![step], ..Default::default() }, "{k:?}");
        // On the alternate screen the program gets them.
        let got = collect(&[key(k, true, shift)], &mut None, Opts::default());
        assert_eq!((got.inputs, got.scroll), (vec![input(term, false, false, true)], vec![]), "{k:?}");
    }
    // Plain Page Up, and Ctrl+Shift+Page Up (move tab), are never steps.
    let got = collect(&[key(Key::PageUp, true, Modifiers::NONE)], &mut None, HISTORY);
    assert_eq!((got.inputs, got.scroll), (vec![input(TermKey::PageUp, false, false, false)], vec![]));
    let got = collect(&[key(Key::PageUp, true, Modifiers::CTRL | shift)], &mut None, HISTORY);
    assert!(got.scroll.is_empty());
}

#[test]
fn alt_digits_and_punctuation_send_escape_and_the_character() {
    let (alt, shift) = (Modifiers::ALT, Modifiers::SHIFT);
    let alt_key = |k, m: Modifiers, text: Option<&str>| {
        let mut events = vec![key(k, true, m)];
        events.extend(text.map(|t| Event::Text(t.into())));
        bytes(&events, META)
    };
    assert_eq!(alt_key(Key::Period, alt, Some(".")), b"\x1b.", "Alt+. (yank the last argument)");
    assert_eq!(alt_key(Key::Period, alt, None), b"\x1b.", "no text came");
    assert_eq!(alt_key(Key::Num1, alt, Some("1")), b"\x1b1", "Alt+digit (a count)");
    assert_eq!(alt_key(Key::Num0, alt, None), b"\x1b0");
    assert_eq!(alt_key(Key::Minus, alt | shift, Some("_")), b"\x1b_", "Alt+_");
    assert_eq!(alt_key(Key::Minus, alt | shift, None), b"\x1b_", "Shift as on a US keyboard");
    // The text the layout typed wins over the key's US character (Shift+7 is / on German).
    assert_eq!(alt_key(Key::Slash, alt | shift, Some("/")), b"\x1b/");
    // Once: the next key's text is typed, not eaten.
    let got = bytes(&[key(Key::Period, true, alt), key(Key::X, true, Modifiers::NONE), Event::Text("x".into())], META);
    assert_eq!(got, b"\x1b.x");
    // Space and named keys keep their own encoding.
    assert!(meta_char(Key::Space, alt).is_none() && meta_char(Key::Enter, alt).is_none());
    assert!(meta_char(Key::Period, alt | Modifiers::CTRL).is_none(), "AltGr");
    assert!(meta_char(Key::Period, alt | Modifiers::MAC_CMD | Modifiers::COMMAND).is_none(), "the app's Cmd chords");
}

#[test]
fn option_as_meta_sends_the_keys_character_not_the_composed_one() {
    let (alt, shift) = (Modifiers::ALT, Modifiers::SHIFT);
    let option = |k, m, composed: &str, opts| bytes(&[key(k, true, m), Event::Text(composed.into())], opts);
    assert_eq!(option(Key::B, alt, "∫", META), b"\x1bb", "Option+B (word back)");
    assert_eq!(option(Key::F, alt, "ƒ", META), b"\x1bf");
    assert_eq!(option(Key::Period, alt, "≥", META), b"\x1b.");
    assert_eq!(option(Key::Minus, alt | shift, "—", META), b"\x1b_");
    // Text longer than one character is typed after the ESC and the key.
    assert_eq!(option(Key::B, alt, "ab", META), b"\x1bbab");
    // Without Option as Meta, Option types what it composes.
    assert_eq!(option(Key::B, alt, "∫", Opts::default()), "∫".as_bytes());
}

#[test]
fn each_systems_alt_follows_its_settings() {
    use glowfly_core::settings::TerminalPrefs;
    let alt_b = [key(Key::B, true, Modifiers::ALT), Event::Text("∫".into())];
    let sent = |prefs: &TerminalPrefs, macos| bytes(&alt_b, Opts { alt_is_meta: prefs.alt_is_meta_for(macos), ..Opts::default() });
    let (default, meta) = (TerminalPrefs::default(), TerminalPrefs { option_as_meta: true, ..Default::default() });
    assert_eq!(sent(&default, true), "∫".as_bytes(), "macOS composes by default");
    assert_eq!(sent(&meta, true), b"\x1bb", "macOS with Option as Meta");
    assert_eq!(sent(&default, false), b"\x1bb", "Windows and Linux");
    assert_eq!(sent(&meta, false), b"\x1bb");
}
