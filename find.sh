#!/bin/bash

find . -name .git -prune -o -name node_modules -prune -o -name target -prune -o -type f -exec grep -H "$1" {} \;
