# Everything a change must pass, in one run (Windows PowerShell 5.1+).
#   powershell -ExecutionPolicy Bypass -File scripts/ci.ps1            (KH2_ISO_FM or KH2_ISO = the Final Mix disc: the disc tests run too)
# Steps: release build, clippy, the test suite (synthetic, disc, golden, layout, oracle, objects,
# package, soak skipped unless KH2_SOAK_SECS), a longer fuzz run, the binding diff (DLL / kh2.h /
# Kh2.cs), the API reference check, the package build and its scan, the C# example build.
# Exit code 0 only when every step passed.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
$failed = @()
function Step($name, [scriptblock]$body) {
    Write-Host "== $name"
    & $body
    if ($LASTEXITCODE -ne 0) { $script:failed += $name; Write-Host "!! $name failed ($LASTEXITCODE)" }
}
try {
    if ($env:KH2_ISO_FM) { $env:KH2_ISO = $env:KH2_ISO_FM }
    if (-not $env:KH2_ISO) { Write-Host "note: neither KH2_ISO_FM nor KH2_ISO is set; the disc tests skip themselves" }
    Step "build" { cargo build --release -p libkh2 }
    Step "clippy" { cargo clippy --release -p libkh2 --all-targets -- -D warnings }
    Step "tests" { cargo test --release }
    $env:KH2_FUZZ_STEPS = "20000"
    Step "fuzz" { cargo test --release --test fuzz }
    Remove-Item Env:KH2_FUZZ_STEPS
    Step "bindings" { python scripts/check_bindings.py }
    Step "api-reference" { python scripts/gen_api_ref.py --check }
    Step "package" { powershell -ExecutionPolicy Bypass -File scripts/package.ps1 -Zip }
    Step "package-scan" { cargo test --release --test package }
    if (Get-Command dotnet -ErrorAction SilentlyContinue) {
        Step "csharp" { dotnet build -c Release examples/csharp }
    } else { Write-Host "note: no dotnet; the C# example is not built" }
} finally {
    Pop-Location
}
if ($failed.Count -gt 0) { Write-Host "FAILED: $($failed -join ', ')"; exit 1 }
Write-Host "all steps passed"
exit 0
