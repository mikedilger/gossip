#!/bin/bash
set -e

# Support older MacOS version (11.0 is Big Sur, released June 2020)
export MACOSX_DEPLOYMENT_TARGET=11.0

# NOTE: you generally need to install these first:
#    cmake, pkg-config, sdl2, ffmpeg

# This is in case you installed sdl2 with homebrew:
export CPPFLAGS="-I${HOMEBREW_PREFIX}/include${CPPFLAGS+ ${CPPFLAGS}}"
export LDFLAGS="-L${HOMEBREW_PREFIX}/lib -Wl,-rpath,${HOMEBREW_PREFIX}/lib${LDFLAGS+ ${LDFLAGS}}"

cargo build --target=x86_64-apple-darwin --release --features=lang-cjk

VERSION=$(cat ../../Cargo.toml | grep ^version | awk -F= '{print $2}' | awk -F\" '{print $2}')
set +e
echo $VERSION | grep -s unstable
if [ $? -eq 0 ] ; then
  GITHASH=$(git rev-parse --short HEAD)
  VERSION=${VERSION}-$GITHASH
fi
set -e
NAME=gossip
BIN_NAME=gossip-bin
APP_NAME=Gossip
APP_DIR=$APP_NAME.app

echo "Creating app directory structure"
rm -rf $APP_DIR
mkdir -p $APP_DIR/Contents/MacOS

echo "Copying binary"
cp ../../target/x86_64-apple-darwin/release/$NAME $APP_DIR/Contents/MacOS/$BIN_NAME

echo "Copying launcher"
cp macos_launch.sh $APP_DIR/Contents/MacOS/$APP_NAME

echo "Copying Icon"
mkdir -p $APP_DIR/Contents/Resources
cat Info.plist | sed s/__VERSION__/$VERSION/g > $APP_DIR/Contents/Info.plist
cp ../../logo/$NAME.png ../../logo/$NAME.svg $APP_DIR/Contents/Resources

echo "Creating dmg"
mkdir -p $APP_NAME
mv $APP_DIR $APP_NAME/
rm -rf $APP_NAME/.Trashes

OS=$(uname -s)
MACHINE=x86_64
FULL_NAME=$NAME-$VERSION-$OS-$MACHINE

hdiutil create $FULL_NAME.dmg -srcfolder $APP_NAME -ov
rm -rf $APP_NAME
