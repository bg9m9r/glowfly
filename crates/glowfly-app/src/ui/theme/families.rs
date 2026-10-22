//! The token families a palette groups its colours into, besides the flat neutrals, brand,
//! diff and syntax colours (see docs/COLOUR.md for what each colour is for and the rules).

use eframe::egui::Color32;
use glowfly_core::settings::BranchPalette;

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

/// Colours that ask for the user's attention. The attention hue band (35 to 60 deg OKLCH)
/// belongs to the coral `needs_you` alone.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Attention {
    /// Warnings (and, until each job has its own colour, favourites and pending checks).
    pub warn: Color32,
    /// The fill of banners.
    pub banner: Color32,
    /// The coral "needs you" (a conflict, a decision Glowfly waits on): the one solid-filled
    /// status, always drawn with the warning mark (`ui::needs_you`).
    pub needs_you: Color32,
    /// The ink on a `needs_you` fill.
    pub on_needs_you: Color32,
}

/// How many repository hues there are (`glowfly_core::settings::RepoColour::ALL`).
pub const REPO_HUES: usize = 8;

/// The colours of repositories: the 8 quiet hues in `RepoColour` order, at OKLCH chroma 0.08.
/// They mark tab swatches, the window's top line, the repository list and the worktree
/// tab-group strips.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Repository {
    pub hues: [Color32; REPO_HUES],
}

/// The colours of branches, one per slot of `glowfly_core::branch_colour` (the main branch
/// takes the accent instead): their lines, dots and bands, and their name chips. A preset with
/// fewer hues than `LANE_COUNT` repeats them to fill the arrays.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Branch {
    /// Lines, dots and row bands (3:1 on the background and panel).
    pub lanes: [Color32; LANE_COUNT],
    /// The tone ref chips are tinted with and their text is pushed from.
    pub chips: [Color32; LANE_COUNT],
    /// The preset these tones come from; its `slots()` is how many distinct hues there are.
    pub preset: BranchPalette,
    /// Each slot's hue name (`branch_names`), repeating like the tones.
    pub names: [super::branch_names::Hue; LANE_COUNT],
}

/// How many branch hues the standard preset has (`glowfly_core::branch_colour::SLOTS`).
pub const LANE_COUNT: usize = glowfly_core::branch_colour::SLOTS as usize;

/// The terminal's colours: its background, default text, cursor and selection, and its 16 basic
/// colours (ANSI 0-7 and their bright 8-15). The built-ins draw on `input` in `text`, with the
/// accent for the cursor and selection.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Ansi {
    pub background: Color32,
    pub foreground: Color32,
    /// Drawn as is for a focused block cursor's outline and lines, faded for a filled block.
    pub cursor: Color32,
    /// Drawn at 35% over the background, so text stays readable through it.
    pub selection: Color32,
    pub colors: [Color32; 16],
}
