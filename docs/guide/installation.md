# Installation & Deployment

NexusKVM is distributed in multiple native Linux packaging formats, including `.deb` packages (Ubuntu/Debian), `.rpm` packages (Fedora/RHEL/openSUSE), portable `AppImage` binaries, and direct compilation from source code.

---

## 1. Installation via `.deb` Package (Recommended for Ubuntu/Debian)

The `.deb` package is the simplest and recommended method. Its post-installation script (`deb-postinst.sh`) automatically configures:
1. Adding your user to the system `input` group.
2. Automatically loading the `uinput` kernel module.
3. Reloading and triggering `udev` rules.
4. Opening firewall ports `5258/tcp` and `5259/tcp` if `ufw` is active.

### Installation Command

```bash
# Install the downloaded or built deb package
sudo apt install ./NexusKVM_0.1.0_amd64.deb
```

::: warning ⚠️ MANDATORY STEP AFTER INSTALLATION: Log Out
After the initial installation, **you must log out of your Linux desktop session (Log Out) and log back in** (or reboot) for the new `input` group permissions to take effect in your graphical session.
:::

---

## 2. Building from Source Code

If you wish to compile the latest version directly from the repository:

### System Prerequisites

#### On Ubuntu / Debian:
```bash
sudo apt update
sudo apt install -y \
  build-essential \
  pkg-config \
  libssl-dev \
  libevdev-dev \
  libayatana-appindicator3-dev \
  libwebkit2gtk-4.1-dev \
  curl \
  git
```

#### On Fedora / RHEL:
```bash
sudo dnf groupinstall "Development Tools"
sudo dnf install \
  openssl-devel \
  libevdev-devel \
  libayatana-appindicator3-devel \
  webkit2gtk4.1-devel
```

#### On Arch Linux:
```bash
sudo pacman -S --needed \
  base-devel \
  openssl \
  libevdev \
  libappindicator-gtk3 \
  webkit2gtk-4.1
```

---

### Build Steps

1. **Clone the repository:**
   ```bash
   git clone https://github.com/jigonzalez930209/NexusKVM.git
   cd NexusKVM
   ```

2. **Install Node.js dependencies:**
   ```bash
   npm install
   ```

3. **Run in Development Mode (Hot Reload):**
   ```bash
   npm run tauri dev
   ```
   > This command automatically compiles the Rust background binaries (`nexus-kvmd`, `nexus-agent`, `nexusctl`, and `rkvm-client`) before opening the development UI.

4. **Build Production Packages:**
   ```bash
   # Build all Linux packages (.deb, .appimage)
   npm run package:linux

   # Or build a specific target:
   npm run package:deb
   npm run package:appimage
   npm run package:rpm
   ```

The generated artifacts will be located in:
```
target/release/bundle/deb/NexusKVM_0.1.0_amd64.deb
target/release/bundle/appimage/NexusKVM_0.1.0_amd64.AppImage
```

---

## 3. Developer Environment Setup (`install-dev.sh`)

For developers working locally on the source code who wish to apply permissions without generating a `.deb`:

```bash
# Installs local udev rules and adds current user to the input group
sudo bash scripts/install-dev.sh
```

---

## 4. Security & Execution Guidelines

::: danger 🛑 NEVER RUN THE GUI AS ROOT
NexusKVM adheres to strict privilege separation.
- **NEVER** run `sudo nexuskvm` or launch the graphical application as `root`.
- The UI runs completely unprivileged under your regular user account.
- Background capture and virtual device components access kernel interfaces safely via `input` group membership and a restricted Unix socket (`0660`).
:::
