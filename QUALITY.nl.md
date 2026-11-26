# Kwaliteit

pep: 10

[English](QUALITY.md) | Nederlands

Gemeten staat van commit `4e8eb69` (volledige hash
`4e8eb69554b990703509237ab00820c2e2c4cdb5`), gemeten op 2026-11-25 op Linux
met Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). De werkmap was schoon op die
commit. Latere commits die alleen documentatie wijzigen veranderen deze
uitkomsten niet.

## Gemeten

| Opdracht | Uitkomst |
| --- | --- |
| `cargo fmt --all -- --check` | geslaagd, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | geslaagd, exit 0, geen waarschuwingen |
| `cargo test --locked` | geslaagd, exit 0: 105 geslaagd, 0 mislukt, 0 genegeerd |
| `cargo test --release --locked` | geslaagd, exit 0: 105 geslaagd, 0 mislukt, 0 genegeerd |
| `cargo +1.85 fmt --check` | geslaagd, exit 0 |
| `cargo +1.85 clippy --all-targets --locked -- -D warnings` | geslaagd, exit 0, geen waarschuwingen |
| `cargo +1.85 test --locked` | geslaagd, exit 0: 105 geslaagd, 0 mislukt, 0 genegeerd |

De drie `+1.85`-rijen zijn gemeten op 2026-11-25 op Linux met `cargo 1.85.1`, op dezelfde code (tip `4d37022` van tak `kwaliteit`, die na `4e8eb69` alleen documentatie en metadata toevoegt). `cargo +1.85 test --release --locked` is niet uitgevoerd (not run).

De 105 tests zijn 28 in de bibliotheek, 5 in het programma en 72 in de
integratietests onder `tests/` (branches 31, cli 10, docker_logs 8, buildkit 3,
handbook_help 3, inodes 3, journald 3, readme_sync 3, deleted_open 2,
df_du_gap 2, stdout_closed 3, package_fixtures 1).

De pakketcontrole (`cargo package`, uitpakken, `cargo test --locked`) is
gemeten op de eerdere commit `555e8c2` (104 geslaagd, 0 mislukt; vóór die fix
1 mislukt, `fixture_reports_deleted_fd_bytes_and_pid`). Op deze commit is ze
niet herhaald.

## Gaten gecontroleerd

| Gat | Uitkomst |
| --- | --- |
| Mislukt schrijven naar standaarduitvoer | Echt. `>/dev/full` gaf een panic met exit 101. Gesloten in `555e8c2`: exit 2, geen panic (`tests/stdout_closed.rs`). `/dev/null` blijft exit 0. |
| Gesloten standaarduitvoer bij de start | Echt. De Rust-runtime heropent een gesloten descriptor 1 als `/dev/null` vóór `main`, dus een gesloten stroom gaf exit 0. Gesloten in `256738a`, alleen Linux: een constructor in het programma (`src/stdout_probe.rs`, de enige `unsafe`-code, één `allow` in `src/main.rs`) legt de staat eerder vast. `>&-` geeft nu exit 2 met een melding; `/dev/null` blijft exit 0 (`tests/stdout_closed.rs`, rood vóór de fix). De bibliotheek verbiedt `unsafe` nog steeds. Andere platformen vallen erbuiten. |
| `--output` volgt een geplante symlink of vervangt een speciaal bestand | Niet van toepassing. Er is geen `--output`-optie. |
| Uitgepakt crate faalt een test | Echt. `cargo package` maakte van de vastgelegde fd-symlinks gewone bestanden, waardoor de deleted_open-test in het uitgepakte crate faalde. Gesloten: de test maakt de symlinks tijdens het draaien (`tests/package_fixtures.rs` bewaakt dit). Er ontbrak geen bestand in het pakket. |
| `build.rs` leest `.git/HEAD` en faalt in een worktree | Niet van toepassing. Er is geen `build.rs`. |

## Niet uitgevoerd

Dit is voor deze meting niet gedraaid (not run). De uitkomst is hier onbekend.

- `cargo package`-controle op deze commit: niet uitgevoerd (not run).
- `cargo audit`: niet uitgevoerd (not run).
- `cargo deny check licenses`: niet uitgevoerd (not run).
- `cargo +1.85 test --release --locked`: niet uitgevoerd (not run).
- Tests op Windows en macOS: niet uitgevoerd (not run). Deze commit zet `tests/stdout_closed.rs` alleen achter `cfg` (`unix`, `target_os = "linux"`) zodat Windows hem kan bouwen; dat is niet op Windows bevestigd.
- Uitkomst van de GitHub-CI-jobs voor deze commit: niet uitgevoerd, niet gelezen (not run).
- Fuzzen, mutatietests en dekking: niet uitgevoerd (not run).
- `THIRD-PARTY-LICENSES.html` opnieuw maken met `cargo about`: niet uitgevoerd (not run).
