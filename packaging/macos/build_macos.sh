#!/bin/bash
set -e

# NOTE: you generally need to install these first:
#    cmake, pkg-config, sdl2, ffmpeg

# This is in case you installed sdl2 with homebrew:
export CPPFLAGS="-I${HOMEBREW_PREFIX}/include${CPPFLAGS+ ${CPPFLAGS}}"
export LDFLAGS="-L${HOMEBREW_PREFIX}/lib -Wl,-rpath,${HOMEBREW_PREFIX}/lib${LDFLAGS+ ${LDFLAGS}}"

cargo build --release --features=lang-cjk

VERSION=0.6.0
NAME=gossip
BIN_NAME=gossip-bin
APP_NAME=Gossip
APP_DIR=$APP_NAME.app

echo "Creating app directory structure"
rm -rf $APP_DIR
mkdir -p $APP_DIR/Contents/MacOS

echo "Copying binary"
cp ../../target/release/$NAME $APP_DIR/Contents/MacOS/$BIN_NAME

echo "Copying launcher"
cp macos_launch.sh $APP_DIR/Contents/MacOS/$APP_NAME

echo "Copying Icon"
mkdir -p $APP_DIR/Contents/Resources
cat Info.plist | sed s/__VERSION__/$VERSION/g > $APP_DIR/Contents/Info.plist
cp ../../$NAME.png ../../$NAME.svg $APP_DIR/Contents/Resources

echo "Creating dmg"
mkdir -p $APP_NAME
mv $APP_DIR $APP_NAME/
rm -rf $APP_NAME/.Trashes

OS=$(uname -s)
MACHINE=$(uname -m)
FULL_NAME=$NAME-$VERSION-$OS-$MACHINE

hdiutil create $FULL_NAME.dmg -srcfolder $APP_NAME -ov
rm -rf $APP_NAME
