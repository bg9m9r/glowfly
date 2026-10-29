//! The systems' process lists for `git_procs.rs`: every process's pid and name (`list`), and a
//! git process's command line (`args`), read only for the processes named git. Windows: the
//! ToolHelp snapshot (`terminal/procs.rs`) and `NtQueryInformationProcess`'s command line;
//! Linux: `/proc/<pid>/comm` and `cmdline`; macOS: `proc_listallpids`, `proc_name` and
//! `KERN_PROCARGS2`. Anything that can't be read is None, or an `Unknown` for the whole list.

use super::{Proc, Unknown};

/// Every process, from a ToolHelp snapshot.
#[cfg(windows)]
pub(super) fn list() -> Result<Vec<Proc>, Unknown> {
    let table = crate::terminal::procs::process_table().ok_or(Unknown::NoList)?;
    Ok(table.into_iter().map(|p| (p.pid, Some(p.exe))).collect())
}

/// Every process, from `/proc`, unless it hides other users' processes.
#[cfg(target_os = "linux")]
pub(super) fn list() -> Result<Vec<Proc>, Unknown> {
    if std::fs::read_to_string("/proc/mounts").is_ok_and(|m| super::proc_hidden(&m)) {
        return Err(Unknown::Hidden);
    }
    let dir = std::fs::read_dir("/proc").map_err(|_| Unknown::NoList)?;
    let pids = dir.flatten().filter_map(|e| e.file_name().to_str()?.parse::<u32>().ok());
    let list = pids
        .filter_map(|pid| {
            let name = i32::try_from(pid).ok().and_then(crate::terminal::procs::process_name);
            // Ended since the folder was listed: not running.
            if name.is_none() && !std::path::Path::new(&format!("/proc/{pid}")).exists() {
                return None;
            }
            Some((pid, name))
        })
        .collect();
    Ok(list)
}

/// Every process, from libproc.
#[cfg(target_os = "macos")]
pub(super) fn list() -> Result<Vec<Proc>, Unknown> {
    let mut pids = vec![0i32; 8192];
    let bytes = libc::c_int::try_from(std::mem::size_of_val(pids.as_slice())).map_err(|_| Unknown::NoList)?;
    // SAFETY: `pids` is writable for `bytes` bytes; proc_listallpids fills at most that many and
    // returns how many pids it filled (negative on failure).
    let n = unsafe { libc::proc_listallpids(pids.as_mut_ptr().cast(), bytes) };
    let n = usize::try_from(n).map_err(|_| Unknown::NoList)?;
    // A full buffer may have left processes out.
    if n == 0 || n >= pids.len() {
        return Err(Unknown::NoList);
    }
    let list = pids[..n]
        .iter()
        .filter(|&&p| p > 0)
        .filter_map(|&pid| {
            let name = crate::terminal::procs::process_name(pid);
            // SAFETY: signal 0 only checks that the process exists; nothing is sent.
            let alive = || unsafe { libc::kill(pid, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM);
            // Ended since the list was read: not running.
            (name.is_some() || alive()).then_some((u32::try_from(pid).ok()?, name))
        })
        .collect();
    Ok(list)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(super) fn list() -> Result<Vec<Proc>, Unknown> {
    Err(Unknown::NoList)
}

/// A process's command line, the program first.
#[cfg(windows)]
pub(super) fn args(pid: u32) -> Option<Vec<String>> {
    use windows::Wdk::System::Threading::{NtQueryInformationProcess, ProcessCommandLineInformation};
    use windows::Win32::Foundation::{CloseHandle, UNICODE_STRING};
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};
    // SAFETY: the call takes no pointers; the returned handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    // A UNICODE_STRING followed by its text; u64s keep it aligned.
    let mut buf = vec![0u64; 4096];
    let size = u32::try_from(std::mem::size_of_val(buf.as_slice())).ok()?;
    let mut got = 0u32;
    // SAFETY: `process` is open with the query right the call needs; `buf` is writable for
    // `size` bytes and the call writes at most that many, setting `got`.
    let status = unsafe { NtQueryInformationProcess(process, ProcessCommandLineInformation, buf.as_mut_ptr().cast(), size, &mut got) };
    // SAFETY: `process` is open and not used after this.
    let _ = unsafe { CloseHandle(process) };
    let got = usize::try_from(got).ok()?;
    if status.is_err() || got < std::mem::size_of::<UNICODE_STRING>() || got > std::mem::size_of_val(buf.as_slice()) {
        return None;
    }
    // SAFETY: the call succeeded, so `buf` starts with a UNICODE_STRING whose Buffer points at
    // `Length` bytes of UTF-16; it is used only when they lie inside the `got` bytes written.
    let us = unsafe { &*buf.as_ptr().cast::<UNICODE_STRING>() };
    let base = buf.as_ptr() as usize;
    let (start, len) = (us.Buffer.0 as usize, usize::from(us.Length));
    if us.Buffer.is_null() || start < base || start + len > base + got || start % 2 != 0 {
        return None;
    }
    // SAFETY: checked just above: `len` bytes at `start` are inside `buf`, aligned for u16.
    let units = unsafe { std::slice::from_raw_parts(us.Buffer.0.cast_const(), len / 2) };
    Some(super::split_command_line(&String::from_utf16_lossy(units)))
}

/// A process's command line, from `/proc/<pid>/cmdline`.
#[cfg(target_os = "linux")]
pub(super) fn args(pid: u32) -> Option<Vec<String>> {
    let raw = std::fs::read(format!("/proc/{pid}/cmdline")).ok()?;
    let args: Vec<String> = raw.split(|&b| b == 0).filter(|a| !a.is_empty()).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    (!args.is_empty()).then_some(args)
}

/// A process's command line, from `KERN_PROCARGS2`: argc, the executable's path, padding, then
/// argv.
#[cfg(target_os = "macos")]
pub(super) fn args(pid: u32) -> Option<Vec<String>> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, libc::c_int::try_from(pid).ok()?];
    let mut buf = vec![0u8; 64 * 1024];
    let mut len: libc::size_t = buf.len();
    // SAFETY: `mib` holds the three ints named; `buf` is writable for `len` bytes and sysctl
    // writes at most that many, setting `len`; no new value is given.
    let rc = unsafe { libc::sysctl(mib.as_mut_ptr(), 3, buf.as_mut_ptr().cast(), &mut len, std::ptr::null_mut(), 0) };
    if rc != 0 || len < 4 {
        return None;
    }
    let data = &buf[..len.min(buf.len())];
    let argc = usize::try_from(i32::from_ne_bytes(data[..4].try_into().ok()?)).ok()?;
    let mut parts = data[4..].split(|&b| b == 0).filter(|p| !p.is_empty());
    // The executable's path comes first.
    parts.next()?;
    let args: Vec<String> = parts.take(argc).map(|a| String::from_utf8_lossy(a).into_owned()).collect();
    (argc > 0 && args.len() == argc).then_some(args)
}

#[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
pub(super) fn args(_pid: u32) -> Option<Vec<String>> {
    None
}
