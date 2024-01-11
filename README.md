# AZ Safe Send

Safely send crypto via an allowance/collect mechanism on Aleph Zero.

### Rules & notes

## Getting Started
### Prerequisites

* [Cargo](https://doc.rust-lang.org/cargo/)
* [Rust](https://www.rust-lang.org/)
* [ink!](https://use.ink/)
* [Cargo Contract v3.2.0](https://github.com/paritytech/cargo-contract)
```zsh
cargo install --force --locked cargo-contract --version 3.2.0
```

### Checking code

```zsh
cargo checkmate
cargo sort
```

## Testing

### Run unit tests

```sh
cargo test
```

### Run integration tests

```sh
export CONTRACTS_NODE="/Users/myname/.cargo/bin/substrate-contracts-node"
cargo test --features e2e-tests
```

## Deployment

1. Build contract:
```sh
# You may need to run
# chmod +x build.sh f
./build.sh
```
2. If setting up locally, start a local development chain. 
```sh
substrate-contracts-node --dev
```
3. Upload, initialise and interact with contract at [Contracts UI](https://contracts-ui.substrate.io/).

## References

- [INK Multi-Contract-Caller Example](https://github.com/paritytech/ink-examples/tree/61f69a77b3e32fe18c1f144a2863d25471778bee/multi-contract-caller)
- [ink_e2e Docs](https://docs.rs/ink_e2e/4.3.0/ink_e2e/index.html)
