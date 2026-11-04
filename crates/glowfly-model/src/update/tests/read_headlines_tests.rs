//! What the graph's and the diff's failed reads show (#367): each result with an error, in
//! English, German and Spanish, as the activity log, the toast and the panels show it; English
//! word for word as before, and a parked tab's failure in that tab's log.

use super::read_errors_tests::GIT_SAID;
use super::tests::{git, scratch_repo, state};
use super::update;
use crate::effect::ConflictRead;
use crate::msg::{ConflictMsg, DiffMsg, HistoryMsg, Msg, SearchMsg, SelectionMsg, TabsMsg};
use crate::state::conflict::ConflictView;
use crate::state::diff_preview::DiffPreview;
use crate::state::history::FileHistoryView;
use crate::state::history_index::Building;
use crate::state::selection::MultiSelection;
use crate::state::selection_wip::WorkdirCompare;
use crate::state::{AppState, Selection, Tab};
use git2::Oid;
use glowfly_core::commit_index::IndexKey;
use glowfly_core::error::ReadFailed as R;
use glowfly_core::repo::{self, DiffTarget, LoadDiffOptions, LoadOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

const LANGS: [&str; 3] = ["en", "de", "es"];

/// A repository with four commits, newest first in the graph, and a.txt changed.
fn four_commits(name: &str) -> PathBuf {
    let dir = scratch_repo(name);
    for n in 2..=4 {
        std::fs::write(dir.join("b.txt"), format!("{n}\n")).unwrap();
        git(&dir, &["add", "b.txt"]);
        git(&dir, &["commit", "-q", "-m", &format!("commit {n}")]);
    }
    dir
}

/// A state showing `dir` in `code`'s language.
fn shown(dir: &Path, code: &str) -> AppState {
    let mut st = state();
    st.settings.language = code.into();
    st.tabs[0] = Tab::unloaded(dir.to_path_buf());
    st.snap = Some(repo::load(dir, &LoadOptions::all(100)).unwrap());
    st
}

/// The graph's commits, newest first.
fn commits(st: &AppState) -> Vec<Oid> {
    st.snap.as_ref().unwrap().commits.iter().map(|c| c.oid).collect()
}

/// a.txt's working-tree diff, shown.
fn diff_shown(st: &mut AppState, dir: &Path) -> (DiffTarget, u64) {
    let target = DiffTarget::Unstaged("a.txt".into());
    let diff = repo::load_diff_with(dir, &target, LoadDiffOptions::default()).unwrap();
    st.session.diff.view = Some((target.clone(), Arc::new(diff)));
    (target, st.session.diff.shown_gen)
}

fn send(st: &mut AppState, msg: Msg) {
    update(st, msg, Instant::now());
}

fn last_log(st: &AppState) -> (bool, String) {
    st.log.last().map(|l| (l.ok, l.text.clone())).expect("logged")
}

/// `heads[i]` (a headline ending in ": ") followed by `words[i]`, for each language.
fn headed(heads: [&str; 3], words: [&str; 3]) -> [String; 3] {
    [0, 1, 2].map(|i| format!("{}{}", heads[i], words[i]))
}

const MISSING: [&str; 3] = [
    "the contents of a.txt haven't been downloaded from origin yet",
    "der Inhalt von a.txt wurde noch nicht von origin heruntergeladen",
    "el contenido de a.txt aún no se ha descargado de origin",
];

fn missing() -> R {
    R::MissingContent { path: "a.txt".into(), remote: "origin".into() }
}

#[test]
fn a_failed_path_search_is_logged_in_each_language() {
    let dir = scratch_repo("read-heads-paths");
    let want = headed(
        [
            "Could not search changed files: ",
            "Die geänderten Dateien konnten nicht durchsucht werden: ",
            "No se pudieron buscar los archivos cambiados: ",
        ],
        [
            "Glowfly hit an internal error (search-paths)",
            "In Glowfly ist ein interner Fehler aufgetreten (search-paths)",
            "Glowfly tuvo un error interno (search-paths)",
        ],
    );
    assert_eq!(want[0], "Could not search changed files: Glowfly hit an internal error (search-paths)");
    for (code, want) in LANGS.into_iter().zip(want) {
        let mut st = shown(&dir, code);
        st.session.search.paths.pending = Some(7);
        let result = Err(R::Internal { effect: "search-paths".into() });
        send(&mut st, Msg::Search(SearchMsg::PathsSearched { request: 7, result }));
        assert_eq!(last_log(&st), (false, want), "{code}");
        assert_eq!(st.session.search.paths.pending, None, "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

fn building(build: u64) -> Building {
    Building { build, key: IndexKey { starts: Vec::new(), shallow: Vec::new() }, indexed: 0, estimate: None, fresh: true }
}

const INDEX_HEADS: [&str; 3] = [
    "Could not index the history for search: ",
    "Der Verlauf konnte nicht für die Suche indiziert werden: ",
    "No se pudo indexar el historial para la búsqueda: ",
];

#[test]
fn a_failed_index_build_and_query_are_logged_in_each_language() {
    let dir = scratch_repo("read-heads-index");
    let built = headed(INDEX_HEADS, [GIT_SAID; 3]);
    assert_eq!(built[0], format!("Could not index the history for search: {GIT_SAID}"));
    let queried = headed(
        [
            "Could not search the history index: ",
            "Der Verlaufsindex konnte nicht durchsucht werden: ",
            "No se pudo buscar en el índice del historial: ",
        ],
        [
            "could not start a thread: out of memory",
            "konnte keinen Thread starten: out of memory",
            "no se pudo iniciar un hilo: out of memory",
        ],
    );
    assert_eq!(queried[0], "Could not search the history index: could not start a thread: out of memory");
    for (i, code) in LANGS.into_iter().enumerate() {
        let mut st = shown(&dir, code);
        st.session.search.index.building = Some(building(3));
        send(&mut st, Msg::Search(SearchMsg::IndexBuilt { build: 3, result: Err(R::Said { said: GIT_SAID.into() }) }));
        assert_eq!(last_log(&st), (false, built[i].clone()), "{code}");

        st.session.search.index.pending = Some(5);
        let result = Err(R::NoThread { said: "out of memory".into() });
        send(&mut st, Msg::Search(SearchMsg::IndexQueried { request: 5, result }));
        assert_eq!(last_log(&st), (false, queried[i].clone()), "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn unchanged_lines_that_cant_be_read_are_logged_in_each_language() {
    let dir = scratch_repo("read-heads-gaps");
    let want = headed(
        [
            "These unchanged lines can't be shown: ",
            "Diese unveränderten Zeilen können nicht angezeigt werden: ",
            "No se pueden mostrar estas líneas sin cambios: ",
        ],
        ["not found: the file changed", "nicht gefunden: die Datei hat sich geändert", "no encontrado: el archivo cambió"],
    );
    assert_eq!(want[0], "These unchanged lines can't be shown: not found: the file changed");
    for (code, want) in LANGS.into_iter().zip(want) {
        let mut st = shown(&dir, code);
        let (target, generation) = diff_shown(&mut st, &dir);
        send(&mut st, Msg::Diff(DiffMsg::GapsLoaded { target, generation, result: Err(R::FileChanged) }));
        assert_eq!(last_log(&st), (false, want), "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_stale_hunk_copy_is_toasted_in_each_language() {
    let want = headed(
        [
            "Couldn't copy the hunk as a patch: ",
            "Der Hunk konnte nicht als Patch kopiert werden: ",
            "No se pudo copiar el bloque como parche: ",
        ],
        [
            "not found: the diff changed; try again on the updated diff",
            "nicht gefunden: der Diff hat sich geändert; versuche es mit dem aktualisierten Diff erneut",
            "no encontrado: el diff cambió; inténtalo de nuevo con el diff actualizado",
        ],
    );
    assert_eq!(want[0], "Couldn't copy the hunk as a patch: not found: the diff changed; try again on the updated diff");
    for (code, want) in LANGS.into_iter().zip(want) {
        let mut st = state();
        st.settings.language = code.into();
        send(&mut st, Msg::Diff(DiffMsg::HunkPatchRead { result: Err(R::DiffChanged) }));
        assert!(st.toasts.last().is_some_and(|t| !t.ok && t.text == want), "{code}: {:?}", st.toasts.last().map(|t| &t.text));
    }
}

#[test]
fn binary_sides_that_cant_be_read_say_why_in_each_language() {
    let dir = scratch_repo("read-heads-binary");
    for (code, want) in LANGS.into_iter().zip(MISSING) {
        let mut st = shown(&dir, code);
        let (target, generation) = diff_shown(&mut st, &dir);
        st.session.diff_preview = Some(DiffPreview { target: target.clone(), generation, sides: None });
        send(&mut st, Msg::Diff(DiffMsg::BinarySidesLoaded { target, generation, result: Err(missing()) }));
        let sides = st.session.diff_preview.as_ref().and_then(|p| p.sides.clone());
        assert_eq!(sides.map(|s| s.err()), Some(Some(want.to_owned())), "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_file_history_says_why_in_each_language() {
    let dir = scratch_repo("read-heads-history");
    let heads = [
        "Could not load the history of a.txt: ",
        "Der Verlauf von a.txt konnte nicht geladen werden: ",
        "No se pudo cargar el historial de a.txt: ",
    ];
    let want = headed(heads, MISSING);
    assert_eq!(want[0], "Could not load the history of a.txt: the contents of a.txt haven't been downloaded from origin yet");
    for (i, code) in LANGS.into_iter().enumerate() {
        let mut st = shown(&dir, code);
        st.session.history.view = Some(FileHistoryView::loading("a.txt".into(), None));
        let generation = st.session.history.generation;
        send(&mut st, Msg::History(HistoryMsg::FileHistoryLoaded { path: "a.txt".into(), generation, result: Err(missing()) }));
        let view = st.session.history.view.as_ref().expect("the history stays open");
        assert_eq!(view.error.as_deref(), Some(MISSING[i]), "{code}");
        assert!(!view.loading, "{code}");
        assert_eq!(last_log(&st), (false, want[i].clone()), "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

const CONFLICT_HEADS: [&str; 3] = [
    "Could not read the conflicted files: ",
    "Die Dateien mit Konflikten konnten nicht gelesen werden: ",
    "No se pudieron leer los archivos con conflictos: ",
];

#[test]
fn conflicted_files_that_cant_be_read_are_logged_in_each_language() {
    let dir = scratch_repo("read-heads-conflict-files");
    let want = headed(CONFLICT_HEADS, [GIT_SAID; 3]);
    assert_eq!(want[0], format!("Could not read the conflicted files: {GIT_SAID}"));
    for (code, want) in LANGS.into_iter().zip(want) {
        for purpose in [ConflictRead::Summary { generation: 0 }, ConflictRead::BeforeMarkResolved] {
            let mut st = shown(&dir, code);
            let purpose = match purpose {
                ConflictRead::Summary { .. } => ConflictRead::Summary { generation: st.session.conflict_files.generation },
                p => p,
            };
            let result = Err(R::Said { said: GIT_SAID.into() });
            send(&mut st, Msg::Conflict(ConflictMsg::ConflictFilesRead { purpose, paths: Vec::new(), result }));
            assert_eq!(last_log(&st), (false, want.clone()), "{code}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_conflict_that_cant_be_opened_is_logged_and_closed_in_each_language() {
    let dir = scratch_repo("read-heads-conflict");
    let words =
        ["not found: f.txt is not conflicted", "nicht gefunden: f.txt hat keinen Konflikt", "no encontrado: f.txt no tiene conflictos"];
    for (code, want) in LANGS.into_iter().zip(words) {
        let mut st = shown(&dir, code);
        st.session.conflict.view = Some(ConflictView::loading("f.txt".into()));
        let generation = st.session.conflict.generation;
        let result = Err(R::NotConflicted { path: "f.txt".into() });
        send(&mut st, Msg::Conflict(ConflictMsg::ConflictLoaded { path: "f.txt".into(), generation, result, saved: None }));
        assert_eq!(last_log(&st), (false, want.to_owned()), "{code}");
        assert!(st.session.conflict.view.is_none(), "{code}: the view closes");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn comparisons_that_cant_be_read_say_why_in_each_language() {
    let dir = four_commits("read-heads-compare");
    for (code, want) in LANGS.into_iter().zip(MISSING) {
        let mut st = shown(&dir, code);
        let c = commits(&st);
        let snap = st.snap.as_ref().unwrap();
        st.session.selection = MultiSelection::build([c[0], c[1]], c[0], snap);
        let Selection::Commits(m) = &st.session.selection else { panic!("two commits") };
        let (from, to) = m.compare_ends().expect("a pair");
        send(&mut st, Msg::Selection(SelectionMsg::CompareLoaded { from, to, result: Err(missing()) }));
        let Selection::Commits(m) = &st.session.selection else { panic!("{code}: still two commits") };
        assert_eq!(m.changes, Some(Err(want.to_owned())), "{code}");
        assert_eq!(last_log(&st), (false, want.to_owned()), "{code}");

        // Three commits off one line: each one's own changes.
        let snap = st.snap.as_ref().unwrap();
        st.session.selection = MultiSelection::build([c[0], c[1], c[3]], c[0], snap);
        let Selection::Commits(m) = &st.session.selection else { panic!("three commits") };
        let oids = m.stack_oids().expect("a stacked list").to_vec();
        send(&mut st, Msg::Selection(SelectionMsg::StackedLoaded { oids, result: Err(missing()) }));
        let Selection::Commits(m) = &st.session.selection else { panic!("{code}: still three commits") };
        assert_eq!(m.stacked, Some(Err(want.to_owned())), "{code}");

        st.session.selection = Selection::VsWorkdir(WorkdirCompare::new(c[1]));
        let msg = SelectionMsg::WorkdirCompareLoaded { oid: c[1], generation: 0, result: Err(missing()) };
        send(&mut st, Msg::Selection(msg));
        let Selection::VsWorkdir(w) = &st.session.selection else { panic!("{code}: still compared") };
        assert_eq!(w.changes, Some(Err(want.to_owned())), "{code}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// `shown`, then parked behind a new empty tab.
fn parked(dir: &Path, code: &str) -> AppState {
    let mut st = shown(dir, code);
    send(&mut st, Msg::Tabs(TabsMsg::NewTab));
    assert_eq!(st.active_tab, 1);
    st
}

fn scoped(dir: &Path, msg: Msg) -> Msg {
    Msg::Scoped { workdir: dir.to_path_buf(), msg: Box::new(msg) }
}

fn parked_log(st: &AppState) -> Vec<&str> {
    st.tabs[0].log.iter().filter(|l| !l.ok).map(|l| l.text.as_str()).collect()
}

#[test]
fn a_parked_tabs_failed_reads_reach_its_own_log_in_its_language() {
    let dir = scratch_repo("read-heads-parked");
    for (i, code) in LANGS.into_iter().enumerate() {
        let mut st = parked(&dir, code);
        let read = ConflictMsg::ConflictFilesRead {
            purpose: ConflictRead::BeforeMarkResolved,
            paths: Vec::new(),
            result: Err(R::Said { said: GIT_SAID.into() }),
        };
        send(&mut st, scoped(&dir, Msg::Conflict(read)));
        assert_eq!(parked_log(&st), [format!("{}{GIT_SAID}", CONFLICT_HEADS[i])], "{code}");

        let p = st.tabs[0].parked.as_mut().expect("parked");
        p.session.search.index.building = Some(building(9));
        let built = SearchMsg::IndexBuilt { build: 9, result: Err(R::Internal { effect: "index".into() }) };
        send(&mut st, scoped(&dir, Msg::Search(built)));
        let words = [
            "Glowfly hit an internal error (index)",
            "In Glowfly ist ein interner Fehler aufgetreten (index)",
            "Glowfly tuvo un error interno (index)",
        ];
        assert_eq!(parked_log(&st).last().copied(), Some(format!("{}{}", INDEX_HEADS[i], words[i]).as_str()), "{code}");
        assert!(st.log.iter().all(|l| l.ok), "{code}: nothing in the new tab's log");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
