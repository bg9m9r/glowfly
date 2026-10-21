use super::*;
use crate::testutil::{scratch_repo, set_config};

fn cred(url: &str, user: &str, password: &str) -> Option<HttpsCredential> {
    HttpsCredential::from_prompt(url, user, Secret::new(password))
}

#[test]
fn reads_the_prompt_url() {
    let c = cred("https://github.com", "ada", "hunter2").unwrap();
    assert_eq!((c.protocol.as_str(), c.host.as_str(), c.path.as_deref()), ("https", "github.com", None));
    assert_eq!(c.url(), "https://github.com");
    let c = cred("HTTPS://ada@Example.com:8443/team/app.git", "ada", "x").unwrap();
    assert_eq!((c.protocol.as_str(), c.host.as_str(), c.path.as_deref()), ("https", "example.com:8443", Some("team/app.git")));
    assert_eq!(c.url(), "https://example.com:8443/team/app.git", "the userinfo is dropped");
    assert!(cred("http://intranet.local/r.git", "ada", "x").is_some());
    assert!(is_http_url("HTTP://h") && is_http_url("https://h") && !is_http_url("ssh://h") && !is_http_url("ada@host"));
}

#[test]
fn refuses_what_git_would_refuse() {
    assert!(cred("ada@host", "ada", "x").is_none(), "an ssh target");
    assert!(cred("ssh://host/r.git", "ada", "x").is_none());
    assert!(cred("https://", "ada", "x").is_none(), "no host");
    assert!(cred("https://ada@/r.git", "ada", "x").is_none(), "no host");
    assert!(cred("https://github.com", "", "x").is_none(), "no user");
    assert!(cred("https://github.com", "ada\nhost=evil", "x").is_none(), "a newline in the user");
    assert!(cred("https://github.com", "ada", "x\npassword=y").is_none(), "a newline in the password");
    assert!(cred("https://github.com", "ada", "x\0").is_none(), "a NUL");
}

#[test]
fn writes_the_credential_protocol() {
    let c = cred("https://example.com/o/r.git", "ada", "hunter2").unwrap();
    assert_eq!(c.request(true), "protocol=https\nhost=example.com\npath=o/r.git\nusername=ada\npassword=hunter2\n\n");
    assert_eq!(c.request(false), "protocol=https\nhost=example.com\npath=o/r.git\nusername=ada\n\n");
    assert!(!format!("{c:?}").contains("hunter2"), "Debug hides the password");
}

#[test]
fn reads_the_helper_setting() {
    let out = |code, stdout: &str| GitOutput { ok: code == 0, code: Some(code), stdout: stdout.into(), ..GitOutput::default() };
    assert_eq!(helper_config_from(&out(0, "manager\n")), HelperConfig::Set);
    assert_eq!(helper_config_from(&out(0, "\n")), HelperConfig::TurnedOff);
    assert_eq!(helper_config_from(&out(1, "")), HelperConfig::None);
    assert_eq!(helper_config_from(&GitOutput::default()), HelperConfig::None, "git didn't run");
}

#[test]
fn knows_each_platforms_store() {
    assert_eq!(platform_helpers("windows"), ["manager", "wincred"]);
    assert_eq!(platform_helpers("macos"), ["osxkeychain"]);
    assert_eq!(platform_helpers("linux"), ["libsecret"]);
    assert!(platform_helpers("freebsd").is_empty());
    assert!(setup_hint("linux").contains("git-credential-libsecret"));
}

/// A helper that appends its action and its input to log.txt in the repository.
const LOGGING_HELPER: &str = "!f() { echo \"$1\" >> log.txt; cat >> log.txt; }; f";

/// A repository and an environment that keeps git away from the machine's own config (and so
/// from the developer's real credential store).
fn isolated(name: &str) -> (PathBuf, String) {
    let dir = scratch_repo(name);
    let global = crate::paths::temp_dir().join(format!("trellis-test-{name}-{}.gitconfig", std::process::id()));
    let _ = std::fs::remove_file(&global);
    (dir, global.to_string_lossy().into_owned())
}

fn env(global: &str) -> [(&str, &str); 3] {
    [("GIT_CONFIG_NOSYSTEM", "1"), ("GIT_CONFIG_GLOBAL", global), ("GCM_INTERACTIVE", "never")]
}

fn log(dir: &Path) -> String {
    std::fs::read_to_string(dir.join("log.txt")).unwrap_or_default().replace("\r\n", "\n")
}

#[test]
fn approves_and_rejects_through_the_configured_helper() {
    let (dir, global) = isolated("cred-helper");
    set_config(&dir, &[("credential.helper", LOGGING_HELPER)]);
    let c = cred("https://example.com", "ada", "hunter2").unwrap();
    let none = |_: &Path| -> Option<String> { panic!("a configured helper needs no set-up") };
    assert_eq!(remember_in(&dir, &c, &env(&global), none), Ok(Remembered::Helper));
    let text = log(&dir);
    assert!(text.starts_with("store\n"), "{text}");
    for field in ["protocol=https", "host=example.com", "username=ada", "password=hunter2"] {
        assert!(text.contains(field), "{field} in {text}");
    }
    assert_eq!(forget_in(&dir, &c, &env(&global)), Ok(()));
    assert!(log(&dir).contains("\nerase\n"), "{}", log(&dir));
    assert!(!Path::new(&global).exists(), "the global config is untouched");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stores_nothing_when_helpers_are_turned_off() {
    let (dir, global) = isolated("cred-off");
    set_config(&dir, &[("credential.helper", "")]);
    let c = cred("https://example.com", "ada", "hunter2").unwrap();
    let found = |_: &Path| Some(LOGGING_HELPER.to_owned());
    assert!(remember_in(&dir, &c, &env(&global), found).is_err());
    assert!(!dir.join("log.txt").exists(), "no helper ran");
    assert!(!Path::new(&global).exists(), "no helper was set up");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn sets_up_the_platform_store_when_git_has_none() {
    let (dir, global) = isolated("cred-setup");
    let c = cred("https://example.com", "ada", "hunter2").unwrap();
    assert!(remember_in(&dir, &c, &env(&global), |_: &Path| None).unwrap_err().contains("no credential store is set up"));
    let found = |_: &Path| Some(LOGGING_HELPER.to_owned());
    assert_eq!(remember_in(&dir, &c, &env(&global), found), Ok(Remembered::SetUp(LOGGING_HELPER.into())));
    let config = std::fs::read_to_string(&global).unwrap();
    assert!(config.contains("helper"), "{config}");
    assert!(log(&dir).starts_with("store\n"), "{}", log(&dir));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&global);
}
