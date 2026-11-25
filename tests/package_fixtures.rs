//! `cargo package` stores a symlink as the regular file it points to, so a
//! committed symlink silently changes meaning in the unpacked crate. Fixtures
//! that need a symlink create it at test time instead.

use std::fs;
use std::path::{Path, PathBuf};

fn symlinks_under(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read fixture dir").flatten() {
        let path = entry.path();
        let kind = entry.file_type().expect("file type");
        if kind.is_symlink() {
            found.push(path);
        } else if kind.is_dir() {
            symlinks_under(&path, found);
        }
    }
}

#[test]
fn no_committed_fixture_is_a_symlink() {
    let mut found = Vec::new();
    symlinks_under(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures"),
        &mut found,
    );
    assert!(found.is_empty(), "symlinks in fixtures: {found:?}");
}
