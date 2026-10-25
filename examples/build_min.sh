#!/bin/sh
# Builds examples/min.c against the release library with the system C compiler (Linux/macOS),
# after `cargo build --release -p libkh2`, from the repository root.
set -e
mkdir -p build
${CC:-cc} -O2 -Wall -Iinclude examples/min.c -Ltarget/release -lkh2 -Wl,-rpath,"$PWD/target/release" -o build/min
echo built build/min
