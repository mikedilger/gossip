#!/bin/bash

# This is the binary we are inspecting
BIN=./target/release/gossip

# This uses ldd to get it's dependencies into a list
DEPS=$(pacman -F $(ldd "$BIN" | awk '{print $3}') | awk '{print $5}'  | awk -F/ '{print $2}' | tr " " "\n" | sort -u | tr "\n" " ")

# This array will amass dependencies that we won't need to install because they
# will be pulled in by other dependenices
TODELETE=

for b in ${DEPS[@]}; do
    SUBDEPS=$(pacman -Si $b | sed -n "/^Depends On/{s/Depends On *: \(.*\)/\1/;p}" | tr " " "\n" | sort -u | tr "\n" " ")

    CLEANSUBDEPS=""
    for subdep in $SUBDEPS ; do
        NEXT=$(echo $subdep | sed -e 's/=.*//' | sed -e 's/>.*//')
        CLEANSUBDEPS="$CLEANSUBDEPS $NEXT"
    done

    TODELETE="$TODELETE $CLEANSUBDEPS"
done

# GET A SORTED UNIQUE LIST
TODELETE=$(echo $TODELETE | tr " " "\n" | sort -u | tr "\n" " ")

echo $DEPS | tr " " "\n" > /tmp/file1.txt
echo $TODELETE | tr " " "\n" > /tmp/file2.txt

comm -3 /tmp/file1.txt /tmp/file2.txt | sed '/\t/d'

# json-glib
# libstemmer
# libutil-linux  (util-linux-libs)
# webkit2gtk
#
# pacman -S json-glib libstemmer util-linux-libs webkit2gtk
