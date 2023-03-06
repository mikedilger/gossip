#!/bin/bash

RUST_BACKTRACE=1 RUST_LOG="info,gossip=debug" cargo run
