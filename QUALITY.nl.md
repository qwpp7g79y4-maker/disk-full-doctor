# Kwaliteit

[English](QUALITY.md) | Nederlands

Gemeten staat van commit `63b2223` (volledige hash
`63b2223d72c667698add589472276b95b4fe5408`), gemeten op 2026-11-25 op Linux
met Rust 1.99.0 (`cargo 1.99.0`, `rustc 1.99.0`). De werkmap was schoon op die
commit. Latere commits die alleen documentatie wijzigen veranderen deze
uitkomsten niet.

## Gemeten

| Opdracht | Uitkomst |
| --- | --- |
| `cargo fmt --all -- --check` | geslaagd, exit 0 |
| `cargo clippy --all-targets --locked -- -D warnings` | geslaagd, exit 0, geen waarschuwingen |
| `cargo test --locked` | geslaagd, exit 0: 101 geslaagd, 0 mislukt, 0 genegeerd |
| `cargo test --release --locked` | geslaagd, exit 0: 101 geslaagd, 0 mislukt, 0 genegeerd |

De 101 tests zijn 28 in de bibliotheek, 5 in het programma en 68 in de
integratietests onder `tests/` (branches 31, cli 10, docker_logs 8, buildkit 3,
handbook_help 3, inodes 3, journald 3, readme_sync 3, deleted_open 2,
df_du_gap 2).

## Niet uitgevoerd

Dit is voor deze meting niet gedraaid (not run). De uitkomst is hier onbekend.

- `cargo audit`: niet uitgevoerd (not run).
- `cargo deny check licenses`: niet uitgevoerd (not run).
- Bouwen met de minimale Rust-versie 1.85: niet uitgevoerd (not run).
- Tests op Windows en macOS: niet uitgevoerd (not run).
- Uitkomst van de GitHub-CI-jobs voor deze commit: niet uitgevoerd, niet gelezen (not run).
- Fuzzen, mutatietests en dekking: niet uitgevoerd (not run).
- `THIRD-PARTY-LICENSES.html` opnieuw maken met `cargo about`: niet uitgevoerd (not run).
