# 🚀 AirDropd

<p align="center">
  <img src="https://img.shields.io/badge/rust-202124?style=for-the-badge&logo=rust&logoColor=white" alt="Rust Badge"/>
  <img src="https://img.shields.io/badge/version-v0.1.0b-blueviolet?style=for-the-badge" alt="Version Badge"/>
  <img src="https://img.shields.io/github/stars/seregonwar/AirDropd?style=for-the-badge&logo=github&color=yellow" alt="GitHub Stars"/>
  <img src="https://img.shields.io/badge/license-MIT-2ea44f?style=for-the-badge&logo=open-source-initiative&logoColor=white" alt="License Badge"/>
  <img src="https://img.shields.io/github/downloads/seregonwar/AirDropd/total.svg?style=for-the-badge&color=orange&logo=cloud-download" alt="Total Downloads"/>
</p>

---

## 📌 Overview

**AirDropd** is a Windows application that implements Apple’s **AirDrop** protocol, allowing your PC to exchange files, folders, and links with iPhones, iPads, Macs — and other AirDropd PCs.

With AirDropd you can:
- 📤 Send files, folders, and links to nearby Apple devices
- 📥 Receive AirDrop transfers with an accept/decline prompt (or auto-accept)
- 🖱️ Drag and drop files straight onto a device in the radar  
All operations work **locally** over your network, without cloud or third-party services.

---

## ✨ Features

- 🔁 **AirDrop**: Send and receive files between Windows PCs and Apple devices  
- 📂 **Folder transfers**: Whole folders preserved with their structure  
- 📡 **Distance radar**: Devices placed by Bluetooth signal strength, closest in the middle  
- 🏷️ **Device-type icons**: iPhone, iPad, MacBook, Mac, Watch, Apple TV identified from their mDNS hardware records  
- 📱 **iPhone detection**: Apple Continuity BLE beacons reveal nearby iPhones/iPads (they never advertise AirDrop on regular Wi-Fi — AWDL only), including "AirDrop open" status when their share sheet is up  
- 🧭 **Lost-device finder**: enable *Show all nearby devices* to also see AirPods, AirTags, and Find My beacons with live dBm signal readout — walk around and watch the signal rise  
- 📲 **QR Web Drop + DJ Mode**: guests scan a QR code (iPhone Camera or Android) and send files in the browser — no app, no internet. **DJ Mode** shows a full-screen QR and auto-saves every upload. Each phone gets its own folder under `Downloads/AirDropd/WebDrop/` so repeat sends before a show stay organized  
- 🖱️ **Drag & drop**: Drop files on the window — they go to the selected device  
- 🔍 **Device Discovery**: Automatic discovery via **mDNS** + **Bluetooth LE**  
- 📊 **Live progress**: Real streamed uploads with progress reporting  
- 🗜️ **Wire-format compatible**: gzip cpio archives in both **odc** (what Apple's sharingd/libarchive produces) and **newc** formats  
- 🎨 **Modern Interface**: Clean, responsive macOS-style UI

---

## 💻 System Requirements

- 🧩 **Windows 10+** or **macOS 12+** (Apple Silicon or Intel)  
- 🌐 Network adapter on the same Wi-Fi as guest phones  
- 🔐 Windows: run as administrator for full mDNS (optional for QR Web Drop only)

---

## 🧱 Project layout

AirDropd is split into platform-specific app folders plus shared core code:

```
AirDropd/
├── core/          # Shared Rust library (UI, protocols, Web Drop, services)
├── apple/         # macOS app → builds AirDropd.app
├── windows/       # Windows app → builds AirDropd.exe + installer
├── assets/        # Icons shared by both platforms
└── OWDL/          # AWDL protocol library
```

Platform-specific code (system tray, firewall, BLE advertising) lives in `core/` behind `#[cfg(...)]` gates, while each platform folder owns its own binary entry point and packaging.

---

## 🧱 Building

### Download (recommended)

Pre-built binaries are produced by GitHub Actions on every push to `main`:

| Platform | Workflow | Artifact |
|---|---|---|
| Windows | [Build Windows](https://github.com/gigguru/AirDropd/actions/workflows/build-windows.yml) | `AirDropd.exe` + installer |
| macOS | [Build macOS](https://github.com/gigguru/AirDropd/actions/workflows/build-macos.yml) | `AirDropd.app` (`.app` bundle) |

### Build from source (macOS)

```bash
chmod +x apple/build-macos.sh
./apple/build-macos.sh
# → apple/dist/AirDropd.app
open apple/dist/AirDropd.app
```

To install: drag `AirDropd.app` to **Applications**, or run:

```bash
cp -R apple/dist/AirDropd.app /Applications/
```

On first launch, macOS may ask you to allow incoming network connections (for the local Web Drop server on port 8771).

### Build from source (Windows)

```bat
cargo build --release --manifest-path windows\Cargo.toml
```

Output: `target\release\AirDropd.exe` (portable, statically linked MSVC runtime).

For the installer, run Inno Setup on `windows\installer\AirDropd.iss`.

### Build from source (Rust)

```bash
git clone https://github.com/gigguru/AirDropd.git
cd AirDropd

# macOS
cargo build --release --manifest-path apple/Cargo.toml

# Windows
cargo build --release --manifest-path windows/Cargo.toml
```

---

## ▶️ Usage

1. Run the application (allow the Windows Firewall ports when prompted)  
2. AirDropd automatically discovers nearby Apple devices and shows them in the radar  
3. **Send**: click a device, then *Send Files*, *Send Folder*, or *Send Link* — or just drag files onto the window  
4. **Receive**: have the iPhone/Mac set AirDrop visibility to *Everyone*, send to your PC, and accept the prompt (files land in `Downloads\AirDropd`)  
5. **DJ mode**: tap **DJ Mode** for a full-screen QR — guests scan and send; each phone's files land in `Downloads/AirDropd/WebDrop/<their name>/`. Enable *Automatically accept incoming transfers* in Settings for legacy AirDrop receives too

---

## 🧠 Architecture

- 🔍 **mDNS + BLE Discovery**: Multicast DNS plus Bluetooth LE beacons for discovery and distance  
- 💾 **AirDrop Protocol**: Apple-compatible HTTPS `/Discover → /Ask → /Upload` with binary plists and gzip cpio archives  
- 🧰 **UI**: Modern, responsive user interface built with iced (canvas radar)

---

## 📜 License

This project is licensed under the **MIT License**.  
See the `LICENSE` file for more details.

---

## 🤝 Contributing

Contributions are **welcome**!  
Feel free to open a **Pull Request** or report issues in the tracker.

---

> Built with ❤️ by [SeregonWar](https://github.com/seregonwar) 




