#!/usr/bin/env bash

echo "This script will ask some questions and then build gossip for you."
echo

RUSTV=$(rustc --version >/dev/null 2>&1)
if [ $? -ne 0 ] ; then
    echo "Please install rust. One easy way is shown here: https://rustup.rs/"
    exit 1
fi

FFMPEG=$(ffmpeg --help >/dev/null 2>&1)
if [ $? -eq 0 ] ; then
    FEATURE_FFMPEG=",video-ffmpeg"
else
    FEATURE_FFMPEG=""
fi

while true; do
    read -p "Do you need to use locally installed certificates (usually no)? (y/n) " yn
    case $yn in
        [Yy]* ) FEATURE_TLS="rustls-tls-native"; break;;
        [Nn]* ) FEATURE_TLS="rustls-tls"; break;;
        * ) echo "Please answer y or n.";;
    esac
done

while true; do
    read -p "Are you having compile issues with TLS crates (including rustls, ring, request)? " yn
    case $yn in
        [Yy]* ) FEATURE_TLS="native-tls"; break;;
        [Nn]* ) break;;
        * ) echo "Please answer y or n.";;
    esac
done

while true; do
    read -p "Do you want to enable Chinese-Japanese-Korean font characters, which makes a large binary? (y/n) " yn
    case $yn in
        [Yy]* ) FEATURE_CJK=",lang-cjk"; break;;
        [Nn]* ) FEATURE_CJK=""; break;;
        * ) echo "Please answer y or n.";;
    esac
done

FEATURES=$FEATURE_TLS$FEATURE_CJK$FEATURE_FFMPEG

echo "Building with FEATURES=$FEATURES"

export RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable"
cargo build --release --features=$FEATURES
strip ./target/release/gossip

echo
echo "The gossip binary is at ./target/release/gossip and can be moved anywhere you want."
