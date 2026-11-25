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
De tool verwijdert, schrijft of wijzigt nooit iets, en raakt de Docker-socket
of -CLI, `lsof` en `journalctl` nooit aan.

## Gebruik

```sh
disk-full-doctor          # text report
disk-full-doctor --json   # JSON report
disk-full-doctor -h --help
disk-full-doctor -V --version
```

De man-pagina is `man/disk-full-doctor.1`.

Exitcode `0` betekent dat er niets actiegerichts is gevonden, `1` betekent dat
een check `Warn`, `NeedsRoot` of `Error` meldde, en `2` betekent een fout in
het gebruik van de opdrachtregel.

Zonder root meldt een check die een pad niet kan lezen `NeedsRoot` met een
korte tip in platte tekst, in plaats van stilletjes te falen of te panikeren.
Het rapport eindigt met een samenvattingsregel die elke check noemt die root
nodig heeft om meer te zien.

## Licentie

Gelicentieerd onder [MIT](LICENSE-MIT) of [Apache-2.0](LICENSE-APACHE), naar
keuze.

De licenties van de externe crates in het programma staan in [THIRD-PARTY-LICENSES.html](THIRD-PARTY-LICENSES.html).
