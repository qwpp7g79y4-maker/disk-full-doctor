# Security policy

disk-full-doctor is a read-only command-line tool. It never writes, deletes,
or changes any file, never calls the Docker socket or CLI, `lsof`, or
`journalctl`, and never opens a network connection. The worst outcome of a
bug is a wrong or missing report line, not data loss.

## Reporting a vulnerability

Email pep_ai@icloud.com with a description and reproduction steps. There is
currently no bug bounty.
