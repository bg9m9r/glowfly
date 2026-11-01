//! The rules `layering.rs` checks: the crates that must stay pure, the families of outside work
//! they may not reach, the functions of core's I/O modules that are pure anyway and the justified
//! exceptions.

/// The crates that must do no I/O, under `crates/`, each with the files under its `src/` that
/// are skipped: the model's `testing.rs`, the fixture helpers other crates' tests use.
pub const CHECKED: &[(&str, &[&str])] = &[("glowfly-ui", &[]), ("glowfly-model", &["testing.rs"])];

const FILES: &str = "the file system";
const CLOCK: &str = "the clock (update gets `now`; views read `st.view.now`)";

/// Families of outside work, as (pattern, what it reaches). A pattern that starts with a letter
/// matches only at the start of a word, so `fs::` doesn't match `prefs::`.
pub const DENIED: &[(&str, &str)] = &[
    ("std::fs", FILES),
    ("fs::", FILES),
    ("File::", FILES),
    ("OpenOptions", FILES),
    (".is_dir()", FILES),
    (".is_file()", FILES),
    (".is_symlink()", FILES),
    (".exists()", FILES),
    (".try_exists(", FILES),
    (".metadata(", FILES),
    ("symlink_metadata", FILES),
    ("canonicalize", FILES),
    ("read_dir", FILES),
    ("read_link", FILES),
    ("remove_file", FILES),
    ("remove_dir", FILES),
    ("create_dir", FILES),
    ("trash::", FILES),
    ("std::process", "other processes"),
    ("Command::new", "other processes"),
    ("portable_pty", "other processes"),
    ("std::net", "the network"),
    ("TcpStream", "the network"),
    ("TcpListener", "the network"),
    ("UdpSocket", "the network"),
    ("ureq", "the network"),
    ("keyring::", "the credential store"),
    ("notify_rust", "desktop notifications"),
    ("open::", "other programs (opening files and links)"),
    ("Repository::", "the repository"),
    ("std::thread", "threads"),
    ("thread::spawn", "threads"),
    ("thread::Builder", "threads"),
    ("thread::sleep", "threads"),
    ("std::env", "the environment"),
    ("env::var", "the environment"),
    ("env::current_dir", "the environment"),
    ("Instant::now", CLOCK),
    ("SystemTime::now", CLOCK),
    ("Local::now", CLOCK),
    ("Utc::now", CLOCK),
    ("Settings::load", "the settings file"),
    ("themes::load_all", "the theme files"),
    ("themes::load_dir", "the theme files"),
    ("themes::create", "the theme files"),
    (".save()", "the settings file"),
    ("send_viewport_cmd", "the window (views push AppMsg::Window; the shell sends viewport commands)"),
    ("platform::native_window", "the OS window (only the shell talks to it)"),
    ("platform::native_menu", "the OS menu bar (only the shell talks to it)"),
    ("platform::primary_selection", "the PRIMARY selection (only the shell talks to it)"),
    ("windows::Win32", "the OS (Win32 calls belong in platform/)"),
    ("git_program::set(", "process-wide state (the git program every later call runs); only the shell and runner set it"),
    ("gitcli::observe_runs(", "process-wide state (the observer of every git run); only the shell and runner set it"),
    ("ActivityFile::open", "core I/O"),
    ("FolderId::of", "core I/O"),
    ("CommonDir::at", "core I/O"),
];

/// Text a `DENIED` pattern may start without reaching anything: compile-time constants.
pub const NOT_DENIED: &[&str] = &["std::env::consts", "env::consts"];

/// `git2::` items that are plain data and may be used anywhere.
pub const GIT2_TYPES: &[&str] = &["Oid", "RepositoryState"];

/// `git2::` associated functions that only check or build plain values, no repository.
pub const GIT2_PURE_CALLS: &[&str] = &[
    "Reference::is_valid_name",
    "Remote::is_valid_name",
    "Branch::name_is_valid",
    "Config::parse_bool",
    "Signature::new",
    "Time::new",
    "Error::new",
];

/// Functions of core modules that do I/O (`core_io.rs`) which are pure themselves, as their path
/// under core. Every other function of such a module counts as I/O.
pub const PURE_FUNCTIONS: &[&str] = &[];

/// Justified exceptions, as (file under `crates/`, pattern as reported, reason).
pub const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "glowfly-model/src/state/updates.rs",
        "app_update::current_platform",
        "the default update state asks the processor once (cached, no disk), so an emulated build is offered the Arm file",
    ),
    (
        "glowfly-model/src/state/app_queries.rs",
        "std::env",
        "`outside_dir` (and `any_dir` without a repository) is the temp folder's path; nothing is read",
    ),
    (
        "glowfly-model/src/state/trust.rs",
        "trust::scan_at",
        "`record_as_found` is a test-support helper (`cfg(any(test, feature = \"test-support\"))`), built only for tests",
    ),
    (
        "glowfly-ui/src/prefs/language.rs",
        "dates::system_date_locale",
        "names the system date locale in the picker: an environment read, no disk; found when the core rule was derived (#365)",
    ),
    (
        "glowfly-ui/src/prefs/language.rs",
        "dates::format",
        "previews a date pattern, in the system locale when that is chosen (an environment read); found in #365",
    ),
    (
        "glowfly-ui/src/repo_menu_list.rs",
        "native_path::name_and_parent",
        "shows the home folder as `~`, read from HOME (an environment read, no disk); found in #365",
    ),
];
