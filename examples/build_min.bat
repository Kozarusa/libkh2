@echo off
rem Builds examples\min.c against the release DLL. Run from a "Developer Command Prompt for VS"
rem (cl.exe on PATH), from the repository root (after `cargo build --release -p libkh2`) or from
rem the root of the unpacked package (bin\ holds the DLL there).
setlocal
set LIBDIR=target\release
if exist bin\kh2.dll.lib set LIBDIR=bin
if not exist build mkdir build
cl /nologo /W3 /I include examples\min.c %LIBDIR%\kh2.dll.lib /Fo:build\ /Fe:build\min.exe || exit /b 1
copy /y %LIBDIR%\kh2.dll build\ >nul
echo built build\min.exe
