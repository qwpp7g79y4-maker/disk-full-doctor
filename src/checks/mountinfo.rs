//! Mount points from `<root>/proc/self/mountinfo`.
//!
//! This is the only mount source. A missing or unreadable file yields no
//! mounts here; callers decide whether that is NotPresent or NeedsRoot.

use std::fs;

use crate::Root;

/// Mount points in the order they appear, duplicates removed.
pub(crate) fn read_mount_points(root: &Root) -> Result<Vec<String>, std::io::Error> {
    let path = root.join("/proc/self/mountinfo");
    // A single non-UTF-8 byte in one mount point name (legal on Linux;
    // paths are byte strings) must not blind every other mount. Read
    // bytes and decode lossily, the same fallback `unescape_octal`
    // already uses for an escaped byte that does not form valid UTF-8.
    let bytes = fs::read(&path)?;
    let text = String::from_utf8_lossy(&bytes);
    Ok(parse_mountinfo(&text))
}

pub(crate) fn parse_mountinfo(text: &str) -> Vec<String> {
    let mut mounts = Vec::new();
    for line in text.lines() {
        let Some(mount) = parse_mount_point(line) else {
            continue;
        };
        if !mounts.contains(&mount) {
            mounts.push(mount);
        }
    }
    mounts
}

/// Field 5 (index 4) of the pre-separator section is the mount point.
fn parse_mount_point(line: &str) -> Option<String> {
    let (before, _) = line.split_once(" - ")?;
    let mount = before.split_whitespace().nth(4)?;
    Some(unescape_octal(mount))
}

/// mountinfo encodes spaces and other bytes as `\040`-style octal escapes.
/// Escaped bytes and the raw bytes of a multi-byte UTF-8 sequence must be
/// collected first and decoded together, never converted one byte at a
/// time with `char::from(u8)` (that is a Latin-1 mapping, not UTF-8
/// decoding, and corrupts any non-ASCII mount point name).
fn unescape_octal(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\\' && index + 3 < bytes.len() {
            let digits = &bytes[index + 1..index + 4];
            if digits.iter().all(|byte| byte.is_ascii_digit()) {
                let text = std::str::from_utf8(digits).unwrap_or("");
                if let Ok(value) = u8::from_str_radix(text, 8) {
                    decoded.push(value);
                    index += 4;
                    continue;
                }
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    String::from_utf8(decoded)
        .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
}

/// Longest mount point that contains `logical`.
pub(crate) fn mount_for<'a>(logical: &str, mounts: &'a [String]) -> &'a str {
    mounts
        .iter()
        .filter(|mount| path_on_mount(logical, mount))
        .max_by_key(|mount| mount.len())
        .map(String::as_str)
        .unwrap_or("unknown")
}

fn path_on_mount(logical: &str, mount: &str) -> bool {
    if mount == "/" {
        logical.starts_with('/')
    } else {
        logical == mount || logical.starts_with(&format!("{mount}/"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_mount_points_and_unescapes_spaces() {
        let text = "\
22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n\
26 22 8:2 / /var rw shared:1 - ext4 /dev/sda2 rw\n\
27 22 8:3 / /mnt/my\\040disk rw - ext4 /dev/sda3 rw\n\
26 22 8:2 / /var rw - ext4 /dev/sda2 rw\n";
        assert_eq!(
            parse_mountinfo(text),
            vec![
                "/".to_string(),
                "/var".to_string(),
                "/mnt/my disk".to_string(),
            ]
        );
    }

    #[test]
    fn unescapes_octal_without_mangling_multibyte_utf8() {
        let text = "27 22 8:3 / /mnt/café rw - ext4 /dev/sda3 rw\n";
        assert_eq!(parse_mountinfo(text), vec!["/mnt/café".to_string()]);
    }

    #[test]
    fn a_mount_with_invalid_utf8_does_not_blind_the_other_mounts() {
        let dir =
            std::env::temp_dir().join(format!("dfd-mountinfo-invalid-utf8-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("proc/self")).unwrap();
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"22 1 8:1 / / rw,relatime - ext4 /dev/sda1 rw\n");
        bytes.extend_from_slice(b"27 22 8:3 / /mnt/bad\xffname rw - ext4 /dev/sda3 rw\n");
        bytes.extend_from_slice(b"26 22 8:2 / /var rw - ext4 /dev/sda2 rw\n");
        std::fs::write(dir.join("proc/self/mountinfo"), &bytes).unwrap();

        let root = crate::Root::new(&dir);
        let mounts = read_mount_points(&root).expect("one bad mount must not fail the whole read");
        assert!(mounts.contains(&"/".to_string()));
        assert!(mounts.contains(&"/var".to_string()));
        assert_eq!(mounts.len(), 3); // the bad one survives as a lossily-decoded name, not dropped

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn longest_mount_wins() {
        let mounts = vec!["/".to_string(), "/var".to_string(), "/var/lib".to_string()];
        assert_eq!(mount_for("/var/lib/docker/x", &mounts), "/var/lib");
        assert_eq!(mount_for("/var", &mounts), "/var");
        assert_eq!(mount_for("/varix/nope", &mounts), "/");
        assert_eq!(mount_for("relative/path", &mounts), "unknown");
    }

    #[test]
    fn skips_lines_that_have_no_mount_point() {
        let text = "\
not a mount line\n\
22 1 8:1 / - ext4 /dev/sda1 rw\n\
22 1 8:1 / /var rw - ext4 /dev/sda2 rw\n";
        assert_eq!(parse_mountinfo(text), vec!["/var".to_string()]);
    }

    #[test]
    fn invalid_octal_escapes_stay_literal() {
        let text = "27 22 8:3 / /mnt/a\\xyz\\008\\ rw - ext4 /dev/sda3 rw\n";
        assert_eq!(
            parse_mountinfo(text),
            vec!["/mnt/a\\xyz\\008\\".to_string()]
        );
    }

    #[test]
    fn invalid_utf8_octal_is_decoded_lossily() {
        let text = "27 22 8:3 / /mnt/\\377 rw - ext4 /dev/sda3 rw\n";
        assert_eq!(parse_mountinfo(text), vec!["/mnt/\u{FFFD}".to_string()]);
    }
}
