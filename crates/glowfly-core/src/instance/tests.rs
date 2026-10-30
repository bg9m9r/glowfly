//! The protocol: what a second process sends, what the listener accepts and answers, and what
//! a window that is gone leaves behind. The claim and its hand-over are `tests_claim.rs`.

use super::file::{self, InstanceFile};
use super::*;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::time::Instant;

/// A fresh folder for one test's instance file.
pub(super) fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("glowfly-instance-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

/// Listens at `path`, holding its claim.
pub(super) fn listen(path: &Path) -> (Listener, mpsc::Receiver<Handed>) {
    let claim = claim_at(&path.with_file_name(LOCK_NAME)).unwrap().expect("no other holder");
    listen_with(path, Some(claim))
}

pub(super) fn listen_with(path: &Path, claim: Option<Claim>) -> (Listener, mpsc::Receiver<Handed>) {
    let (tx, rx) = mpsc::channel();
    let l = serve_at(path, claim, move |item| {
        let _ = tx.send(item);
    })
    .unwrap();
    (l, rx)
}

fn link(s: &str) -> Handed {
    Handed::Link(s.to_owned())
}

/// Sends `line` as it is to the listener `path` names; its reply line, if it gave one.
fn raw(path: &Path, line: &str) -> Option<String> {
    let info = file::read(path).unwrap();
    let mut s = TcpStream::connect(info.addr.as_str()).unwrap();
    s.set_read_timeout(Some(REQUEST_TIMEOUT * 2)).unwrap();
    // A listener that stops reading may reset the connection before the whole line is out.
    let _ = s.write_all(format!("{line}\n").as_bytes());
    let mut reply = String::new();
    match BufReader::new(&s).read_line(&mut reply) {
        Ok(n) if n > 0 => Some(reply.trim_end().to_owned()),
        _ => None,
    }
}

fn token(path: &Path) -> String {
    file::read(path).unwrap().token
}

/// An absolute folder with a space and a non-ASCII letter, which needn't exist.
fn folder() -> PathBuf {
    crate::paths::temp_dir().join("My Repö")
}

#[test]
fn a_link_reaches_the_running_window() {
    let dir = scratch("deliver");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    assert!(listener.is_listening());
    hand_off_via(&path, &link("glowfly://repo?root=abc")).unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), link("glowfly://repo?root=abc"));
    hand_off_via(&path, &link("glowfly://tag/v1?root=abc")).unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), link("glowfly://tag/v1?root=abc"));
    let info = file::read(&path).unwrap();
    assert_eq!(Some(info.addr.parse().unwrap()), listener.addr());
    assert_eq!(info.pid, std::process::id());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(std::fs::metadata(dir.join(LOCK_NAME)).unwrap().permissions().mode() & 0o777, 0o600);
    }
    drop(listener);
    assert!(!path.exists(), "the window's own file goes with it");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn an_absolute_folder_reaches_the_running_window() {
    let dir = scratch("folder");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    let f = folder();
    assert!(f.is_absolute());
    hand_off_via(&path, &Handed::Folder(f.clone())).unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), Handed::Folder(f.clone()));
    // On the wire: the path as text, and no link field an older window would act on.
    let wire = serde_json::to_value(Request::new("t".into(), &Handed::Folder(f.clone())).unwrap()).unwrap();
    assert_eq!(wire, serde_json::json!({ "token": "t", "folder": f.to_str().unwrap() }));
    drop(listener);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_relative_folder_is_never_sent() {
    let dir = scratch("relative");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    let err = hand_off_via(&path, &Handed::Folder(PathBuf::from("repo"))).unwrap_err();
    assert!(matches!(&err, HandOffError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput), "{err:?}");
    assert!(rx.recv_timeout(std::time::Duration::from_millis(200)).is_err());
    drop(listener);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn malformed_items_are_refused_and_nothing_opens() {
    let dir = scratch("malformed");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    let t = token(&path);
    let abs = folder().to_str().unwrap().to_owned();
    let refused = serde_json::json!({ "ok": false }).to_string();
    for bad in [
        serde_json::json!({ "token": t, "folder": "repo" }),
        serde_json::json!({ "token": t, "folder": "" }),
        serde_json::json!({ "token": t, "folder": format!("{abs}\0x") }),
        serde_json::json!({ "token": t }),
        serde_json::json!({ "token": t, "open": "glowfly://repo?root=abc", "folder": abs }),
        serde_json::json!({ "token": t, "open": "https://example.com/" }),
        serde_json::json!({ "token": "0".repeat(32), "folder": abs }),
    ] {
        assert_eq!(raw(&path, &bad.to_string()).as_deref(), Some(refused.as_str()), "{bad}");
    }
    // Not JSON, or a field of the wrong type: no reply at all.
    assert_eq!(raw(&path, "open the pod bay doors"), None);
    assert_eq!(raw(&path, &serde_json::json!({ "token": t, "folder": ["a"] }).to_string()), None);
    assert!(rx.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "nothing was opened");
    // The listener still serves well-formed requests.
    hand_off_via(&path, &Handed::Folder(folder())).unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), Handed::Folder(folder()));
    drop(listener);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_line_over_the_limit_is_cut_off() {
    let dir = scratch("long");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    let long = folder().join("x".repeat(MAX_LINE as usize));
    let err = hand_off_via(&path, &Handed::Folder(long.clone())).unwrap_err();
    assert!(matches!(&err, HandOffError::Io(e) if e.kind() == std::io::ErrorKind::InvalidInput), "the sender checks: {err:?}");
    // Sent anyway, the listener reads only the limit, which is no request.
    let line = serde_json::json!({ "token": token(&path), "folder": long.to_str().unwrap() }).to_string();
    assert_eq!(raw(&path, &line), None);
    assert!(rx.recv_timeout(std::time::Duration::from_millis(200)).is_err());
    drop(listener);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_wrong_token_is_refused() {
    let dir = scratch("token");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    let mut info = file::read(&path).unwrap();
    info.token = "0".repeat(32);
    file::write(&path, &info).unwrap();
    assert!(matches!(hand_off_via(&path, &link("glowfly://repo?root=abc")), Err(HandOffError::Rejected)));
    assert!(rx.recv_timeout(std::time::Duration::from_millis(200)).is_err(), "nothing was opened");
    // The file now holds another token: dropping the listener leaves it.
    drop(listener);
    assert!(path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn no_window_means_not_running() {
    let dir = scratch("none");
    let path = dir.join(FILE_NAME);
    assert!(matches!(hand_off_via(&path, &link("glowfly://repo?root=abc")), Err(HandOffError::NotRunning)));

    // A file left by a window that is gone: its port is closed.
    let (addr, _held) = crate::testutil::closed_port();
    file::write(&path, &InstanceFile { addr, token: "1".repeat(32), pid: 1 }).unwrap();
    let start = Instant::now();
    assert!(matches!(hand_off_via(&path, &Handed::Folder(folder())), Err(HandOffError::NotRunning)));
    assert!(start.elapsed() < CONNECT_TIMEOUT * 3, "gave up in {:?}", start.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
