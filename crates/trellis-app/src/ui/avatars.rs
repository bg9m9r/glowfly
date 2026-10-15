//! Avatar textures for the graph and commit details. Views ask with `get`; the shell sends the
//! new requests as `AvatarMsg::WantAvatars` and hands the fetched images back through `insert`.

use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use std::borrow::Cow;
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
        // Called for every visible row each frame: emails are nearly always lowercase ASCII
        // already, and then need no new string.
        let email = email.trim();
        let key = if email.bytes().any(|b| !b.is_ascii() || b.is_ascii_uppercase()) {
            Cow::Owned(email.to_lowercase())
        } else {
            Cow::Borrowed(email)
        };
        if key.is_empty() {
            return None;
        }
        if !self.map.contains_key(key.as_ref()) {
            self.map.insert(key.to_string(), State::Pending);
            self.wanted.push(key.to_string());
        }
        match self.map.get(key.as_ref()) {
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

#[cfg(test)]
mod tests {
    use super::Avatars;

    #[test]
    fn emails_are_requested_once_by_their_normalized_form() {
        let mut a = Avatars::default();
        for email in ["ada@example.com", " Ada@Example.COM ", "ada@example.com", "Ünal@Example.com", "ünal@example.com", "  "] {
            assert!(a.get(email).is_none());
        }
        assert_eq!(a.take_wanted(), ["ada@example.com", "ünal@example.com"]);
        assert!(a.get("ADA@example.com").is_none());
        assert!(a.take_wanted().is_empty(), "already pending");
    }
}
