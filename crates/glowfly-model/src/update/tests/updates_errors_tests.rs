//! The words of Glowfly's own failed updates (#367): a check (`UpdateMsg::Checked`), a download
//! (`Downloaded`) and Install and restart (`RestartReady`), each with an `UpdateError`. Every
//! variant's text in English, German and Spanish, as the toast, Preferences and the log show it;
//! the paths, sizes and the system's own words are kept as they are, and the English reads as
//! it did before the errors were typed.

use super::tests::state;
use super::update;
use crate::msg::{Msg, UpdateMsg};
use crate::state::AppState;
use crate::state::error_words::typed;
use crate::state::updates::UpdateStatus;
use glowfly_core::app_update::{Asset, InstallKind, InstallMethod, Offer, Release, SumsSource, UpdateError as E, Version};
use glowfly_core::forge::ServiceError;
use glowfly_core::i18n::Lang;
use std::path::PathBuf;
use std::time::Instant;

const LANGS: [&str; 3] = ["en", "de", "es"];
const FILE: &str = "/downloads/glowfly.msi";
const SAID: &str = "Access is denied. (os error 5)";

/// Each variant, with its English, German and Spanish text.
fn variants() -> Vec<(E, [&'static str; 3])> {
    let read = E::ReadFailed { path: FILE.into(), said: SAID.into() };
    let write = E::WriteFailed { path: "/tmp/glowfly-update-1/plan.json".into(), said: SAID.into() };
    let rollback = E::RollbackFailed { failed: vec![("/opt/glowfly/glowfly.bak".into(), SAID.into())] };
    let late = E::DownloadFailed { name: "glowfly.msi".into(), why: Box::new(E::LargerThanExpected { name: "glowfly.msi".into() }) };
    vec![
        (
            E::FeedNotAbsolute { value: "feed".into() },
            [
                "GLOWFLY_UPDATE_FEED must name an absolute folder (got \"feed\")",
                "GLOWFLY_UPDATE_FEED muss einen absoluten Ordner angeben (angegeben: „feed“)",
                "GLOWFLY_UPDATE_FEED debe indicar una carpeta absoluta (se indicó «feed»)",
            ],
        ),
        (
            E::FeedFolderMissing { dir: "/feed".into() },
            [
                "The update feed folder /feed doesn't exist",
                "Den Update-Feed-Ordner /feed gibt es nicht",
                "La carpeta de actualizaciones /feed no existe",
            ],
        ),
        (
            E::Service(ServiceError::Network { said: "timed out".into() }),
            ["network error: timed out", "Netzwerkfehler: timed out", "error de red: timed out"],
        ),
        (
            read.clone(),
            [
                "Couldn't read /downloads/glowfly.msi: Access is denied. (os error 5)",
                "/downloads/glowfly.msi konnte nicht gelesen werden: Access is denied. (os error 5)",
                "No se pudo leer /downloads/glowfly.msi: Access is denied. (os error 5)",
            ],
        ),
        (
            E::ReadTooLong { path: "/feed/release.json".into(), name: "release.json".into() },
            [
                "Couldn't read /feed/release.json: release.json is larger than expected",
                "/feed/release.json konnte nicht gelesen werden: release.json ist größer als erwartet",
                "No se pudo leer /feed/release.json: release.json es más grande de lo esperado",
            ],
        ),
        (
            E::NoOwnProgram { said: SAID.into() },
            [
                "Couldn't find Glowfly's own program: Access is denied. (os error 5)",
                "Glowflys eigenes Programm wurde nicht gefunden: Access is denied. (os error 5)",
                "No se encontró el programa de Glowfly: Access is denied. (os error 5)",
            ],
        ),
        (
            E::NoDownload,
            [
                "This release has no download for this system.",
                "Diese Version hat keinen Download für dieses System.",
                "Esta versión no tiene descarga para este sistema.",
            ],
        ),
        (E::NoDownloadsFolder, ["No folder to download into", "Kein Ordner für den Download", "No hay ninguna carpeta donde descargar"]),
        (
            E::NoChecksum { name: "glowfly.msi".into() },
            [
                "This release doesn't publish a checksum for glowfly.msi, so it wasn't downloaded.",
                "Diese Version veröffentlicht keine Prüfsumme für glowfly.msi, deshalb wurde die Datei nicht heruntergeladen.",
                "Esta versión no publica una suma de comprobación para glowfly.msi, así que no se descargó.",
            ],
        ),
        (
            E::Checksums(Box::new(E::Service(ServiceError::Http { status: 404, message: "Not Found".into() }))),
            [
                "Couldn't read the checksums: HTTP 404: Not Found",
                "Die Prüfsummen konnten nicht gelesen werden: HTTP 404: Not Found",
                "No se pudieron leer las sumas de comprobación: HTTP 404: Not Found",
            ],
        ),
        (
            E::TooLarge { name: "glowfly.msi".into(), mb: 512 },
            [
                "glowfly.msi is larger than 512 MB, so it wasn't downloaded.",
                "glowfly.msi ist größer als 512 MB, deshalb wurde die Datei nicht heruntergeladen.",
                "glowfly.msi ocupa más de 512 MB, así que no se descargó.",
            ],
        ),
        (
            E::CreateFailed { path: "/downloads".into(), said: SAID.into() },
            [
                "Couldn't create /downloads: Access is denied. (os error 5)",
                "/downloads konnte nicht erstellt werden: Access is denied. (os error 5)",
                "No se pudo crear /downloads: Access is denied. (os error 5)",
            ],
        ),
        (
            late,
            [
                "Couldn't download glowfly.msi: glowfly.msi is larger than expected",
                "glowfly.msi konnte nicht heruntergeladen werden: glowfly.msi ist größer als erwartet",
                "No se pudo descargar glowfly.msi: glowfly.msi es más grande de lo esperado",
            ],
        ),
        (
            E::BadFileName { name: "../x".into() },
            [
                "../x isn't a file name Glowfly reads",
                "../x ist kein Dateiname, den Glowfly liest",
                "../x no es un nombre de archivo que Glowfly lea",
            ],
        ),
        (
            E::ChecksumMismatch,
            [
                "The download didn't match its published checksum and was discarded.",
                "Der Download stimmte nicht mit seiner veröffentlichten Prüfsumme überein und wurde verworfen.",
                "La descarga no coincidía con su suma de comprobación publicada y se descartó.",
            ],
        ),
        (
            E::SaveFailed { path: FILE.into(), said: SAID.into() },
            [
                "Couldn't save /downloads/glowfly.msi: Access is denied. (os error 5)",
                "/downloads/glowfly.msi konnte nicht gespeichert werden: Access is denied. (os error 5)",
                "No se pudo guardar /downloads/glowfly.msi: Access is denied. (os error 5)",
            ],
        ),
        (
            E::WrongSize { name: "glowfly.msi".into(), got: 99, want: 100 },
            [
                "The download of glowfly.msi was 99 bytes instead of 100, so it was discarded.",
                "Der Download von glowfly.msi war 99 statt 100 Bytes groß und wurde deshalb verworfen.",
                "La descarga de glowfly.msi tenía 99 bytes en lugar de 100, así que se descartó.",
            ],
        ),
        (E::Stopped, ["The download was stopped.", "Der Download wurde angehalten.", "La descarga se detuvo."]),
        (
            E::ChangedSinceChecked,
            [
                "The downloaded file changed after it was checked, so it wasn't installed. Download it again.",
                "Die heruntergeladene Datei hat sich nach der Prüfung geändert und wurde deshalb nicht installiert. Lade sie erneut herunter.",
                "El archivo descargado cambió después de comprobarse, así que no se instaló. Vuelve a descargarlo.",
            ],
        ),
        (
            E::OpenFailed { path: "/downloads/glowfly.dmg".into(), said: SAID.into(), install_with: None },
            [
                "Couldn't open /downloads/glowfly.dmg: Access is denied. (os error 5)",
                "/downloads/glowfly.dmg konnte nicht geöffnet werden: Access is denied. (os error 5)",
                "No se pudo abrir /downloads/glowfly.dmg: Access is denied. (os error 5)",
            ],
        ),
        (
            E::OpenFailed { path: "/downloads/glowfly.deb".into(), said: "no opener".into(), install_with: Some("apt".into()) },
            [
                "Couldn't open /downloads/glowfly.deb: no opener\nInstall it with: sudo apt install /downloads/glowfly.deb",
                "/downloads/glowfly.deb konnte nicht geöffnet werden: no opener\nInstalliere es mit: sudo apt install /downloads/glowfly.deb",
                "No se pudo abrir /downloads/glowfly.deb: no opener\nInstálalo con: sudo apt install /downloads/glowfly.deb",
            ],
        ),
        (
            E::NotInPlace,
            [
                "This update can't be installed in place.",
                "Dieses Update kann nicht direkt installiert werden.",
                "Esta actualización no se puede instalar en su sitio.",
            ],
        ),
        (
            E::NotRunningFromAppImage,
            [
                "This copy isn't running from an AppImage.",
                "Diese Kopie läuft nicht aus einem AppImage.",
                "Esta copia no se ejecuta desde un AppImage.",
            ],
        ),
        (E::NotAnAppImage, ["The download isn't an AppImage.", "Der Download ist kein AppImage.", "La descarga no es un AppImage."]),
        (
            E::ArchiveUnreadable { file: "glowfly.zip".into(), said: "invalid Zip archive".into() },
            [
                "Couldn't read the archive glowfly.zip: invalid Zip archive",
                "Das Archiv glowfly.zip konnte nicht gelesen werden: invalid Zip archive",
                "No se pudo leer el archivo comprimido glowfly.zip: invalid Zip archive",
            ],
        ),
        (
            E::NotAnArchive { file: "glowfly.msi".into() },
            [
                "Couldn't read the archive glowfly.msi: it isn't a zip or tar.gz file",
                "Das Archiv glowfly.msi konnte nicht gelesen werden: Es ist keine ZIP- oder tar.gz-Datei",
                "No se pudo leer el archivo comprimido glowfly.msi: no es un archivo zip ni tar.gz",
            ],
        ),
        (
            E::ArchiveTooLarge { mb: 1024 },
            [
                "The archive is larger than 1024 MB when unpacked.",
                "Das Archiv ist entpackt größer als 1024 MB.",
                "El archivo comprimido ocupa más de 1024 MB al descomprimirlo.",
            ],
        ),
        (
            E::TwoCopies { name: "glowfly.exe".into() },
            [
                "The archive has two copies of glowfly.exe.",
                "Das Archiv enthält glowfly.exe zweimal.",
                "El archivo comprimido contiene dos copias de glowfly.exe.",
            ],
        ),
        (
            E::NotRegular { name: "glowfly".into() },
            [
                "glowfly in the archive isn't a regular file.",
                "glowfly im Archiv ist keine normale Datei.",
                "glowfly en el archivo comprimido no es un archivo normal.",
            ],
        ),
        (
            E::NoProgram { program: "glowfly.exe".into() },
            ["The archive has no glowfly.exe.", "Das Archiv enthält kein glowfly.exe.", "El archivo comprimido no contiene glowfly.exe."],
        ),
        (
            write.clone(),
            [
                "Couldn't write /tmp/glowfly-update-1/plan.json: Access is denied. (os error 5)",
                "/tmp/glowfly-update-1/plan.json konnte nicht geschrieben werden: Access is denied. (os error 5)",
                "No se pudo escribir /tmp/glowfly-update-1/plan.json: Access is denied. (os error 5)",
            ],
        ),
        (
            E::RemoveFailed { path: "/tmp/glowfly-update-1".into(), said: SAID.into() },
            [
                "Couldn't remove /tmp/glowfly-update-1: Access is denied. (os error 5)",
                "/tmp/glowfly-update-1 konnte nicht entfernt werden: Access is denied. (os error 5)",
                "No se pudo eliminar /tmp/glowfly-update-1: Access is denied. (os error 5)",
            ],
        ),
        (
            E::NoHelperPlace { tried: vec![("/tmp".into(), "noexec".into()), ("/opt/glowfly".into(), SAID.into())] },
            [
                "Couldn't find a folder the update helper can run from (/tmp: noexec; /opt/glowfly: Access is denied. (os error 5)).",
                "Es wurde kein Ordner gefunden, aus dem der Update-Helfer laufen kann (/tmp: noexec; /opt/glowfly: Access is denied. (os error 5)).",
                "No se encontró ninguna carpeta desde la que pueda ejecutarse el asistente de actualización (/tmp: noexec; /opt/glowfly: Access is denied. (os error 5)).",
            ],
        ),
        (
            E::ReplaceFailed { folder: "/opt/glowfly".into(), said: "busy".into(), left: vec!["/opt/glowfly/glowfly.bak".into()] },
            [
                "Couldn't replace Glowfly's program in /opt/glowfly: busy. Use Show file to install the update by hand. The old file is at /opt/glowfly/glowfly.bak.",
                "Glowflys Programm in /opt/glowfly konnte nicht ersetzt werden: busy. Installiere das Update über „Datei zeigen“ von Hand. Die alte Datei liegt unter /opt/glowfly/glowfly.bak.",
                "No se pudo reemplazar el programa de Glowfly en /opt/glowfly: busy. Usa «Mostrar archivo» para instalar la actualización a mano. El archivo antiguo está en /opt/glowfly/glowfly.bak.",
            ],
        ),
        (rollback.clone(), ["/opt/glowfly/glowfly.bak: Access is denied. (os error 5)"; 3]),
        (
            E::NotUndone { error: Box::new(write), undo: Box::new(rollback) },
            [
                "Couldn't write /tmp/glowfly-update-1/plan.json: Access is denied. (os error 5) /opt/glowfly/glowfly.bak: Access is denied. (os error 5)",
                "/tmp/glowfly-update-1/plan.json konnte nicht geschrieben werden: Access is denied. (os error 5) /opt/glowfly/glowfly.bak: Access is denied. (os error 5)",
                "No se pudo escribir /tmp/glowfly-update-1/plan.json: Access is denied. (os error 5) /opt/glowfly/glowfly.bak: Access is denied. (os error 5)",
            ],
        ),
        (
            E::HelperNotStarted { said: SAID.into() },
            [
                "Couldn't start the update helper: Access is denied. (os error 5)",
                "Der Update-Helfer konnte nicht gestartet werden: Access is denied. (os error 5)",
                "No se pudo iniciar el asistente de actualización: Access is denied. (os error 5)",
            ],
        ),
        (
            E::HelperNoInput,
            [
                "Couldn't start the update helper: it has no input",
                "Der Update-Helfer konnte nicht gestartet werden: Er hat keine Eingabe",
                "No se pudo iniciar el asistente de actualización: no tiene entrada",
            ],
        ),
        (
            E::Internal { effect: "update-restart".into(), panic: "boom".into() },
            [
                "Glowfly hit an internal error (update-restart): boom",
                "In Glowfly ist ein interner Fehler aufgetreten (update-restart): boom",
                "Glowfly tuvo un error interno (update-restart): boom",
            ],
        ),
        (E::Said { said: "GitHub's rate limit was reached".into() }, ["GitHub's rate limit was reached"; 3]),
    ]
}

fn in_lang(code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st
}

fn send(st: &mut AppState, msg: UpdateMsg) {
    update(st, Msg::Updates(msg), Instant::now());
}

/// Glowfly 0.3.0's Windows installer, on offer.
fn the_offer() -> Box<Offer> {
    let asset = Asset {
        name: "glowfly.msi".into(),
        size: 100,
        url: "https://github.com/bg9m9r/glowfly/releases/download/v0.3.0/glowfly.msi".into(),
    };
    let release = Release {
        tag: "v0.3.0".into(),
        name: "v0.3.0".into(),
        html_url: "https://github.com/bg9m9r/glowfly/releases/tag/v0.3.0".into(),
        body: String::new(),
        published_at: None,
        assets: vec![asset.clone()],
    };
    let version = Version::parse("0.3.0").unwrap();
    Box::new(Offer { release, version, asset: Some(asset), sums: SumsSource::None, install: InstallKind::Installer })
}

/// Whether the last toast and the activity log say `want` as a failure.
fn said(st: &AppState, want: &str) -> bool {
    st.toasts.last().is_some_and(|t| !t.ok && t.text == want) && st.log.iter().any(|l| !l.ok && l.text == want)
}

#[test]
fn every_variant_reads_in_english_as_it_did() {
    for (e, words) in variants() {
        assert_eq!(e.to_string(), words[0], "{e:?}");
        assert_eq!(typed::update(Lang::En, &e), words[0], "{e:?}");
    }
}

#[test]
fn a_failed_check_says_why_in_each_language() {
    let heads = ["Couldn't check for updates: ", "Die Suche nach Updates ist fehlgeschlagen: ", "No se pudo buscar actualizaciones: "];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = in_lang(code);
            send(&mut st, UpdateMsg::CheckNow);
            send(&mut st, UpdateMsg::Checked { manual: true, result: Err(e.clone()) });
            let want = format!("{}{}", heads[i], words[i]);
            assert!(said(&st, &want), "{code} {e:?}: {:?}", st.toasts.last().map(|t| &t.text));
            assert_eq!(st.updates.status, UpdateStatus::CheckFailed(words[i].to_owned()), "{code} {e:?}: Preferences says why");
        }
    }
}

#[test]
fn a_failed_download_says_why_in_each_language() {
    let heads = ["The download failed: ", "Der Download ist fehlgeschlagen: ", "La descarga ha fallado: "];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = in_lang(code);
            st.updates.status = UpdateStatus::Downloading(the_offer());
            send(&mut st, UpdateMsg::Downloaded(Err(e.clone())));
            let want = format!("{}{}", heads[i], words[i]);
            assert!(said(&st, &want), "{code} {e:?}: {:?}", st.toasts.last().map(|t| &t.text));
            let failed = UpdateStatus::DownloadFailed { offer: the_offer(), error: words[i].to_owned() };
            assert_eq!(st.updates.status, failed, "{code} {e:?}: Download again is offered");
        }
    }
}

#[test]
fn a_failed_restart_says_why_in_each_language() {
    let heads = ["Couldn't install the update: ", "Das Update konnte nicht installiert werden: ", "No se pudo instalar la actualización: "];
    for (e, words) in variants() {
        for (i, code) in LANGS.into_iter().enumerate() {
            let mut st = in_lang(code);
            let file = PathBuf::from(FILE);
            st.updates.status = UpdateStatus::Downloaded { offer: the_offer(), file, sha256: [7; 32], method: InstallMethod::RunMsi };
            st.updates.restarting = true;
            send(&mut st, UpdateMsg::RestartReady(Err(e.clone())));
            let want = format!("{}{}", heads[i], words[i]);
            assert!(said(&st, &want), "{code} {e:?}: {:?}", st.toasts.last().map(|t| &t.text));
            assert!(!st.updates.restarting && matches!(st.updates.status, UpdateStatus::Downloaded { .. }), "{code}: nothing changed");
        }
    }
}
