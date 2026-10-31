//! Theme files (`Effect::Theme`): reading the themes folder, showing it in the file manager and
//! writing a new theme file, and importing terminal schemes (a file dialog, then reading the
//! file and writing its schemes to the themes folder's `terminal` folder).

use super::execute::{Cx, Execute};
use crate::msg::Msg;
use glowfly_core::settings::{terminal_schemes, themes};
use glowfly_model::themes::{ThemeEffect, ThemeMsg};

pub(super) fn run(effect: ThemeEffect) -> Option<Msg> {
    match effect {
        ThemeEffect::Load => {
            let cat = themes::load_all();
            glowfly_model::themes::trace_problems(&cat);
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
        ThemeEffect::PickScheme => {
            let path = rfd::FileDialog::new()
                .set_title("Import terminal color scheme")
                .add_filter("Terminal schemes (Windows Terminal, iTerm2)", &["json", "itermcolors"])
                .add_filter("All files", &["*"])
                .pick_file();
            Some(Msg::Theme(ThemeMsg::SchemePicked(path)))
        }
        ThemeEffect::ImportScheme { path } => {
            let file = path.file_name().map_or_else(|| glowfly_core::native_path::shown(&path), |n| n.to_string_lossy().into_owned());
            let result = match themes::themes_dir() {
                Some(dir) => terminal_schemes::import_path(&path, &terminal_schemes::schemes_dir(&dir)).map_err(|e| e.to_string()),
                None => Err("there is no config folder to keep it in".into()),
            };
            Some(Msg::Theme(ThemeMsg::SchemesImported { file, result }))
        }
    }
}

impl Execute for ThemeEffect {
    fn execute(self, _cx: &Cx<'_>) -> Option<Msg> {
        run(self)
    }
}
