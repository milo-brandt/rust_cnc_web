#!/bin/bash

# See: https://medium.com/swlh/compiling-rust-for-raspberry-pi-arm-922b55dbb050
# need to have gcc-arm-linux-gnueabihf installed
# also need rustup target add armv7-unknown-linux-gnueabihf

set -exo pipefail

readonly REMOTE_MACHINE=pi@cnc
readonly TARGET_ARCH=armv7-unknown-linux-gnueabihf
readonly BINARY=target/${TARGET_ARCH}/release/axum_web
readonly REMOTE_BINARY=/home/pi/axum_web/axum_web

cargo build --release --target=${TARGET_ARCH}
du -h ${BINARY}
rsync ${BINARY} ${REMOTE_MACHINE}:${REMOTE_BINARY}
ssh -t ${REMOTE_MACHINE} ${REMOTE_BINARY}