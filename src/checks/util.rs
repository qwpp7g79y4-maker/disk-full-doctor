use std::fs;
use std::io;
use std::path::Path;

/// `EACCES` and `EPERM` both surface as [`io::ErrorKind::PermissionDenied`].
pub(crate) fn is_permission(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::PermissionDenied
}

/// `Ok(None)` when `path` does not exist, `Ok(Some(bytes))` for the total
/// size of regular files under it (recursive, symlinks not followed),
/// `Err` on a read failure.
pub(crate) fn dir_bytes(path: &Path) -> io::Result<Option<u64>> {
    let top = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let mut total = 0u64;
    let mut stack = vec![top];
    while let Some(entries) = stack.pop() {
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            let entry_path = entry.path();
            let meta = match fs::symlink_metadata(&entry_path) {
                Ok(meta) => meta,
                Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
                Err(err) => return Err(err),
            };
            if meta.is_dir() {
                match fs::read_dir(&entry_path) {
                    Ok(sub) => stack.push(sub),
                    Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                    Err(err) => return Err(err),
                }
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(Some(total))
}
