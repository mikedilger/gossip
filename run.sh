#!/bin/bash

#RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable"
cargo build --features=lang-cjk --release && \
    RUST_BACKTRACE=1 RUST_LOG="warn,gossip_bin=info,gossip_lib=info,nostr_types=info,gossip_relay_picker=info" ./target/release/gossip "$@"

