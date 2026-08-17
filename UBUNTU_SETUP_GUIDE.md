# JARVIS — Ubuntu Linux Setup & Testing Guide

This guide provides step-by-step instructions for setting up, building, running, and testing **JARVIS** on a fresh **Ubuntu Linux** installation (Ubuntu 22.04 LTS / 24.04 LTS, X11 or Wayland).

---

## 1. Install Ubuntu System Dependencies

Open a terminal on your Ubuntu machine and run the following command to install required system build tools, audio libraries (`ALSA`), GTK/WebKit display packages for Tauri 2, and desktop utilities:

```bash
sudo apt update && sudo apt install -y \
    build-essential \
    curl \
    wget \
    git \
    pkg-config \
    libssl-dev \
    libasound2-dev \
    libgtk-3-dev \
    libwebkit2gtk-4.1-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    xdotool \
    wmctrl \
    xclip \
    wl-clipboard \
    libnotify-bin \
    xdg-utils
```

> **Note for Ubuntu 22.04 LTS:** If `libwebkit2gtk-4.1-dev` is not found, install `libwebkit2gtk-4.0-dev` instead:
> ```bash
> sudo apt install -y libwebkit2gtk-4.0-dev
> ```

---

## 2. Install Rust Toolchain & Node.js

### A. Install Rust (`rustup`)
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```

Verify Rust installation:
```bash
rustc --version
cargo --version
```

### B. Install Node.js (v20 LTS)
```bash
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install -y nodejs
```

Verify Node installation:
```bash
node -v
npm -v
```

---

## 3. Clone Repository & Install Frontend Dependencies

```bash
git clone https://github.com/ASHUVINAYAK1/Jarvis.git
cd Jarvis
```

Install npm packages for the desktop HUD app:
```bash
cd apps/desktop
npm install
cd ../..
```

---

## 4. Run Cargo Test Suite

Run the full Rust workspace test suite across all 20 crates to verify that the build compiles cleanly on Linux:

```bash
cargo test --workspace
```

*Expected output:*
```text
test result: ok. 71 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

---

## 5. Launch JARVIS Desktop (Tauri Dev)

Launch the JARVIS desktop interface:

```bash
cd apps/desktop
npm run tauri dev
```

---

## 6. Testing JARVIS Capabilities on Ubuntu

Once the futuristic JARVIS HUD window opens:

1. **Physical Microphone Capture & Audio Energy:**
   - Check your terminal output for physical microphone initialization:
     ```text
     [MIC DIAGNOSTIC] Microphone initialized successfully
     [MIC TEST] Live voice audio input detected! rms=0.0125 peak=0.0380
     ```

2. **Hands-Free "Jarvis" Wake Word:**
   - Speak **"Jarvis"** into your microphone without clicking any buttons.
   - The HUD will transition from `IDLE` $\rightarrow$ `WAKE_DETECTED` $\rightarrow$ `LISTENING`.

3. **Speech Command Execution:**
   - Say: **"Jarvis, open Chrome"**
     - Transcribes spoken audio into text in real time.
     - Spawns Google Chrome / Chromium browser via `LinuxPlatformAdapter` multi-stage resolver.
   - Say: **"Jarvis, open Spotify"**
     - Spawns Spotify client on Ubuntu desktop.
   - Say: **"What is the time?"**
     - Returns local time and speaks voice response.

---

## 7. Troubleshooting & Linux Tips

### A. Microphone Permission & Audio Device Access
If audio input is not detected:
- Ensure your user belongs to the `audio` group:
  ```bash
  sudo usermod -aG audio $USER
  ```
- Re-login or reboot for group changes to take effect.

### B. Display Server (X11 vs Wayland)
JARVIS automatically detects whether your session is running on X11 or Wayland:
- On **X11**: Full window management (`xdotool`, `wmctrl`) and window focus are supported.
- On **Wayland**: Global window manipulation APIs return structured explicit permission warnings as required by Wayland security policy. Application launching, speech recognition, and notifications work seamlessly on both Wayland and X11.

---

*JARVIS — Multiplatform Local Personal AI Assistant*
