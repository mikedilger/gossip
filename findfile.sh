#!/bin/bash

find . -name .git -prune -o -name node_modules -prune -o -name target -prune -o -name "$1" -print

