# Quality

English | [Nederlands](QUALITY.nl.md)

Measured state of commit `63b2223` (full hash
`63b2223d72c667698add589472276b95b4fe5408`), measured on 2026-11-25 on
Linux with Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). The working tree was
clean at that commit. Commits after it that only change documentation do not
change these results.

## Measured

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | pass, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | pass, exit 0, no warnings |
| `cargo test --locked` | pass, exit 0: 101 passed, 0 failed, 0 ignored |
| `cargo test --release --locked` | pass, exit 0: 101 passed, 0 failed, 0 ignored |

The 101 tests are 28 in the library, 5 in the binary, and 68 in the
integration tests under `tests/` (branches 31, cli 10, docker_logs 8,
buildkit 3, handbook_help 3, inodes 3, journald 3, readme_sync 3,
deleted_open 2, df_du_gap 2).

## Not run

These were not run for this measurement. Their result is unknown here.

- `cargo audit`: not run.
- `cargo deny check licenses`: not run.
- Build with the minimum Rust version 1.85: not run.
- Tests on Windows and macOS: not run.
- Result of the GitHub CI jobs for this commit: not run, not read.
- Fuzzing, mutation testing, and coverage: not run.
- Regenerating `THIRD-PARTY-LICENSES.html` with `cargo about`: not run.
