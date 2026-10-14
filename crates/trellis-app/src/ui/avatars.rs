//! Avatar textures for the graph and commit details. Views ask with `get`; the shell sends the
//! new requests as `Msg::WantAvatars` and hands the fetched images back through `insert`.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::collections::HashMap;
use trellis_core::avatars::Image;

enum State {
    Pending,
    Ready(TextureHandle),
    Missing,
}

#[derive(Default)]
pub struct Avatars {
    map: HashMap<String, State>,
    wanted: Vec<String>,
}

impl Avatars {
    /// The avatar for `email`, requesting it on first use.
    pub fn get(&mut self, email: &str) -> Option<&TextureHandle> {
        let key = email.trim().to_lowercase();
        if key.is_empty() {
            return None;
        }
        if !self.map.contains_key(&key) {
            self.map.insert(key.clone(), State::Pending);
            self.wanted.push(key.clone());
        }
        match self.map.get(&key) {
            Some(State::Ready(t)) => Some(t),
            _ => None,
        }
    }

    /// Emails requested since the last call.
    pub fn take_wanted(&mut self) -> Vec<String> {
        std::mem::take(&mut self.wanted)
    }

    pub fn insert(&mut self, ctx: &egui::Context, email: String, img: Option<Image>) {
        let state = match img {
            Some(img) => {
                let image = ColorImage::from_rgba_unmultiplied([img.width, img.height], &img.rgba);
                State::Ready(ctx.load_texture(format!("avatar:{email}"), image, TextureOptions::LINEAR))
            }
            None => State::Missing,
        };
        self.map.insert(email, state);
    }
}
