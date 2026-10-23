# Security Policy

## Supported versions

Security fixes are made against the current default branch. Once stable release
lines exist, the release notes will state which versions continue to receive
fixes.

## Reporting a vulnerability

Do not disclose a suspected vulnerability in a public issue. Use the
repository's private vulnerability reporting page:

https://github.com/Cyclotronic/appliance-display/security/advisories/new

Include the affected version and operating system, the hardware profile, clear
reproduction steps, and the expected impact. Do not include credentials or
private network information.

## Security boundaries

The daemon exposes only a local Unix socket. Requests are versioned and limited
to 4096 bytes, and client reads and writes have deadlines. The daemon owns the
serial device exclusively and rejects the XG 330 recovery console. It does not
listen on TCP, execute commands, or accept arbitrary protocol bytes from
clients.

The service account requires write access to the configured serial device.
Grant access to that one device through the platform's normal device group; do
not run unrelated collectors or integrations as root merely to reach the panel.
