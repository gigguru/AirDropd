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
- 🖱️ **Drag & drop**: Drop files on the window — they go to the selected device  
- 🔍 **Device Discovery**: Automatic discovery via **mDNS** + **Bluetooth LE**  
- 📊 **Live progress**: Real streamed uploads with progress reporting  
- 🎨 **Modern Interface**: Clean, responsive macOS-style UI

---

## 💻 System Requirements

- 🧩 Windows 10 or later  
- 🌐 Network adapter with **multicast** support  
- 🔐 Run as administrator (required for mDNS service)

---

## 🧱 Building

### Download (recommended)

Pre-built portable **`AirDropd.exe`** is produced by GitHub Actions on every push to `main`:

1. Open [Actions → Build Windows](https://github.com/gigguru/AirDropd/actions/workflows/build-windows.yml)
2. Download the **AirDropd-windows-x86_64** artifact — it contains a single `AirDropd.exe` with no extra DLLs to ship.

Tag a release (`v0.1.0`, etc.) to attach the exe to a GitHub Release automatically.

### Build from source (Windows)

```bat
build.bat
```

Output: `target\release\AirDropd.exe` (portable, statically linked MSVC runtime).

### Build from source (Rust)

```bash
git clone https://github.com/gigguru/AirDropd.git
cd AirDropd
cargo build --release --bin AirDropd
```

---

## ▶️ Usage

1. Run the application (allow the Windows Firewall ports when prompted)  
2. AirDropd automatically discovers nearby Apple devices and shows them in the radar  
3. **Send**: click a device, then *Send Files*, *Send Folder*, or *Send Link* — or just drag files onto the window  
4. **Receive**: have the iPhone/Mac set AirDrop visibility to *Everyone*, send to your PC, and accept the prompt (files land in `Downloads\AirDropd`)  
5. **DJ mode**: enable *Automatically accept incoming transfers* in Settings so guests' tracks save without interaction

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




