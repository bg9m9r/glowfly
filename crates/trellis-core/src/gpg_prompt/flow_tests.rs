use super::MAX_TRIES;
use super::args::{self, SignCall};
use super::flow::{AskResult, Asker, Finish, Gpg, GpgRun, sign};
use crate::askpass::{GpgAsk, Secret};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io;

const PAYLOAD: &[u8] = b"tree 4b825dc642cb6eb9a060e54bf8d69288fbee4904\n\nmessage\n";

/// Scripted gpg runs, recording each call's arguments and input.
#[derive(Default)]
struct FakeGpg {
    runs: VecDeque<GpgRun>,
    calls: Vec<(Vec<OsString>, Vec<u8>)>,
}

impl Gpg for FakeGpg {
    fn run(&mut self, args: &[OsString], input: &[u8]) -> io::Result<GpgRun> {
        self.calls.push((args.to_vec(), input.to_vec()));
        self.runs.pop_front().ok_or_else(|| io::Error::other("no more scripted runs"))
    }

    fn describe(&mut self, _key: Option<&str>) -> (String, Option<String>) {
        ("ABCDEF0123456789".into(), Some("Ada Lovelace <ada@example.com>".into()))
    }

    fn program(&self) -> &str {
        "gpg"
    }
}

/// Scripted answers, recording each request.
#[derive(Default)]
struct FakeAsker {
    answers: VecDeque<AskResult>,
    asks: Vec<GpgAsk>,
}

impl Asker for FakeAsker {
    fn ask(&mut self, ask: &GpgAsk) -> AskResult {
        self.asks.push(ask.clone());
        self.answers.pop_front().unwrap_or(AskResult::Unavailable)
    }
}

fn call() -> SignCall {
    args::parse(&["--status-fd=2", "-bsau", "ABCDEF0123456789"].map(OsString::from)).unwrap()
}

fn run(ok: bool, stdout: &str, stderr: &str) -> GpgRun {
    GpgRun { ok, code: Some(if ok { 0 } else { 2 }), stdout: stdout.into(), stderr: stderr.into() }
}

fn created() -> GpgRun {
    run(true, "-----BEGIN PGP SIGNATURE-----\n", "[GNUPG:] BEGIN_SIGNING H10\n[GNUPG:] SIG_CREATED D 1 10 00 1 ABCD\n")
}

fn no_pinentry() -> GpgRun {
    run(false, "", "[GNUPG:] FAILURE sign 67108949\n")
}

fn bad() -> GpgRun {
    run(false, "", "[GNUPG:] BAD_PASSPHRASE ABCDEF0123456789\n[GNUPG:] FAILURE sign 67108875\n")
}

fn answer(s: &str) -> AskResult {
    AskResult::Answer(Secret::new(s))
}

fn scripted(runs: Vec<GpgRun>, answers: Vec<AskResult>) -> (FakeGpg, FakeAsker) {
    (FakeGpg { runs: runs.into(), ..FakeGpg::default() }, FakeAsker { answers: answers.into(), ..FakeAsker::default() })
}

fn with_payload(prefix: &[u8]) -> Vec<u8> {
    [prefix, PAYLOAD].concat()
}

#[test]
fn a_cached_key_signs_without_asking() {
    let (mut gpg, mut asker) = scripted(vec![created()], vec![]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
    assert_eq!(gpg.calls, [(args::probe(&call()), PAYLOAD.to_vec())]);
    assert!(asker.asks.is_empty());
}

#[test]
fn the_right_passphrase_signs_with_loopback() {
    let (mut gpg, mut asker) = scripted(vec![no_pinentry(), created()], vec![answer("right")]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
    assert_eq!(gpg.calls.len(), 2);
    assert_eq!(gpg.calls[0].0, args::probe(&call()));
    assert_eq!(gpg.calls[1], (args::loopback(&call()), with_payload(b"right\n")));
    let user = Some("Ada Lovelace <ada@example.com>".to_owned());
    assert_eq!(asker.asks, [GpgAsk { key: "ABCDEF0123456789".into(), user, retry: false }]);
}

#[test]
fn wrong_passphrases_ask_again() {
    let (mut gpg, mut asker) = scripted(vec![no_pinentry(), bad(), bad(), created()], vec![answer("a"), answer("b"), answer("right")]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
    assert_eq!(asker.asks.iter().map(|a| a.retry).collect::<Vec<_>>(), [false, true, true]);
    assert_eq!(gpg.calls[3].1, with_payload(b"right\n"));
}

#[test]
fn three_wrong_passphrases_fail() {
    let runs = vec![no_pinentry(), bad(), bad(), bad()];
    let (mut gpg, mut asker) = scripted(runs, vec![answer("a"), answer("b"), answer("c"), answer("d")]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(bad()));
    assert_eq!(asker.asks.len(), MAX_TRIES, "no fourth ask");
    assert_eq!(gpg.calls.len(), 1 + MAX_TRIES);
}

#[test]
fn cancelling_or_timing_out_stops_signing() {
    for (reply, want) in [(AskResult::Cancelled, Finish::Cancelled), (AskResult::TimedOut, Finish::TimedOut)] {
        let (mut gpg, mut asker) = scripted(vec![no_pinentry()], vec![reply]);
        assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), want);
        assert_eq!(gpg.calls.len(), 1, "nothing runs after the ask");
    }
}

#[test]
fn gnupgs_own_window_runs_gits_call_unchanged() {
    for reply in [AskResult::UseSystem, AskResult::Unavailable] {
        let (mut gpg, mut asker) = scripted(vec![no_pinentry(), created()], vec![reply]);
        assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
        assert_eq!(gpg.calls.last().unwrap(), &(call().original, PAYLOAD.to_vec()));
    }
}

#[test]
fn unsupported_gnupg_falls_back_to_gits_call() {
    let old = run(false, "", "gpg: invalid option \"--pinentry-mode\"\n");
    let (mut gpg, mut asker) = scripted(vec![old, created()], vec![]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
    assert!(asker.asks.is_empty());
    assert_eq!(gpg.calls[1], (call().original, PAYLOAD.to_vec()));

    let refused = run(false, "", "[GNUPG:] FAILURE sign 67108924\n");
    let (mut gpg, mut asker) = scripted(vec![no_pinentry(), refused, created()], vec![answer("right")]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(created()));
    assert_eq!(gpg.calls[2], (call().original, PAYLOAD.to_vec()), "loopback refused: GnuPG's own window");
}

#[test]
fn other_loopback_failures_are_reported() {
    let failed = run(false, "", "gpg: signing failed: No secret key\n[GNUPG:] FAILURE sign 17\n");
    let (mut gpg, mut asker) = scripted(vec![no_pinentry(), failed.clone()], vec![answer("right")]);
    assert_eq!(sign(&call(), PAYLOAD, &mut gpg, &mut asker), Finish::Done(failed));
    assert_eq!(gpg.calls.len(), 2, "no fallback run");
}

#[test]
fn a_program_that_cannot_run_reports_why() {
    let (mut gpg, mut asker) = scripted(vec![], vec![]);
    let Finish::Done(run) = sign(&call(), PAYLOAD, &mut gpg, &mut asker) else { panic!("expected a run") };
    assert!(!run.ok && run.code.is_none());
    assert!(String::from_utf8_lossy(&run.stderr).starts_with("Trellis: could not run gpg: "));
}
