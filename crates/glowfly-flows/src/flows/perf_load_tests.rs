//! Timings of one whole `load` and its spans (`load.refs.*`, `load.starts`, ...), against a
//! repository named by `GLOWFLY_PERF_REPO`. Ignored; run by hand in release:
//! `GLOWFLY_PERF_REPO=<repo> cargo test -p glowfly-flows --release perf_load -- --ignored --nocapture`.
//! Each span is logged when it closes. The first run warms the OS cache: read runs 2 and 3.

use glowfly_core::repo::{GraphCache, LoadOptions, load_cached};
use std::time::Instant;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt::format::FmtSpan;
use tracing_subscriber::prelude::*;

#[test]
#[ignore]
fn perf_load() {
    let dir = std::env::var("GLOWFLY_PERF_REPO").expect("set GLOWFLY_PERF_REPO to a large repository");
    let layer = tracing_subscriber::fmt::layer().with_ansi(false).with_span_events(FmtSpan::CLOSE).with_test_writer();
    let _ =
        tracing_subscriber::registry().with(layer.with_filter(Targets::new().with_target("glowfly_core", LevelFilter::DEBUG))).try_init();
    let opts = LoadOptions::all(glowfly_core::settings::Settings::default().commit_limit());
    let cache = GraphCache::default();
    for run in 1..=3 {
        println!("--- run {run}{}", if run == 1 { " (warms the OS cache)" } else { "" });
        let t = Instant::now();
        let snap = load_cached(std::path::Path::new(&dir), &opts, &cache).unwrap();
        println!(
            "run {run}: load of {} commits, {} branches, {} tags in {:?}",
            snap.commits.len(),
            snap.locals.len(),
            snap.tags.len(),
            t.elapsed()
        );
    }
}
