# Translations

Each file here is one language's text for the UI, compiled into Trellis
(`src/i18n/catalog.rs`). `en.json` is the base: a text missing from another language is shown
in English, and a key missing from English too is shown as the key itself (the tests make sure
neither happens in shipped code).

## Format

One flat JSON object per language, keys sorted:

```json
{
  "commit.blocked.no_repo": "Open a repository first.",
  "status.open_pull_requests.one": "{count} open pull request",
  "status.open_pull_requests.other": "{count} open pull requests"
}
```

- **Keys** are `<area>.<page or part>.<item>[.<role>]`: lowercase ASCII, digits and `_`,
  separated by dots. Roles: `label` (a row's label), `check` (a checkbox's text), `help`, `warn`,
  `hint` (a field's placeholder text), `button`, `tip` (a hover text).
- **Plurals** are two keys ending in `.one` and `.other`; the code asks for the base key with a
  count (`trn`), which fills `{count}`. Every English `.other` shows `{count}`.
- **Placeholders** are `{name}`. They are filled in by the code and must stay exactly as they
  are in English, in every language: `{path}`, `{line}`, `{repo}`, `{local}`, `{remote}`,
  `{base}`, `{ours}`, `{theirs}` and `{output}` are also what users type in External Tools.
  Move them where the sentence needs them; never translate or drop them.
- **Whole sentences**: a key holds a complete text. The commit button's `{push}` and `{skip}`
  are the one exception: they are slots for " and push" and " (skip hooks)", so a language can
  place them.
- Row labels on Preferences pages are drawn right-aligned in a fixed column without wrapping:
  keep them to about 32 characters.
- Only Latin-1 letters render with the bundled fonts. Don't use symbol glyphs such as arrows or
  pencils in texts; the UI paints those.

## Adding a key

Add it to `en.json` and to every other language. `cargo test -p trellis-core i18n` lists keys a
language lacks, keys English lacks and changed placeholders; `cargo test -p trellis --test
i18n_keys` lists keys the code uses that `en.json` lacks, and English keys nothing uses.

## Adding a language

1. Copy `en.json` to `<code>.json` and translate every text.
2. Add a variant to `Lang` in `src/i18n.rs`, with its code and its name in its own language, and
   put it in `Lang::ALL` (sorted by that name).
3. Add its `include_str!` and table to `src/i18n/catalog.rs`.
4. If its plurals have more forms than one and other, extend `plural_suffix`.

## Glossary

Keep terms the same across pages.

| English | Deutsch | Español |
| --- | --- | --- |
| commit (noun / verb) | Commit / committen | commit / confirmar |
| push | Push / pushen | enviar |
| fetch | Abrufen / abrufen | obtener |
| branch | Branch | rama |
| repository | Repository (plural Repositorys) | repositorio |
| stage | bereitstellen | preparar |
| amend | nachbessern | enmendar |
| remote | Remote | remoto |
| working tree | Arbeitsverzeichnis | árbol de trabajo |
| hook | Hook | hook |
| Preferences | Einstellungen | Preferencias |

German uses the informal "du"; Spanish uses "tú".
