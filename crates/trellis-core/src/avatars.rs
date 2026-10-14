//! Author avatar lookup. GitHub noreply addresses use GitHub's
//! avatar service; everything else asks Gravatar (by SHA-256 of the email, never the address
//! itself). Images and misses are cached in `avatars` under the config folder.

use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

const SIZE: u32 = 64;
/// The largest avatar download accepted, and the largest image decoded from it.
const MAX_BYTES: u64 = 2 * 1024 * 1024;
const MAX_SIDE: u32 = 1024;
/// How long a "no avatar" answer is trusted before asking again.
const MISS_TTL: Duration = Duration::from_secs(7 * 24 * 3600);

fn hash(email: &str) -> String {
    let digest = Sha256::digest(email.trim().to_lowercase().as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

fn cache_dir() -> Option<PathBuf> {
    crate::settings::config_dir().map(|d| d.join("avatars"))
}

/// The avatar URL for an email address.
pub fn url_for(email: &str) -> String {
    let e = email.trim().to_lowercase();
    if let Some(local) = e.strip_suffix("@users.noreply.github.com") {
        // "12345+login@…" carries the numeric id; older addresses are just "login@…".
        return match local.split_once('+') {
            Some((id, _)) if id.chars().all(|c| c.is_ascii_digit()) => format!("https://avatars.githubusercontent.com/u/{id}?s={SIZE}"),
            _ => format!("https://github.com/{local}.png?size={SIZE}"),
        };
    }
    format!("https://gravatar.com/avatar/{}?s={SIZE}&d=404", hash(&e))
}

/// Decoded RGBA pixels.
#[derive(Clone)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgba: Vec<u8>,
}

/// Size only: the pixels would flood the debug log.
impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Image({}x{})", self.width, self.height)
    }
}

fn decode(bytes: &[u8]) -> Option<Image> {
    if bytes.len() as u64 > MAX_BYTES {
        return None;
    }
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format().ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let img = reader.decode().ok()?.to_rgba8();
    Some(Image { width: img.width() as usize, height: img.height() as usize, rgba: img.into_raw() })
}

/// Downloads (or loads from the disk cache) the avatar for `email`. Blocking: call from a
/// worker thread.
pub fn fetch(email: &str) -> Option<Image> {
    let dir = cache_dir();
    let key = hash(email);
    if let Some(dir) = &dir {
        if let Ok(bytes) = std::fs::read(dir.join(format!("{key}.img"))) {
            return decode(&bytes);
        }
        let miss = dir.join(format!("{key}.none"));
        let fresh = std::fs::metadata(&miss)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < MISS_TTL);
        if fresh {
            return None;
        }
    }
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(10)).build();
    let bytes = match agent.get(&url_for(email)).call() {
        // Oversized images are treated as "no avatar" (and cached as such).
        Ok(resp) => crate::forge::read_capped(resp.into_reader(), MAX_BYTES).ok(),
        Err(ureq::Error::Status(404, _)) => None,
        // Network errors: don't cache, try again next session.
        Err(_) => return None,
    };
    if let Some(dir) = &dir {
        let _ = std::fs::create_dir_all(dir);
        match &bytes {
            Some(b) => {
                let _ = std::fs::write(dir.join(format!("{key}.img")), b);
            }
            None => {
                let _ = std::fs::write(dir.join(format!("{key}.none")), b"");
            }
        }
    }
    decode(&bytes?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_avatar_urls() {
        assert_eq!(url_for("12345+ada@users.noreply.github.com"), "https://avatars.githubusercontent.com/u/12345?s=64");
        assert_eq!(url_for("ada@users.noreply.github.com"), "https://github.com/ada.png?size=64");
        let g = url_for(" Ada@Example.com ");
        assert!(g.starts_with("https://gravatar.com/avatar/") && g.ends_with("?s=64&d=404"));
        assert_eq!(g, url_for("ada@example.com"), "normalized before hashing");
        assert!(!g.contains("example"), "the address itself is never sent");
    }

    fn png(w: u32, h: u32) -> Vec<u8> {
        let mut out = Vec::new();
        image::RgbaImage::new(w, h).write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    #[test]
    fn decodes_small_images_and_refuses_huge_ones() {
        let img = decode(&png(64, 64)).expect("a normal avatar");
        assert_eq!((img.width, img.height, img.rgba.len()), (64, 64, 64 * 64 * 4));
        assert!(decode(&png(MAX_SIDE + 1, 1)).is_none(), "too wide");
        assert!(decode(b"not an image").is_none());
    }
}
