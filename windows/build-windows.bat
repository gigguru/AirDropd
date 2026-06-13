@echo off
setlocal
echo Building AirDropd Setup.exe...
set RUSTFLAGS=-C target-feature=+crt-static
cargo build --release --manifest-path windows\Cargo.toml
if errorlevel 1 exit /b 1
if not exist dist mkdir dist
copy /Y target\release\AirDropd.exe dist\AirDropd.exe >nul
"C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "%~dp0installer\AirDropd.iss"
if errorlevel 1 exit /b 1
echo Done:
echo   target\release\AirDropd.exe
echo   dist\AirDropd.exe
echo   dist\AirDropd Setup.exe
