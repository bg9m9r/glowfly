//! The token families a palette groups its colours into, besides the flat neutrals, brand,
//! diff and syntax colours (see docs/COLOUR.md for what each colour is for and the rules).

use eframe::egui::Color32;

/// The change and state colours: status letters, line counts, bars and change chips.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Status {
    /// Added lines and files; also success and done.
    pub added: Color32,
    /// Removed lines and deleted files; also errors and failures.
    pub removed: Color32,
    /// Modified files.
    pub modified: Color32,
    /// Renames and info banners; also links to services.
    pub info: Color32,
    /// Running operations; also pull requests, and squash or fixup in a rebase.
    pub progress: Color32,
}

/// Colours that ask for the user's attention. The attention hue band (35 to 60 deg OKLCH) is
/// kept free for the later "needs you" colour.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Attention {
    /// Warnings (and, until each job has its own colour, favourites and pending checks).
    pub warn: Color32,
    /// The fill of banners.
    pub banner: Color32,
}

/// The colours of repositories: the worktree tab-group strips.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Repository {
    pub groups: [Color32; 4],
}

/// The colours of branches, one per slot of `trellis_core::branch_colour` (the main branch
/// takes the accent instead): their lines, dots and bands, and their name chips.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Branch {
    /// Lines, dots and row bands (3:1 on the background and panel).
    pub lanes: [Color32; LANE_COUNT],
    /// The tone ref chips are tinted with and their text is pushed from.
    pub chips: [Color32; LANE_COUNT],
}

/// How many branch hues there are (`trellis_core::branch_colour::SLOTS`).
pub const LANE_COUNT: usize = trellis_core::branch_colour::SLOTS as usize;

/// The terminal's 16 basic colours (ANSI 0-7 and their bright 8-15), drawn on `input`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ansi {
    pub colors: [Color32; 16],
}
