# Contributing

Thanks for looking at disk-full-doctor. It is a small, read-only tool, and
changes should keep it that way: no writing, no deleting, no network, no
Docker socket or CLI.

## Before you send a change

Run the same four commands the project is measured with:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --locked -- -D warnings
cargo test --locked
cargo test --release --locked
```

All four must pass. Every check reads through the `FsProbe` trait, so a new
check gets a test against an invented fixture tree under `tests/fixtures`,
not against the machine it runs on.

## Documentation

`README.md` and `README.nl.md` carry the same code blocks and the same inline
code; `cargo test` checks that. If you change one, change the other. The
manual page `man/disk-full-doctor.1` must list the same long options as
`--help`; that is tested too.

Add a line under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) for any
change a user can see.

## Reporting problems

Security problems go through [SECURITY.md](SECURITY.md), not a public issue.
Anything else can be a normal issue with the report text and the exit code.

## License

By contributing you agree that your work is licensed under
`MIT OR Apache-2.0`, as in [LICENSE-MIT](LICENSE-MIT) and
[LICENSE-APACHE](LICENSE-APACHE).
