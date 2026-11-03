//! The README screenshots' demo repository: `tidewatch`, a small made-up Rust tool with three
//! weeks of history by four made-up people, a merged branch, two tags, an open branch pushed for
//! review and one kept local, a remote, a stash and a few uncommitted changes. Fixed dates and
//! identities keep its commits the same on every run.

use super::demo_files::*;
use glowfly_core::testkit::try_git_env;
use std::path::PathBuf;

type Who = (&'static str, &'static str);

/// The made-up people who work on the demo project.
const MIRA: Who = ("Mira Okafor", "mira@example.com");
const TOMAS: Who = ("Tomas Lindqvist", "tomas@example.com");
const PRIYA: Who = ("Priya Natarajan", "priya@example.com");
const JONAH: Who = ("Jonah Reyes", "jonah@example.com");

/// Where the demo's origin lives (never contacted: the tests make no network calls).
pub(super) const ORIGIN: &str = "https://github.com/tidewatch-example/tidewatch.git";

/// The demo project in its folder.
pub(super) struct Demo {
    pub dir: PathBuf,
}

impl Demo {
    /// Runs git as `who` on `date`.
    pub fn run(&self, who: Who, date: &str, a: &[&str]) {
        let env = [
            ("GIT_AUTHOR_NAME", who.0),
            ("GIT_AUTHOR_EMAIL", who.1),
            ("GIT_COMMITTER_NAME", who.0),
            ("GIT_COMMITTER_EMAIL", who.1),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_DATE", date),
        ];
        let (ok, out) = try_git_env(&self.dir, a, &env);
        assert!(ok, "git {a:?} failed: {out}");
    }

    /// Like `run`, for a command that may fail (a merge that stops on conflicts).
    pub fn try_run(&self, a: &[&str]) -> bool {
        let date = "2026-01-31T10:00:00+00:00";
        let env = [
            ("GIT_AUTHOR_NAME", MIRA.0),
            ("GIT_AUTHOR_EMAIL", MIRA.1),
            ("GIT_COMMITTER_NAME", MIRA.0),
            ("GIT_COMMITTER_EMAIL", MIRA.1),
            ("GIT_AUTHOR_DATE", date),
            ("GIT_COMMITTER_DATE", date),
        ];
        try_git_env(&self.dir, a, &env).0
    }

    /// Writes `files` (path, text) and commits everything as `who` on `date`, with `message`
    /// (a summary, then any description paragraphs).
    fn commit(&self, who: Who, date: &str, files: &[(&str, &str)], message: &[&str]) {
        for (path, text) in files {
            self.write(path, text);
        }
        self.run(who, date, &["add", "-A"]);
        let mut a = vec!["commit", "-q"];
        for m in message {
            a.extend(["-m", m]);
        }
        self.run(who, date, &a);
    }

    pub fn write(&self, path: &str, text: &str) {
        let path = self.dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn checkout(&self, a: &[&str]) {
        let mut args = vec!["checkout", "-q"];
        args.extend_from_slice(a);
        self.run(MIRA, "2026-01-08T09:00:00+00:00", &args);
    }

    fn set_ref(&self, name: &str, to: &str) {
        self.run(MIRA, "2026-01-08T09:00:00+00:00", &["update-ref", name, to]);
    }

    /// Removes the demo's folder.
    pub fn remove(&self) {
        let _ = std::fs::remove_dir_all(self.dir.parent().unwrap());
    }
}

/// The whole demo history in a folder of its own (`name`), on `main` with a clean tree.
pub(super) fn build(name: &str) -> Demo {
    let dir = glowfly_core::paths::temp_dir().join(format!("glowfly-readme-{}-{name}", std::process::id())).join("tidewatch");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let d = Demo { dir };
    d.run(MIRA, "2026-01-08T09:00:00+00:00", &["init", "-q", "-b", "main"]);
    d.run(MIRA, "2026-01-08T09:00:00+00:00", &["config", "commit.gpgsign", "false"]);
    first_release(&d);
    second_release(&d);
    open_work(&d);
    d
}

/// Up to v0.1.0: stations, the API and the table.
fn first_release(d: &Demo) {
    let start = [("Cargo.toml", CARGO_TOML), (".gitignore", GITIGNORE), ("README.md", README), ("src/main.rs", MAIN)];
    d.commit(MIRA, "2026-01-08T09:12:00+00:00", &start, &["Initial commit"]);
    d.commit(
        MIRA,
        "2026-01-09T10:30:00+00:00",
        &[("stations.toml", STATIONS_TOML), ("src/stations.rs", STATIONS), ("src/main.rs", MAIN_STATIONS)],
        &["Read stations from stations.toml", "Two stations to start with; more once the API is in."],
    );
    d.commit(JONAH, "2026-01-09T15:05:00+00:00", &[(".github/workflows/ci.yml", CI)], &["Run tests on Linux, macOS and Windows"]);
    let cargo = format!("{CARGO_TOML}ureq = {{ version = \"2\", features = [\"json\"] }}\n");
    d.commit(
        JONAH,
        "2026-01-12T11:20:00+00:00",
        &[("src/forecast.rs", FORECAST), ("Cargo.toml", &cargo)],
        &["Fetch predictions from the tides API"],
    );
    let spaces = STATIONS.replace("s.id == id", "s.id == id.trim()");
    d.commit(TOMAS, "2026-01-12T16:45:00+00:00", &[("src/stations.rs", &spaces)], &["Fix lookup of station names with spaces"]);
    d.commit(
        MIRA,
        "2026-01-13T09:40:00+00:00",
        &[("src/render.rs", RENDER), ("src/paint.rs", PAINT), ("src/main.rs", &main_rs(false))],
        &["Print tides as a table"],
    );
    d.commit(JONAH, "2026-01-14T14:10:00+00:00", &[("src/main.rs", &main_rs(true))], &["Add a --days option"]);
    d.run(JONAH, "2026-01-14T14:10:00+00:00", &["tag", "v0.1.0"]);
}

/// Up to v0.2.0: CSV export on its own branch, caching and colours on main.
fn second_release(d: &Demo) {
    d.checkout(&["-b", "csv-export"]);
    d.commit(PRIYA, "2026-01-15T10:15:00+00:00", &[("src/csv.rs", CSV)], &["Write the table as CSV"]);
    d.checkout(&["main"]);
    let cache = FORECAST.replace(
        "pub fn fetch(station: &str, days: u32) -> Result<Vec<Tide>, ureq::Error> {\n",
        "pub fn fetch(station: &str, days: u32) -> Result<Vec<Tide>, ureq::Error> {\n    if let Some(tides) = cache::read(station, days) {\n        return Ok(tides);\n    }\n",
    );
    d.commit(
        MIRA,
        "2026-01-15T16:30:00+00:00",
        &[("src/forecast.rs", &cache)],
        &["Cache forecasts for an hour", "Saves a request when you check the same station twice."],
    );
    d.checkout(&["csv-export"]);
    let quoted = CSV.replace("out += &format!(\"{},", "out += &format!(\"\\\"{}\\\",");
    d.commit(PRIYA, "2026-01-16T11:00:00+00:00", &[("src/csv.rs", &quoted)], &["Quote times in CSV output"]);
    d.checkout(&["main"]);
    let render = RENDER
        .replace("    let style = Style::new().dim();\n", "    let c = colour(up);\n    let style = Style::new().fg(c);\n")
        .replace("let h = t.height_m;", "let h = t.height;")
        .replace("/// What follows a row.", "/// Rising or falling, after a row.")
        .replace("if up { \"+\" } else { \"-\" }", "if up { \"rise\" } else { \"fall\" }")
        + "\n/// Cyan while the water rises.\nfn colour(up: bool) -> Color {\n    if up { CYAN } else { BLUE }\n}\n";
    let forecast = cache.replace("height_m", "height");
    d.commit(
        JONAH,
        "2026-01-19T13:25:00+00:00",
        &[("src/render.rs", &render), ("src/paint.rs", PAINT_COLOUR), ("src/forecast.rs", &forecast)],
        &["Colour rising and falling tides"],
    );
    d.run(MIRA, "2026-01-20T10:00:00+00:00", &["merge", "-q", "--no-ff", "csv-export", "-m", "Merge branch 'csv-export'"]);
    d.run(MIRA, "2026-01-20T10:00:00+00:00", &["tag", "v0.2.0"]);
}

/// After v0.2.0: the summary, the moon-phase branch (pushed) and metric-units (local).
fn open_work(d: &Demo) {
    let cargo = format!("{CARGO_TOML}ureq = {{ version = \"2.12\", features = [\"json\"] }}\n");
    d.commit(TOMAS, "2026-01-21T09:50:00+00:00", &[("Cargo.toml", &cargo)], &["Update ureq to 2.12"]);
    d.commit(
        MIRA,
        "2026-01-22T14:35:00+00:00",
        &[("src/summary.rs", SUMMARY)],
        &["Add a daily summary line", "The next high water, above the table."],
    );
    d.checkout(&["-b", "moon-phase"]);
    d.commit(JONAH, "2026-01-23T10:40:00+00:00", &[("src/moon.rs", MOON)], &["Work out the moon's phase"]);
    d.checkout(&["main"]);
    let ours =
        SUMMARY.replace("usize = 10;", "usize = 12;").replace("\"High water {time} {height:.1}m\"", "\"High {time}  {height:.1} m\"");
    d.commit(MIRA, "2026-01-26T11:05:00+00:00", &[("src/summary.rs", &ours)], &["Tidy the daily summary"]);
    d.checkout(&["moon-phase"]);
    let theirs = SUMMARY.replace("usize = 10;", "usize = 14;").replace(
        "    format!(\"High water {time} {height:.1}m\")",
        "    let moon = crate::moon::name(tide.phase);\n    format!(\"High {time}  {height:.1} m  {moon}\")",
    );
    d.commit(JONAH, "2026-01-27T09:15:00+00:00", &[("src/summary.rs", &theirs)], &["Add moon phase to the daily summary"]);
    d.checkout(&["main"]);
    d.commit(PRIYA, "2026-01-27T15:30:00+00:00", &[("README.md", README_CONFIG)], &["Document the config file"]);
    d.checkout(&["moon-phase"]);
    let round = MOON.replace("(days_since_new / MONTH).fract()", "(days_since_new.round() / MONTH).fract()");
    d.commit(JONAH, "2026-01-28T10:20:00+00:00", &[("src/moon.rs", &round)], &["Round the phase to the nearest day"]);
    d.checkout(&["main"]);
    d.run(MIRA, "2026-01-28T12:00:00+00:00", &["remote", "add", "origin", ORIGIN]);
    d.set_ref("refs/remotes/origin/main", "main");
    d.set_ref("refs/remotes/origin/moon-phase", "moon-phase");
    d.run(MIRA, "2026-01-28T12:00:00+00:00", &["config", "branch.main.remote", "origin"]);
    d.run(MIRA, "2026-01-28T12:00:00+00:00", &["config", "branch.main.merge", "refs/heads/main"]);
    d.run(MIRA, "2026-01-28T12:00:00+00:00", &["config", "branch.moon-phase.remote", "origin"]);
    d.run(MIRA, "2026-01-28T12:00:00+00:00", &["config", "branch.moon-phase.merge", "refs/heads/moon-phase"]);
    d.checkout(&["-b", "metric-units"]);
    d.commit(TOMAS, "2026-01-29T09:00:00+00:00", &[("src/units.rs", UNITS)], &["Add a --metric flag"]);
    d.checkout(&["main"]);
    let render = std::fs::read_to_string(d.dir.join("src/render.rs")).unwrap().replace("{h:.1}\"", "{h:>4.1}\"");
    d.commit(MIRA, "2026-01-30T10:10:00+00:00", &[("src/render.rs", &render)], &["Line up the height column"]);
    // A stash, then the changes the overview shows: one staged file, two unstaged.
    d.write("stations.toml", &format!("{STATIONS_TOML}aliases = [\"hp\"]\n"));
    d.run(MIRA, "2026-01-31T09:30:00+00:00", &["stash", "push", "-q", "-m", "WIP: station aliases"]);
}

/// The overview's uncommitted changes: src/stations.rs staged, src/render.rs and README.md not.
pub(super) fn leave_changes(d: &Demo) {
    let stations = std::fs::read_to_string(d.dir.join("src/stations.rs")).unwrap();
    d.write("src/stations.rs", &stations.replace("    pub name: String,\n", "    pub name: String,\n    pub zone: String,\n"));
    d.run(MIRA, "2026-01-31T10:00:00+00:00", &["add", "src/stations.rs"]);
    let render = std::fs::read_to_string(d.dir.join("src/render.rs")).unwrap();
    d.write("src/render.rs", &render.replace("/// Prints one row per tide.", "/// Prints one row per tide, in local time."));
    d.write("README.md", &format!("{README_CONFIG}\nTimes are shown in each station's time zone.\n"));
}

/// main.rs once the tool prints a table, with the --days option when `days`.
fn main_rs(days: bool) -> String {
    let days_line = if days {
        "    let days = env::args().nth(2).and_then(|d| d.parse().ok());\n    let days = days.unwrap_or(1);\n"
    } else {
        "    let days = 1;\n"
    };
    format!(
        "mod forecast;\nmod paint;\nmod render;\nmod stations;\n\nuse std::env;\n\nfn main() {{\n    let id = env::args().nth(1);\n    let id = id.as_deref().unwrap_or(\"harbour-point\");\n{days_line}    let all = stations::load(\"stations.toml\").unwrap();\n    let Some(s) = all.find(id) else {{\n        eprintln!(\"unknown station: {{id}}\");\n        std::process::exit(2);\n    }};\n    match forecast::fetch(&s.id, days) {{\n        Ok(tides) => render::table(&tides),\n        Err(e) => eprintln!(\"no forecast: {{e}}\"),\n    }}\n}}\n"
    )
}
