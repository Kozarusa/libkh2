# Builds libkh2 in release and lays out dist/libkh2 for hosts:
#   bin/      kh2.dll (and its import library kh2.dll.lib on Windows)
#   include/  kh2.h
#   docs/     README.md, DESIGN.md and docs/*.md
#   unity/    Kh2.cs and the sample folder
#   examples/ min.c with its build scripts, the Python ctypes example
# No game data goes in: the library reads the user's own disc image at run time.
# Usage: powershell -ExecutionPolicy Bypass -File scripts/package.ps1 [-Zip]

param([switch]$Zip)
$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    cargo build --release -p libkh2
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    $dist = Join-Path $root "dist\libkh2"
    if (Test-Path $dist) { Remove-Item -Recurse -Force $dist }
    foreach ($d in "bin", "include", "docs", "unity", "examples") { New-Item -ItemType Directory -Force (Join-Path $dist $d) | Out-Null }
    $rel = Join-Path $root "target\release"
    foreach ($f in "kh2.dll", "kh2.dll.lib", "libkh2.so", "libkh2.dylib") {
        $p = Join-Path $rel $f
        if (Test-Path $p) { Copy-Item $p (Join-Path $dist "bin") }
    }
    Copy-Item include\kh2.h (Join-Path $dist "include")
    Copy-Item README.md, DESIGN.md (Join-Path $dist "docs")
    Copy-Item docs\*.md (Join-Path $dist "docs")
    Copy-Item README.md, CHANGELOG.md $dist
    Copy-Item unity\Kh2.cs (Join-Path $dist "unity")
    Copy-Item -Recurse unity\sample (Join-Path $dist "unity\sample")
    Copy-Item examples\min.c, examples\build_min.bat, examples\build_min.sh (Join-Path $dist "examples")
    Copy-Item -Recurse examples\python (Join-Path $dist "examples\python")
    # Guard: nothing from a disc may be in the package.
    $bad = Get-ChildItem -Recurse $dist | Where-Object { $_.Extension -in ".iso", ".bar", ".mdlx", ".mset", ".bin", ".map", ".a.us", ".bdx" }
    if ($bad) { throw "game data in the package: $($bad.FullName -join ', ')" }
    $files = Get-ChildItem -Recurse -File $dist
    "dist/libkh2: {0} files, {1:N0} bytes" -f $files.Count, ($files | Measure-Object Length -Sum).Sum
    if ($Zip) {
        $z = Join-Path $root "dist\libkh2.zip"
        if (Test-Path $z) { Remove-Item $z }
        Compress-Archive -Path $dist -DestinationPath $z
        "wrote $z"
    }
} finally {
    Pop-Location
}
