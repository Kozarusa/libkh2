#!/bin/sh
# Builds examples/min.c against the release library with the system C compiler (Linux/macOS), from
# the repository root (after `cargo build --release -p libkh2`) or from the unpacked package root.
set -e
LIBDIR=target/release
[ -d bin ] && LIBDIR=bin
mkdir -p build
${CC:-cc} -O2 -Wall -Iinclude examples/min.c -L"$LIBDIR" -lkh2 -Wl,-rpath,"$PWD/$LIBDIR" -o build/min
echo built build/min
