# appliance-display

`appliance-display` is an MIT-licensed Rust library, command-line tool, and
local service for the character display and front buttons used in network
appliances with a Portwell EZIO-100 or EZIO-300 panel.

The first verified hardware profile is the Sophos XG 330 Rev. 2. On that unit,
the project directly controls both rows of the 16×2 display and reads the four
front buttons as typed, debounced **Up**, **Down**, **Enter**, and **Escape**
events. The implementation talks to the panel directly and does not require
LCDproc.

## Project layout

- `appliance-display-core` handles fixed-cell frames, EZIO operations, button
  decoding, and debouncing without operating-system I/O.
- `appliance-display-transport` owns and configures the Unix serial device.
- `appliance-display` provides the CLI and a single-owner local daemon.
- `packaging/debian` builds the Debian package used on the development unit and
  is suitable as the base for a Proxmox installation.

The default hardware profile uses `/dev/ttyS1` at 2400-N-8-1. The program
always refuses `/dev/ttyS0`, which is commonly the appliance recovery console.
Other device paths can be selected explicitly.

## Commands

```text
appliance-display probe [--device PATH]
appliance-display write --line1 TEXT [--line2 TEXT] [--device PATH] [--dry-run]
appliance-display clear [--device PATH] [--dry-run]
appliance-display buttons [--watch] [--device PATH]
appliance-display status
appliance-display serve [--device PATH] [--socket PATH]
```

When the daemon is running, client commands use a versioned, size-limited JSON
protocol on `/run/appliance-display/control.sock`. The daemon holds an exclusive
lock on the serial device. The socket is local only and group restricted.

## Build and test

Rust 1.75 or newer is required.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Build a Debian package on a Debian-compatible system with the standard Rust and
Debian packaging tools installed:

```sh
./packaging/build-deb.sh
sudo dpkg -i appliance-display_0.1.0_amd64.deb
```

The package installs an enabled systemd service under a dedicated
`appliance-display` account. It grants only the supplementary `dialout` group
needed for the serial device and applies systemd hardening.

## Verified status

Version 0.1.0 was built and tested on the XG 330 itself under Debian 13. The
release binary is about 437 KB on amd64 and the running service uses about
2 MB RSS. Both display rows, all four buttons, exclusive ownership, malformed
request handling, service restart, and package upgrade were exercised on the
physical appliance.

FreeBSD packaging and the thin OPNsense configuration plugin are planned but
have not yet been validated on OPNsense. See [PROTOCOL.md](PROTOCOL.md) for the
confirmed byte-level interface and hardware notes.

## License and independence

This project is licensed under the [MIT License](LICENSE). It is an independent
project and is not affiliated with or endorsed by Sophos or Portwell. Product
names are used only to identify compatible hardware.
