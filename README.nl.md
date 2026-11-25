# disk-full-doctor

[English](README.md) | Nederlands

Alleen-lezen diagnose voor "No space left on device" als `df` en `du`
het oneens zijn. Het loopt de gebruikelijke blinde vlekken in één keer na en
print één rapport, gesorteerd op bytes, zodat je geen zes verschillende
tools hoeft te draaien.

## Wat het controleert

- **deleted_open** — verwijderde maar nog geopende bestanden, via `/proc/<pid>/fd`.
- **inodes** — uitgeputte inodes per mount.
- **docker_logs** — grootte van Docker `json-file` containerlogs, en of
  `log-opts.max-size` ontbreekt.
- **journald** — grootte van `/var/log/journal` en `/run/log/journal`, en
  `SystemMaxUse` als `journald.conf` leesbaar is.
- **buildkit** — grootte van Dockers ingebouwde BuildKit-cache en een externe
  `buildkitd`-cache.
- **df_du_gap** — per mount de met `statvfs` gemeten gebruikte bytes naast een
  `du`-achtige som van de bestanden op dat bestandssysteem, en het verschil
  tussen die twee.

Elke bevinding geeft bytes (of inodes), één regel bewijs en een korte tip.
De tool verwijdert, schrijft of wijzigt nooit iets, opent nooit een
netwerkverbinding, en raakt de Docker-socket of -CLI, `lsof` en `journalctl`
nooit aan.

## Installatie

De crate staat niet op crates.io. Bouw hem uit de broncode met een
Rust-toolchain (1.85 of nieuwer):

```sh
git clone https://github.com/qwpp7g79y4-maker/disk-full-doctor
cd disk-full-doctor
cargo build --locked --release
```

Het programma is `target/release/disk-full-doctor`. Kopieer het naar een map in
je `PATH`.

## Snelstart

Start het zonder argumenten. Het leest alleen bestanden en schrijft niets.

```sh
target/release/disk-full-doctor
```

Start het als root, dan kan elke check alles zien. Zonder root zegt een check
die een pad niet kan lezen dat, in plaats van te gokken.

## Aan de slag in 5 minuten

Bouw het programma (zie Installatie), start het zonder argumenten en lees het
rapport van boven naar beneden: het is gesorteerd op bytes, dus de grootste
bevinding staat bovenaan. Meldt een check `NeedsRoot`, start dan als root.

## Gebruik

```sh
disk-full-doctor          # text report
disk-full-doctor --json   # JSON report
disk-full-doctor -h --help
disk-full-doctor -V --version
```

De man-pagina is `man/disk-full-doctor.1`.

Zonder root meldt een check die een pad niet kan lezen `NeedsRoot` met een
korte tip in platte tekst, in plaats van stilletjes te falen of te panikeren.
Het rapport eindigt met een samenvattingsregel die elke check noemt die root
nodig heeft om meer te zien.

## Voorbeeld

In de repository zit een kleine verzonnen boom. De tool erop richten leest `/`
niet:

```sh
disk-full-doctor --root examples/quiet
```

Elke check meldt `NotPresent` en de exitcode is `0`. Zie
[examples/README.md](examples/README.md).

## Afsluitcodes

- `0`: er is niets actiegerichts gevonden.
- `1`: een check meldde `Warn`, `NeedsRoot` of `Error`.
- `2`: een fout in het gebruik van de opdrachtregel.

## Beperkingen

- Het leest Linux-paden zoals `/proc`, `/var/lib/docker` en
  `/var/log/journal`. Bestaat een pad niet, dan meldt de check
  `NotPresent`.
- Het meldt en geeft tips. Het maakt zelf nooit ruimte vrij.
- De `du`-achtige som in **df_du_gap** is een schatting uit de bestanden die
  het kan lezen, dus ze kan afwijken van wat `df` meldt.
- Docker wordt uit bestanden op schijf gelezen, nooit via de socket of CLI, dus
  een daemon die zijn data elders bewaart wordt niet gevonden.

## Onderhoud

Wijzigingen staan in [CHANGELOG.md](CHANGELOG.md). Beveiligingsmeldingen gaan
via [SECURITY.md](SECURITY.md). Wil je een wijziging insturen, volg dan
[CONTRIBUTING.md](CONTRIBUTING.md). De gemeten staat van een commit staat in
[QUALITY.nl.md](QUALITY.nl.md).

## Licentie

Gelicentieerd onder [MIT](LICENSE-MIT) of [Apache-2.0](LICENSE-APACHE), naar
keuze.

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).
