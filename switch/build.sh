#!/bin/sh
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Builds switch/nhse.nro without installing devkitPro: the Rust library on this machine
# (cargo-build.sh), then the C glue and the .nro in ounce-devkit's Docker image.
#
#   sh switch/build.sh
#
# IMAGE names the image (default: ounce-devkit). Build it once from a checkout of
# https://github.com/ouncebrew/ounce-devkit with:  docker build -t ounce-devkit .
# With devkitPro and ounce-devkit installed natively, run `make -C switch` instead.
set -eu

HERE=$(cd "$(dirname "$0")" && pwd)
IMAGE="${IMAGE:-ounce-devkit}"

sh "$HERE/cargo-build.sh"
docker run --rm -v "$HERE/..:/nhse" -w /nhse/switch "$IMAGE" make NO_CARGO=1
ls -la "$HERE/nhse.nro"
