# AZ Safe Send

A smart contract for Aleph Zero that enables the safe sending of crypto via a decentralised chequeing (send/collect) system. Also integrates AZERO.ID to provide an optional, extra check by validating that the receiver's wallet address matches a provided AZERO.ID.

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

1. https://github.com/btn-group/safe_send
2. https://github.com/btn-group/az_safe_send
3. https://github.com/btn-group/squid_safe_send
