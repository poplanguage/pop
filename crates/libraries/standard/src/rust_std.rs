//! Direct Rust-standard-library implementations for Pop Standard host APIs.

use std::io::{IsTerminal, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use pop_library_bridge::{NativeExport, poplib};

use crate::native_output::{
    POP_STD_TERMINAL_FLUSH_POPLIB_EXPORT, POP_STD_TERMINAL_WRITE_ERROR_POPLIB_EXPORT,
    POP_STD_TERMINAL_WRITE_LINE_POPLIB_EXPORT, POP_STD_TERMINAL_WRITE_POPLIB_EXPORT,
};

#[poplib(
    bubble = Standard,
    namespace = "Pop.Process",
    name = "id",
    parameters(),
    results(Int),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_process_id() -> i64 {
    i64::from(std::process::id())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Process",
    name = "availableParallelism",
    parameters(),
    results(Int),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_available_parallelism() -> i64 {
    std::thread::available_parallelism()
        .ok()
        .and_then(|count| i64::try_from(count.get()).ok())
        .unwrap_or(1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Process",
    name = "executable",
    parameters(),
    results(OptionalString),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_process_executable() -> u64 {
    std::env::current_exe()
        .ok()
        .and_then(|path| path.into_os_string().into_string().ok())
        .map_or(0, |path| {
            pop_internal::runtime::allocate_string(path.as_bytes())
        })
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Platform",
    name = "nativeOperatingSystem",
    parameters(),
    results(Byte),
    effects(),
)]
pub extern "C" fn pop_std_rust_native_operating_system() -> u8 {
    if cfg!(target_os = "linux") {
        1
    } else if cfg!(target_os = "windows") {
        2
    } else if cfg!(target_os = "macos") {
        3
    } else if cfg!(target_os = "android") {
        4
    } else if cfg!(target_os = "ios") {
        5
    } else if cfg!(target_family = "wasm") {
        6
    } else if cfg!(target_family = "unix") {
        7
    } else {
        0
    }
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Platform",
    name = "nativeArchitecture",
    parameters(),
    results(Byte),
    effects(),
)]
pub extern "C" fn pop_std_rust_native_architecture() -> u8 {
    if cfg!(target_arch = "x86") {
        1
    } else if cfg!(target_arch = "x86_64") {
        2
    } else if cfg!(target_arch = "arm") {
        3
    } else if cfg!(target_arch = "aarch64") {
        4
    } else if cfg!(target_arch = "wasm32") {
        5
    } else if cfg!(target_arch = "wasm64") {
        6
    } else {
        0
    }
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Terminal",
    name = "stdoutIsTerminal",
    parameters(),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_stdout_is_terminal() -> bool {
    std::io::stdout().is_terminal()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Terminal",
    name = "stderrIsTerminal",
    parameters(),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_stderr_is_terminal() -> bool {
    std::io::stderr().is_terminal()
}

fn managed_string(reference: u64) -> Option<String> {
    let bytes = pop_internal::runtime::string_bytes(reference)?;
    String::from_utf8(bytes).ok()
}

const MAX_FILE_READ_BYTES: u64 = 64 * 1024 * 1024;

static FILE_ACCESS_NEXT: AtomicU64 = AtomicU64::new(1);
static FILE_ACCESS_ROOTS: LazyLock<Mutex<std::collections::BTreeMap<u64, PathBuf>>> =
    LazyLock::new(|| Mutex::new(std::collections::BTreeMap::new()));
static DIRECTORY_ACCESS_NEXT: AtomicU64 = AtomicU64::new(1);
static DIRECTORY_ACCESS_ROOTS: LazyLock<Mutex<std::collections::BTreeMap<u64, PathBuf>>> =
    LazyLock::new(|| Mutex::new(std::collections::BTreeMap::new()));
static FILE_HANDLE_NEXT: AtomicU64 = AtomicU64::new(1);
static FILE_HANDLES: LazyLock<Mutex<std::collections::BTreeMap<u64, (std::fs::File, bool)>>> =
    LazyLock::new(|| Mutex::new(std::collections::BTreeMap::new()));
static DIRECTORY_SNAPSHOT_NEXT: AtomicU64 = AtomicU64::new(1);
static DIRECTORY_SNAPSHOTS: LazyLock<Mutex<std::collections::BTreeMap<u64, Vec<String>>>> =
    LazyLock::new(|| Mutex::new(std::collections::BTreeMap::new()));

fn file_access_root(token: u64) -> Option<PathBuf> {
    FILE_ACCESS_ROOTS.lock().ok()?.get(&token).cloned()
}

fn file_access_path(token: u64, relative: u64) -> Option<PathBuf> {
    let relative = managed_string(relative)?;
    let path = Path::new(&relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir
        )
    }) {
        return None;
    }
    let root = file_access_root(token)?;
    let candidate = root.join(path);
    let canonical = std::fs::canonicalize(&candidate).ok()?;
    canonical.starts_with(&root).then_some(canonical)
}

fn file_access_creation_path(token: u64, relative: u64) -> Option<PathBuf> {
    let relative = managed_string(relative)?;
    let path = Path::new(&relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir
        )
    }) {
        return None;
    }
    let root = file_access_root(token)?;
    let candidate = root.join(path);
    if candidate.exists() {
        return None;
    }
    let parent = candidate.parent()?;
    let canonical_parent = std::fs::canonicalize(parent).ok()?;
    canonical_parent.starts_with(&root).then_some(candidate)
}

fn directory_access_path(token: u64, relative: u64) -> Option<PathBuf> {
    let relative = managed_string(relative)?;
    let path = Path::new(&relative);
    if path.components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir
        )
    }) {
        return None;
    }
    let root = DIRECTORY_ACCESS_ROOTS.lock().ok()?.get(&token).cloned()?;
    let candidate = root.join(path);
    let canonical = std::fs::canonicalize(&candidate).ok()?;
    canonical.starts_with(&root).then_some(canonical)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory.Access",
    name = "open",
    parameters(String),
    results(DirectoryAccess),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_access_open(path: u64) -> u64 {
    let Some(path) = managed_string(path) else {
        return 0;
    };
    let Ok(root) = std::fs::canonicalize(path) else {
        return 0;
    };
    if !root.is_dir() {
        return 0;
    }
    let token = DIRECTORY_ACCESS_NEXT.fetch_add(1, Ordering::Relaxed);
    if token == 0 {
        return 0;
    }
    let Ok(mut roots) = DIRECTORY_ACCESS_ROOTS.lock() else {
        return 0;
    };
    roots.insert(token, root);
    token
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory.Access",
    name = "close",
    parameters(DirectoryAccess),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_directory_access_close(token: u64) -> bool {
    DIRECTORY_ACCESS_ROOTS
        .lock()
        .ok()
        .and_then(|mut roots| roots.remove(&token))
        .is_some()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "exists",
    parameters(DirectoryAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_exists_at(token: u64, path: u64) -> bool {
    directory_access_path(token, path).is_some_and(|path| path.exists())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "isDirectory",
    parameters(DirectoryAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_is_directory_at(token: u64, path: u64) -> bool {
    directory_access_path(token, path).is_some_and(|path| path.is_dir())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "create",
    parameters(DirectoryAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_create(token: u64, path: u64) -> bool {
    let Some(relative) = managed_string(path) else {
        return false;
    };
    let root = DIRECTORY_ACCESS_ROOTS
        .lock()
        .ok()
        .and_then(|roots| roots.get(&token).cloned());
    let Some(root) = root else {
        return false;
    };
    let candidate = root.join(Path::new(&relative));
    if Path::new(&relative).components().any(|component| {
        matches!(
            component,
            Component::RootDir | Component::Prefix(_) | Component::ParentDir
        )
    }) || candidate.exists()
    {
        return false;
    }
    candidate.starts_with(&root) && std::fs::create_dir(candidate).is_ok()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "remove",
    parameters(DirectoryAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_remove(token: u64, path: u64) -> bool {
    let Some(path) = directory_access_path(token, path) else {
        return false;
    };
    std::fs::remove_dir(path).is_ok()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "list",
    parameters(DirectoryAccess, String, UInt64),
    results(DirectorySnapshot),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_list(token: u64, path: u64, maximum_entries: u64) -> u64 {
    if maximum_entries > 65_536 {
        return 0;
    }
    let Some(path) = directory_access_path(token, path) else {
        return 0;
    };
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    let Ok(limit) = usize::try_from(maximum_entries) else {
        return 0;
    };
    let mut names = Vec::new();
    for entry in entries {
        let Ok(entry) = entry else { return 0 };
        let Ok(name) = entry.file_name().into_string() else {
            return 0;
        };
        names.push(name);
        if names.len() > limit {
            return 0;
        }
    }
    names.sort();
    let snapshot = DIRECTORY_SNAPSHOT_NEXT.fetch_add(1, Ordering::Relaxed);
    if snapshot == 0 {
        return 0;
    }
    let Ok(mut snapshots) = DIRECTORY_SNAPSHOTS.lock() else {
        return 0;
    };
    snapshots.insert(snapshot, names);
    snapshot
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory.Snapshot",
    name = "close",
    parameters(DirectorySnapshot),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_directory_snapshot_close(snapshot: u64) -> bool {
    DIRECTORY_SNAPSHOTS
        .lock()
        .ok()
        .and_then(|mut snapshots| snapshots.remove(&snapshot))
        .is_some()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory.Snapshot",
    name = "count",
    parameters(DirectorySnapshot),
    results(UInt64),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_directory_snapshot_count(snapshot: u64) -> u64 {
    DIRECTORY_SNAPSHOTS
        .lock()
        .ok()
        .and_then(|snapshots| snapshots.get(&snapshot).map(|names| names.len() as u64))
        .unwrap_or(0)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory.Snapshot",
    name = "name",
    parameters(DirectorySnapshot, UInt64),
    results(OptionalString),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_directory_snapshot_name(snapshot: u64, index: u64) -> u64 {
    let Ok(index) = usize::try_from(index) else {
        return 0;
    };
    let Ok(snapshots) = DIRECTORY_SNAPSHOTS.lock() else {
        return 0;
    };
    let Some(name) = snapshots.get(&snapshot).and_then(|names| names.get(index)) else {
        return 0;
    };
    pop_internal::runtime::allocate_string(name.as_bytes())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File.Access",
    name = "open",
    parameters(String),
    results(FileAccess),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_access_open(path: u64) -> u64 {
    let Some(path) = managed_string(path) else {
        return 0;
    };
    let Ok(root) = std::fs::canonicalize(path) else {
        return 0;
    };
    if !root.is_dir() {
        return 0;
    }
    let token = FILE_ACCESS_NEXT.fetch_add(1, Ordering::Relaxed);
    if token == 0 {
        return 0;
    }
    let Ok(mut roots) = FILE_ACCESS_ROOTS.lock() else {
        return 0;
    };
    roots.insert(token, root);
    token
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File.Access",
    name = "close",
    parameters(FileAccess),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_access_close(token: u64) -> bool {
    FILE_ACCESS_ROOTS
        .lock()
        .ok()
        .and_then(|mut roots| roots.remove(&token))
        .is_some()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "exists",
    parameters(FileAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_exists_at(token: u64, path: u64) -> bool {
    file_access_path(token, path).is_some_and(|path| path.exists())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "isFile",
    parameters(FileAccess, String),
    results(Boolean),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_is_file_at(token: u64, path: u64) -> bool {
    file_access_path(token, path).is_some_and(|path| path.is_file())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "open",
    parameters(FileAccess, String),
    results(FileHandle),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_open(token: u64, path: u64) -> u64 {
    let Some(path) = file_access_path(token, path) else {
        return 0;
    };
    if !path.is_file() {
        return 0;
    }
    let Ok(file) = std::fs::File::open(path) else {
        return 0;
    };
    let handle = FILE_HANDLE_NEXT.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return 0;
    };
    files.insert(handle, (file, false));
    handle
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "openWrite",
    parameters(FileAccess, String),
    results(FileHandle),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_open_write(token: u64, path: u64) -> u64 {
    let Some(path) = file_access_path(token, path) else {
        return 0;
    };
    if !path.is_file() {
        return 0;
    }
    let Ok(file) = std::fs::OpenOptions::new().write(true).open(path) else {
        return 0;
    };
    let handle = FILE_HANDLE_NEXT.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return 0;
    };
    files.insert(handle, (file, true));
    handle
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "create",
    parameters(FileAccess, String),
    results(FileHandle),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_create(token: u64, path: u64) -> u64 {
    let Some(path) = file_access_creation_path(token, path) else {
        return 0;
    };
    let Ok(file) = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
    else {
        return 0;
    };
    let handle = FILE_HANDLE_NEXT.fetch_add(1, Ordering::Relaxed);
    if handle == 0 {
        return 0;
    }
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return 0;
    };
    files.insert(handle, (file, true));
    handle
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "read",
    parameters(FileHandle, ManagedReference, UInt64),
    results(Int),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_handle_read(handle: u64, buffer: u64, maximum: u64) -> i64 {
    if maximum > MAX_FILE_READ_BYTES || !pop_internal::runtime::byte_buffer_clear(buffer) {
        return -1;
    }
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return -1;
    };
    let Some((file, _)) = files.get_mut(&handle) else {
        return -1;
    };
    let mut bytes = Vec::new();
    if file.take(maximum).read_to_end(&mut bytes).is_err() {
        return -1;
    }
    for byte in bytes.iter().copied() {
        if !pop_internal::runtime::byte_buffer_write_byte(buffer, byte) {
            return -1;
        }
    }
    i64::try_from(bytes.len()).unwrap_or(-1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "write",
    parameters(FileHandle, ManagedReference, UInt64),
    results(Int),
    effects(AmbientIo, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_write(handle: u64, buffer: u64, maximum: u64) -> i64 {
    if maximum > MAX_FILE_READ_BYTES {
        return -1;
    }
    let Some(bytes) = pop_internal::runtime::byte_buffer_bytes(buffer) else {
        return -1;
    };
    let Ok(limit) = usize::try_from(maximum) else {
        return -1;
    };
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return -1;
    };
    let Some((file, writable)) = files.get_mut(&handle) else {
        return -1;
    };
    if !*writable {
        return -1;
    }
    let bytes = &bytes[..bytes.len().min(limit)];
    if file.write_all(bytes).is_err() {
        return -1;
    }
    i64::try_from(bytes.len()).unwrap_or(-1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Io",
    name = "copyFiles",
    parameters(FileHandle, FileHandle, UInt64),
    results(Int),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_io_copy_files(source: u64, destination: u64, maximum: u64) -> i64 {
    if source == destination || maximum > MAX_FILE_READ_BYTES {
        return -1;
    }
    let Ok(mut files) = FILE_HANDLES.lock() else {
        return -1;
    };
    let Some((mut input, input_writable)) = files.remove(&source) else {
        return -1;
    };
    let Some((mut output, output_writable)) = files.remove(&destination) else {
        files.insert(source, (input, input_writable));
        return -1;
    };
    if output_writable && !input_writable {
        let mut copied = 0_u64;
        let mut buffer = vec![0_u8; 64 * 1024];
        let result = (|| {
            while copied < maximum {
                let remaining = maximum - copied;
                let chunk = usize::try_from(remaining.min(buffer.len() as u64)).ok()?;
                let read = input.read(&mut buffer[..chunk]).ok()?;
                if read == 0 {
                    break;
                }
                output.write_all(&buffer[..read]).ok()?;
                copied = copied.checked_add(u64::try_from(read).ok()?)?;
            }
            Some(copied)
        })();
        files.insert(source, (input, input_writable));
        files.insert(destination, (output, output_writable));
        return result
            .and_then(|value| i64::try_from(value).ok())
            .unwrap_or(-1);
    }
    files.insert(source, (input, input_writable));
    files.insert(destination, (output, output_writable));
    -1
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "close",
    parameters(FileHandle),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_close(handle: u64) -> bool {
    FILE_HANDLES
        .lock()
        .ok()
        .and_then(|mut files| files.remove(&handle))
        .is_some()
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "read",
    parameters(FileAccess, String, ManagedReference, UInt64),
    results(Int),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_read_at(
    token: u64,
    path: u64,
    buffer: u64,
    maximum: u64,
) -> i64 {
    let Some(path) = file_access_path(token, path) else {
        return -1;
    };
    if maximum > MAX_FILE_READ_BYTES || !pop_internal::runtime::byte_buffer_clear(buffer) {
        return -1;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return -1;
    };
    let mut bytes = Vec::new();
    if std::io::Read::by_ref(&mut file)
        .take(maximum)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return -1;
    }
    for byte in bytes.iter().copied() {
        if !pop_internal::runtime::byte_buffer_write_byte(buffer, byte) {
            return -1;
        }
    }
    i64::try_from(bytes.len()).unwrap_or(-1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "read",
    parameters(String, ManagedReference, UInt64),
    results(Int),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_file_read(path: u64, buffer: u64, maximum: u64) -> i64 {
    let Some(path) = managed_string(path) else {
        return -1;
    };
    if maximum > MAX_FILE_READ_BYTES {
        return -1;
    }
    // SAFETY: these are the stable native byte-buffer ABI functions and the
    // caller supplies an opaque buffer token produced by Pop.Bytes.
    if !pop_internal::runtime::byte_buffer_clear(buffer) {
        return -1;
    }
    let Ok(mut file) = std::fs::File::open(path) else {
        return -1;
    };
    let Ok(limit) = usize::try_from(maximum) else {
        return -1;
    };
    let mut bytes = Vec::new();
    if std::io::Read::by_ref(&mut file)
        .take(maximum)
        .read_to_end(&mut bytes)
        .is_err()
    {
        return -1;
    }
    if bytes.len() > limit {
        return -1;
    }
    for byte in bytes.iter().copied() {
        // SAFETY: `buffer` remains an opaque buffer token for this call.
        if !pop_internal::runtime::byte_buffer_write_byte(buffer, byte) {
            return -1;
        }
    }
    i64::try_from(bytes.len()).unwrap_or(-1)
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Environment",
    name = "has",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_environment_has(name: u64) -> bool {
    managed_string(name).is_some_and(|name| std::env::var_os(name).is_some())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Environment",
    name = "get",
    parameters(String),
    results(OptionalString),
    effects(AmbientIo, Allocates, MayTrap),
)]
pub extern "C" fn pop_std_rust_environment_get(name: u64) -> u64 {
    let Some(name) = managed_string(name) else {
        return 0;
    };
    let Some(value) = std::env::var_os(name).and_then(|value| value.into_string().ok()) else {
        return 0;
    };
    pop_internal::runtime::allocate_string(value.as_bytes())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "exists",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_exists(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).exists())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.File",
    name = "isFile",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_file_is_file(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).is_file())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.Directory",
    name = "exists",
    parameters(String),
    results(Boolean),
    effects(AmbientIo),
)]
pub extern "C" fn pop_std_rust_directory_exists(path: u64) -> bool {
    managed_string(path).is_some_and(|path| Path::new(&path).is_dir())
}

fn ipv4(bits: u64) -> Option<Ipv4Addr> {
    u32::try_from(bits).ok().map(Ipv4Addr::from)
}

fn ipv6(first: u64, second: u64, third: u64, fourth: u64) -> Option<Ipv6Addr> {
    let words = [
        u32::try_from(first).ok()?,
        u32::try_from(second).ok()?,
        u32::try_from(third).ok()?,
        u32::try_from(fourth).ok()?,
    ];
    let mut octets = [0_u8; 16];
    for (index, word) in words.into_iter().enumerate() {
        octets[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    Some(Ipv6Addr::from(octets))
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsLinkLocal",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_link_local(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_link_local())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsMulticast",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_multicast(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_multicast())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsBroadcast",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_broadcast(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_broadcast())
}

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv4IsDocumentation",
    parameters(UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv4_is_documentation(bits: u64) -> bool {
    ipv4(bits).is_some_and(|address| address.is_documentation())
}

macro_rules! ipv6_classifier {
    ($function:ident, $binding:literal, $method:ident) => {
        #[poplib(
                                                    bubble = Standard,
                                                    namespace = "Pop.RustNet",
                                                    name = $binding,
                                                    parameters(UInt64, UInt64, UInt64, UInt64),
                                                    results(Boolean),
                                                    effects(),
                                                )]
        pub extern "C" fn $function(first: u64, second: u64, third: u64, fourth: u64) -> bool {
            ipv6(first, second, third, fourth).is_some_and(|address| address.$method())
        }
    };
}

ipv6_classifier!(
    pop_std_rust_net_ipv6_is_multicast,
    "ipv6IsMulticast",
    is_multicast
);
ipv6_classifier!(
    pop_std_rust_net_ipv6_is_unique_local,
    "ipv6IsUniqueLocal",
    is_unique_local
);
ipv6_classifier!(
    pop_std_rust_net_ipv6_is_unicast_link_local,
    "ipv6IsUnicastLinkLocal",
    is_unicast_link_local
);

#[poplib(
    bubble = Standard,
    namespace = "Pop.RustNet",
    name = "ipv6IsDocumentation",
    parameters(UInt64, UInt64, UInt64, UInt64),
    results(Boolean),
    effects(),
)]
pub extern "C" fn pop_std_rust_net_ipv6_is_documentation(
    first: u64,
    second: u64,
    third: u64,
    fourth: u64,
) -> bool {
    ipv6(first, second, third, fourth)
        .is_some_and(|address| address.segments()[..2] == [0x2001, 0x0db8])
}

pub const RUST_STD_EXPORTS: &[NativeExport] = &[
    POP_STD_RUST_PROCESS_ID_POPLIB_EXPORT,
    POP_STD_RUST_AVAILABLE_PARALLELISM_POPLIB_EXPORT,
    POP_STD_RUST_STDOUT_IS_TERMINAL_POPLIB_EXPORT,
    POP_STD_RUST_STDERR_IS_TERMINAL_POPLIB_EXPORT,
    POP_STD_TERMINAL_WRITE_POPLIB_EXPORT,
    POP_STD_TERMINAL_WRITE_LINE_POPLIB_EXPORT,
    POP_STD_TERMINAL_WRITE_ERROR_POPLIB_EXPORT,
    POP_STD_TERMINAL_FLUSH_POPLIB_EXPORT,
    POP_STD_RUST_ENVIRONMENT_HAS_POPLIB_EXPORT,
    POP_STD_RUST_ENVIRONMENT_GET_POPLIB_EXPORT,
    POP_STD_RUST_PROCESS_EXECUTABLE_POPLIB_EXPORT,
    POP_STD_RUST_FILE_EXISTS_POPLIB_EXPORT,
    POP_STD_RUST_FILE_IS_FILE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_EXISTS_POPLIB_EXPORT,
    POP_STD_RUST_FILE_READ_POPLIB_EXPORT,
    POP_STD_RUST_FILE_ACCESS_OPEN_POPLIB_EXPORT,
    POP_STD_RUST_FILE_ACCESS_CLOSE_POPLIB_EXPORT,
    POP_STD_RUST_FILE_EXISTS_AT_POPLIB_EXPORT,
    POP_STD_RUST_FILE_IS_FILE_AT_POPLIB_EXPORT,
    POP_STD_RUST_FILE_READ_AT_POPLIB_EXPORT,
    POP_STD_RUST_FILE_OPEN_POPLIB_EXPORT,
    POP_STD_RUST_FILE_HANDLE_READ_POPLIB_EXPORT,
    POP_STD_RUST_FILE_CLOSE_POPLIB_EXPORT,
    POP_STD_RUST_FILE_OPEN_WRITE_POPLIB_EXPORT,
    POP_STD_RUST_FILE_CREATE_POPLIB_EXPORT,
    POP_STD_RUST_FILE_WRITE_POPLIB_EXPORT,
    POP_STD_RUST_IO_COPY_FILES_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_ACCESS_OPEN_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_ACCESS_CLOSE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_EXISTS_AT_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_IS_DIRECTORY_AT_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_CREATE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_REMOVE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_LIST_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_SNAPSHOT_CLOSE_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_SNAPSHOT_COUNT_POPLIB_EXPORT,
    POP_STD_RUST_DIRECTORY_SNAPSHOT_NAME_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_LINK_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_MULTICAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_BROADCAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV4_IS_DOCUMENTATION_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_MULTICAST_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_UNIQUE_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_UNICAST_LINK_LOCAL_POPLIB_EXPORT,
    POP_STD_RUST_NET_IPV6_IS_DOCUMENTATION_POPLIB_EXPORT,
    POP_STD_RUST_NATIVE_OPERATING_SYSTEM_POPLIB_EXPORT,
    POP_STD_RUST_NATIVE_ARCHITECTURE_POPLIB_EXPORT,
];
