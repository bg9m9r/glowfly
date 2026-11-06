//! Fixture helpers shared by `glowfly-core` tests: throwaway repositories (on the shared kit's
//! `git`, `set_config` and `write`, in `testkit/`).

use crate::gitcli::args;
pub(crate) use crate::testkit::{git, set_config, write};
use std::path::{Path, PathBuf};

/// An empty repository on `main` with a fixed identity, in a fresh temporary folder.
pub(crate) fn scratch_repo(name: &str) -> PathBuf {
    let dir = scratch_dir(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    init_repo(&dir);
    dir
}

/// A copy of `template` in the folder `scratch_repo(name)` would make. When the folder is a
/// repository, its index is refreshed for the copied files' new times; a template of several
/// repositories leaves that to the caller.
pub(crate) fn scratch_copy(name: &str, template: &crate::testkit::Template) -> PathBuf {
    let dir = scratch_dir(name);
    template.copy_to(&dir);
    if dir.join(".git").is_dir() {
        git(&dir, &["update-index", "-q", "--refresh"]);
    }
    dir
}

/// The empty repository `scratch_repo` makes, in the existing folder `dir` (where a template's
/// build starts).
pub(crate) fn init_repo(dir: &Path) {
    git(dir, &["init", "-b", "main"]);
    set_config(
        dir,
        &[
            ("user.name", "Test"),
            ("user.email", "test@example.com"),
            ("core.autocrlf", "false"),
            ("commit.gpgsign", "false"),
            // Otherwise every commit starts a `git maintenance run --auto` process.
            ("maintenance.auto", "false"),
        ],
    );
}

fn scratch_dir(name: &str) -> PathBuf {
    crate::partial::allow_extension();
    crate::paths::temp_dir().join(format!("glowfly-test-{name}-{}", std::process::id()))
}

/// A second name for the folder `dir`, made next to it (`<name>-alias`): a symlink, or on Windows
/// a junction where symlinks need rights the account lacks. None when neither can be made.
pub(crate) fn alias_of(dir: &Path) -> Option<PathBuf> {
    let link = dir.with_file_name(format!("{}-alias", dir.file_name()?.to_string_lossy()));
    let _ = std::fs::remove_dir(&link).or_else(|_| std::fs::remove_file(&link));
    #[cfg(unix)]
    let made = std::os::unix::fs::symlink(dir, &link).is_ok();
    #[cfg(windows)]
    let made = std::os::windows::fs::symlink_dir(dir, &link).is_ok()
        || std::process::Command::new("cmd").args(["/c", "mklink", "/J"]).arg(&link).arg(dir).output().is_ok_and(|o| o.status.success());
    made.then_some(link)
}

/// A network place for tests of what a repository's files may name: a share on Windows, an
/// automounted host elsewhere. Its host is under `.invalid` (RFC 6761), which never resolves, so
/// even a bug can't reach a real machine; the code under test must never touch it.
pub(crate) fn net_place() -> PathBuf {
    if cfg!(windows) { PathBuf::from(r"\\glowfly-test.invalid\share") } else { PathBuf::from("/net/glowfly-test.invalid") }
}

/// Fails the test when `path` is on `net_place()`, in any spelling: test builds of the helpers
/// that touch a path (`paths::resolve`, `FolderId::of`, `read_file_bounded`) call it, so code
/// that reaches the place fails its test instead of only failing to resolve the host.
pub(crate) fn never_net_place(path: &Path) {
    let text = path.to_string_lossy().to_ascii_lowercase();
    assert!(!text.contains("glowfly-test.invalid"), "touched the test network place: {}", path.display());
}

/// `p` as git writes it in a `.git` file or a worktree record: forward slashes.
pub(crate) fn git_spelling(p: &Path) -> String {
    p.display().to_string().replace('\\', "/")
}

/// Makes `link` a symbolic link to `target` (a folder link on Windows when `dir`); false where
/// the system doesn't allow it, with a note. The target is never touched.
pub(crate) fn link_to(target: &Path, link: &Path, dir: bool) -> bool {
    #[cfg(unix)]
    let made = {
        let _ = dir;
        std::os::unix::fs::symlink(target, link).is_ok()
    };
    #[cfg(windows)]
    let made = if dir {
        std::os::windows::fs::symlink_dir(target, link).is_ok()
    } else {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    };
    if !made {
        eprintln!("skipped: symbolic links can't be made here");
    }
    made
}

/// A loopback address (`127.0.0.1:<port>`) where nothing listens, and the socket that keeps it
/// so: bound, without SO_REUSEADDR, but never listening. While the socket lives no other
/// listener (another test's included) can take the port, unlike a listener bound and dropped,
/// whose freed port can be reused at once. Connecting there is refused on Linux and Windows;
/// BSD stacks such as macOS's may drop the attempt instead, so it fails at the caller's connect
/// timeout.
pub(crate) fn closed_port() -> (String, socket2::Socket) {
    use socket2::{Domain, Protocol, SockAddr, Socket, Type};
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP)).unwrap();
    socket.bind(&SockAddr::from(std::net::SocketAddr::from(([127, 0, 0, 1], 0)))).unwrap();
    let addr = socket.local_addr().unwrap().as_socket().unwrap();
    (addr.to_string(), socket)
}

/// `file:///srv/x` on Unix, `file:///C:/x` on Windows.
pub(crate) fn file_url(p: &Path) -> String {
    let s = p.display().to_string().replace('\\', "/");
    if s.starts_with('/') { format!("file://{s}") } else { format!("file:///{s}") }
}

/// A blobless partial clone (`--filter=blob:none`, through a file:// URL) of a source with three
/// commits: a.txt "one", then d/b.txt, then a.txt "one\ntwo". HEAD's blobs are checked out;
/// HEAD~2's a.txt (also HEAD~1's) is missing. Returns (source, clone).
pub(crate) fn partial_clone(name: &str) -> (PathBuf, PathBuf) {
    partial_clone_of(name, |src| {
        write(src, "a.txt", "one\n");
        git(src, &["add", "."]);
        git(src, &["commit", "-q", "-m", "first"]);
        std::fs::create_dir_all(src.join("d")).unwrap();
        write(src, "d/b.txt", "b\n");
        git(src, &["add", "."]);
        git(src, &["commit", "-q", "-m", "second"]);
        write(src, "a.txt", "one\ntwo\n");
        git(src, &["add", "."]);
        git(src, &["commit", "-q", "-m", "third"]);
    })
}

/// A blobless partial clone of a source whose history `commit` makes. Returns (source, clone).
pub(crate) fn partial_clone_of(name: &str, commit: impl FnOnce(&Path)) -> (PathBuf, PathBuf) {
    let src = scratch_repo(&format!("{name}-src"));
    set_config(&src, &[("uploadpack.allowFilter", "true"), ("uploadpack.allowAnySHA1InWant", "true")]);
    commit(&src);
    let clone = src.with_file_name(format!("{}-partial", src.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&clone);
    git(src.parent().unwrap(), &["clone", "-q", "--filter=blob:none", &file_url(&src), &clone.display().to_string()]);
    set_config(
        &clone,
        &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false"), ("maintenance.auto", "false")],
    );
    (src, clone)
}

/// git-lfs's version, or None (with a note) so tests that need it can skip.
pub(crate) fn lfs_or_skip(test: &str) -> Option<String> {
    let v = crate::lfs::installed();
    if v.is_none() {
        eprintln!("{test}: git-lfs is not installed, skipping");
    }
    v
}

/// A repository with one commit on `main`, pushed with upstream to a bare `origin` next to it.
/// Returns (working repository, bare repository).
pub(crate) fn with_origin(name: &str) -> (PathBuf, PathBuf) {
    let work = scratch_repo(name);
    write(&work, "a.txt", "one\n");
    git(&work, &["add", "."]);
    git(&work, &["commit", "-q", "-m", "first"]);
    let bare = work.with_file_name(format!("{}-origin.git", work.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&bare);
    std::fs::create_dir_all(&bare).unwrap();
    git(&bare, &["init", "-q", "--bare", "-b", "main"]);
    git(&work, &["remote", "add", "origin", &bare.display().to_string()]);
    git(&work, &["push", "-q", "-u", "origin", "main"]);
    (work, bare)
}

/// Commits `file` on `branch` in the bare repository `bare` (through a temporary clone), so
/// the remote gets ahead of local copies.
pub(crate) fn remote_commit(bare: &Path, branch: &str, file: &str, msg: &str) {
    let tmp = bare.with_file_name(format!("{}-clone", bare.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_dir_all(&tmp);
    let parent = tmp.parent().unwrap();
    git(parent, &["clone", "-q", "-b", branch, &bare.display().to_string(), &tmp.display().to_string()]);
    set_config(&tmp, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
    write(&tmp, file, msg);
    git(&tmp, &["add", "."]);
    git(&tmp, &["commit", "-q", "-m", msg]);
    git(&tmp, &["push", "-q", "origin", branch]);
    let _ = std::fs::remove_dir_all(&tmp);
}

/// A repository with one submodule at libs/lib, added from a sibling repository by its relative
/// URL and committed. Returns (parent, the submodule's checkout), with an identity set inside
/// the checkout so tests can commit there.
pub(crate) fn with_submodule(name: &str) -> (PathBuf, PathBuf) {
    let lib = scratch_repo(&format!("{name}-lib"));
    write(&lib, "lib.txt", "lib\n");
    git(&lib, &["add", "."]);
    git(&lib, &["commit", "-q", "-m", "lib"]);
    let parent = scratch_repo(name);
    write(&parent, "a.txt", "one\n");
    git(&parent, &["add", "."]);
    git(&parent, &["commit", "-q", "-m", "first"]);
    let url = format!("../{}", lib.file_name().unwrap().to_string_lossy());
    // Git refuses the file transport for submodules unless allowed on the command line.
    git(&parent, &["-c", "protocol.file.allow=always", "submodule", "add", "-q", &url, "libs/lib"]);
    git(&parent, &["commit", "-q", "-m", "add lib"]);
    let sub = parent.join("libs/lib");
    set_config(&sub, &[("user.name", "Test"), ("user.email", "test@example.com"), ("commit.gpgsign", "false")]);
    (parent, sub)
}

/// Environment that keeps the developer's global and system git config out of a command: an
/// empty file (next to `dir`) as GIT_CONFIG_GLOBAL, and GIT_CONFIG_NOSYSTEM=1.
pub(crate) fn empty_global_config(dir: &Path) -> Vec<(String, String)> {
    let file = dir.with_file_name(format!("{}-gitconfig", dir.file_name().unwrap().to_string_lossy()));
    std::fs::write(&file, "").unwrap();
    vec![("GIT_CONFIG_GLOBAL".into(), file.display().to_string()), ("GIT_CONFIG_NOSYSTEM".into(), "1".into())]
}

/// Commits `file` (containing `msg`) in `dir`, signed with a new SSH key made next to `dir`.
/// Returns the commit and the public key file, or None (after saying so) when ssh-keygen is
/// missing or signing fails (e.g. key-permission complaints on Windows), so the caller skips.
pub(crate) fn ssh_signed_commit(dir: &Path, file: &str, msg: &str) -> Option<(git2::Oid, PathBuf)> {
    let skip = |why: &str| {
        eprintln!("ssh-keygen unavailable or signing failed, skipping: {why}");
        None
    };
    let key = dir.with_file_name(format!("{}-key", dir.file_name().unwrap().to_string_lossy()));
    let _ = std::fs::remove_file(&key);
    let _ = std::fs::remove_file(key.with_extension("pub"));
    let keygen = std::process::Command::new("ssh-keygen")
        .args(["-q", "-t", "ed25519", "-N", "", "-C", "test@example.com", "-f"])
        .arg(&key)
        .stdin(std::process::Stdio::null())
        .output();
    match keygen {
        Ok(o) if o.status.success() => {}
        Ok(o) => return skip(&String::from_utf8_lossy(&o.stderr)),
        Err(e) => return skip(&e.to_string()),
    }
    let public = PathBuf::from(format!("{}.pub", key.display()));
    write(dir, file, msg);
    git(dir, &["add", file]);
    let env = empty_global_config(dir);
    let env: Vec<(&str, &str)> = env.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let signing_key = format!("user.signingkey={}", public.display());
    let a = ["-c", "gpg.format=ssh", "-c", &signing_key, "commit", "-q", "-S", "-m", msg];
    let (ok, out) = crate::gitcli::run_with_input(dir, &args(&a), "", &env);
    if !ok {
        return skip(&out);
    }
    let oid = git2::Repository::open(dir).ok()?.head().ok()?.target()?;
    Some((oid, public))
}

/// Writes a commit object by hand, byte for byte, so it can hold what git's porcelain won't
/// make: `who` (`Name <email>`, any bytes) as author and committer at 1700000900 +0000, an
/// optional `encoding` header and a message in any encoding. `parent` None makes a root commit.
pub(crate) fn raw_commit(
    repo: &git2::Repository,
    tree: git2::Oid,
    parent: Option<git2::Oid>,
    who: &[u8],
    encoding: Option<&str>,
    msg: &[u8],
) -> git2::Oid {
    let mut text = format!("tree {tree}\n").into_bytes();
    if let Some(p) = parent {
        text.extend_from_slice(format!("parent {p}\n").as_bytes());
    }
    for role in ["author", "committer"] {
        text.extend_from_slice(format!("{role} ").as_bytes());
        text.extend_from_slice(who);
        text.extend_from_slice(b" 1700000900 +0000\n");
    }
    if let Some(e) = encoding {
        text.extend_from_slice(format!("encoding {e}\n").as_bytes());
    }
    text.push(b'\n');
    text.extend_from_slice(msg);
    repo.odb().unwrap().write(git2::ObjectType::Commit, &text).unwrap()
}

/// A Latin-1 author line for `raw_commit`: "René Müller <rené@example.com>".
pub(crate) const LATIN1_WHO: &[u8] = b"Ren\xe9 M\xfcller <ren\xe9@example.com>";

/// A Latin-1 message for `raw_commit`: "Café au lait", then "Crème brûlée for everyöne.".
pub(crate) const LATIN1_MSG: &[u8] = b"Caf\xe9 au lait\n\nCr\xe8me br\xfbl\xe9e for every\xf6ne.\n";

/// Replaces every `from` in the reflog of `reference` (e.g. "refs/stash") in `dir` with `to`,
/// failing the test when `from` isn't there. It writes bytes git itself would keep, such as a
/// Latin-1 message, which the git CLI can't take on Windows.
pub(crate) fn rewrite_reflog(dir: &Path, reference: &str, from: &[u8], to: &[u8]) {
    let file = reference.split('/').fold(dir.join(".git").join("logs"), |p, part| p.join(part));
    let text = std::fs::read(&file).unwrap();
    assert!(!from.is_empty() && text.windows(from.len()).any(|w| w == from), "{} has no {from:?}", file.display());
    let mut out = Vec::with_capacity(text.len());
    let mut rest = &text[..];
    while !rest.is_empty() {
        if rest.starts_with(from) {
            out.extend_from_slice(to);
            rest = &rest[from.len()..];
        } else {
            out.push(rest[0]);
            rest = &rest[1..];
        }
    }
    std::fs::write(&file, out).unwrap();
}

/// Appends to `dir`'s `.git/config` a `[<section>]` holding `key` with no value (no `=`), which
/// git reads as true. `section` may name a subsection, e.g. `remote "origin"`. libgit2's
/// `set_str` can't write such a key.
pub(crate) fn add_bare_key(dir: &Path, section: &str, key: &str) {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().append(true).open(dir.join(".git").join("config")).unwrap();
    f.write_all(format!("[{section}]\n\t{key}\n").as_bytes()).unwrap();
}

/// Makes `link` a symbolic link to the file `target` (a Windows file link), returning whether
/// it could: Windows refuses without Developer Mode or admin rights, so link tests skip then.
pub(crate) fn file_link(target: &str, link: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link).is_ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_file(target, link).is_ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        false
    }
}
