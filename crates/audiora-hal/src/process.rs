//! Which programs are running — used to avoid claiming virtual devices another mixer owns.

use windows::Win32::Foundation::CloseHandle;
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};

/// Lower-case executable names (without `.exe`) of every running process.
pub fn running_process_names() -> Vec<String> {
    process_table().into_iter().map(|p| p.name).collect()
}

#[derive(Clone, Debug)]
pub struct ProcessEntry {
    pub pid: u32,
    pub parent: u32,
    /// Lower-case executable name without `.exe`.
    pub name: String,
}

/// Every running process with its parent.
pub fn process_table() -> Vec<ProcessEntry> {
    let mut out = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else { return out };
        let mut e = PROCESSENTRY32W { dwSize: size_of::<PROCESSENTRY32W>() as u32, ..Default::default() };
        if Process32FirstW(snap, &mut e).is_ok() {
            loop {
                let len = e.szExeFile.iter().position(|&c| c == 0).unwrap_or(e.szExeFile.len());
                let name = String::from_utf16_lossy(&e.szExeFile[..len]).to_lowercase();
                let name = name.strip_suffix(".exe").map(str::to_owned).unwrap_or(name);
                out.push(ProcessEntry { pid: e.th32ProcessID, parent: e.th32ParentProcessID, name });
                if Process32NextW(snap, &mut e).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snap);
    }
    out
}

/// The top of the chain of processes with the same executable as `pid` (a browser's main process
/// for one of its audio helpers). Capturing that process tree catches every sound the program makes.
pub fn process_tree_root(table: &[ProcessEntry], pid: u32) -> u32 {
    let find = |p: u32| table.iter().find(|e| e.pid == p);
    let Some(me) = find(pid) else { return pid };
    let mut root = me;
    for _ in 0..32 {
        match find(root.parent) {
            Some(parent) if parent.name == me.name && parent.pid != root.pid => root = parent,
            _ => break,
        }
    }
    root.pid
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_root_follows_same_named_parents() {
        let e = |pid, parent, name: &str| ProcessEntry { pid, parent, name: name.into() };
        let t = [e(1, 0, "explorer"), e(10, 1, "chrome"), e(11, 10, "chrome"), e(12, 11, "chrome"), e(20, 1, "discord")];
        assert_eq!(process_tree_root(&t, 12), 10);
        assert_eq!(process_tree_root(&t, 20), 20);
        assert_eq!(process_tree_root(&t, 99), 99);
    }
}
