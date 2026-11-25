# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten staat van commit `555e8c2` (volledige hash
`555e8c2b5ade7c8280cd791f6f4e4a6dec665192`), gemeten op 2026-11-25 op Linux
met Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). De werkmap was schoon op die
commit. Latere commits die alleen documentatie wijzigen veranderen deze
uitkomsten niet.

## Gemeten

| Opdracht | Uitkomst |
| --- | --- |
| `cargo fmt --all -- --check` | geslaagd, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | geslaagd, exit 0, geen waarschuwingen |
| `cargo test --locked` | geslaagd, exit 0: 104 geslaagd, 0 mislukt, 0 genegeerd |
| `cargo test --release --locked` | geslaagd, exit 0: 104 geslaagd, 0 mislukt, 0 genegeerd |

De 104 tests zijn 28 in de bibliotheek, 5 in het programma en 71 in de
integratietests onder `tests/` (branches 31, cli 10, docker_logs 8, buildkit 3,
handbook_help 3, inodes 3, journald 3, readme_sync 3, deleted_open 2,
df_du_gap 2, stdout_closed 2, package_fixtures 1).

Ook gemeten op deze commit: `cargo package --locked --allow-dirty
--no-verify`, de `.crate` uitpakken in een schone map en daar
`cargo test --locked` draaien: 104 geslaagd, 0 mislukt. Voor de fix gaf
dezelfde controle 1 mislukt (`fixture_reports_deleted_fd_bytes_and_pid`).

## Gaten gecontroleerd

| Gat | Uitkomst |
| --- | --- |
| Mislukt schrijven naar standaarduitvoer | Echt. `>/dev/full` gaf een panic met exit 101. Gesloten in deze commit: exit 2, geen panic (`tests/stdout_closed.rs`). `/dev/null` blijft exit 0. |
| Gesloten standaarduitvoer bij de start | Niet gesloten. De Rust-runtime heropent een gesloten descriptor 1 als `/dev/null` vóór `main`; het programma geeft dan exit 0 en kan dit zonder `unsafe`-code niet onderscheiden van een echte `/dev/null`. Een test is niet mogelijk. |
| `--output` volgt een geplante symlink of vervangt een speciaal bestand | Niet van toepassing. Er is geen `--output`-optie. |
| Uitgepakt crate faalt een test | Echt. `cargo package` maakte van de vastgelegde fd-symlinks gewone bestanden, waardoor de deleted_open-test in het uitgepakte crate faalde. Gesloten: de test maakt de symlinks tijdens het draaien (`tests/package_fixtures.rs` bewaakt dit). Er ontbrak geen bestand in het pakket. |
| `build.rs` leest `.git/HEAD` en faalt in een worktree | Niet van toepassing. Er is geen `build.rs`. |

## Niet uitgevoerd

Dit is voor deze meting niet gedraaid (not run). De uitkomst is hier onbekend.

- `cargo audit`: niet uitgevoerd (not run).
- `cargo deny check licenses`: niet uitgevoerd (not run).
- Bouwen met de minimale Rust-versie 1.85: niet uitgevoerd (not run).
- Tests op Windows en macOS: niet uitgevoerd (not run).
- Uitkomst van de GitHub-CI-jobs voor deze commit: niet uitgevoerd, niet gelezen (not run).
- Fuzzen, mutatietests en dekking: niet uitgevoerd (not run).
- `THIRD-PARTY-LICENSES.html` opnieuw maken met `cargo about`: niet uitgevoerd (not run).
