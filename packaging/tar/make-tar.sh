#!/bin/sh

cd ../../..
tar -cv --exclude=gossip/.git --exclude=gossip/target --exclude=gossip/packaging -f - gossip | bzip2 -c > gossip/packaging/tar/gossip-0.3.93-unstable.tar.bz2
