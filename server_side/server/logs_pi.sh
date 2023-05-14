#!/bin/bash

readonly REMOTE_MACHINE=pi@cnc

ssh -t ${REMOTE_MACHINE} journalctl -f -u cnc_server.service