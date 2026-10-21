//! Theme files (`Effect::Theme`): reading the themes folder, showing it in the file manager and
//! writing a new theme file.

use crate::msg::Msg;
use crate::themes::{ThemeEffect, ThemeMsg};
use glowfly_core::settings::themes;

pub(super) fn run(effect: ThemeEffect) -> Option<Msg> {
    match effect {
        ThemeEffect::Load => {
            let cat = themes::load_all();
            crate::themes::trace_problems(&cat);
            Some(Msg::Theme(ThemeMsg::Loaded(cat)))
        }
        ThemeEffect::OpenFolder => {
            let Some(dir) = themes::themes_dir() else {
                return Some(Msg::Logged { ok: false, text: "Could not open the themes folder: there is no config folder".into() });
            };
            if let Err(e) = std::fs::create_dir_all(&dir) {
                let text = format!("Could not create {}: {e}", glowfly_core::native_path::shown(&dir));
                return Some(Msg::Logged { ok: false, text });
            }
            super::os::open_folder(&dir)
        }
        ThemeEffect::Create { json } => {
            let result = match themes::themes_dir() {
                Some(dir) => themes::create(&dir, &json).map_err(|e| e.to_string()),
                None => Err("no config folder".into()),
            };
            Some(Msg::Theme(ThemeMsg::Created(result)))
        }
    }
}
