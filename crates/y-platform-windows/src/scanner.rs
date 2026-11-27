use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use windows_sys::Win32::Foundation::{ERROR_NO_MORE_ITEMS, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_REPARSE_POINT,
    FILE_ATTRIBUTE_SPARSE_FILE, FIND_FIRST_EX_LARGE_FETCH, FindClose, FindExInfoBasic,
    FindExSearchNameMatch, FindFirstFileExW, FindNextFileW, GetCompressedFileSizeW,
    WIN32_FIND_DATAW,
};
use windows_sys::Win32::System::Threading::{
    GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN, THREAD_MODE_BACKGROUND_END,
    THREAD_PRIORITY_LOWEST,
};
use y_core::attribution::{
    AttributionOptions, RootExeCandidate, attribute_scan_with_options,
    classify_drive_root_os_folder, merge_drive_rescan,
};
use y_core::config::{AppConfig, DEFAULT_PERSONAL_NAMES, PinKind, ZoneRuleKind, ZoneRulebook};
use y_core::dependencies::{
    parse_epic_manifest_item, parse_steam_appmanifest_full, parse_steam_libraryfolders_vdf,
};
use y_core::discovery::{
    build_registry_catalog, is_subpath_of, normalize_app_token, normalize_path_key,
    normalize_windows_path, path_drive_letter,
};
use y_core::history::{HistoryEntryRef, ScanHistory, recompute_report_growth_from_history};
use y_core::model::{
    AttributionBasis, DriveInfo, FolderNode, InstalledApp, ManifestAppEntry, ScanReport,
    ScannedUnit,
};
use y_core::pe::{is_plausible_main_exe_name, parse_pe_version_product_name};
use y_core::tree::{
    DecompositionOptions, decompose_scanned_tree_with_options, is_root_system_file_name,
};

use crate::paths::{
    current_local_timestamp, enumerate_fixed_drives, get_windows_directory,
    resolve_personal_shell_folders, to_wide_null,
};
use crate::registry::{enumerate_uninstall_registry, read_steam_install_path};

const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x0004_0000;
const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x0040_0000;
const IO_REPARSE_TAG_MOUNT_POINT: u32 = 0xA000_0003;
const IO_REPARSE_TAG_SYMLINK: u32 = 0xA000_000C;
const INVALID_FILE_SIZE_U32: u32 = 0xFFFF_FFFF;

#[derive(Debug, Clone, Default)]
pub struct ScanProgressSnapshot {
    pub phase: String,
    pub current_path: String,
    pub dirs_walked: u64,
    pub files_counted: u64,
    pub bytes_counted: u64,
}

#[derive(Clone)]
struct ProgressCounters {
    dirs_walked: Arc<AtomicU64>,
    files_counted: Arc<AtomicU64>,
    bytes_counted: Arc<AtomicU64>,
    current_path: Arc<Mutex<String>>,
    cancel: Arc<AtomicBool>,
}

impl ProgressCounters {
    fn new(cancel: Arc<AtomicBool>) -> Self {
        Self {
            dirs_walked: Arc::new(AtomicU64::new(0)),
            files_counted: Arc::new(AtomicU64::new(0)),
            bytes_counted: Arc::new(AtomicU64::new(0)),
            current_path: Arc::new(Mutex::new(String::new())),
            cancel,
        }
    }
}

fn prepare_search_pattern(dir_path: &str) -> Vec<u16> {
    let trimmed = dir_path.trim_end_matches('\\');
    let pat = if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
        format!("{trimmed}\\*")
    } else if trimmed.len() >= 230 && !trimmed.starts_with("\\\\?\\") {
        format!("\\\\?\\{trimmed}\\*")
    } else {
        format!("{trimmed}\\*")
    };
    to_wide_null(&pat)
}

fn prepare_file_path_wide(file_path: &str) -> Vec<u16> {
    if file_path.len() >= 240 && !file_path.starts_with("\\\\?\\") {
        to_wide_null(&format!("\\\\?\\{file_path}"))
    } else {
        to_wide_null(file_path)
    }
}

fn join_child_path(parent: &str, name: &str) -> String {
    let trimmed = parent.trim_end_matches('\\');
    if trimmed.len() == 2 && trimmed.as_bytes()[1] == b':' {
        format!("{trimmed}\\{name}")
    } else {
        format!("{trimmed}\\{name}")
    }
}

fn file_size_from_find_data(full_path: &str, fd: &WIN32_FIND_DATAW) -> u64 {
    let attrs = fd.dwFileAttributes;
    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) != 0 {
        let tag = fd.dwReserved0;
        if tag == IO_REPARSE_TAG_SYMLINK || tag == IO_REPARSE_TAG_MOUNT_POINT {
            return 0;
        }
        return compressed_size_or_zero(full_path);
    }
    if (attrs
        & (FILE_ATTRIBUTE_SPARSE_FILE
            | FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS))
        != 0
    {
        return compressed_size_or_zero(full_path);
    }
    ((fd.nFileSizeHigh as u64) << 32) | (fd.nFileSizeLow as u64)
}

fn compressed_size_or_zero(full_path: &str) -> u64 {
    let w = prepare_file_path_wide(full_path);
    let mut high: u32 = 0;
    let low = unsafe { GetCompressedFileSizeW(w.as_ptr(), &mut high) };
    if low == INVALID_FILE_SIZE_U32 {
        let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
        if err != 0 {
            return 0;
        }
    }
    ((high as u64) << 32) | (low as u64)
}

struct FindHandleGuard(windows_sys::Win32::Foundation::HANDLE);

impl Drop for FindHandleGuard {
    fn drop(&mut self) {
        if self.0 != INVALID_HANDLE_VALUE && !self.0.is_null() {
            unsafe { FindClose(self.0) };
        }
    }
}

fn is_excluded_path(
    path: &str,
    exclusions: &[String],
    sys_win_key: &str,
    leaf_lower: &str,
) -> bool {
    let pk = normalize_path_key(path);
    // Constitutional guard: NEVER walk Windows or WinSxS.
    if !sys_win_key.is_empty() && (pk == sys_win_key || is_subpath_of(&pk, sys_win_key)) {
        return true;
    }
    if leaf_lower == "winsxs" && pk.contains("\\windows\\") {
        return true;
    }
    for ex in exclusions {
        let ek = normalize_path_key(ex);
        if !ek.is_empty() && (pk == ek || is_subpath_of(&pk, &ek)) {
            return true;
        }
    }
    false
}

/// Fast iterative subtree byte counter for directories below container-retention depth.
fn sum_subtree_bytes_iterative(
    start_dir: &str,
    exclusions: &[String],
    sys_win_key: &str,
    progress: &ProgressCounters,
) -> u64 {
    let mut stack = Vec::with_capacity(64);
    stack.push(start_dir.to_string());
    let mut total_bytes: u64 = 0;
    let mut local_dirs: u64 = 0;
    let mut local_files: u64 = 0;
    let mut local_bytes: u64 = 0;

    while let Some(dir) = stack.pop() {
        if progress.cancel.load(Ordering::Relaxed) {
            break;
        }
        local_dirs += 1;
        let pattern = prepare_search_pattern(&dir);
        let mut fd: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
        let h = unsafe {
            FindFirstFileExW(
                pattern.as_ptr(),
                FindExInfoBasic,
                (&mut fd as *mut WIN32_FIND_DATAW).cast(),
                FindExSearchNameMatch,
                std::ptr::null(),
                FIND_FIRST_EX_LARGE_FETCH,
            )
        };
        if h == INVALID_HANDLE_VALUE {
            continue;
        }
        let _guard = FindHandleGuard(h);

        loop {
            let name_len = fd
                .cFileName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(fd.cFileName.len());
            let name = String::from_utf16_lossy(&fd.cFileName[..name_len]);
            if name != "." && name != ".." {
                let full = join_child_path(&dir, &name);
                let attrs = fd.dwFileAttributes;
                if (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0 {
                    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) == 0 {
                        let leaf_lower = name.to_ascii_lowercase();
                        if !is_excluded_path(&full, exclusions, sys_win_key, &leaf_lower) {
                            stack.push(full);
                        }
                    }
                } else {
                    let sz = file_size_from_find_data(&full, &fd);
                    total_bytes = total_bytes.saturating_add(sz);
                    local_files += 1;
                    local_bytes = local_bytes.saturating_add(sz);
                }
            }

            let ok = unsafe { FindNextFileW(h, &mut fd) };
            if ok == 0 {
                let err = unsafe { windows_sys::Win32::Foundation::GetLastError() };
                if err == ERROR_NO_MORE_ITEMS {
                    break;
                }
                break;
            }
        }

        if local_dirs >= 64 {
            progress.dirs_walked.fetch_add(local_dirs, Ordering::Relaxed);
            progress
                .files_counted
                .fetch_add(local_files, Ordering::Relaxed);
            progress
                .bytes_counted
                .fetch_add(local_bytes, Ordering::Relaxed);
            local_dirs = 0;
            local_files = 0;
            local_bytes = 0;
        }
    }

    progress.dirs_walked.fetch_add(local_dirs, Ordering::Relaxed);
    progress
        .files_counted
        .fetch_add(local_files, Ordering::Relaxed);
    progress
        .bytes_counted
        .fetch_add(local_bytes, Ordering::Relaxed);
    total_bytes
}

struct SubtreeTask {
    path: String,
    name: String,
    depth: usize,
    is_drive_root: bool,
}

fn should_retain_children(
    path_lower: &str,
    depth: usize,
    shell_keys: &HashSet<String>,
    retain_ancestors: &HashSet<String>,
) -> bool {
    if retain_ancestors.contains(path_lower) {
        return true;
    }
    if shell_keys.contains(path_lower) {
        return false;
    }
    depth <= 5 || (depth == 6 && path_lower.ends_with("\\appdata\\local\\microsoft\\windows"))
}

fn scan_node_recursive(
    task: SubtreeTask,
    exclusions: &[String],
    sys_win_key: &str,
    shell_keys: &HashSet<String>,
    retain_ancestors: &HashSet<String>,
    progress: &ProgressCounters,
    root_system_files_out: Option<&Mutex<Vec<(String, u64)>>>,
) -> FolderNode {
    let path_lower = normalize_path_key(&task.path);
    if !should_retain_children(&path_lower, task.depth, shell_keys, retain_ancestors) {
        if progress.dirs_walked.load(Ordering::Relaxed) % 256 == 0 {
            if let Ok(mut cp) = progress.current_path.try_lock() {
                *cp = task.path.clone();
            }
        }
        let total = sum_subtree_bytes_iterative(&task.path, exclusions, sys_win_key, progress);
        return FolderNode {
            path: task.path,
            name: task.name,
            direct_file_bytes: total,
            total_bytes: total,
            children: Vec::new(),
        };
    }

    progress.dirs_walked.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut cp) = progress.current_path.try_lock() {
        *cp = task.path.clone();
    }

    let mut direct_file_bytes: u64 = 0;
    let mut subdirs: Vec<(String, String)> = Vec::new();
    let pattern = prepare_search_pattern(&task.path);
    let mut fd: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    let h = unsafe {
        FindFirstFileExW(
            pattern.as_ptr(),
            FindExInfoBasic,
            (&mut fd as *mut WIN32_FIND_DATAW).cast(),
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        )
    };
    if h != INVALID_HANDLE_VALUE {
        let _guard = FindHandleGuard(h);
        loop {
            let name_len = fd
                .cFileName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(fd.cFileName.len());
            let name = String::from_utf16_lossy(&fd.cFileName[..name_len]);
            if name != "." && name != ".." {
                let full = join_child_path(&task.path, &name);
                let attrs = fd.dwFileAttributes;
                if (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0 {
                    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) == 0 {
                        let leaf_lower = name.to_ascii_lowercase();
                        if !is_excluded_path(&full, exclusions, sys_win_key, &leaf_lower) {
                            subdirs.push((full, name));
                        }
                    }
                } else {
                    let sz = file_size_from_find_data(&full, &fd);
                    progress.files_counted.fetch_add(1, Ordering::Relaxed);
                    progress.bytes_counted.fetch_add(sz, Ordering::Relaxed);
                    if task.is_drive_root && is_root_system_file_name(&name) {
                        if let Some(rs_out) = root_system_files_out {
                            if let Ok(mut guard) = rs_out.lock() {
                                guard.push((full, sz));
                            }
                        }
                    } else {
                        direct_file_bytes = direct_file_bytes.saturating_add(sz);
                    }
                }
            }
            if unsafe { FindNextFileW(h, &mut fd) } == 0 {
                break;
            }
        }
    }

    let mut children = Vec::with_capacity(subdirs.len());
    for (sub_full, sub_name) in subdirs {
        if progress.cancel.load(Ordering::Relaxed) {
            break;
        }
        children.push(scan_node_recursive(
            SubtreeTask {
                path: sub_full,
                name: sub_name,
                depth: task.depth + 1,
                is_drive_root: false,
            },
            exclusions,
            sys_win_key,
            shell_keys,
            retain_ancestors,
            progress,
            None,
        ));
    }

    let child_sum: u64 = children.iter().map(|c| c.total_bytes).sum();
    let total_bytes = direct_file_bytes.saturating_add(child_sum);
    FolderNode {
        path: task.path,
        name: task.name,
        direct_file_bytes,
        total_bytes,
        children,
    }
}

struct BackgroundIoGuard(bool);

impl BackgroundIoGuard {
    fn enter() -> Self {
        let ok = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN as i32) };
        if ok == 0 {
            unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_LOWEST) };
            Self(false)
        } else {
            Self(true)
        }
    }
}

impl Drop for BackgroundIoGuard {
    fn drop(&mut self) {
        if self.0 {
            unsafe { SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_END as i32) };
        }
    }
}

fn parallel_scan_drive_root(
    drive_root: &str,
    exclusions: &[String],
    sys_win_key: &str,
    shell_keys: &HashSet<String>,
    retain_ancestors: &HashSet<String>,
    progress: &ProgressCounters,
) -> (FolderNode, Vec<(String, u64)>) {
    // First enumerate depth 0..=2 on coordinator so we get ~100-300 independent work items
    // for a 6-thread low-I/O worker pool, preserving exact tree hierarchy.
    let root_node_shell = scan_drive_parallel_tier(
        drive_root,
        exclusions,
        sys_win_key,
        shell_keys,
        retain_ancestors,
        progress,
    );
    root_node_shell
}

fn scan_drive_parallel_tier(
    drive_root: &str,
    exclusions: &[String],
    sys_win_key: &str,
    shell_keys: &HashSet<String>,
    retain_ancestors: &HashSet<String>,
    progress: &ProgressCounters,
) -> (FolderNode, Vec<(String, u64)>) {
    let root_sys = Mutex::new(Vec::<(String, u64)>::new());

    // Read immediate children of drive root + expand depth 1 containers (Users\<User>\AppData\Local etc.)
    // so parallel workers share big subtrees evenly.
    let mut root_direct_bytes: u64 = 0;
    let mut top_dirs: Vec<(String, String)> = Vec::new();
    let pattern = prepare_search_pattern(drive_root);
    let mut fd: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
    let h = unsafe {
        FindFirstFileExW(
            pattern.as_ptr(),
            FindExInfoBasic,
            (&mut fd as *mut WIN32_FIND_DATAW).cast(),
            FindExSearchNameMatch,
            std::ptr::null(),
            FIND_FIRST_EX_LARGE_FETCH,
        )
    };
    if h != INVALID_HANDLE_VALUE {
        let _guard = FindHandleGuard(h);
        loop {
            let name_len = fd
                .cFileName
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(fd.cFileName.len());
            let name = String::from_utf16_lossy(&fd.cFileName[..name_len]);
            if name != "." && name != ".." {
                let full = join_child_path(drive_root, &name);
                let attrs = fd.dwFileAttributes;
                if (attrs & FILE_ATTRIBUTE_DIRECTORY) != 0 {
                    if (attrs & FILE_ATTRIBUTE_REPARSE_POINT) == 0 {
                        let pk = normalize_path_key(&full);
                        let leaf_lower = name.to_ascii_lowercase();
                        if !is_excluded_path(&full, exclusions, sys_win_key, &leaf_lower) {
                            top_dirs.push((full, name));
                        }
                        let _ = pk;
                    }
                } else {
                    let sz = file_size_from_find_data(&full, &fd);
                    progress.files_counted.fetch_add(1, Ordering::Relaxed);
                    progress.bytes_counted.fetch_add(sz, Ordering::Relaxed);
                    if is_root_system_file_name(&name) {
                        if let Ok(mut g) = root_sys.lock() {
                            g.push((full, sz));
                        }
                    } else {
                        root_direct_bytes = root_direct_bytes.saturating_add(sz);
                    }
                }
            }
            if unsafe { FindNextFileW(h, &mut fd) } == 0 {
                break;
            }
        }
    }

    // Build parallel job list from top_dirs (expanding depth 1 if top_dir has many subdirs like Program Files).
    let num_threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get().clamp(4, 6));

    let queue: Arc<(Mutex<VecDeque<(usize, String, String)>>, Condvar)> = Arc::new((
        Mutex::new(
            top_dirs
                .into_iter()
                .enumerate()
                .map(|(idx, (full, name))| (idx, full, name))
                .collect(),
        ),
        Condvar::new(),
    ));
    let results: Arc<Mutex<Vec<(usize, FolderNode)>>> = Arc::new(Mutex::new(Vec::new()));

    // Note: top_dirs items like C:\Users, C:\Program Files, C:\ProgramData are huge, so let's expand
    // one level of major containers BEFORE dispatching to threads so all 6 threads stay busy!
    let mut expanded_slots: Vec<FolderNode> = Vec::new();
    let mut leaf_jobs: Vec<(usize, Option<usize>, String, String, usize)> = Vec::new();
    {
        let mut guard = queue.0.lock().unwrap();
        let initial_items: Vec<(usize, String, String)> = guard.drain(..).collect();
        for (_orig_idx, full, name) in initial_items {
            let pk = normalize_path_key(&full);
            let is_major = pk.ends_with("\\users")
                || pk.ends_with("\\program files")
                || pk.ends_with("\\program files (x86)")
                || pk.ends_with("\\programdata");
            if is_major && (!shell_keys.contains(&pk) || retain_ancestors.contains(&pk)) {
                let slot_idx = expanded_slots.len();
                let mut direct_b = 0u64;
                let mut sub_items = Vec::new();
                let pat = prepare_search_pattern(&full);
                let mut sub_fd: WIN32_FIND_DATAW = unsafe { std::mem::zeroed() };
                let sh = unsafe {
                    FindFirstFileExW(
                        pat.as_ptr(),
                        FindExInfoBasic,
                        (&mut sub_fd as *mut WIN32_FIND_DATAW).cast(),
                        FindExSearchNameMatch,
                        std::ptr::null(),
                        FIND_FIRST_EX_LARGE_FETCH,
                    )
                };
                if sh != INVALID_HANDLE_VALUE {
                    let _g = FindHandleGuard(sh);
                    loop {
                        let nlen = sub_fd
                            .cFileName
                            .iter()
                            .position(|&c| c == 0)
                            .unwrap_or(sub_fd.cFileName.len());
                        let sname = String::from_utf16_lossy(&sub_fd.cFileName[..nlen]);
                        if sname != "." && sname != ".." {
                            let sfull = join_child_path(&full, &sname);
                            let sattrs = sub_fd.dwFileAttributes;
                            if (sattrs & FILE_ATTRIBUTE_DIRECTORY) != 0 {
                                if (sattrs & FILE_ATTRIBUTE_REPARSE_POINT) == 0 {
                                    let llow = sname.to_ascii_lowercase();
                                    if !is_excluded_path(&sfull, exclusions, sys_win_key, &llow) {
                                        sub_items.push((sfull, sname));
                                    }
                                }
                            } else {
                                let sz = file_size_from_find_data(&sfull, &sub_fd);
                                direct_b = direct_b.saturating_add(sz);
                                progress.files_counted.fetch_add(1, Ordering::Relaxed);
                                progress.bytes_counted.fetch_add(sz, Ordering::Relaxed);
                            }
                        }
                        if unsafe { FindNextFileW(sh, &mut sub_fd) } == 0 {
                            break;
                        }
                    }
                }
                expanded_slots.push(FolderNode {
                    path: full,
                    name,
                    direct_file_bytes: direct_b,
                    total_bytes: direct_b,
                    children: Vec::new(),
                });
                for (sf, sn) in sub_items {
                    leaf_jobs.push((slot_idx, None, sf, sn, 2));
                }
            } else {
                let slot_idx = expanded_slots.len();
                expanded_slots.push(FolderNode {
                    path: full.clone(),
                    name: name.clone(),
                    direct_file_bytes: 0,
                    total_bytes: 0,
                    children: Vec::new(),
                });
                leaf_jobs.push((slot_idx, Some(slot_idx), full, name, 1));
            }
        }
    }

    let work_q: Arc<Mutex<VecDeque<(usize, bool, String, String, usize)>>> = Arc::new(Mutex::new(
        leaf_jobs
            .into_iter()
            .map(|(slot_idx, replace_flag, p, n, d)| (slot_idx, replace_flag.is_some(), p, n, d))
            .collect(),
    ));

    let slot_children: Arc<Mutex<Vec<Vec<FolderNode>>>> =
        Arc::new(Mutex::new(vec![Vec::new(); expanded_slots.len()]));
    let slot_replacements: Arc<Mutex<Vec<Option<FolderNode>>>> =
        Arc::new(Mutex::new(vec![None; expanded_slots.len()]));

    std::thread::scope(|s| {
        for _ in 0..num_threads {
            let work_q = Arc::clone(&work_q);
            let slot_children = Arc::clone(&slot_children);
            let slot_replacements = Arc::clone(&slot_replacements);
            let progress = progress.clone();
            s.spawn(move || {
                let _io_guard = BackgroundIoGuard::enter();
                loop {
                    let next = {
                        let mut q = work_q.lock().unwrap();
                        q.pop_front()
                    };
                    let Some((slot_idx, is_whole_slot, path, name, depth)) = next else {
                        break;
                    };
                    let node = scan_node_recursive(
                        SubtreeTask {
                            path,
                            name,
                            depth,
                            is_drive_root: false,
                        },
                        exclusions,
                        sys_win_key,
                        shell_keys,
                        retain_ancestors,
                        &progress,
                        None,
                    );
                    if is_whole_slot {
                        slot_replacements.lock().unwrap()[slot_idx] = Some(node);
                    } else {
                        slot_children.lock().unwrap()[slot_idx].push(node);
                    }
                }
            });
        }
    });

    let mut children_by_slot = slot_children.lock().unwrap();
    let mut replacements_by_slot = slot_replacements.lock().unwrap();
    let mut final_top_children = Vec::with_capacity(expanded_slots.len());
    for (idx, mut slot) in expanded_slots.into_iter().enumerate() {
        if let Some(rep) = replacements_by_slot[idx].take() {
            final_top_children.push(rep);
        } else {
            let ch = std::mem::take(&mut children_by_slot[idx]);
            let sum_ch: u64 = ch.iter().map(|c| c.total_bytes).sum();
            slot.children = ch;
            slot.total_bytes = slot.direct_file_bytes.saturating_add(sum_ch);
            final_top_children.push(slot);
        }
    }
    let _ = results;

    let top_sum: u64 = final_top_children.iter().map(|c| c.total_bytes).sum();
    let root_node = FolderNode {
        path: drive_root.to_string(),
        name: drive_root.to_string(),
        direct_file_bytes: root_direct_bytes,
        total_bytes: root_direct_bytes.saturating_add(top_sum),
        children: final_top_children,
    };
    let rs = root_sys.into_inner().unwrap();
    (root_node, rs)
}

fn find_steamapps_dirs_in_node(node: &FolderNode, depth: usize, out: &mut BTreeSet<String>) {
    let pk = normalize_path_key(&node.path);
    if pk.ends_with("\\steamapps") {
        out.insert(normalize_windows_path(&node.path));
        return;
    }
    if depth >= 4 {
        return;
    }
    for ch in &node.children {
        find_steamapps_dirs_in_node(ch, depth + 1, out);
    }
}

fn discover_steam_libraries_and_apps(
    walked_roots: &[FolderNode],
    enabled_letters: &HashSet<char>,
    steam_path: Option<&str>,
    apps: &mut Vec<InstalledApp>,
) -> (Vec<String>, Vec<ManifestAppEntry>) {
    let mut steamapps_dirs: BTreeSet<String> = BTreeSet::new();
    let mut seed_steamapps: Vec<String> = Vec::new();

    if let Some(sp) = steam_path {
        if let Some(dl) = path_drive_letter(sp) {
            if enabled_letters.contains(&dl.to_ascii_uppercase()) {
                let sa = format!("{}\\steamapps", normalize_windows_path(sp).trim_end_matches('\\'));
                if Path::new(&sa).is_dir() {
                    seed_steamapps.push(sa);
                }
            }
        }
    }

    for root in walked_roots {
        let mut found = BTreeSet::new();
        find_steamapps_dirs_in_node(root, 0, &mut found);
        for sa in found {
            seed_steamapps.push(sa);
        }
    }

    // Parse libraryfolders.vdf in every seed steamapps directory, filtered strictly to scanned drives.
    let mut queue: VecDeque<String> = seed_steamapps.into_iter().collect();
    let mut vdf_checked: HashSet<String> = HashSet::new();
    while let Some(sa_dir) = queue.pop_front() {
        let sa_norm = normalize_windows_path(&sa_dir);
        let sa_key = normalize_path_key(&sa_norm);
        if let Some(dl) = path_drive_letter(&sa_norm) {
            if !enabled_letters.contains(&dl.to_ascii_uppercase()) {
                continue;
            }
        } else {
            continue;
        }
        if !Path::new(&sa_norm).is_dir() {
            continue;
        }
        steamapps_dirs.insert(sa_norm.clone());
        if vdf_checked.insert(sa_key) {
            let vdf_path = Path::new(&sa_norm).join("libraryfolders.vdf");
            if let Ok(vdf_text) = std::fs::read_to_string(vdf_path) {
                for lib_root in parse_steam_libraryfolders_vdf(&vdf_text) {
                    if let Some(dl) = path_drive_letter(&lib_root) {
                        if enabled_letters.contains(&dl.to_ascii_uppercase()) {
                            let next_sa = format!("{}\\steamapps", lib_root.trim_end_matches('\\'));
                            if Path::new(&next_sa).is_dir() {
                                queue.push_back(next_sa);
                            }
                        }
                    }
                }
            }
        }
    }

    let mut steam_lib_roots: Vec<String> = Vec::new();
    let mut steam_manifest_entries: Vec<ManifestAppEntry> = Vec::new();
    let mut seen_libs: HashSet<String> = HashSet::new();
    for sa in &steamapps_dirs {
        let lib_root = sa
            .trim_end_matches('\\')
            .strip_suffix("\\steamapps")
            .unwrap_or(sa)
            .to_string();
        let lk = normalize_path_key(&lib_root);
        if seen_libs.insert(lk) {
            steam_lib_roots.push(lib_root.clone());
        }

        // Ensure a Steam client bucket exists when scanning a drive containing steam.exe.
        if Path::new(&lib_root).join("steam.exe").is_file() {
            let steam_idx = apps
                .iter()
                .position(|a| normalize_app_token(&a.display_name) == "steam");
            if let Some(idx) = steam_idx {
                let norm = normalize_windows_path(&lib_root);
                if !apps[idx]
                    .all_install_locations
                    .iter()
                    .any(|p| normalize_path_key(p) == normalize_path_key(&norm))
                {
                    apps[idx].all_install_locations.push(norm);
                }
            } else {
                let norm = normalize_windows_path(&lib_root);
                apps.push(InstalledApp {
                    id: "steam:client".to_string(),
                    display_name: "Steam".to_string(),
                    display_version: String::new(),
                    uninstall_guids: Vec::new(),
                    aliases: Vec::new(),
                    publishers: vec!["Valve Corporation".to_string()],
                    direct_install_location: Some(norm.clone()),
                    inferred_install_location: None,
                    all_install_locations: vec![norm],
                    no_touch: None,
                    is_store_fallback: false,
                    display_icon: Some(format!("{}\\steam.exe", lib_root.trim_end_matches('\\'))),
                });
            }
        }

        let Ok(rd) = std::fs::read_dir(sa) else {
            continue;
        };
        for ent in rd.flatten() {
            let fname = ent.file_name().to_string_lossy().to_string();
            let fl = fname.to_ascii_lowercase();
            if !fl.starts_with("appmanifest_") || !fl.ends_with(".acf") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(ent.path()) else {
                continue;
            };
            let Some(man) = parse_steam_appmanifest_full(&text) else {
                continue;
            };
            let game_dir = normalize_windows_path(&format!("{sa}\\common\\{}", man.installdir));
            if !Path::new(&game_dir).is_dir() {
                continue;
            }
            let gk = normalize_path_key(&game_dir);
            let man_tok = normalize_app_token(&man.name);
            let container_dir = format!("{}\\common", sa.trim_end_matches('\\'));
            steam_manifest_entries.push(ManifestAppEntry {
                id: format!("steam:app:{}", man.appid),
                display_name: man.name.clone(),
                publisher: None,
                install_dir: game_dir.clone(),
                container_dir,
                basis: AttributionBasis::SteamManifest,
                display_icon: None,
            });

            let matched_pos = apps.iter().position(|a| {
                a.all_install_locations
                    .iter()
                    .any(|loc| normalize_path_key(loc) == gk)
                    || (!man_tok.is_empty() && normalize_app_token(&a.display_name) == man_tok)
                    || (!man_tok.is_empty()
                        && a.aliases
                            .iter()
                            .any(|al| normalize_app_token(al) == man_tok))
            });

            if let Some(pos) = matched_pos {
                if !apps[pos]
                    .all_install_locations
                    .iter()
                    .any(|loc| normalize_path_key(loc) == gk)
                {
                    apps[pos].all_install_locations.push(game_dir);
                }
            } else {
                apps.push(InstalledApp {
                    id: format!("steam:app:{}", man.appid),
                    display_name: man.name,
                    display_version: String::new(),
                    uninstall_guids: Vec::new(),
                    aliases: Vec::new(),
                    publishers: Vec::new(),
                    direct_install_location: Some(game_dir.clone()),
                    inferred_install_location: None,
                    all_install_locations: vec![game_dir],
                    no_touch: None,
                    is_store_fallback: false,
                    display_icon: None,
                });
            }
        }
    }

    (steam_lib_roots, steam_manifest_entries)
}

fn discover_epic_manifest_apps(
    enabled_letters: &HashSet<char>,
    manifest_entries: &mut Vec<ManifestAppEntry>,
) {
    let prog_data = std::env::var_os("ProgramData")
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| r"C:\ProgramData".to_string());
    let manifests_dir = Path::new(&prog_data)
        .join("Epic")
        .join("EpicGamesLauncher")
        .join("Data")
        .join("Manifests");
    let Ok(rd) = std::fs::read_dir(&manifests_dir) else {
        return;
    };
    for ent in rd.flatten() {
        let name = ent.file_name().to_string_lossy().to_ascii_lowercase();
        if !name.ends_with(".item") {
            continue;
        }
        let Ok(txt) = std::fs::read_to_string(ent.path()) else {
            continue;
        };
        let Some(epic) = parse_epic_manifest_item(&txt) else {
            continue;
        };
        let Some(dl) = path_drive_letter(&epic.install_location) else {
            continue;
        };
        if !enabled_letters.contains(&dl.to_ascii_uppercase()) {
            continue;
        }
        if !Path::new(&epic.install_location).is_dir() {
            continue;
        }
        let container_dir = Path::new(&epic.install_location)
            .parent()
            .map(|p| normalize_windows_path(&p.to_string_lossy()))
            .unwrap_or_default();
        let display_icon = if epic.launch_executable.is_empty() {
            None
        } else {
            let joined = Path::new(&epic.install_location)
                .join(epic.launch_executable.replace('/', "\\"));
            Some(normalize_windows_path(&joined.to_string_lossy()))
        };
        manifest_entries.push(ManifestAppEntry {
            id: format!("epic:{}", epic.app_name),
            display_name: epic.display_name,
            publisher: None,
            install_dir: epic.install_location,
            container_dir,
            basis: AttributionBasis::LauncherManifest,
            display_icon,
        });
    }
}

fn probe_dir_for_main_exe(dir_path: &str, folder_name: &str) -> Option<RootExeCandidate> {
    let rd = std::fs::read_dir(dir_path).ok()?;
    let folder_tok = normalize_app_token(folder_name);
    let mut best: Option<(i32, u64, String, Option<String>)> = None;
    for ent in rd.flatten() {
        let Ok(ft) = ent.file_type() else {
            continue;
        };
        if !ft.is_file() {
            continue;
        }
        let fname = ent.file_name().to_string_lossy().to_string();
        if !is_plausible_main_exe_name(&fname) {
            continue;
        }
        let p = ent.path();
        let size = ent.metadata().map(|m| m.len()).unwrap_or(0);
        if size < 16_384 {
            continue;
        }
        let Ok(mut f) = File::open(&p) else {
            continue;
        };
        let mut mz = [0u8; 2];
        if f.read_exact(&mut mz).is_err() || mz != [b'M', b'Z'] {
            continue;
        }
        let product_name = parse_pe_version_product_name(&mut f);
        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let stem_tok = normalize_app_token(stem);
        let mut score = 0i32;
        if !folder_tok.is_empty() && stem_tok == folder_tok {
            score += 100;
        }
        if product_name.is_some() {
            score += 30;
        }
        if size >= 200_000 {
            score += 10;
        }
        let exe_str = normalize_windows_path(&p.to_string_lossy());
        if best
            .as_ref()
            .is_none_or(|(bs, bsz, _, _)| (score, size) > (*bs, *bsz))
        {
            best = Some((score, size, exe_str, product_name));
        }
    }
    best.map(|(_, _, exe_path, product_name)| RootExeCandidate {
        exe_path,
        product_name,
    })
}

fn probe_candidate_executables(
    walked_roots: &[FolderNode],
    config: &AppConfig,
    catalog_apps: &[InstalledApp],
    manifest_entries: &[ManifestAppEntry],
    steam_lib_roots: &[String],
    shell_folders: &[String],
) -> HashMap<String, RootExeCandidate> {
    let mut out = HashMap::new();
    let known_loc_keys: HashSet<String> = catalog_apps
        .iter()
        .flat_map(|a| a.all_install_locations.iter())
        .chain(manifest_entries.iter().map(|m| &m.install_dir))
        .chain(steam_lib_roots.iter())
        .map(|p| normalize_path_key(p))
        .collect();
    let mut probed: HashSet<String> = HashSet::new();

    let mut try_probe = |path: &str, name: &str| {
        let pk = normalize_path_key(path);
        if pk.len() <= 3
            || !probed.insert(pk.clone())
            || known_loc_keys.contains(&pk)
            || config.zones.matches_personal_dir(path, shell_folders)
            || config.zones.matches_app_container_dir(path).is_some()
        {
            return;
        }
        if let Some(cand) = probe_dir_for_main_exe(path, name) {
            out.insert(pk, cand);
        }
    };

    for root in walked_roots {
        for child in &root.children {
            let pk = normalize_path_key(&child.path);
            let leaf_lower = child.name.to_ascii_lowercase();
            if classify_drive_root_os_folder(&pk, &leaf_lower).is_some()
                || config.zones.matches_personal_dir(&child.path, shell_folders)
            {
                continue;
            }
            if config.zones.matches_app_container_dir(&child.path).is_some() {
                for gc in &child.children {
                    let gpk = normalize_path_key(&gc.path);
                    let g_lower = gc.name.to_ascii_lowercase();
                    if y_core::tree::is_program_files_inbox_folder(&gpk, &g_lower)
                        || known_loc_keys.contains(&gpk)
                    {
                        continue;
                    }
                    try_probe(&gc.path, &gc.name);
                    for ggc in &gc.children {
                        try_probe(&ggc.path, &ggc.name);
                    }
                }
            } else if !known_loc_keys.contains(&pk) {
                try_probe(&child.path, &child.name);
                for gc in &child.children {
                    try_probe(&gc.path, &gc.name);
                }
            }
        }
    }

    for pin in &config.zones.pins {
        let Some(dl) = path_drive_letter(&pin.path) else {
            continue;
        };
        if !config.is_drive_enabled(dl) {
            continue;
        }
        let leaf = Path::new(&pin.path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        match pin.kind {
            PinKind::App => {
                let pk = normalize_path_key(&pin.path);
                if let Some(cand) = probe_dir_for_main_exe(&pin.path, leaf) {
                    out.insert(pk, cand);
                }
            }
            PinKind::Group => {
                if let Ok(rd) = std::fs::read_dir(&pin.path) {
                    for ent in rd.flatten() {
                        if ent.file_type().is_ok_and(|ft| ft.is_dir()) {
                            let child_p = normalize_windows_path(&ent.path().to_string_lossy());
                            let child_n = ent.file_name().to_string_lossy().to_string();
                            let cpk = normalize_path_key(&child_p);
                            if let Some(cand) = probe_dir_for_main_exe(&child_p, &child_n) {
                                out.insert(cpk, cand);
                            }
                        }
                    }
                }
            }
            PinKind::Personal | PinKind::Ignore => {}
        }
    }

    out
}

fn collect_retain_ancestors(
    catalog_apps: &[InstalledApp],
    manifest_entries: &[ManifestAppEntry],
    zones: &ZoneRulebook,
) -> HashSet<String> {
    let mut ancestors = HashSet::new();
    let add_ancestors_of = |raw: &str, out: &mut HashSet<String>| {
        let pk = normalize_path_key(raw);
        let mut cur = pk.as_str();
        while let Some((parent, _)) = cur.trim_end_matches('\\').rsplit_once('\\') {
            if parent.len() <= 2 && parent.ends_with(':') {
                break;
            }
            out.insert(parent.to_string());
            cur = parent;
        }
    };
    for app in catalog_apps {
        for loc in &app.all_install_locations {
            add_ancestors_of(loc, &mut ancestors);
        }
    }
    for m in manifest_entries {
        add_ancestors_of(&m.install_dir, &mut ancestors);
        add_ancestors_of(&m.container_dir, &mut ancestors);
    }
    for ez in &zones.explicit_zones {
        add_ancestors_of(&ez.path, &mut ancestors);
    }
    for pin in &zones.pins {
        add_ancestors_of(&pin.path, &mut ancestors);
    }
    ancestors
}

fn resolve_effective_shell_folders(drives: &[DriveInfo], zones: &ZoneRulebook) -> Vec<String> {
    let os_shells = resolve_personal_shell_folders(drives);
    let mut out = Vec::new();
    let mut seen = HashSet::new();

    for path in os_shells {
        let leaf = Path::new(&path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let is_default_personal = DEFAULT_PERSONAL_NAMES
            .iter()
            .any(|d| d.eq_ignore_ascii_case(leaf));
        let allowed = if is_default_personal {
            zones.root_zones.iter().any(|r| {
                r.kind == ZoneRuleKind::PersonalFiles && r.name.trim().eq_ignore_ascii_case(leaf)
            })
        } else {
            true
        };
        if allowed && zones.pin_for_path(&path).is_none_or(|p| p == PinKind::Personal) {
            let pk = normalize_path_key(&path);
            if seen.insert(pk) {
                out.push(path);
            }
        }
    }

    for drv in drives.iter().filter(|d| d.full_walk_enabled) {
        for rz in zones
            .root_zones
            .iter()
            .filter(|r| r.kind == ZoneRuleKind::PersonalFiles)
        {
            let cand = format!("{}{}", drv.root, rz.name.trim());
            if Path::new(&cand).is_dir() {
                let pk = normalize_path_key(&cand);
                if seen.insert(pk) {
                    out.push(cand);
                }
            }
        }
    }

    for ez in zones
        .explicit_zones
        .iter()
        .filter(|e| e.kind == ZoneRuleKind::PersonalFiles)
    {
        if Path::new(&ez.path).is_dir() {
            let pk = normalize_path_key(&ez.path);
            if seen.insert(pk) {
                out.push(ez.path.clone());
            }
        }
    }

    out
}

pub fn scan_directory_tree_for_test(
    root_path: &str,
    exclusions: &[String],
    sys_win_dir: &str,
) -> FolderNode {
    let sys_win_key = normalize_path_key(sys_win_dir);
    let progress = ProgressCounters::new(Arc::new(AtomicBool::new(false)));
    scan_node_recursive(
        SubtreeTask {
            path: normalize_windows_path(root_path),
            name: root_path.to_string(),
            depth: 0,
            is_drive_root: true,
        },
        exclusions,
        &sys_win_key,
        &HashSet::new(),
        &HashSet::new(),
        &progress,
        None,
    )
}

/// Fixture helper exercising real scoped walking (`drv.full_walk_enabled`) + tree decomposition + attribution
/// without enumerating host volumes or touching registry/disk outside `fixture_drives`.
pub fn execute_scoped_fixture_scan_for_test(
    fixture_drives: Vec<(DriveInfo, String)>,
    installed_apps: Vec<InstalledApp>,
    sys_win_dir: &str,
    config: &AppConfig,
) -> (ScanReport, Vec<char>) {
    let sys_win_key = normalize_path_key(sys_win_dir);
    let progress = ProgressCounters::new(Arc::new(AtomicBool::new(false)));
    let shell_keys: HashSet<String> = HashSet::new();
    let retain_ancestors = collect_retain_ancestors(&installed_apps, &[], &config.zones);

    let mut walked_letters = Vec::new();
    let mut all_units = Vec::new();
    let mut drives = Vec::with_capacity(fixture_drives.len());

    let replace_root_prefix = |path: &str, src_root: &str, dst_drive: char| -> String {
        let norm_p = normalize_windows_path(path);
        let norm_r = normalize_windows_path(src_root).trim_end_matches('\\').to_string();
        if let Some(rest) = norm_p.strip_prefix(&norm_r) {
            let trimmed = rest.trim_start_matches('\\');
            if trimmed.is_empty() {
                format!("{dst_drive}:\\")
            } else {
                format!("{dst_drive}:\\{trimmed}")
            }
        } else {
            norm_p
        }
    };

    for (mut drv, root_dir) in fixture_drives {
        let dl = drv.letter.to_ascii_uppercase();
        drv.full_walk_enabled = config.is_drive_enabled(dl);
        drv.exclusions = config.exclusions_for_drive(dl);
        if drv.full_walk_enabled {
            walked_letters.push(dl);
            let (mut tree, mut root_sys) = parallel_scan_drive_root(
                &root_dir,
                &drv.exclusions,
                &sys_win_key,
                &shell_keys,
                &retain_ancestors,
                &progress,
            );
            fn rewrite_tree(node: &mut FolderNode, src_root: &str, dl: char) {
                let norm_p = normalize_windows_path(&node.path);
                let norm_r = normalize_windows_path(src_root).trim_end_matches('\\').to_string();
                if let Some(rest) = norm_p.strip_prefix(&norm_r) {
                    let trimmed = rest.trim_start_matches('\\');
                    node.path = if trimmed.is_empty() {
                        format!("{dl}:\\")
                    } else {
                        format!("{dl}:\\{trimmed}")
                    };
                }
                for ch in &mut node.children {
                    rewrite_tree(ch, src_root, dl);
                }
            }
            rewrite_tree(&mut tree, &root_dir, dl);
            for (rp, _) in &mut root_sys {
                *rp = replace_root_prefix(rp, &root_dir, dl);
            }
            let units = decompose_scanned_tree_with_options(
                &tree,
                &installed_apps,
                &[],
                &root_sys,
                &DecompositionOptions {
                    zones: Some(&config.zones),
                    manifest_entries: &[],
                    exe_candidates: None,
                },
            );
            all_units.extend(units);
        }
        drives.push(drv);
    }

    let stats = y_core::model::RegistryDiscoveryStats {
        raw_display_name_entries: installed_apps.len(),
        raw_with_install_location: installed_apps
            .iter()
            .filter(|a| a.direct_install_location.is_some())
            .count(),
        canonical_apps: installed_apps.len(),
        canonical_with_direct_install_location: installed_apps
            .iter()
            .filter(|a| a.direct_install_location.is_some())
            .count(),
        canonical_with_inferred_install_location: 0,
    };
    let report = attribute_scan_with_options(
        1000,
        "2026-11-27 19:00:00".to_string(),
        1,
        drives,
        stats,
        &installed_apps,
        &HashMap::new(),
        &all_units,
        None,
        &AttributionOptions {
            containers: Some(&config.containers),
            zones: Some(&config.zones),
            manifest_entries: &[],
            steam_library_roots: &[],
            root_exe_candidates: None,
        },
    );
    (report, walked_letters)
}

pub fn execute_scan_with_progress(
    config: &AppConfig,
    previous_history: Option<&HistoryEntryRef>,
    on_progress: Option<&(dyn Fn(ScanProgressSnapshot) + Sync)>,
) -> ScanReport {
    execute_scan_internal(config, previous_history, None, None, on_progress)
}

pub fn execute_scan_with_history(
    config: &AppConfig,
    history: &ScanHistory,
    on_progress: Option<&(dyn Fn(ScanProgressSnapshot) + Sync)>,
) -> ScanReport {
    execute_scan_internal(config, history.latest_per_app(), Some(history), None, on_progress)
}

pub fn execute_single_drive_rescan(
    base_report: &ScanReport,
    drive_letter: char,
    config: &AppConfig,
    history: &ScanHistory,
    on_progress: Option<&(dyn Fn(ScanProgressSnapshot) + Sync)>,
) -> ScanReport {
    let target = drive_letter.to_ascii_uppercase();
    let single = execute_scan_internal(
        config,
        history.latest_for_drive(target),
        Some(history),
        Some(target),
        on_progress,
    );
    let mut merged = merge_drive_rescan(base_report, target, single, history);
    y_core::categories::classify_report_apps(&mut merged, crate::paths::has_game_manifest_dir);
    crate::icons::populate_report_icons(&mut merged);
    merged
}

fn execute_scan_internal(
    config: &AppConfig,
    previous_history: Option<&HistoryEntryRef>,
    history_ref: Option<&ScanHistory>,
    only_drive: Option<char>,
    on_progress: Option<&(dyn Fn(ScanProgressSnapshot) + Sync)>,
) -> ScanReport {
    let start = std::time::Instant::now();
    let sys_win = get_windows_directory();
    let sys_win_key = normalize_path_key(&sys_win);

    let mut drives: Vec<DriveInfo> = enumerate_fixed_drives();
    for d in &mut drives {
        d.full_walk_enabled = config.is_drive_enabled(d.letter);
        d.exclusions = config.exclusions_for_drive(d.letter);
        if d.is_system_drive
            && !d
                .exclusions
                .iter()
                .any(|e| normalize_path_key(e) == sys_win_key)
        {
            d.exclusions.insert(0, sys_win.clone());
        }
    }

    let raw_reg = enumerate_uninstall_registry();
    let steam_path = read_steam_install_path();
    let mut catalog = build_registry_catalog(&raw_reg, steam_path.as_deref());
    let shell_folders = resolve_effective_shell_folders(&drives, &config.zones);
    let shell_keys: HashSet<String> = shell_folders
        .iter()
        .map(|p| normalize_path_key(p))
        .collect();
    let enabled_letters_pre: HashSet<char> = drives
        .iter()
        .filter(|drv| {
            let dl = drv.letter.to_ascii_uppercase();
            config.is_drive_enabled(dl) && only_drive.is_none_or(|only| only.to_ascii_uppercase() == dl)
        })
        .map(|drv| drv.letter.to_ascii_uppercase())
        .collect();
    let mut manifest_entries: Vec<ManifestAppEntry> = Vec::new();
    discover_epic_manifest_apps(&enabled_letters_pre, &mut manifest_entries);
    let retain_ancestors = collect_retain_ancestors(&catalog.apps, &manifest_entries, &config.zones);

    let cancel = Arc::new(AtomicBool::new(false));
    let progress = ProgressCounters::new(cancel);
    let active_phase = Arc::new(Mutex::new("Preparing scan".to_string()));
    let stop_reporter = Arc::new(AtomicBool::new(false));

    let mut enabled_letters: HashSet<char> = HashSet::new();
    let mut walked_trees: Vec<(FolderNode, Vec<(String, u64)>)> = Vec::new();

    std::thread::scope(|scope| {
        if let Some(cb) = on_progress {
            let p = progress.clone();
            let stop = Arc::clone(&stop_reporter);
            let phase = Arc::clone(&active_phase);
            scope.spawn(move || {
                while !stop.load(Ordering::Relaxed) {
                    let cur_phase = phase.lock().unwrap().clone();
                    let cur_path = p.current_path.lock().unwrap().clone();
                    cb(ScanProgressSnapshot {
                        phase: cur_phase,
                        current_path: cur_path,
                        dirs_walked: p.dirs_walked.load(Ordering::Relaxed),
                        files_counted: p.files_counted.load(Ordering::Relaxed),
                        bytes_counted: p.bytes_counted.load(Ordering::Relaxed),
                    });
                    std::thread::sleep(std::time::Duration::from_millis(120));
                }
            });
        }

        for drv in &drives {
            let dl = drv.letter.to_ascii_uppercase();
            let should_walk = drv.full_walk_enabled
                && only_drive.is_none_or(|only| only.to_ascii_uppercase() == dl);
            if should_walk {
                enabled_letters.insert(dl);
                *active_phase.lock().unwrap() = format!("Scanning {}", drv.root);
                let (root_tree, root_sys) = parallel_scan_drive_root(
                    &drv.root,
                    &drv.exclusions,
                    &sys_win_key,
                    &shell_keys,
                    &retain_ancestors,
                    &progress,
                );
                walked_trees.push((root_tree, root_sys));
            }
        }
        stop_reporter.store(true, Ordering::Relaxed);
    });

    let walked_roots: Vec<FolderNode> = walked_trees.iter().map(|(n, _)| n.clone()).collect();
    let (steam_lib_roots, steam_manifests) = discover_steam_libraries_and_apps(
        &walked_roots,
        &enabled_letters,
        steam_path.as_deref(),
        &mut catalog.apps,
    );
    manifest_entries.extend(steam_manifests);
    let root_exe_candidates = probe_candidate_executables(
        &walked_roots,
        config,
        &catalog.apps,
        &manifest_entries,
        &steam_lib_roots,
        &shell_folders,
    );

    let mut all_units: Vec<ScannedUnit> = Vec::new();
    let decomp_opts = DecompositionOptions {
        zones: Some(&config.zones),
        manifest_entries: &manifest_entries,
        exe_candidates: Some(&root_exe_candidates),
    };
    for (root_tree, root_sys) in &walked_trees {
        let units = decompose_scanned_tree_with_options(
            root_tree,
            &catalog.apps,
            &shell_folders,
            root_sys,
            &decomp_opts,
        );
        all_units.extend(units);
    }

    if let Some(cb) = on_progress {
        cb(ScanProgressSnapshot {
            phase: "Attributing folders".to_string(),
            current_path: String::new(),
            dirs_walked: progress.dirs_walked.load(Ordering::Relaxed),
            files_counted: progress.files_counted.load(Ordering::Relaxed),
            bytes_counted: progress.bytes_counted.load(Ordering::Relaxed),
        });
    }

    let (ts_unix, ts_display) = current_local_timestamp();
    for drv in &mut drives {
        let dl = drv.letter.to_ascii_uppercase();
        if enabled_letters.contains(&dl) {
            drv.last_scanned_unix = Some(ts_unix);
            drv.last_scanned_display = Some(ts_display.clone());
        } else {
            drv.last_scanned_unix = None;
            drv.last_scanned_display = None;
        }
    }

    let options = AttributionOptions {
        containers: Some(&config.containers),
        zones: Some(&config.zones),
        manifest_entries: &manifest_entries,
        steam_library_roots: &steam_lib_roots,
        root_exe_candidates: Some(&root_exe_candidates),
    };
    let mut report = attribute_scan_with_options(
        ts_unix,
        ts_display,
        0,
        drives,
        catalog.stats,
        &catalog.apps,
        &catalog.guid_to_app_name,
        &all_units,
        previous_history,
        &options,
    );
    if let Some(hist) = history_ref {
        recompute_report_growth_from_history(&mut report, hist);
    }

    if let Some(cb) = on_progress {
        cb(ScanProgressSnapshot {
            phase: "Analyzing dependencies".to_string(),
            current_path: String::new(),
            dirs_walked: progress.dirs_walked.load(Ordering::Relaxed),
            files_counted: progress.files_counted.load(Ordering::Relaxed),
            bytes_counted: progress.bytes_counted.load(Ordering::Relaxed),
        });
    }

    crate::dep_scan::enrich_report_with_dependencies(&mut report);
    y_core::categories::classify_report_apps(&mut report, crate::paths::has_game_manifest_dir);
    crate::icons::populate_report_icons(&mut report);
    report.duration_ms = start.elapsed().as_millis() as u64;
    report
}
