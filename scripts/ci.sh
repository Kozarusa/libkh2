#!/bin/sh
# Everything a change must pass, in one run (Linux / macOS / Git Bash). Written for them, run here
# only on Windows: see README "Status and what a human must test".
#   sh scripts/ci.sh            (KH2_ISO_FM or KH2_ISO = the Final Mix disc: the disc tests run too)
# Steps: release build, clippy, the test suite, a longer fuzz run, the binding diff (the library's
# exports through nm / the DLL's export table), the API reference check, the C example build, the
# C# example build when dotnet is installed. The package step needs PowerShell (scripts/package.ps1).
set -u
cd "$(dirname "$0")/.." || exit 1
failed=""
step() {
    name=$1
    shift
    echo "== $name"
    if ! "$@"; then
        failed="$failed $name"
        echo "!! $name failed"
    fi
}
[ -n "${KH2_ISO_FM:-}" ] && export KH2_ISO="$KH2_ISO_FM"
[ -n "${KH2_ISO:-}" ] || echo "note: neither KH2_ISO_FM nor KH2_ISO is set; the disc tests skip themselves"
step build cargo build --release -p libkh2
step clippy cargo clippy --release -p libkh2 --all-targets -- -D warnings
step tests cargo test --release
step fuzz env KH2_FUZZ_STEPS=20000 cargo test --release --test fuzz
case "$(uname -s)" in
    MINGW* | MSYS* | CYGWIN*) lib=target/release/kh2.dll ;;
    Darwin) lib=target/release/libkh2.dylib ;;
    *) lib=target/release/libkh2.so ;;
esac
step bindings python3 scripts/check_bindings.py "$lib"
step api-reference python3 scripts/gen_api_ref.py --check
if command -v cc >/dev/null 2>&1; then
    step c-example sh examples/build_min.sh
fi
if command -v dotnet >/dev/null 2>&1; then
    step csharp dotnet build -c Release examples/csharp
fi
if command -v pwsh >/dev/null 2>&1; then
    step package pwsh -File scripts/package.ps1 -Zip
    step package-scan cargo test --release --test package
else
    echo "note: no pwsh; the package is not built"
fi
if [ -n "$failed" ]; then
    echo "FAILED:$failed"
    exit 1
fi
echo "all steps passed"
