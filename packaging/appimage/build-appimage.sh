#!/bin/bash

# Exit on first failure
set -e

if [ ! -f build-appimage.sh ] ; then
    echo Run from the "gossip/packaging/appimage" directory
    exit 1
fi

DIR=$(readlink -f $(dirname $0))

cleanup() {
    cd "$DIR"
}
trap cleanup EXIT

cd ../..

RUSTFLAGS="-C target-cpu=native --cfg tokio_unstable"
cargo build --features=lang-cjk --release

cd target/
rm -rf ./appimage
mkdir -p appimage
cd appimage/

wget https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage
chmod +x linuxdeploy-x86_64.AppImage

./linuxdeploy-x86_64.AppImage --appdir AppDir -e ../release/gossip -i ../../logo/gossip.png -d ../../packaging/debian/gossip.desktop --output=appimage

echo "AppImage is at ../../target/appimage/gossip-x86_64.AppImage"

exit 0
