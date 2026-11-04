//! The words of the graph's and the diff's failed reads (#367, `ReadFailed`): every variant in
//! English, German and Spanish through `typed::read`, English exactly as the error has always
//! read (its `Display`), and libgit2's words, paths and remote names kept as they came.

use crate::state::error_words::typed;
use glowfly_core::error::ReadFailed as R;
use glowfly_core::i18n::Lang;

const LANGS: [Lang; 3] = [Lang::En, Lang::De, Lang::Es];

/// libgit2's own words, as `Error::Git`'s Display shows them.
pub(super) const GIT_SAID: &str = "could not find repository at 'x'; class=Repository (6); code=NotFound (-3)";

/// Each variant, with its English, German and Spanish words.
pub(super) fn variants() -> Vec<(R, [&'static str; 3])> {
    vec![
        (R::Said { said: GIT_SAID.into() }, [GIT_SAID; 3]),
        (R::Other { said: "not found: no changes in a.txt".into() }, ["not found: no changes in a.txt"; 3]),
        (
            R::MissingContent { path: "big.bin".into(), remote: "origin".into() },
            [
                "the contents of big.bin haven't been downloaded from origin yet",
                "der Inhalt von big.bin wurde noch nicht von origin heruntergeladen",
                "el contenido de big.bin aún no se ha descargado de origin",
            ],
        ),
        (
            R::DiffChanged,
            [
                "not found: the diff changed; try again on the updated diff",
                "nicht gefunden: der Diff hat sich geändert; versuche es mit dem aktualisierten Diff erneut",
                "no encontrado: el diff cambió; inténtalo de nuevo con el diff actualizado",
            ],
        ),
        (
            R::FileChanged,
            ["not found: the file changed", "nicht gefunden: die Datei hat sich geändert", "no encontrado: el archivo cambió"],
        ),
        (
            R::OldFileUnreadable,
            ["the old file can't be read", "die alte Datei kann nicht gelesen werden", "no se puede leer el archivo anterior"],
        ),
        (
            R::NotConflicted { path: "src/a.rs".into() },
            [
                "not found: src/a.rs is not conflicted",
                "nicht gefunden: src/a.rs hat keinen Konflikt",
                "no encontrado: src/a.rs no tiene conflictos",
            ],
        ),
        (
            R::NoFolder,
            ["A folder history needs a folder", "Ein Ordnerverlauf braucht einen Ordner", "Un historial de carpeta necesita una carpeta"],
        ),
        (
            R::NoThread { said: "Resource temporarily unavailable (os error 11)".into() },
            [
                "could not start a thread: Resource temporarily unavailable (os error 11)",
                "konnte keinen Thread starten: Resource temporarily unavailable (os error 11)",
                "no se pudo iniciar un hilo: Resource temporarily unavailable (os error 11)",
            ],
        ),
        (
            R::Internal { effect: "index".into() },
            [
                "Glowfly hit an internal error (index)",
                "In Glowfly ist ein interner Fehler aufgetreten (index)",
                "Glowfly tuvo un error interno (index)",
            ],
        ),
        (R::Stopped, ["stopped", "angehalten", "detenido"]),
    ]
}

#[test]
fn every_variant_reads_in_each_language() {
    for (e, words) in variants() {
        for (t, want) in LANGS.into_iter().zip(words) {
            assert_eq!(typed::read(t, &e), want, "{t:?} {e:?}");
        }
    }
}

#[test]
fn english_reads_as_the_error_always_has() {
    for (e, _) in variants() {
        assert_eq!(typed::read(Lang::En, &e), e.to_string(), "{e:?}");
    }
}

#[test]
fn paths_remotes_and_said_text_are_kept_in_every_language() {
    for (e, _) in variants() {
        let kept: Vec<&str> = match &e {
            R::Said { said } | R::Other { said } | R::NoThread { said } => vec![said],
            R::MissingContent { path, remote } => vec![path, remote],
            R::NotConflicted { path } => vec![path],
            R::Internal { effect } => vec![effect],
            R::DiffChanged | R::FileChanged | R::OldFileUnreadable | R::NoFolder | R::Stopped => vec![],
        };
        for t in LANGS {
            let text = typed::read(t, &e);
            for k in &kept {
                assert!(text.contains(k), "{t:?}: {text:?} lacks {k:?}");
            }
        }
    }
}
