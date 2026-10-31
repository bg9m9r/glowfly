# Test-only font

`NotoSansCJKsc-TestSubset.otf` stands in for the operating system's Chinese, Japanese and Korean
fonts in tests (`crates/glowfly-ui/src/theme/fonts.rs`, under `cfg(test)` or the `test-support`
feature, which only the app's tests turn on), so snapshots and the glyph check give the same
result on every machine. It is not compiled into the app, which
uses the system's own fonts (`crates/glowfly-app/src/platform/system_fonts.rs`).

It is a subset of Noto Sans CJK SC Regular, version 2.004, licensed under the SIL Open Font
License, Version 1.1 (`NotoSansCJK-OFL.txt`, and `THIRD-PARTY-NOTICES.md`). The subset keeps
only 79 characters: the month, weekday and half-day words of the ja_JP, zh_CN and ko_KR dates
(as chrono formats them), あ, and the names, subjects, branches and paths of the `fonts_cjk`
snapshot. Nothing else in the font was changed.

| File | Source | SHA-256 |
| --- | --- | --- |
| `NotoSansCJKsc-TestSubset.otf` | made from `Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf` (SHA-256 `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`) of [notofonts/noto-cjk](https://github.com/notofonts/noto-cjk) at commit `f8d157532fbfaeda587e826d4cd5b21a49186f7c`, with the command below | `341252d34c67b67856d5a5a9e1d845c95ea65b44623c71582c2bd84c544b8394` |
| `NotoSansCJK-OFL.txt` | `Sans/LICENSE` at the same commit, unmodified | `6a73f9541c2de74158c0e7cf6b0a58ef774f5a780bf191f2d7ec9cc53efe2bf2` |

To add characters, add their code points to the list and cut the subset again with fontTools
4.68 (`pip install fonttools`):

```sh
pyftsubset NotoSansCJKsc-Regular.otf --output-file=NotoSansCJKsc-TestSubset.otf \
  --layout-features='' --no-hinting --name-IDs='*' --name-languages='*' --notdef-outline \
  --drop-tables+=VORG,vhea,vmtx,DSIG,BASE,GDEF,GPOS,GSUB \
  --unicodes=UNICODES
```

with `UNICODES` set to this list, joined into one line:

```text
U+3042,U+3092,U+4E00,U+4E03,U+4E09,U+4E0A,U+4E0B,U+4E2D,U+4E5D,U+4E8C,U+4E94,U+4FEE,
U+516B,U+516D,U+524D,U+5341,U+5348,U+56DB,U+56FE,U+571F,U+590D,U+592A,U+5B9A,U+5C0F,
U+5F55,U+5F8C,U+65E5,U+660E,U+661F,U+66DC,U+6708,U+671F,U+6728,U+672C,U+6807,U+691C,
U+6A5F,U+6B63,U+6C34,U+6E90,U+706B,U+738B,U+7530,U+753B,U+767B,U+7D22,U+80FD,U+8A2D,
U+8A9E,U+8D44,U+90CE,U+91D1,U+9762,U+9875,U+AC00,U+AC80,U+AD6D,U+AE08,U+AE30,U+AE40,
U+B2A5,U+B4C8,U+BAA8,U+BAA9,U+BBFC,U+C0C9,U+C218,U+C5B4,U+C624,U+C694,U+C6D4,U+C77C,
U+C804,U+C900,U+CD94,U+D1A0,U+D55C,U+D654,U+D6C4
```

Then update the checksum above. Keep it free of Latin, digits and symbols: it is the last
fallback in tests, and a character it adds must not change which font draws the rest.
