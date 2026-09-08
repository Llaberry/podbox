#!/usr/bin/env bash
# `zig ar`, for the same reason scripts/zig-cc.sh exists: cc-rs takes a
# PROGRAM. Paired with it, so an archive is built by the toolchain that built
# the objects rather than by whatever `ar` the host happens to carry.
set -u
exec zig ar "$@"
