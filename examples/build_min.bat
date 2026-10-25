@echo off
rem Builds examples\min.c against the release DLL. Run from a "Developer Command Prompt for VS"
rem (cl.exe on PATH) after `cargo build --release -p libkh2`, from the repository root.
setlocal
if not exist build mkdir build
cl /nologo /W3 /I include examples\min.c target\release\kh2.dll.lib /Fo:build\ /Fe:build\min.exe || exit /b 1
copy /y target\release\kh2.dll build\ >nul
echo built build\min.exe
