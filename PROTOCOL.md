# Portwell EZIO text display protocol

This project implements the documented Portwell EZIO serial protocol without
including or translating LCDproc source code.

## Confirmed XG 330 Rev. 2 profile

- Device on Debian: `/dev/ttyS1` (COM2, I/O `0x2f8`, IRQ 3)
- Serial settings: 2400 baud, 8 data bits, no parity, 1 stop bit
- Display: 16 columns by 2 rows, HD44780-compatible
- Command prefix: `0xfe`
- Key-response prefix: `0xfd`
- Backlight: always on; no software control in this profile
- Command delay: 40 ms before and after each command byte

The recovery console is `/dev/ttyS0` and is always rejected by the transport.

## Display commands used

| Operation | Bytes |
| --- | --- |
| Clear | `fe 01` |
| Cursor home | `fe 02` |
| Left-to-right entry | `fe 06` |
| Display off | `fe 08` |
| Display on, cursor off | `fe 0c` |
| 4-bit, two-line function set | `fe 28` |
| First row position | `fe 80` |
| Second row position | `fe c0` |

Printable display data follows a row-position command without a prefix. Each
row is padded or truncated to 16 cells. Control characters are rejected and
unsupported Unicode characters are replaced with `?`.

## Buttons

A key poll uses `fe 06`. The panel replies with `fd` and a status byte.

| Event | EZIO-300 | EZIO-100 compatibility |
| --- | --- | --- |
| Escape | `b7` | `4e` |
| Up | `be` | `4d` |
| Down | `bd` | `47` |
| Enter | `bb` | `4b` |
| No key | `bf` | `4f` |

All four EZIO-300 events were physically verified on the Sophos XG 330 on
2026-10-23. The public API debounces polling into one event per press.

## Provenance

- Portwell Linux EZIO-300 User Manual v1.0.3 describes the serial interface,
  command prefix, display commands, and key statuses.
- The HD44780 instruction set defines row addressing and display setup.
- LCDproc was used only as a temporary hardware-identification probe before
  this implementation existed. It is not a dependency and its GPL code is not
  included in this MIT-licensed project.
