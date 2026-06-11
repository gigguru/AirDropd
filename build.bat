@echo off
setlocal
echo Building portable AirDropd.exe (release)...
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --manifest-path windows\Cargo.toml
if errorlevel 1 (
    echo Build failed.
    exit /b 1
)
echo.
echo Done: target\release\AirDropd.exe
dir target\release\AirDropd.exe
