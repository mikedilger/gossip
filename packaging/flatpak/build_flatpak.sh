#!/usr/bin/env sh

set -e

cd "$(realpath "$(dirname "$0")")" # cwd is now packaging/flatpak

mkdir -p .build-tmp

flatpak-builder-tools/cargo/flatpak-cargo-generator.py ../../Cargo.lock -o .build-tmp/cargo-sources.json

original_file_allow_option=$(git config --global --get protocol.file.allow || echo "")
git config --global protocol.file.allow always # unfortunately seems to be needed outside the build sandbox for source-copying stage
flatpak-builder --force-clean .build-tmp/build_dir com.mikedilger.gossip.yml
git config --global protocol.file.allow "$original_file_allow_option"

flatpak build-export .build-tmp/repo .build-tmp/build_dir
flatpak build-bundle .build-tmp/repo gossip.flatpak com.mikedilger.gossip
