#!/bin/bash

cargo build --features=lang-cjk,video-ffmpeg && \
    RUST_BACKTRACE=1 RUST_LOG="info,gossip=debug" ./target/debug/gossip
