//! The fonts Glowfly ships (`assets/fonts/`, with their licences): Noto Sans Regular and
//! SemiBold for interface text and JetBrains Mono for monospace text. Compiled in once here and
//! shared by the interface (glowfly-ui's `theme::fonts`) and the SVG preview's text
//! (`preview_svg`), which sets text in these alone.

/// Noto Sans Regular.
pub const NOTO_SANS: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
/// Noto Sans SemiBold.
pub const NOTO_SANS_SEMIBOLD: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-SemiBold.ttf");
/// JetBrains Mono Regular.
pub const JETBRAINS_MONO: &[u8] = include_bytes!("../../../assets/fonts/JetBrainsMono-Regular.ttf");
