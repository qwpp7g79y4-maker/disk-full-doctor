# Changelog

All notable changes to this project are documented in this file.

## [Unreleased]

### Added

- Manual page `man/disk-full-doctor.1`, the invented `examples/quiet` tree,
  and a test that the manual page lists the same long options as `--help`.

### Fixed

- `mountinfo`: `unescape_octal` no longer mangles multi-byte UTF-8 mount
  point names.
- `mountinfo`: a single non-UTF-8 byte in one mount point name no longer
  fails the read of the whole `mountinfo` file and blinds every mount.

## [0.1.0] - 2026-10-24

### Changed

- First release of `disk-full-doctor`, a read-only diagnostic for "No space
  left on device" when `df` and `du` disagree.
- Project skeleton: clap CLI, `MIT OR Apache-2.0` license, `deny.toml`,
  `cargo-about` config.
- Root-prefix abstraction and `FsProbe` trait so every check can run
  against a fixture instead of the real system; text and `--json` report.
- `deleted_open`: deleted-but-open files via `/proc/<pid>/fd`.
- `inodes`: inode use per mount via `statvfs`.
- `docker_logs`: Docker `json-file` container log size and missing
  `max-size`.
- `journald`: journald log size and `SystemMaxUse`.
- `buildkit`: BuildKit cache size.
- `df_du_gap`: `statvfs`-vs-`du` gap per mount.
- Needs-root summary line in the report; exit codes `0`/`1`/`2`.
- README, SECURITY, CHANGELOG, and CI workflow.
- Keep build notes local; add Dependabot config.

### Fixed

- `docker_logs`: a `daemon.json` `max-size` no longer hides an unlimited
  container log.
