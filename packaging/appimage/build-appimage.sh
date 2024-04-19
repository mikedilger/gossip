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
cargo build --features=lang-cjk,appimage --release

cd target/
rm -rf ./appimage
mkdir -p appimage
cd appimage/

# Build AppDir
mkdir -p AppDir/usr/bin/
mkdir -p AppDir/usr/lib/
mkdir -p AppDir/usr/share/applications/
mkdir -p AppDir/usr/share/icons/hicolor/scalable/apps/
mkdir -p AppDir/usr/share/icons/hicolor/256x256/apps/
mkdir -p AppDir/usr/share/icons/hicolor/128x128/apps/
mkdir -p AppDir/usr/share/icons/hicolor/64x64/apps/
mkdir -p AppDir/usr/share/icons/hicolor/32x32/apps/
mkdir -p AppDir/usr/share/icons/hicolor/16x16/apps/
cp ../release/gossip AppDir/usr/bin/gossip
strip AppDir/usr/bin/gossip
cp ../../logo/gossip.png AppDir/usr/share/icons/hicolor/128x128/apps/gossip.png
cp ../../logo/gossip.svg AppDir/usr/share/icons/hicolor/scalable/apps/gossip.svg
cp ../../packaging/debian/gossip.desktop AppDir/usr/share/applications/gossip.desktop
ln -s usr/bin/gossip AppDir/AppRun
ln -s gossip.png AppDir/.DirIcon
ln -s usr/share/applications/gossip.desktop AppDir/gossip.desktop
ln -s usr/share/icons/hicolor/128x128/apps/gossip.png AppDir/gossip.png

# Get appimagetool
wget "https://github.com/AppImage/AppImageKit/releases/download/continuous/appimagetool-x86_64.AppImage"
chmod a+x appimagetool-x86_64.AppImage

# Use appimagetool to build the AppImage
./appimagetool-x86_64.AppImage AppDir

# Bundle for portable mode
mkdir -p gossip-x86_64.AppImage.home/.local/share
tar cvf - gossip-x86_64.AppImage gossip-x86_64.AppImage.home | gzip -c > gossip-x86_64.AppImage.tar.gz

echo "Portable AppImage is at ../../target/appimage/gossip-x86_64.AppImage.tar.gz"

exit 0
