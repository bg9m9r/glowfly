# Bundled fonts

Glowfly draws its interface in these fonts, embedded in the binary with `include_bytes!`
(`crates/glowfly-ui/src/theme/fonts.rs`). The files are copied unmodified from the upstream
releases below: no subsetting, no renaming of the font data. Both families are licensed under
the SIL Open Font License, Version 1.1; the license texts are next to them and in
`THIRD-PARTY-NOTICES.md`, which Help > About lists. egui's own built-in fonts stay behind them as
fallbacks; their licences are generated into the same file from the `epaint_default_fonts` crate.

| File | Source | SHA-256 |
| --- | --- | --- |
| `NotoSans-Regular.ttf` | [NotoSans-v2.015](https://github.com/notofonts/latin-greek-cyrillic/releases/tag/NotoSans-v2.015), `NotoSans-v2.015.zip`: `NotoSans/unhinted/ttf/NotoSans-Regular.ttf` | `f3961a9cde016d41a4879aecda1474d3a36d6bf54fa0e4643de029cc2248b0e8` |
| `NotoSans-SemiBold.ttf` | [NotoSans-v2.015](https://github.com/notofonts/latin-greek-cyrillic/releases/tag/NotoSans-v2.015), `NotoSans-v2.015.zip`: `NotoSans/unhinted/ttf/NotoSans-SemiBold.ttf` | `a7af8d4a11e8e49fea9a19786a6469210b512cd4061413e8036bdd2425722795` |
| `NotoSans-OFL.txt` | [NotoSans-v2.015](https://github.com/notofonts/latin-greek-cyrillic/releases/tag/NotoSans-v2.015), `NotoSans-v2.015.zip`: `OFL.txt` | `cee9892f9f0cc8fe882c9e9537ee6a89621d86ee7ceaf70b02e2b2b1c25c061a` |
| `JetBrainsMono-Regular.ttf` | [v2.304](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304), `JetBrainsMono-2.304.zip`: `fonts/ttf/JetBrainsMono-Regular.ttf` | `a0bf60ef0f83c5ed4d7a75d45838548b1f6873372dfac88f71804491898d138f` |
| `JetBrainsMono-OFL.txt` | [v2.304](https://github.com/JetBrains/JetBrainsMono/releases/tag/v2.304), `JetBrainsMono-2.304.zip`: `OFL.txt` | `30f0c136e3c88e422d0791acd97238870f9054a9729bc34cf2ff0d4ed8cac4ad` |

The checksums are of the files as released (LF line endings for the licence texts). They are
static TrueType files (for Noto Sans its unhinted build), not the variable fonts: egui draws
only a variable font's default instance. To update a font, replace the file with the new release's, update its row
here and the version in `THIRD-PARTY-NOTICES.md`.

Chinese, Japanese and Korean are drawn in the operating system's own fonts, found at startup
(`crates/glowfly-app/src/platform/system_fonts.rs`); none is bundled. `test/` holds a small
subset of Noto Sans CJK that only the tests use in their place; its README says how it was made.
