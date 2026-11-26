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
hint. The tool never deletes, writes, or changes anything, never opens a
network connection, and never touches the Docker socket or CLI, `lsof`, or
`journalctl`.

## Installation

The crate is not on crates.io. Build it from source with a Rust toolchain
(1.85 or newer):

```sh
git clone https://github.com/qwpp7g79y4-maker/disk-full-doctor
cd disk-full-doctor
cargo build --locked --release
```

The binary is `target/release/disk-full-doctor`. Copy it anywhere on your
`PATH`.

## Quick start

Run it without arguments. It reads files only and writes nothing.

```sh
target/release/disk-full-doctor
```

Run it as root to let every check see everything. Without root, a check that
cannot read a path says so instead of guessing.

## Quickstart in 5 minutes

The Dutch quickstart is in [README.nl.md](README.nl.md).

## Usage

```sh
disk-full-doctor          # text report
disk-full-doctor --json   # JSON report
disk-full-doctor -h --help
disk-full-doctor -V --version
```

The manual page is `man/disk-full-doctor.1`.

Without root, a check that cannot read a path reports `NeedsRoot` with a
plain-text hint instead of failing silently or panicking. The report ends
with a summary line naming every check that needs root to see more.

## Example

The repository ships a small invented tree. Pointing the tool at it does not
read `/`:

```sh
disk-full-doctor --root examples/quiet
```

Every check reports `NotPresent` and the exit code is `0`. See
[examples/README.md](examples/README.md).

## Exit codes

- `0`: nothing actionable was found.
- `1`: a check reported `Warn`, `NeedsRoot`, or `Error`.
- `2`: a command-line usage error.

## Limitations

- It reads Linux paths such as `/proc`, `/var/lib/docker`, and
  `/var/log/journal`. Where a path does not exist, the check reports
  `NotPresent`.
- It reports and hints. It never frees space for you.
- The `du`-style sum in **df_du_gap** is an estimate built from the files it
  can read, so it can differ from what `df` reports.
- Docker is read from files on disk, never through the socket or CLI, so a
  daemon that keeps its data elsewhere is not found.

## Maintenance

Changes are listed in [CHANGELOG.md](CHANGELOG.md). Security reports go
through [SECURITY.md](SECURITY.md). To send a change, follow
[CONTRIBUTING.md](CONTRIBUTING.md). The measured state of a commit is in
[QUALITY.md](QUALITY.md).

## License

Licensed under either of [MIT](LICENSE-MIT) or
[Apache-2.0](LICENSE-APACHE) at your option.

The licenses of the third-party crates built into the binary are listed in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).
