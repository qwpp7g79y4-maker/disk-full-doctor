//! Command-line entry point for disk-full-doctor.

#![forbid(unsafe_code)]

use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;
use disk_full_doctor::{scan, RealFsProbe, Report, Root};

/// Read-only report of why a disk looks full when df and du disagree.
#[derive(Parser, Debug)]
#[command(name = "disk-full-doctor", version, about)]
struct Cli {
    /// Print the report as JSON.
    #[arg(long)]
    json: bool,

    /// Read system paths under this directory instead of /.
    #[arg(long, hide = true)]
    root: Option<PathBuf>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let root = match root_from_cli(cli.root) {
        Ok(root) => root,
        Err(message) => {
            eprintln!("disk-full-doctor: {message}");
            return ExitCode::from(1);
        }
    };
    let probe = RealFsProbe;
    let report = scan(&root, &probe);
    print_report(&report, cli.json);
    ExitCode::from(report.exit_code() as u8)
}

/// `--root` must name a path that exists. A missing path is a caller error:
/// the checks would report NotPresent and exit 0. Permission denied stays
/// with the checks, which report NeedsRoot.
fn root_from_cli(path: Option<PathBuf>) -> Result<Root, String> {
    let Some(path) = path else {
        return Ok(Root::system());
    };
    match std::fs::metadata(&path) {
        Ok(_) => Ok(Root::new(path)),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            Err(format!("root path {} does not exist", path.display()))
        }
        Err(err) if err.kind() == ErrorKind::PermissionDenied => Ok(Root::new(path)),
        Err(err) => Err(format!("cannot read root path {}: {err}", path.display())),
    }
}

fn print_report(report: &Report, json: bool) {
    if json {
        print!("{}", report.render_json());
    } else {
        print!("{}", report.render_text());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn absent_flag_is_the_system_prefix_and_does_not_read_it() {
        let root = root_from_cli(None).expect("system prefix");
        assert_eq!(root.prefix(), std::path::Path::new("/"));
    }

    #[test]
    fn missing_root_is_an_error() {
        let missing = std::env::temp_dir().join(format!("dfd-main-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&missing);
        let err = root_from_cli(Some(missing.clone())).expect_err("missing path");
        assert!(err.contains("does not exist"), "{err}");
        assert!(err.contains(&missing.display().to_string()));
    }

    #[test]
    fn existing_directory_is_accepted() {
        let dir = std::env::temp_dir().join(format!("dfd-main-ok-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let root = root_from_cli(Some(dir.clone())).expect("directory");
        assert_eq!(root.prefix(), dir);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn permission_denied_is_left_to_the_checks() {
        let parent = std::env::temp_dir().join(format!("dfd-main-lock-{}", std::process::id()));
        let child = parent.join("child");
        let _ = fs::remove_dir_all(&parent);
        fs::create_dir_all(&child).expect("child");
        let mut perms = fs::metadata(&parent).expect("meta").permissions();
        perms.set_mode(0o000);
        fs::set_permissions(&parent, perms).expect("chmod");
        let result = root_from_cli(Some(child.clone()));
        let mut perms = fs::metadata(&parent)
            .expect("meta after chmod")
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&parent, perms).expect("restore");
        let _ = fs::remove_dir_all(&parent);
        let root = result.expect("an unreadable root is still scanned");
        assert_eq!(root.prefix(), child);
    }

    #[test]
    fn symlink_loop_is_an_error_without_panic() {
        let dir = std::env::temp_dir().join(format!("dfd-main-loop-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("dir");
        let loop_path = dir.join("loop");
        std::os::unix::fs::symlink("loop", &loop_path).expect("symlink");
        let err = root_from_cli(Some(loop_path)).expect_err("symlink loop");
        assert!(err.contains("cannot read"), "{err}");
        assert!(!err.contains("panicked"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
