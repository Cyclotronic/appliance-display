# Contributing to appliance-display

Issues and focused pull requests are welcome. Protocol and hardware-profile
changes must distinguish published protocol facts, behavior measured on real
hardware, and compatibility assumptions.

## Development checks

Run the same checks as CI before submitting a change:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --locked
```

Do not commit build output, credentials, private network details, serial-numbered
equipment records, or captures from unrelated hardware. Preserve third-party
license notices when dependencies change.

The recovery console safeguard is part of the safety boundary. A new hardware
profile must explicitly identify protected console devices and prove its
display transport on the target platform before it becomes a default.

All contributions are accepted under the repository's MIT License.

## Commit identity and privacy

Do not add tooling or assistant attribution to commits. Contributors who do not
want an email address recorded permanently in public history should enable
GitHub email privacy and use their GitHub no-reply address before committing.
