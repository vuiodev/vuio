# VuIO Installation Guide

This guide covers all official installation methods for VuIO Media Server across Linux, macOS, Windows, Docker, Kubernetes, and BSD.

---

## Table of Contents

1. [Linux Installation](#linux-installation)
   - [APT Repository (Ubuntu / Debian / Mint / Pop!_OS)](#apt-repository-ubuntu--debian--mint--pop_os)
   - [DNF / YUM Repository (Fedora / RHEL / CentOS / Rocky / Alma)](#dnf--yum-repository-fedora--rhel--centos--rocky--alma)
   - [Alpine Linux APK Repository](#alpine-linux-apk-repository)
   - [Direct DEB, RPM & APK Downloads](#direct-deb-rpm--apk-downloads)
   - [Arch Linux](#arch-linux)
   - [Generic & Legacy Linux (Musl Static Binaries)](#generic--legacy-linux-musl-static-binaries)
2. [Homebrew (macOS & Linux)](#homebrew-macos--linux)
3. [Docker Container](#docker-container)
4. [Kubernetes (Helm 3)](#kubernetes-helm-3)
5. [Windows](#windows)
6. [FreeBSD & BSD](#freebsd--bsd)
7. [Systemd Service Management](#systemd-service-management)
8. [Automatic Self-Updater](#automatic-self-updater)

---

## Linux Installation

### APT Repository (Ubuntu / Debian / Mint / Pop!_OS)

Add our official APT repository to get automatic background updates via `apt`:

```bash
# Add the VuIO APT repository
echo "deb [trusted=yes] https://vuiodev.github.io/vuio/apt stable main" | sudo tee /etc/apt/sources.list.d/vuio.list

# Update package index and install vuio
sudo apt update
sudo apt install vuio
```

> **Distro Codename Alternative**: If your setup uses codenames, you can also use `deb [trusted=yes] https://vuiodev.github.io/vuio/apt $(lsb_release -cs) main`.

---

### DNF / YUM Repository (Fedora / RHEL / CentOS / Rocky / Alma)

Add our official RPM repository:

```bash
# Add the VuIO DNF repository
sudo dnf config-manager --add-repo https://vuiodev.github.io/vuio/rpm/vuio.repo

# Install vuio
sudo dnf install vuio
```

For older distributions using `yum`:
```bash
sudo yum-config-manager --add-repo https://vuiodev.github.io/vuio/rpm/vuio.repo
sudo yum install vuio
```

---

### Alpine Linux APK Repository

Add our official Alpine Linux repository:

```bash
# Add the VuIO Alpine repository to /etc/apk/repositories
echo "https://vuiodev.github.io/vuio/alpine/stable/main" | sudo tee -a /etc/apk/repositories

# Update package index and install vuio
sudo apk update
sudo apk add --allow-untrusted vuio
```

---

### Direct DEB & RPM Downloads

If you prefer downloading standalone packages without adding a repository:

#### Debian / Ubuntu (`.deb`)
```bash
# For x86_64 (amd64)
wget https://vuiodev.github.io/vuio/packages/vuio_latest_amd64.deb
sudo dpkg -i vuio_latest_amd64.deb

# For ARM64 (aarch64 / Raspberry Pi 64-bit)
wget https://vuiodev.github.io/vuio/packages/vuio_latest_arm64.deb
sudo dpkg -i vuio_latest_arm64.deb
```

#### Fedora / RHEL (`.rpm`)
```bash
# For x86_64
wget https://vuiodev.github.io/vuio/packages/vuio_latest_x86_64.rpm
sudo rpm -ivh vuio_latest_x86_64.rpm

# For ARM64 (aarch64)
wget https://vuiodev.github.io/vuio/packages/vuio_latest_aarch64.rpm
sudo rpm -ivh vuio_latest_aarch64.rpm
```

#### Alpine Linux (`.apk`)
```bash
# For x86_64
wget https://vuiodev.github.io/vuio/packages/vuio_latest_x86_64.apk
sudo apk add --allow-untrusted vuio_latest_x86_64.apk

# For ARM64 (aarch64)
wget https://vuiodev.github.io/vuio/packages/vuio_latest_aarch64.apk
sudo apk add --allow-untrusted vuio_latest_aarch64.apk
```

---

### Arch Linux (Pacman Repository)

Add our official Arch Linux repository to `/etc/pacman.conf`:

```ini
[vuio]
SigLevel = Optional TrustAll
Server = https://vuiodev.github.io/vuio/arch/os/$arch
```

Then synchronize pacman and install:
```bash
sudo pacman -Sy vuio
```

#### Direct `.pkg.tar.zst` Download
```bash
# For x86_64
wget https://vuiodev.github.io/vuio/packages/vuio_latest_x86_64.pkg.tar.zst
sudo pacman -U vuio_latest_x86_64.pkg.tar.zst

# For ARM64 (aarch64 / Arch Linux ARM)
wget https://vuiodev.github.io/vuio/packages/vuio_latest_aarch64.pkg.tar.zst
sudo pacman -U vuio_latest_aarch64.pkg.tar.zst
```

#### Local PKGBUILD Build
```bash
git clone https://github.com/vuiodev/vuio.git
cd vuio/packaging/linux
./build-arch.sh
sudo pacman -U vuio-*.pkg.tar.zst
```

---

### Generic & Legacy Linux (Musl Static Binaries)

For Linux distributions without `glibc` 2.31+ (such as Alpine Linux, CentOS 7, RHEL 7, or lightweight embedded Linux), use our statically linked `musl` binaries:

```bash
# Download musl release tarball (x86_64)
curl -sSL -o vuio.tar.gz https://github.com/vuiodev/vuio/releases/latest/download/vuio-linux-x86_64.tar.gz
tar -xvf vuio.tar.gz
sudo mv vuio /usr/local/bin/
```

---

## Homebrew (macOS & Linux)

Install VuIO on macOS or Linux using Homebrew:

```bash
brew tap vuiodev/vuio
brew install vuio
```

Run VuIO directly from your terminal:
```bash
vuio /path/to/media
```

---

## Docker Container

Official multi-architecture Docker images are published to GitHub Container Registry (`ghcr.io/vuiodev/vuio`) for `linux/amd64` and `linux/arm64`.

> **Note**: Docker host networking is recommended for SSDP/UPnP LAN discovery to function properly. For full Docker configuration and environment variable details, see [Docker Guide](docker.md).

### Quick Start with Docker CLI

```bash
docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  -v /path/to/media:/media:ro \
  -v ./vuio-config:/config \
  -e VUIO_IP=192.168.1.100 \
  -e VUIO_PORT=8080 \
  -e VUIO_WEB_PORT=8090 \
  -e VUIO_MEDIA_DIRS=/media \
  ghcr.io/vuiodev/vuio:latest
```

### Docker Compose

Create a `docker-compose.yml` file:

```yaml
version: '3.8'

services:
  vuio:
    image: ghcr.io/vuiodev/vuio:latest
    container_name: vuio-server
    restart: unless-stopped
    network_mode: host
    environment:
      - VUIO_IP=192.168.1.100
      - VUIO_PORT=8080
      - VUIO_WEB_PORT=8090
      - VUIO_SERVER_NAME=VuIO Media Server
      - VUIO_MEDIA_DIRS=/media/movies,/media/music,/media/pictures
    volumes:
      - ./vuio-config:/config
      - /path/to/movies:/media/movies:ro
      - /path/to/music:/media/music:ro
      - /path/to/pictures:/media/pictures:ro
```

Run with:
```bash
docker-compose up -d
```

---

## Kubernetes (Helm 3)

Deploy VuIO to a Kubernetes cluster using the official Helm chart from GHCR. For comprehensive cluster setup, see [Kubernetes Guide](kubernetes.md).

```bash
# Install directly from GitHub Container Registry
helm install vuio oci://ghcr.io/vuiodev/charts/vuio --version 0.0.45
```

Or install from local source:
```bash
helm install vuio ./helm/vuio
```

---

## Windows

### Option 1: MSI Installer
Download the latest `.msi` installer from GitHub Releases and double-click to install, or run:
```powershell
msiexec /i vuio_latest_x64.msi
```

### Option 2: Standalone Executable
Download `vuio-windows-x86_64.exe`, rename to `vuio.exe`, and run from PowerShell or Command Prompt:
```powershell
.\vuio.exe C:\Media
```

---

## FreeBSD & BSD

Download the FreeBSD pre-compiled release archive:

```bash
curl -sSL -o vuio-freebsd.tar.gz https://github.com/vuiodev/vuio/releases/latest/download/vuio-freebsd-x86_64.tar.gz
tar -xvf vuio-freebsd.tar.gz
sudo mv vuio /usr/local/bin/
```

Run VuIO:
```bash
vuio /path/to/media
```

---

## Systemd Service Management

When installed via DEB or RPM packages, VuIO includes integrated `systemd` unit files (`/lib/systemd/system/vuio.service`).

### Enable and Start Service
```bash
sudo systemctl daemon-reload
sudo systemctl enable --now vuio
```

### Check Service Status
```bash
sudo systemctl status vuio
```

### View Real-time Logs
```bash
sudo journalctl -u vuio -f
```

### Configuration File Location
The package creates a default configuration file at `/etc/vuio/vuio.toml`. Edit this file to customize server settings, ports, and media paths:
```bash
sudo nano /etc/vuio/vuio.toml
sudo systemctl restart vuio
```

---

## Automatic Self-Updater

VuIO features a built-in self-updater. To update an existing installed binary to the latest GitHub release at any time:

```bash
vuio --update
```
