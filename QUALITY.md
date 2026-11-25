# Quality

English | [Nederlands](QUALITY.nl.md)

Measured state of commit `4e8eb69` (full hash
`4e8eb69554b990703509237ab00820c2e2c4cdb5`), measured on 2026-11-25 on
Linux with Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). The working tree was
clean at that commit. Commits after it that only change documentation do not
change these results.

## Measured

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | pass, exit 0, no warnings |
| `cargo test --locked` | pass, exit 0: 105 passed, 0 failed, 0 ignored |
| `cargo test --release --locked` | pass, exit 0: 105 passed, 0 failed, 0 ignored |

The 105 tests are 28 in the library, 5 in the binary, and 72 in the
integration tests under `tests/` (branches 31, cli 10, docker_logs 8,
buildkit 3, handbook_help 3, inodes 3, journald 3, readme_sync 3,
deleted_open 2, df_du_gap 2, stdout_closed 3, package_fixtures 1).

The package check (`cargo package`, unpack, `cargo test --locked`) was
measured at the earlier commit `555e8c2` (104 passed, 0 failed; before that
fix 1 failed, `fixture_reports_deleted_fd_bytes_and_pid`). It was not repeated
at this commit.

## Gaps checked

| Gap | Result |
| --- | --- |
| Failed write to standard output | Real. `>/dev/full` panicked with exit 101. Fixed in `555e8c2`: exit 2, no panic (`tests/stdout_closed.rs`). `/dev/null` stays exit 0. |
| Closed standard output at start | Real. The Rust runtime reopens a closed descriptor 1 as `/dev/null` before `main`, so a closed stream exited 0. Fixed in `256738a`, Linux only: a constructor in the binary (`src/stdout_probe.rs`, the only `unsafe` code, one `allow` in `src/main.rs`) records the state first. `>&-` is now exit 2 with a message; `/dev/null` stays exit 0 (`tests/stdout_closed.rs`, red before the fix). The library still forbids `unsafe`. Other platforms are not covered. |
| `--output` follows a planted symlink or replaces a special file | Does not apply. There is no `--output` option. |
| Unpacked crate fails a test | Real. `cargo package` turned the committed fd symlinks into regular files, so the deleted_open test failed in the unpacked crate. Fixed: the test creates the symlinks at run time (`tests/package_fixtures.rs` guards it). No file was missing from the package. |
| `build.rs` reads `.git/HEAD` and fails in a worktree | Does not apply. There is no `build.rs`. |

## Not run

These were not run for this measurement. Their result is unknown here.

- `cargo package` check at this commit: not run.
- `cargo audit`: not run.
- `cargo deny check licenses`: not run.
- Build with the minimum Rust version 1.85: not run.
- Tests on Windows and macOS: not run. This commit only gates `tests/stdout_closed.rs` by `cfg` (`unix`, `target_os = "linux"`) so Windows can build it; that is not confirmed on Windows.
- Result of the GitHub CI jobs for this commit: not run, not read.
- Fuzzing, mutation testing, and coverage: not run.
- Regenerating `THIRD-PARTY-LICENSES.html` with `cargo about`: not run.
