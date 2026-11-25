//! The manual page matches the binary's `--help`, and the example command
//! exits with the code `examples/README.md` names.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Cargo 1.99 sets `CARGO_BIN_EXE_disk-full-doctor` (the binary name as
/// written in Cargo.toml). Prefer `CARGO_BIN_EXE_disk_full_doctor` when the
/// toolchain provides that name.
fn doctor() -> Command {
    let path = option_env!("CARGO_BIN_EXE_disk_full_doctor")
        .unwrap_or(env!("CARGO_BIN_EXE_disk-full-doctor"));
    Command::new(path)
}

fn manifest() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn long_options(text: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(
                c,
                '[' | ']' | '(' | ')' | '{' | '}' | ',' | '.' | ';' | ':' | '\'' | '"' | '`'
            )
        });
        let Some(name) = token.strip_prefix("--") else {
            continue;
        };
        if !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            found.insert(format!("--{name}"));
        }
    }
    found
}

/// Subcommand names listed under the `Commands:` heading of `--help`.
fn subcommands_named_by_help(help: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_section = false;
    for line in help.lines() {
        if line.trim() == "Commands:" {
            in_section = true;
            continue;
        }
        if !in_section {
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        if !line.starts_with(' ') && !line.starts_with('\t') {
            break;
        }
        let Some(name) = line.split_whitespace().next() else {
            continue;
        };
        if name.starts_with('-') {
            continue;
        }
        names.push(name.to_string());
    }
    names
}

fn fenced_blocks(text: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current: Option<Vec<&str>> = None;
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            match current.take() {
                Some(lines) => blocks.push(lines.join("\n")),
                None => current = Some(Vec::new()),
            }
        } else if let Some(lines) = current.as_mut() {
            lines.push(line);
        }
    }
    assert!(
        current.is_none(),
        "unterminated code fence in examples/README.md"
    );
    blocks
}

#[test]
fn help_long_options_match_the_manual() {
    let help = doctor().arg("--help").output().expect("run --help");
    assert_eq!(help.status.code(), Some(0));
    let stdout = String::from_utf8(help.stdout).expect("help is utf-8");
    let stderr = String::from_utf8_lossy(&help.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
    assert!(!stdout.is_empty());

    let mut from_help = long_options(&stdout);
    assert!(from_help.contains("--help"), "{stdout}");
    for name in subcommands_named_by_help(&stdout) {
        let child = doctor()
            .args([&name, "--help"])
            .output()
            .expect("run subcommand --help");
        assert_eq!(
            child.status.code(),
            Some(0),
            "{name} --help stderr: {}",
            String::from_utf8_lossy(&child.stderr)
        );
        let child_stdout = String::from_utf8(child.stdout).expect("subcommand help is utf-8");
        from_help.extend(long_options(&child_stdout));
    }

    let man = fs::read_to_string(manifest().join("man/disk-full-doctor.1")).expect("read man page");
    for section in [
        "NAME",
        "SYNOPSIS",
        "DESCRIPTION",
        "OPTIONS",
        "EXIT STATUS",
        "EXAMPLES",
    ] {
        assert!(
            man.contains(&format!(".SH {section}")),
            "man page is missing .SH {section}"
        );
    }
    let from_man = long_options(&man);
    assert_eq!(
        from_man, from_help,
        "man page long options must equal --help"
    );
}

#[test]
fn unknown_option_exits_nonzero_without_panic() {
    let output = doctor()
        .arg("--not-a-real-flag")
        .output()
        .expect("run unknown option");
    assert_ne!(output.status.code(), Some(0));
    assert!(output.status.code().is_some());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn example_command_exits_as_documented() {
    let readme =
        fs::read_to_string(manifest().join("examples/README.md")).expect("examples readme");
    let blocks = fenced_blocks(&readme);
    assert_eq!(blocks.len(), 1, "examples/README.md has one command");
    let command = blocks[0].trim();
    assert!(
        !command.contains('\n'),
        "the example command is one line: {command}"
    );
    let mut parts = command.split_whitespace();
    assert_eq!(parts.next(), Some("disk-full-doctor"));
    let args: Vec<&str> = parts.collect();

    let root = example_root(&args);
    assert_ne!(root, Path::new("/"));
    let root = manifest().join(root);
    assert!(root.is_dir(), "missing example tree {}", root.display());
    let canonical = root.canonicalize().expect("canonicalize example tree");
    assert_ne!(canonical, PathBuf::from("/"));
    assert!(
        canonical.starts_with(manifest()),
        "example tree left the project: {}",
        canonical.display()
    );

    let output = doctor()
        .args(&args)
        .current_dir(manifest())
        .output()
        .expect("run example command");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("panicked"));
    assert_eq!(output.status.code(), Some(expected_exit(&readme)));
}

fn example_root<'a>(args: &[&'a str]) -> &'a str {
    let mut args = args.iter();
    while let Some(arg) = args.next() {
        assert_ne!(*arg, "/", "example must not name /");
        if *arg == "--root" {
            let root = args.next().copied().expect("example --root needs a path");
            assert_ne!(root, "/");
            return root;
        }
        if let Some(root) = arg.strip_prefix("--root=") {
            assert_ne!(root, "/");
            return root;
        }
    }
    panic!("example command has no --root path; refusing to scan /");
}

fn expected_exit(readme: &str) -> i32 {
    let mut found = None;
    for line in readme.lines() {
        let Some(rest) = line.trim().strip_prefix("Expected exit code:") else {
            continue;
        };
        let code: i32 = rest.trim().trim_matches('`').parse().expect("exit code");
        assert!(found.is_none(), "examples/README.md names one exit code");
        found = Some(code);
    }
    found.expect("examples/README.md names an expected exit code")
}
