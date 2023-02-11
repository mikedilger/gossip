#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd $SCRIPT_DIR/../..
DOCKER_BUILDKIT=1 \
  docker build \
  --output packaging/debian \
  -f packaging/debian/Dockerfile \
  .

