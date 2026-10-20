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

/// The colours of branches: the graph's lanes.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Branch {
    pub lanes: [Color32; LANE_COUNT],
}

/// How many lane colours there are before they repeat.
pub const LANE_COUNT: usize = 10;

/// The graph's lane colours.
pub const LANES: [Color32; LANE_COUNT] = [
    Color32::from_rgb(0x1f, 0xb5, 0xc9),
    Color32::from_rgb(0x3d, 0x7d, 0xf7),
    Color32::from_rgb(0x9b, 0x5d, 0xe0),
    Color32::from_rgb(0xd0, 0x4f, 0xb8),
    Color32::from_rgb(0xe8, 0x4a, 0x7a),
    Color32::from_rgb(0xe8, 0x5c, 0x3a),
    Color32::from_rgb(0xf0, 0x8d, 0x2e),
    Color32::from_rgb(0xe8, 0xc4, 0x30),
    Color32::from_rgb(0x7b, 0xc9, 0x3f),
    Color32::from_rgb(0x2e, 0xc2, 0x95),
];

/// The branch family every built-in shares.
pub const BRANCH: Branch = Branch { lanes: LANES };

/// The terminal's 16 basic colours (ANSI 0-7 and their bright 8-15), drawn on `input`.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ansi {
    pub colors: [Color32; 16],
}
