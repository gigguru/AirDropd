@echo off
setlocal
echo Building AirDropd release...
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --manifest-path windows\Cargo.toml
if errorlevel 1 exit /b 1

where iscc >nul 2>&1
if errorlevel 1 (
    echo Inno Setup not found. Install from https://jrsoftware.org/isinfo.php
    echo Then add ISCC.exe to PATH, or run:
    echo   "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" windows\installer\AirDropd.iss
    exit /b 1
)

if not exist dist mkdir dist
copy /Y target\release\AirDropd.exe dist\AirDropd.exe >nul
echo Compiling AirDropd Setup.exe...
iscc windows\installer\AirDropd.iss
if errorlevel 1 exit /b 1

echo.
echo Done:
echo   target\release\AirDropd.exe
echo   dist\AirDropd.exe
echo   dist\AirDropd Setup.exe
