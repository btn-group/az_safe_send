#!/usr/bin/env bash

set -eu

cargo +stable contract build --manifest-path az_button/Cargo.toml --release
cargo +stable contract build --release
