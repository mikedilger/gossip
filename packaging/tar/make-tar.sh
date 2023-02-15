#!/bin/sh

VERSION=$(grep '^version' ../../Cargo.toml | awk -F\" '{print $2}')
echo $VERSION;

cd ../../..
tar -cv --exclude=gossip/.git --exclude=gossip/target --exclude=gossip/packaging -f - gossip | bzip2 -c > gossip/packaging/tar/gossip-${VERSION}.tar.bz2
