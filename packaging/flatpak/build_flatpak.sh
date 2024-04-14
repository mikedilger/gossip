#!/usr/bin/env sh

set -e

cd "$(realpath "$(dirname "$0")")" # cwd is now packaging/flatpak

mkdir -p .build-tmp

# Get flatpak-cargo-generator.py
if [ ! -x flatpak-cargo-generator.py ] ; then
    wget "https://raw.githubusercontent.com/flatpak/flatpak-builder-tools/master/cargo/flatpak-cargo-generator.py"
    chmod a+x flatpak-cargo-generator.py
fi

# Temporarily set a global git config to make flatpak work properly
#   original_file_allow_option=$(git config --global --get protocol.file.allow || echo "")
#   git config --global protocol.file.allow always

./flatpak-cargo-generator.py ../../Cargo.lock -o .build-tmp/cargo-sources.json
flatpak-builder --force-clean .build-tmp/build_dir com.mikedilger.gossip.yml

# Revert the global git config to its original value
#   git config --global protocol.file.allow "$original_file_allow_option"

flatpak build-export .build-tmp/repo .build-tmp/build_dir
flatpak build-bundle .build-tmp/repo gossip.flatpak com.mikedilger.gossip
