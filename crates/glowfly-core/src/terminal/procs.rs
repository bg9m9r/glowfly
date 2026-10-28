//! The systems' process lists for activity.rs: Windows' ToolHelp snapshot and the image path of
//! a process; on Linux `/proc`, on macOS libproc, for a process's name and a shell's children.
//! Everything here only reads, and anything that can't be read is left out.

/// Every process on the system, from a ToolHelp snapshot.
#[cfg(windows)]
pub(super) fn process_table() -> Option<Vec<super::activity::ProcEntry>> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
    };
    // SAFETY: the call takes no pointers; the returned handle is closed below.
    let snap = unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }.ok()?;
    let mut entry = PROCESSENTRY32W { dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
    let mut out = Vec::new();
    // SAFETY: `snap` is an open snapshot handle and `entry` a writable PROCESSENTRY32W whose
    // dwSize is set, as both calls require.
    let mut more = unsafe { Process32FirstW(snap, &mut entry) }.is_ok();
    while more {
        let len = entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(entry.szExeFile.len());
        out.push(super::activity::ProcEntry {
            pid: entry.th32ProcessID,
            parent: entry.th32ParentProcessID,
            exe: String::from_utf16_lossy(&entry.szExeFile[..len]),
        });
        // SAFETY: as above.
        more = unsafe { Process32NextW(snap, &mut entry) }.is_ok();
    }
    // SAFETY: `snap` is open and not used after this.
    let _ = unsafe { CloseHandle(snap) };
    Some(out)
}

/// The full path of the file process `pid` runs, when this user may ask.
#[cfg(windows)]
pub(super) fn image_path(pid: u32) -> Option<String> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
    };
    use windows::core::PWSTR;
    // SAFETY: the call takes no pointers; the returned handle is closed below.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `process` is open with the query right the call needs; `buf` holds `len` u16s and
    // the call writes at most that many, setting `len` to what it wrote.
    let ok = unsafe { QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len) }.is_ok();
    // SAFETY: `process` is open and not used after this.
    let _ = unsafe { CloseHandle(process) };
    ok.then(|| String::from_utf16_lossy(&buf[..(len as usize).min(buf.len())]))
}

/// A process's command name.
#[cfg(target_os = "linux")]
pub(super) fn process_name(pid: i32) -> Option<String> {
    let name = std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?;
    let name = name.trim_end();
    (!name.is_empty()).then(|| name.to_string())
}

/// The children of `shell` that run in its session, from every `/proc/<pid>/stat`.
#[cfg(target_os = "linux")]
pub(super) fn children(shell: u32) -> Vec<i32> {
    use super::activity::{children_in_session, parse_stat};
    let stat = |pid: &str| std::fs::read_to_string(format!("/proc/{pid}/stat")).ok().and_then(|l| parse_stat(&l));
    let Ok(shell) = i32::try_from(shell) else { return Vec::new() };
    let Some(own) = stat(&shell.to_string()) else { return Vec::new() };
    let Ok(dir) = std::fs::read_dir("/proc") else { return Vec::new() };
    let all: Vec<(i32, super::activity::Stat)> = dir
        .flatten()
        .filter_map(|e| {
            let name = e.file_name();
            let pid: i32 = name.to_str()?.parse().ok()?;
            Some((pid, stat(&pid.to_string())?))
        })
        .collect();
    children_in_session(&all, shell, own.session)
}

/// A process's command name, from libproc.
#[cfg(target_os = "macos")]
pub(super) fn process_name(pid: i32) -> Option<String> {
    let mut buf = [0u8; 256];
    // SAFETY: `buf` is writable for the length given; proc_name writes at most that many bytes
    // and returns how many it wrote (0 or less on failure).
    let n = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    let n = usize::try_from(n).ok().filter(|&n| n > 0)?.min(buf.len());
    let name = String::from_utf8_lossy(&buf[..n]).trim_end_matches('\0').to_owned();
    (!name.is_empty()).then_some(name)
}

/// The children of `shell`, from libproc.
#[cfg(target_os = "macos")]
pub(super) fn children(shell: u32) -> Vec<i32> {
    /// libproc.h's `PROC_PPID_ONLY`: list the processes whose parent is `typeinfo`.
    const PROC_PPID_ONLY: u32 = 6;
    let mut pids = [0i32; 256];
    let bytes = std::mem::size_of_val(&pids) as libc::c_int;
    // SAFETY: `pids` is writable for `bytes` bytes; proc_listpids fills at most that many and
    // returns how many bytes it filled (negative on failure).
    let filled = unsafe { libc::proc_listpids(PROC_PPID_ONLY, shell, pids.as_mut_ptr().cast(), bytes) };
    let n = usize::try_from(filled).unwrap_or(0) / std::mem::size_of::<i32>();
    pids[..n.min(pids.len())].iter().copied().filter(|&p| p > 0).collect()
}

/// The other Unix systems give neither.
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub(super) fn process_name(_pid: i32) -> Option<String> {
    None
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
pub(super) fn children(_shell: u32) -> Vec<i32> {
    Vec::new()
}
