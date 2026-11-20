//! Timings of reading a very large work tree's status and line counts (a load, a status read,
//! the line counts of a status read's paths and of the whole tree), against the repository named
//! by `GLOWFLY_PERF_REPO` (`wide-tree` from `scripts/make-perf-repos.ps1`, docs/PERF_REPOS.md).
//! Ignored; run by hand in release, once to warm the file system cache and the pooled handles
//! and once more for the timings that count. Each step runs twice, cold then warm, and the
//! spans of glowfly-core close in the log (`RUST_LOG` narrows them; the default is
//! `glowfly_core=debug`):
//! `GLOWFLY_PERF_REPO=<out>/wide-tree cargo test -p glowfly-flows --release perf_status -- --ignored --nocapture`.

use glowfly_core::repo::{LoadOptions, WipPaths, load, status_and_index_with, wip_line_counts, wip_line_counts_of};
use std::path::Path;
use std::time::Instant;
use tracing_subscriber::fmt::format::FmtSpan;

fn log_spans() {
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "glowfly_core=debug".to_owned());
    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_span_events(FmtSpan::CLOSE).with_test_writer().try_init();
}

/// Runs `f` twice and prints each wall time.
fn twice<T>(what: &str, mut f: impl FnMut() -> T) -> T {
    let mut last = None;
    for run in ["cold", "warm"] {
        println!("--- {what} ({run})");
        let t = Instant::now();
        last = Some(f());
        println!("=== {what} ({run}): {:?}", t.elapsed());
    }
    last.expect("two runs")
}

#[test]
#[ignore]
fn perf_status() {
    log_spans();
    let dir = std::env::var("GLOWFLY_PERF_REPO").expect("set GLOWFLY_PERF_REPO to a large repository");
    let dir = Path::new(&dir);

    let snap = twice("load", || load(dir, &LoadOptions::all(2_000)).unwrap());
    println!("load: {} commits, {} unstaged and {} staged rows", snap.commits.len(), snap.unstaged.len(), snap.staged.len());

    let (unstaged, staged, ..) = twice("status", || status_and_index_with(dir, true, None).unwrap());
    println!("status: {} unstaged and {} staged rows", unstaged.len(), staged.len());

    let paths = WipPaths::of(&snap.staged, &snap.unstaged);
    println!("scoped to {paths:?}");
    let scoped = twice("scoped line counts", || wip_line_counts_of(dir, &paths).unwrap());
    println!("scoped: {} staged and {} unstaged files counted", scoped.staged.files.len(), scoped.unstaged.files.len());

    let whole = twice("whole-tree line counts", || wip_line_counts(dir).unwrap());
    println!("whole tree: {} staged and {} unstaged files counted", whole.staged.files.len(), whole.unstaged.files.len());
}
