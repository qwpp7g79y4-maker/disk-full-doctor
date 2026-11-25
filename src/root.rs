//! Prefix that keeps every check inside one tree.
//!
//! Production uses `/`. Tests pass a fixture directory. Logical paths such as
//! `/proc/self/mountinfo` are joined onto that prefix, so a check never opens
//! the host path by accident.

use std::path::{Component, Path, PathBuf};

/// Filesystem prefix for every path a check reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Root {
    prefix: PathBuf,
}

impl Root {
    pub fn new(prefix: impl Into<PathBuf>) -> Self {
        Self {
            prefix: prefix.into(),
        }
    }

    /// The host filesystem root.
    pub fn system() -> Self {
        Self::new("/")
    }

    pub fn prefix(&self) -> &Path {
        &self.prefix
    }

    /// Map a logical absolute path onto this prefix.
    ///
    /// `/proc/1/fd` with prefix `fixtures` becomes `fixtures/proc/1/fd`.
    /// Parent components (`..`) are dropped so a logical path cannot climb
    /// out of the prefix.
    pub fn join(&self, logical: impl AsRef<Path>) -> PathBuf {
        let logical = logical.as_ref();
        let relative = logical.strip_prefix("/").unwrap_or(logical);
        let mut out = self.prefix.clone();
        if relative.as_os_str().is_empty() {
            return out;
        }
        for component in relative.components() {
            match component {
                Component::Normal(part) => out.push(part),
                Component::CurDir
                | Component::ParentDir
                | Component::RootDir
                | Component::Prefix(_) => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_keeps_logical_proc_under_the_prefix() {
        let root = Root::new("/tmp/dfd-fixture");
        let joined = root.join("/proc/self/mountinfo");
        assert_eq!(
            joined,
            PathBuf::from("/tmp/dfd-fixture/proc/self/mountinfo")
        );
        assert_ne!(joined, PathBuf::from("/proc/self/mountinfo"));
    }

    #[test]
    fn join_of_root_is_the_prefix() {
        let root = Root::new("/tmp/dfd-fixture");
        assert_eq!(root.join("/"), PathBuf::from("/tmp/dfd-fixture"));
    }

    #[test]
    fn join_drops_parent_components() {
        let root = Root::new("/tmp/dfd-fixture");
        let joined = root.join("/proc/../../etc/passwd");
        assert!(joined.starts_with("/tmp/dfd-fixture"));
        assert_eq!(joined, PathBuf::from("/tmp/dfd-fixture/proc/etc/passwd"));
    }

    #[test]
    fn system_prefix_is_slash_and_is_not_read_here() {
        let root = Root::system();
        assert_eq!(root.prefix(), Path::new("/"));
        assert_eq!(
            root.join("/proc/self/mountinfo"),
            PathBuf::from("/proc/self/mountinfo")
        );
    }

    #[test]
    fn join_accepts_a_relative_path_and_drops_dot() {
        let root = Root::new("/tmp/dfd-fixture");
        assert_eq!(
            root.join("proc/self/mountinfo"),
            PathBuf::from("/tmp/dfd-fixture/proc/self/mountinfo")
        );
        assert_eq!(
            root.join("/proc/./self"),
            PathBuf::from("/tmp/dfd-fixture/proc/self")
        );
        assert_eq!(root.join("/./.."), PathBuf::from("/tmp/dfd-fixture"));
        assert_eq!(root.join(""), PathBuf::from("/tmp/dfd-fixture"));
    }
}
