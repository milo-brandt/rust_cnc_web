#!/bin/bash

readonly REMOTE_MACHINE=pi@cnc
readonly NAME=axum_web

ssh -t ${REMOTE_MACHINE} pkill ${NAME}