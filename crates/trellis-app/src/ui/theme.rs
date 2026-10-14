//! Colors: a light and a dark palette, and egui widget styling for both.
//!
//! Views read colors through `color::text()` etc. The shell selects the palette at the start of
//! each frame with `color::set_dark`, on the thread that draws it; that is the only write. The
//! choice is per thread, so tests rendering light and dark in parallel don't affect each other.

use eframe::egui::{self, Stroke, vec2};

/// The active color palette. Call sites read colors through functions so the theme can switch
/// at runtime (Preferences > UI Customization > Theme).
pub mod color {
    use eframe::egui::Color32;
    use std::cell::Cell;
    use trellis_core::highlight::TokenKind;

    pub struct Palette {
        pub bg: Color32,
        pub panel: Color32,
        pub toolbar: Color32,
        pub input: Color32,
        pub border: Color32,
        pub hover: Color32,
        pub selected: Color32,
        pub accent: Color32,
        pub text: Color32,
        /// Text on selected rows and filled accents.
        pub strong: Color32,
        pub weak: Color32,
        pub faint: Color32,
        pub green: Color32,
        pub red: Color32,
        pub amber: Color32,
        pub blue: Color32,
        pub purple: Color32,
        pub diff_add: Color32,
        pub diff_del: Color32,
        /// Selected lines (line-level staging): stronger than `diff_add` / `diff_del`.
        pub diff_add_sel: Color32,
        pub diff_del_sel: Color32,
        /// Changed words inside paired -/+ lines: a step stronger than `diff_*_sel`; text stays
        /// >= 4.5:1 and every `syn_*` >= 2.4:1 on them.
        pub diff_add_word: Color32,
        pub diff_del_word: Color32,
        pub hunk_bg: Color32,
        pub hunk_fg: Color32,
        pub banner: Color32,
        pub primary: Color32,
        pub primary_off: Color32,
        pub primary_border: Color32,
        pub danger: Color32,
        pub danger_off: Color32,
        pub tag_bg: Color32,
        pub tag_fg: Color32,
        /// Syntax colors for diff text; each stays readable on `panel`, `diff_add`, `diff_del`,
        /// `diff_add_word` and `diff_del_word`.
        pub syn_comment: Color32,
        pub syn_string: Color32,
        pub syn_number: Color32,
        pub syn_constant: Color32,
        pub syn_keyword: Color32,
        pub syn_type: Color32,
        pub syn_function: Color32,
        pub syn_attr: Color32,
    }

    const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
        Color32::from_rgb(r, g, b)
    }

    pub const DARK: Palette = Palette {
        bg: rgb(0x1e, 0x22, 0x29),
        panel: rgb(0x24, 0x29, 0x31),
        toolbar: rgb(0x2b, 0x30, 0x39),
        input: rgb(0x19, 0x1c, 0x22),
        border: rgb(0x36, 0x3c, 0x46),
        hover: rgb(0x2f, 0x35, 0x3f),
        selected: rgb(0x1d, 0x4a, 0x58),
        accent: rgb(0x1f, 0xb5, 0xc9),
        text: rgb(0xd3, 0xd8, 0xe0),
        strong: rgb(0xff, 0xff, 0xff),
        weak: rgb(0x86, 0x8f, 0x9c),
        faint: rgb(0x5c, 0x64, 0x70),
        green: rgb(0x5c, 0xc4, 0x5a),
        red: rgb(0xe0, 0x5a, 0x4f),
        amber: rgb(0xe8, 0xb3, 0x39),
        blue: rgb(0x5b, 0x9b, 0xf0),
        purple: rgb(0xa8, 0x7c, 0xe8),
        diff_add: rgb(0x1d, 0x3b, 0x27),
        diff_del: rgb(0x45, 0x21, 0x24),
        diff_add_sel: rgb(0x2a, 0x5c, 0x3a),
        diff_del_sel: rgb(0x6a, 0x2c, 0x31),
        diff_add_word: rgb(0x2c, 0x62, 0x3d),
        diff_del_word: rgb(0x76, 0x30, 0x3a),
        hunk_bg: rgb(0x26, 0x2d, 0x3a),
        hunk_fg: rgb(0x8a, 0xa8, 0xd8),
        banner: rgb(0x4a, 0x38, 0x12),
        primary: rgb(0x1f, 0x8a, 0x5a),
        primary_off: rgb(0x24, 0x3a, 0x2e),
        primary_border: rgb(0x3f, 0xa8, 0x6a),
        danger: rgb(0x8a, 0x2c, 0x2c),
        danger_off: rgb(0x3a, 0x26, 0x28),
        tag_bg: rgb(0x5a, 0x4a, 0x1e),
        tag_fg: rgb(0xf0, 0xd0, 0x80),
        syn_comment: rgb(0x8c, 0x96, 0xa5),
        syn_string: rgb(0xa8, 0xcc, 0x8c),
        syn_number: rgb(0xe0, 0xa4, 0x6e),
        syn_constant: rgb(0x6c, 0xc8, 0xd8),
        syn_keyword: rgb(0xc4, 0x8e, 0xe6),
        syn_type: rgb(0xe6, 0xc0, 0x7a),
        syn_function: rgb(0x6f, 0xae, 0xf2),
        syn_attr: rgb(0x8a, 0xc6, 0xd0),
    };

    pub const LIGHT: Palette = Palette {
        bg: rgb(0xfa, 0xfb, 0xfc),
        panel: rgb(0xf0, 0xf2, 0xf5),
        toolbar: rgb(0xe6, 0xe9, 0xee),
        input: rgb(0xff, 0xff, 0xff),
        border: rgb(0xd0, 0xd5, 0xdd),
        hover: rgb(0xe3, 0xe8, 0xef),
        selected: rgb(0xc8, 0xe8, 0xf0),
        accent: rgb(0x0b, 0x86, 0x9a),
        text: rgb(0x24, 0x29, 0x31),
        strong: rgb(0x0a, 0x0c, 0x10),
        weak: rgb(0x5e, 0x67, 0x74),
        faint: rgb(0x93, 0x9b, 0xa6),
        green: rgb(0x23, 0x8a, 0x2f),
        red: rgb(0xc0, 0x33, 0x2a),
        amber: rgb(0xa8, 0x6b, 0x00),
        blue: rgb(0x1f, 0x63, 0xc8),
        purple: rgb(0x75, 0x3f, 0xc4),
        diff_add: rgb(0xe4, 0xf6, 0xe7),
        diff_del: rgb(0xfb, 0xe6, 0xe6),
        diff_add_sel: rgb(0xb8, 0xe6, 0xc1),
        diff_del_sel: rgb(0xf5, 0xc2, 0xc2),
        diff_add_word: rgb(0xa8, 0xdf, 0xb4),
        diff_del_word: rgb(0xf2, 0xb4, 0xb4),
        hunk_bg: rgb(0xe6, 0xee, 0xf8),
        hunk_fg: rgb(0x2a, 0x55, 0x96),
        banner: rgb(0xfb, 0xec, 0xc8),
        primary: rgb(0x23, 0x8a, 0x56),
        primary_off: rgb(0xd8, 0xe8, 0xdd),
        primary_border: rgb(0x1d, 0x70, 0x46),
        danger: rgb(0xc0, 0x33, 0x2a),
        danger_off: rgb(0xf0, 0xd6, 0xd6),
        tag_bg: rgb(0xf6, 0xe7, 0xb8),
        tag_fg: rgb(0x6b, 0x4e, 0x00),
        syn_comment: rgb(0x5f, 0x68, 0x73),
        syn_string: rgb(0x1a, 0x6f, 0x30),
        syn_number: rgb(0xa3, 0x44, 0x00),
        syn_constant: rgb(0x00, 0x6f, 0x7e),
        syn_keyword: rgb(0x8a, 0x2b, 0xb0),
        syn_type: rgb(0x8a, 0x5a, 0x00),
        syn_function: rgb(0x1f, 0x5f, 0xbf),
        syn_attr: rgb(0x00, 0x6b, 0x75),
    };

    thread_local! {
        static DARK_MODE: Cell<bool> = const { Cell::new(true) };
    }

    pub fn set_dark(dark: bool) {
        DARK_MODE.set(dark);
    }

    pub fn is_dark() -> bool {
        DARK_MODE.get()
    }

    pub fn palette() -> &'static Palette {
        if is_dark() { &DARK } else { &LIGHT }
    }

    macro_rules! accessors {
        ($($name:ident),*) => { $(pub fn $name() -> Color32 { palette().$name })* };
    }
    accessors!(
        bg,
        panel,
        toolbar,
        input,
        border,
        hover,
        selected,
        accent,
        text,
        strong,
        weak,
        faint,
        green,
        red,
        amber,
        blue,
        purple,
        diff_add,
        diff_del,
        diff_add_sel,
        diff_del_sel,
        diff_add_word,
        diff_del_word,
        hunk_bg,
        hunk_fg,
        banner,
        primary,
        primary_off,
        primary_border,
        danger,
        danger_off,
        tag_bg,
        tag_fg,
        syn_comment,
        syn_string,
        syn_number,
        syn_constant,
        syn_keyword,
        syn_type,
        syn_function,
        syn_attr
    );

    /// The color for a syntax token kind in the active palette. `Plain` is `text`, though
    /// callers draw plain runs in the row's own text color.
    pub fn token(k: TokenKind) -> Color32 {
        match k {
            TokenKind::Plain => text(),
            TokenKind::Comment => syn_comment(),
            TokenKind::String => syn_string(),
            TokenKind::Number => syn_number(),
            TokenKind::Constant => syn_constant(),
            TokenKind::Keyword => syn_keyword(),
            TokenKind::Type => syn_type(),
            TokenKind::Function => syn_function(),
            TokenKind::Attribute => syn_attr(),
        }
    }
}

/// Styles egui's built-in widgets from a palette, for both light and dark.
pub fn apply_theme(ctx: &egui::Context) {
    for (theme, p) in [(egui::Theme::Dark, &color::DARK), (egui::Theme::Light, &color::LIGHT)] {
        ctx.style_mut_of(theme, |style| {
            let v = &mut style.visuals;
            v.panel_fill = p.panel;
            v.window_fill = p.panel;
            v.extreme_bg_color = p.input;
            v.faint_bg_color = p.hover;
            v.override_text_color = None;
            v.selection.bg_fill = p.selected;
            v.selection.stroke = Stroke::new(1.0_f32, p.accent);
            v.hyperlink_color = p.accent;
            v.window_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, p.border);
            v.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
            v.widgets.inactive.bg_fill = p.toolbar;
            v.widgets.inactive.weak_bg_fill = p.toolbar;
            v.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
            v.widgets.hovered.bg_fill = p.hover;
            v.widgets.hovered.weak_bg_fill = p.hover;
            v.widgets.active.weak_bg_fill = p.selected;
            v.widgets.open.weak_bg_fill = p.hover;
            style.spacing.item_spacing = vec2(6.0, 4.0);
            style.spacing.button_padding = vec2(8.0, 3.0);
        });
    }
}
