#!/bin/bash

# See: https://medium.com/swlh/compiling-rust-for-raspberry-pi-arm-922b55dbb050
# need to have gcc-arm-linux-gnueabihf installed
# also need rustup target add armv7-unknown-linux-musleabihf

set -exo pipefail

readonly REMOTE_MACHINE=pi@cnc
readonly TARGET_ARCH=armv7-unknown-linux-musleabihf
readonly BINARY=target/${TARGET_ARCH}/release/axum_web

cargo build --release --target=${TARGET_ARCH}
du -h ${BINARY}
ssh -t ${REMOTE_MACHINE} sudo systemctl stop cnc_server 
rsync ${BINARY} ${REMOTE_MACHINE}:/home/pi/axum_web/axum_web
ssh -t ${REMOTE_MACHINE} sudo systemctl start cnc_server 
