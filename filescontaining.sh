#!/bin/bash

find . -name .git -prune -o -name target -prune -o -type f -exec grep -H "$1" {} \; \
     | awk -F: '{print $1}' | uniq
