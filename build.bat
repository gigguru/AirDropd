@echo off
setlocal
echo Building portable AirDropd.exe (release)...
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --manifest-path windows\Cargo.toml
if errorlevel 1 (
    echo Build failed.
    exit /b 1
)
if not exist dist mkdir dist
copy /Y target\release\AirDropd.exe dist\AirDropd.exe >nul
echo.
echo Done:
echo   target\release\AirDropd.exe
echo   dist\AirDropd.exe
dir target\release\AirDropd.exe
