use super::extend::{ExtendFailed, KeyPrints, Mode, extend, key_prints, set_expire_args};
use super::flow::{AskResult, Asker, Gpg, GpgRun};
use crate::askpass::{GpgAsk, Secret};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::io;

const LISTED: &str = "sec:e:4096:1:ABCDEF0123456789:1600000000:1700000000::u:::scESC:::+:::23::0:\n\
fpr:::::::::0123456789ABCDEF0123456789ABCDEF01234567:\n\
grp:::::::::AAAA:\n\
uid:e::::1600000000::HASH::Ada Lovelace <ada@example.com>::::::::::0:\n\
ssb:e:4096:1:1111222233334444:1600000000:1700000000:::::e:::+:::23:\n\
fpr:::::::::11112222333344445555666677778888AAAABBBB:\n\
ssb:r:4096:1:5555666677778888:1600000000::::::s:::+:::23:\n\
fpr:::::::::5555666677778888999900001111222233334444:\n\
ssb:u:4096:1:AAAA000011112222:1600000000:::::::a:::+:::23:\n\
fpr:::::::::AAAA000011112222333344445555666677778888:\n\
ssb:u:4096:1:BBBB000011112222:1600000000:1907680000:::::s:::+:::23:\n\
fpr:::::::::BBBB000011112222333344445555666677778888:\n\
ssb:u:4096:1:CCCC000011112222:1600000000:1781536000:::::s:::+:::23:\n\
fpr:::::::::CCCC000011112222333344445555666677778888:\n";

/// When the tests run: CCCC's subkey expires in about a year, BBBB's in five, AAAA's never.
const NOW: i64 = 1_750_000_000;

/// Scripted runs of gpg, each call's arguments and input kept.
#[derive(Default)]
struct Script {
    runs: VecDeque<GpgRun>,
    seen: Vec<(Vec<String>, Vec<u8>)>,
}

impl Gpg for Script {
    fn run(&mut self, args: &[OsString], input: &[u8]) -> io::Result<GpgRun> {
        self.seen.push((args.iter().map(|a| a.to_string_lossy().into_owned()).collect(), input.to_vec()));
        self.runs.pop_front().ok_or_else(|| io::Error::other("unscripted run"))
    }

    fn describe(&mut self, key: Option<&str>) -> (String, Option<String>) {
        (key.unwrap_or_default().to_owned(), None)
    }

    fn program(&self) -> &str {
        "gpg"
    }
}

/// Scripted answers of the app's prompt.
#[derive(Default)]
struct Prompt {
    replies: VecDeque<AskResult>,
    asked: Vec<GpgAsk>,
}

impl Asker for Prompt {
    fn ask(&mut self, ask: &GpgAsk) -> AskResult {
        self.asked.push(ask.clone());
        self.replies.pop_front().unwrap_or(AskResult::Unavailable)
    }
}

fn ended(ok: bool, stderr: &str) -> GpgRun {
    GpgRun { ok, code: Some(if ok { 0 } else { 2 }), stdout: Vec::new(), stderr: stderr.into() }
}

fn prints() -> KeyPrints {
    key_prints(LISTED, NOW).expect("the key is listed")
}

fn needs_pin() -> GpgRun {
    ended(false, "gpg: setting the expire time failed: No pinentry\n[GNUPG:] FAILURE keyedit.setexpire 67108949\n")
}

fn wrong_pass() -> GpgRun {
    ended(false, "[GNUPG:] BAD_PASSPHRASE ABCDEF0123456789\ngpg: Bad passphrase\n")
}

fn script(runs: Vec<GpgRun>, replies: Vec<AskResult>) -> (Script, Prompt) {
    (Script { runs: runs.into(), ..Script::default() }, Prompt { replies: replies.into(), ..Prompt::default() })
}

#[test]
fn the_listing_names_the_primary_key_and_its_live_subkeys() {
    let p = prints();
    assert_eq!(p.primary, "0123456789ABCDEF0123456789ABCDEF01234567");
    assert_eq!(
        p.subkeys,
        ["11112222333344445555666677778888AAAABBBB", "CCCC000011112222333344445555666677778888"],
        "the expired subkey and the one ending within two years; not the revoked one, the one that never expires or the one that ends later"
    );
    assert_eq!(p.user.as_deref(), Some("Ada Lovelace <ada@example.com>"));
    assert_eq!(key_prints("uid:u::::1::H::Nobody::::\n", NOW), None, "no secret key, no fingerprint");
}

#[test]
fn the_arguments_set_two_years_on_the_key_then_its_subkeys() {
    let p = prints();
    let probe: Vec<String> = set_expire_args(&p, false, Mode::Probe).iter().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(probe, ["--batch", "--pinentry-mode", "error", "--status-fd", "2", "--quick-set-expire", &p.primary, "2y"]);
    let subs = set_expire_args(&p, true, Mode::Loopback);
    assert!(subs.starts_with(&["--batch", "--pinentry-mode", "loopback", "--passphrase-fd", "0"].map(OsString::from)));
    let tail: Vec<String> = subs[subs.len() - 2..].iter().map(|a| a.to_string_lossy().into_owned()).collect();
    assert_eq!(tail, p.subkeys);
    assert_eq!(set_expire_args(&p, false, Mode::System)[0], "--status-fd", "GnuPG's own pinentry: no mode");
}

#[test]
fn an_unprotected_key_is_extended_without_asking() {
    let (mut gpg, mut ask) = script(vec![ended(true, ""), ended(true, "")], vec![]);
    assert_eq!(extend(&prints(), &mut gpg, &mut ask), Ok(()));
    assert!(ask.asked.is_empty());
    assert_eq!(gpg.seen.len(), 2, "the key, then its subkeys");
}

#[test]
fn a_protected_key_asks_in_the_app_and_feeds_the_passphrase_on_stdin() {
    let runs = vec![needs_pin(), wrong_pass(), ended(true, ""), ended(true, "")];
    let replies = vec![AskResult::Answer(Secret::new("wrong")), AskResult::Answer(Secret::new("right"))];
    let (mut gpg, mut ask) = script(runs, replies);
    assert_eq!(extend(&prints(), &mut gpg, &mut ask), Ok(()));
    assert_eq!(ask.asked.len(), 2);
    assert!(!ask.asked[0].retry && ask.asked[1].retry, "the second prompt says the first was wrong");
    assert_eq!(ask.asked[0].user.as_deref(), Some("Ada Lovelace <ada@example.com>"));
    assert_eq!(gpg.seen[2].1, b"right\n");
    assert_eq!(gpg.seen[3].1, b"right\n", "the subkeys get the same passphrase");
    assert!(gpg.seen[3].0.contains(&"--passphrase-fd".to_owned()));
}

#[test]
fn a_cancelled_prompt_changes_nothing() {
    let (mut gpg, mut ask) = script(vec![needs_pin()], vec![AskResult::Cancelled]);
    assert_eq!(extend(&prints(), &mut gpg, &mut ask), Err(ExtendFailed::Cancelled));
    assert_eq!(gpg.seen.len(), 1);
}

#[test]
fn with_the_in_app_prompt_off_gnupg_asks_itself() {
    let (mut gpg, mut ask) = script(vec![needs_pin(), ended(true, ""), ended(true, "")], vec![AskResult::UseSystem]);
    assert_eq!(extend(&prints(), &mut gpg, &mut ask), Ok(()));
    assert!(!gpg.seen[1].0.iter().any(|a| a == "--pinentry-mode"), "{:?}", gpg.seen[1].0);
}

#[test]
fn a_failure_reports_what_gpg_said_without_its_status_lines() {
    let (mut gpg, mut ask) = script(vec![ended(false, "[GNUPG:] ERROR x 1\ngpg: key not found\n")], vec![]);
    assert_eq!(extend(&prints(), &mut gpg, &mut ask), Err(ExtendFailed::Gpg("gpg: key not found".into())));
}
