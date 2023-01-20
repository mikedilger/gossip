#!/usr/bin/env bash

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )
cd $SCRIPT_DIR/..
DOCKER_BUILDKIT=1 \
  docker build \
  --output contrib \
  -f contrib/Dockerfile \
  .

