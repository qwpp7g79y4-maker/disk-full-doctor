# disk-full-doctor

English | [Nederlands](README.nl.md)

Read-only diagnostic for "No space left on device" when `df` and `du`
disagree. It checks the usual blind spots in one pass and prints a single
report, sorted by bytes, so you do not have to run six different tools.

## What it checks

- **deleted_open** — deleted-but-still-open files, via `/proc/<pid>/fd`.
- **inodes** — inode exhaustion per mount.
- **docker_logs** — Docker `json-file` container log size, and whether
  `log-opts.max-size` is missing.
- **journald** — size of `/var/log/journal` and `/run/log/journal`, and
  `SystemMaxUse` if `journald.conf` is readable.
- **buildkit** — size of Docker's built-in BuildKit cache and an external
  `buildkitd` cache.
- **df_du_gap** — per mount, `statvfs` used bytes next to a `du`-style sum
  of files on that filesystem, and the gap between the two.

Each finding reports bytes (or inodes), one line of evidence, and a short
hint. The tool never deletes, writes, or changes anything, and never touches
the Docker socket or CLI, `lsof`, or `journalctl`.

## Usage

```sh
disk-full-doctor          # text report
disk-full-doctor --json   # JSON report
disk-full-doctor -h --help
disk-full-doctor -V --version
```

The manual page is `man/disk-full-doctor.1`.

Exit code `0` means nothing actionable was found, `1` means a check reported
`Warn`, `NeedsRoot`, or `Error`, and `2` means a command-line usage error.

Without root, a check that cannot read a path reports `NeedsRoot` with a
plain-text hint instead of failing silently or panicking. The report ends
with a summary line naming every check that needs root to see more.

## License

Licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.

The licenses of the third-party crates built into the binary are listed in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).
