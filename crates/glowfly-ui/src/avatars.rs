//! Avatar textures for the graph and commit details. Views ask with `get`; the shell sends the
//! new requests as `AvatarMsg::WantAvatars` and hands the fetched images back through `insert`.
//!
//! At most `MAX_TEXTURES` textures stay loaded: once there are more, `end_frame` frees the ones
//! drawn least recently (down to seven eighths of the limit, so it doesn't run every frame).
//! A freed avatar is asked for again when it is next drawn, and comes from the disk cache.
//! Views draw avatars only while `shown` (lookups turned on, and a publicly hosted repository);
//! otherwise initials.

use crate::state::AppState;
use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use glowfly_core::avatars::Image;
use std::borrow::Cow;
use std::collections::HashMap;

/// Whether the active tab's views draw (and so ask for) avatars: see `avatars::lookups_allowed`.
pub(crate) fn shown(st: &AppState) -> bool {
    glowfly_core::avatars::lookups_allowed(st.settings.avatar_lookups, st.snap.as_ref().map_or(&[], |s| &s.remote_urls))
}

/// The most avatar textures kept (64 x 64 RGBA, 16 KB each: 8 MB).
const MAX_TEXTURES: usize = 512;

enum State {
    Pending,
    Ready(TextureHandle),
    Missing,
}

struct Slot {
    state: State,
    /// The frame it was last drawn in.
    drawn: u64,
}

pub struct Avatars {
    map: HashMap<String, Slot>,
    wanted: Vec<String>,
    /// Frames ended so far.
    frame: u64,
    /// Textures in `map`.
    ready: usize,
    limit: usize,
}

impl Default for Avatars {
    fn default() -> Self {
        Self::with_limit(MAX_TEXTURES)
    }
}

impl Avatars {
    /// Keeping at most `limit` textures.
    pub fn with_limit(limit: usize) -> Self {
        Self { map: HashMap::new(), wanted: Vec::new(), frame: 0, ready: 0, limit }
    }

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
            self.map.insert(key.to_string(), Slot { state: State::Pending, drawn: self.frame });
            self.wanted.push(key.to_string());
        }
        let slot = self.map.get_mut(key.as_ref())?;
        slot.drawn = self.frame;
        match &slot.state {
            State::Ready(t) => Some(t),
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
        let ready = matches!(state, State::Ready(_));
        let drawn = self.map.get(&email).map_or(self.frame, |s| s.drawn);
        if let Some(old) = self.map.insert(email, Slot { state, drawn })
            && matches!(old.state, State::Ready(_))
        {
            self.ready -= 1;
        }
        self.ready += usize::from(ready);
    }

    /// Ends a frame: past the limit, frees the textures drawn least recently.
    pub fn end_frame(&mut self) {
        self.frame += 1;
        if self.ready <= self.limit {
            return;
        }
        let keep = self.limit - self.limit / 8;
        let mut drawn: Vec<(u64, &String)> =
            self.map.iter().filter(|(_, s)| matches!(s.state, State::Ready(_))).map(|(k, s)| (s.drawn, k)).collect();
        drawn.sort_unstable();
        let gone: Vec<String> = drawn[..self.ready - keep].iter().map(|(_, k)| (*k).clone()).collect();
        for k in gone {
            self.map.remove(&k);
        }
        self.ready = keep;
    }

    /// Textures loaded now.
    #[cfg(test)]
    fn textures(&self) -> usize {
        self.ready
    }
}

#[cfg(test)]
mod tests {
    use super::Avatars;
    use eframe::egui;
    use glowfly_core::avatars::Image;

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

    fn image() -> Option<Image> {
        Some(Image { width: 1, height: 1, rgba: vec![0, 0, 0, 255] })
    }

    /// `n` avatars loaded, one drawn per frame: avatar `i` was last drawn in frame `i`.
    fn loaded(limit: usize, n: usize) -> (egui::Context, Avatars) {
        let ctx = egui::Context::default();
        let mut a = Avatars::with_limit(limit);
        for i in 0..n {
            let email = format!("p{i}@example.com");
            a.get(&email);
            a.insert(&ctx, email, image());
            a.end_frame();
        }
        a.take_wanted();
        (ctx, a)
    }

    #[test]
    fn textures_up_to_the_limit_stay() {
        let (_ctx, mut a) = loaded(8, 8);
        a.end_frame();
        assert_eq!(a.textures(), 8);
        assert!((0..8).all(|i| a.get(&format!("p{i}@example.com")).is_some()));
    }

    #[test]
    fn past_the_limit_the_least_recently_drawn_go() {
        let (ctx, mut a) = loaded(8, 8);
        // Drawn again just now: the first two are the most recent.
        a.get("p0@example.com");
        a.get("p1@example.com");
        a.get("p9@example.com");
        a.insert(&ctx, "p9@example.com".into(), image());
        a.end_frame();
        // Seven eighths of 8 kept: p0, p1, p9 and the four drawn last of the rest.
        assert_eq!(a.textures(), 7);
        let kept: Vec<usize> = (0..10).filter(|i| a.map.contains_key(&format!("p{i}@example.com"))).collect();
        assert_eq!(kept, [0, 1, 4, 5, 6, 7, 9]);
        assert!(a.take_wanted() == ["p9@example.com"], "only the new one was asked for");
        // A freed avatar is asked for again when drawn.
        assert!(a.get("p2@example.com").is_none());
        assert_eq!(a.take_wanted(), ["p2@example.com"]);
    }

    #[test]
    fn missing_avatars_dont_count_toward_the_limit() {
        let ctx = egui::Context::default();
        let mut a = Avatars::with_limit(2);
        for i in 0..5 {
            a.insert(&ctx, format!("none{i}@example.com"), None);
        }
        a.insert(&ctx, "x@example.com".into(), image());
        a.insert(&ctx, "x@example.com".into(), image());
        a.end_frame();
        assert_eq!(a.textures(), 1, "a replaced texture counts once");
        assert_eq!(a.map.len(), 6);
    }
}
