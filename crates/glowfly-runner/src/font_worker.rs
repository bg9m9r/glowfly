//! The one thread that draws font previews: a `Worker` (svg_worker.rs) of its own, so a font
//! that is slow to shape or outline pins one core at most and never holds up the SVG one. Each
//! caller waits at most `DEADLINE`; past it the side shows as "Can't preview this font".

use crate::svg_worker::{DEADLINE, Worker};
use glowfly_core::preview_font::{self, FontError, FontRender};
use std::sync::{Arc, OnceLock};

/// `bytes` drawn on the app's one font worker, within `DEADLINE`.
pub(crate) fn render(bytes: &[u8]) -> Result<FontRender, FontError> {
    static WORKER: OnceLock<Worker<Result<FontRender, FontError>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| Worker::new("font-preview", Arc::new(preview_font::render), DEADLINE));
    worker.draw(bytes).unwrap_or(Err(FontError::Abandoned))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_font_draws_on_the_worker() {
        let r = render(glowfly_core::bundled_fonts::NOTO_SANS).expect("drawn within the deadline");
        assert_eq!(r.face.family, "Noto Sans");
    }
}
