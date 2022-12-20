#!/bin/bash

find . -name .git -prune -o -name target -prune -o -name "$1" -print

