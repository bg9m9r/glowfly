use super::file::{self, InstanceFile};
use super::*;
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

/// A fresh folder for one test's instance file.
fn scratch(tag: &str) -> PathBuf {
    let dir = crate::paths::temp_dir().join(format!("trellis-instance-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn listen(path: &Path) -> (Listener, mpsc::Receiver<String>) {
    let (tx, rx) = mpsc::channel();
    let l = serve_at(path, move |link| {
        let _ = tx.send(link);
    })
    .unwrap();
    (l, rx)
}

#[test]
fn a_link_reaches_the_running_window() {
    let dir = scratch("deliver");
    let path = dir.join(FILE_NAME);
    let (listener, rx) = listen(&path);
    hand_off_via(&path, "trellis://repo?root=abc").unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), "trellis://repo?root=abc");
    hand_off_via(&path, "trellis://tag/v1?root=abc").unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), "trellis://tag/v1?root=abc");
    let info = file::read(&path).unwrap();
    assert_eq!(info.addr, listener.addr().to_string());
    assert_eq!(info.pid, std::process::id());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
    }
    drop(listener);
    assert!(!path.exists(), "the window's own file goes with it");
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
    assert!(matches!(hand_off_via(&path, "trellis://repo?root=abc"), Err(HandOffError::Rejected)));
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
    assert!(matches!(hand_off_via(&path, "trellis://repo?root=abc"), Err(HandOffError::NotRunning)));

    // A file left by a window that is gone: its port is closed.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = closed.local_addr().unwrap().to_string();
    drop(closed);
    file::write(&path, &InstanceFile { addr, token: "1".repeat(32), pid: 1 }).unwrap();
    let start = Instant::now();
    assert!(matches!(hand_off_via(&path, "trellis://repo?root=abc"), Err(HandOffError::NotRunning)));
    assert!(start.elapsed() < CONNECT_TIMEOUT * 3, "gave up in {:?}", start.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_newer_windows_file_is_left_alone() {
    let dir = scratch("newer");
    let path = dir.join(FILE_NAME);
    let (old, _rx) = listen(&path);
    let (newer, rx) = listen(&path);
    drop(old);
    assert!(path.exists(), "the newer window's file stays");
    hand_off_via(&path, "trellis://repo?root=abc").unwrap();
    assert_eq!(rx.recv_timeout(REQUEST_TIMEOUT).unwrap(), "trellis://repo?root=abc");
    drop(newer);
    assert!(!path.exists());
    let _ = std::fs::remove_dir_all(&dir);
}
