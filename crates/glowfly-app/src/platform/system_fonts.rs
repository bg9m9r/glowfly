//! The operating system's own fonts for the scripts the bundled ones lack: Chinese, Japanese and
//! Korean (Han, kana and Hangul). Searched once per run on a background thread that `start`
//! begins early in `main`; `found` waits for it (the first frame's font setup asks). The search
//! tries a short list of known files for this system (`candidates`), in the order the system
//! language prefers, and keeps a file only while it adds a script still missing, so one to
//! three fonts are kept. Each is memory-mapped, not read: its tables stay in the file and only
//! the pages egui reads (the few glyphs drawn) take memory. A font that is missing, can't be
//! mapped or doesn't parse is skipped without a word; none found leaves the bundled fonts.

mod candidates;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub use candidates::Candidate;

/// A font found on this system, ready for egui's `FontData` (`data` with face `index`).
pub struct Fallback {
    /// Its key in egui's font list: `system-` and the file's name, with the face index.
    pub name: String,
    pub data: &'static [u8],
    pub index: u32,
    pub path: PathBuf,
}

/// A script the bundled fonts lack, with a character that shows whether a font has it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Script {
    Han,
    Kana,
    Hangul,
}

/// Each script and its test character: 日, あ and 한.
pub const SCRIPTS: [(Script, char); 3] = [(Script::Han, '\u{65E5}'), (Script::Kana, '\u{3042}'), (Script::Hangul, '\u{D55C}')];

/// Bigger files are skipped: the largest system CJK fonts are about 25 MB.
const MAX_BYTES: u64 = 96 * 1024 * 1024;

static FOUND: OnceLock<Vec<Fallback>> = OnceLock::new();

/// Begins the search on its own thread, so it overlaps the window's creation.
pub fn start() {
    let _ = std::thread::Builder::new().name("system-fonts".into()).spawn(|| {
        found();
    });
}

/// The fonts found, searching now (or waiting for `start`'s search) on the first call.
pub fn found() -> &'static [Fallback] {
    FOUND.get_or_init(|| {
        let began = std::time::Instant::now();
        let fonts = search();
        let names: Vec<String> = fonts.iter().map(|f| format!("{} #{}", f.path.display(), f.index)).collect();
        tracing::debug!(
            "system fallback fonts in {:?}: {}",
            began.elapsed(),
            if names.is_empty() { "none".into() } else { names.join(", ") }
        );
        fonts
    })
}

/// Searches this system's fonts now (`found` does it once per run).
pub fn search() -> Vec<Fallback> {
    choose(&candidates::for_this_system())
}

/// Whether `font` has a glyph for `c`.
#[cfg(test)]
pub fn has(font: &Fallback, c: char) -> bool {
    use ab_glyph::Font;
    ab_glyph::FontRef::try_from_slice_and_index(font.data, font.index).is_ok_and(|f| f.glyph_id(c).0 != 0)
}

/// The scripts of `SCRIPTS` the face `index` in `data` has, or None when it doesn't parse (egui
/// would panic on it).
pub fn scripts_in(data: &[u8], index: u32) -> Option<Vec<Script>> {
    use ab_glyph::Font;
    let face = ab_glyph::FontRef::try_from_slice_and_index(data, index).ok()?;
    Some(SCRIPTS.iter().filter(|(_, c)| face.glyph_id(*c).0 != 0).map(|(s, _)| *s).collect())
}

/// The candidates to keep, in order: each that parses and has a script no earlier one has,
/// until every script is covered.
pub fn choose(candidates: &[Candidate]) -> Vec<Fallback> {
    let mut missing: Vec<Script> = SCRIPTS.iter().map(|(s, _)| *s).collect();
    let mut out = Vec::new();
    for c in candidates {
        if missing.is_empty() {
            break;
        }
        let Some(map) = map_file(&c.path) else { continue };
        let Some(has) = scripts_in(&map, c.index) else { continue };
        if !has.iter().any(|s| missing.contains(s)) {
            continue;
        }
        missing.retain(|s| !has.contains(s));
        let file = c.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        // Kept for the rest of the run: egui borrows the bytes for as long as it draws.
        let data: &'static [u8] = Box::leak(Box::new(map));
        out.push(Fallback { name: format!("system-{file}-{}", c.index), data, index: c.index, path: c.path.clone() });
    }
    out
}

/// The file mapped read-only, when it is a regular file of a plausible size.
fn map_file(path: &Path) -> Option<memmap2::Mmap> {
    let file = std::fs::File::open(path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() < 1024 || meta.len() > MAX_BYTES {
        return None;
    }
    // SAFETY: the mapping is read-only, and system font files are replaced (a new file renamed
    // over the old one, which keeps the mapped one intact) rather than rewritten in place.
    // Windows refuses to delete or rewrite a mapped file.
    unsafe { memmap2::Mmap::map(&file) }.ok()
}

#[cfg(test)]
mod tests;
