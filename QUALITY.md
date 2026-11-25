# Quality

English | [Nederlands](QUALITY.nl.md)

Measured state of commit `555e8c2` (full hash
`555e8c2b5ade7c8280cd791f6f4e4a6dec665192`), measured on 2026-11-25 on
Linux with Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). The working tree was
clean at that commit. Commits after it that only change documentation do not
change these results.

## Measured

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | pass, exit 0, no warnings |
| `cargo test --locked` | pass, exit 0: 104 passed, 0 failed, 0 ignored |
| `cargo test --release --locked` | pass, exit 0: 104 passed, 0 failed, 0 ignored |

The 104 tests are 28 in the library, 5 in the binary, and 71 in the
integration tests under `tests/` (branches 31, cli 10, docker_logs 8,
buildkit 3, handbook_help 3, inodes 3, journald 3, readme_sync 3,
deleted_open 2, df_du_gap 2, stdout_closed 2, package_fixtures 1).

Also measured at this commit: `cargo package --locked --allow-dirty
--no-verify`, unpacking the `.crate` into a clean directory and running
`cargo test --locked` there: 104 passed, 0 failed. Before the fix the same
check gave 1 failed (`fixture_reports_deleted_fd_bytes_and_pid`).

## Gaps checked

| Gap | Result |
| --- | --- |
| Failed write to standard output | Real. `>/dev/full` panicked with exit 101. Fixed in this commit: exit 2, no panic (`tests/stdout_closed.rs`). `/dev/null` stays exit 0. |
| Closed standard output at start | Not fixed. The Rust runtime reopens a closed descriptor 1 as `/dev/null` before `main`, so a closed stream exits 0 and cannot be told from a real `/dev/null` without `unsafe` code. No test is possible. |
| `--output` follows a planted symlink or replaces a special file | Does not apply. There is no `--output` option. |
| Unpacked crate fails a test | Real. `cargo package` turned the committed fd symlinks into regular files, so the deleted_open test failed in the unpacked crate. Fixed: the test creates the symlinks at run time (`tests/package_fixtures.rs` guards it). No file was missing from the package. |
| `build.rs` reads `.git/HEAD` and fails in a worktree | Does not apply. There is no `build.rs`. |

## Not run

These were not run for this measurement. Their result is unknown here.

- `cargo audit`: not run.
- `cargo deny check licenses`: not run.
- Build with the minimum Rust version 1.85: not run.
- Tests on Windows and macOS: not run.
- Result of the GitHub CI jobs for this commit: not run, not read.
- Fuzzing, mutation testing, and coverage: not run.
- Regenerating `THIRD-PARTY-LICENSES.html` with `cargo about`: not run.
