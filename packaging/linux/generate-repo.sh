#!/bin/bash

# Master script to generate APT, RPM, Alpine APK, and Arch Linux package repository metadata for GitHub Pages.
# Supports universal repos, distro codename aliases, and direct download links.

set -e

PACKAGES_DIR="$(cd "$(dirname "${1:-./builds}")" && pwd)/$(basename "${1:-./builds}")"
OUTPUT_DIR="$(mkdir -p "${2:-./pages_site}" && cd "${2:-./pages_site}" && pwd)"

# Colors
GREEN='\033[0;32m'
CYAN='\033[0;36m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

echo -e "${GREEN}=== VuIO Linux Package Repository Generator ===${NC}"
echo "Source packages directory: $PACKAGES_DIR"
echo "Output Pages directory:    $OUTPUT_DIR"
echo ""

mkdir -p "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR/packages"
mkdir -p "$OUTPUT_DIR/apt/pool/main/v/vuio"
mkdir -p "$OUTPUT_DIR/apt/dists/stable/main/binary-amd64"
mkdir -p "$OUTPUT_DIR/apt/dists/stable/main/binary-arm64"
mkdir -p "$OUTPUT_DIR/rpm/packages"
mkdir -p "$OUTPUT_DIR/alpine/stable/main/x86_64"
mkdir -p "$OUTPUT_DIR/alpine/stable/main/aarch64"
mkdir -p "$OUTPUT_DIR/arch/os/x86_64"
mkdir -p "$OUTPUT_DIR/arch/os/aarch64"

# Copy DEB packages
if ls "$PACKAGES_DIR"/*.deb 1> /dev/null 2>&1; then
    echo -e "${CYAN}Processing DEB packages...${NC}"
    cp "$PACKAGES_DIR"/*.deb "$OUTPUT_DIR/apt/pool/main/v/vuio/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*.deb "$OUTPUT_DIR/packages/" 2>/dev/null || true
fi

# Copy RPM packages
if ls "$PACKAGES_DIR"/*.rpm 1> /dev/null 2>&1; then
    echo -e "${CYAN}Processing RPM packages...${NC}"
    cp "$PACKAGES_DIR"/*.rpm "$OUTPUT_DIR/rpm/packages/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*.rpm "$OUTPUT_DIR/packages/" 2>/dev/null || true
fi

# Copy APK packages (Alpine Linux)
if ls "$PACKAGES_DIR"/*.apk 1> /dev/null 2>&1; then
    echo -e "${CYAN}Processing Alpine APK packages...${NC}"
    cp "$PACKAGES_DIR"/*x86_64.apk "$OUTPUT_DIR/alpine/stable/main/x86_64/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*aarch64.apk "$OUTPUT_DIR/alpine/stable/main/aarch64/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*.apk "$OUTPUT_DIR/packages/" 2>/dev/null || true
fi

# Copy Arch packages (Arch Linux)
if ls "$PACKAGES_DIR"/*.pkg.tar.zst 1> /dev/null 2>&1; then
    echo -e "${CYAN}Processing Arch Linux packages...${NC}"
    cp "$PACKAGES_DIR"/*x86_64.pkg.tar.zst "$OUTPUT_DIR/arch/os/x86_64/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*aarch64.pkg.tar.zst "$OUTPUT_DIR/arch/os/aarch64/" 2>/dev/null || true
    cp "$PACKAGES_DIR"/*.pkg.tar.zst "$OUTPUT_DIR/packages/" 2>/dev/null || true
fi

# Create convenient "latest" symlinks/copies in packages/
pushd "$OUTPUT_DIR/packages" > /dev/null
for arch in amd64 arm64; do
    latest_deb=$(ls vuio_*_${arch}.deb 2>/dev/null | tail -n1 || true)
    if [ -n "$latest_deb" ]; then
        cp -f "$latest_deb" "vuio_latest_${arch}.deb"
    fi
done
for arch in x86_64 aarch64; do
    latest_rpm=$(ls vuio-*-1.${arch}.rpm 2>/dev/null | tail -n1 || true)
    if [ -n "$latest_rpm" ]; then
        cp -f "$latest_rpm" "vuio_latest_${arch}.rpm"
    fi
    latest_apk=$(ls vuio-*.${arch}.apk 2>/dev/null | tail -n1 || true)
    if [ -n "$latest_apk" ]; then
        cp -f "$latest_apk" "vuio_latest_${arch}.apk"
    fi
    latest_archpkg=$(ls vuio-*-1-${arch}.pkg.tar.zst 2>/dev/null | tail -n1 || true)
    if [ -n "$latest_archpkg" ]; then
        cp -f "$latest_archpkg" "vuio_latest_${arch}.pkg.tar.zst"
    fi
done
popd > /dev/null

# -----------------------------------------------------------------------------
# 1. Build APT Repository Metadata
# -----------------------------------------------------------------------------
echo -e "${YELLOW}Building APT repository metadata...${NC}"

APT_DIR="$OUTPUT_DIR/apt"
pushd "$APT_DIR" > /dev/null

if command -v dpkg-scanpackages &> /dev/null; then
    dpkg-scanpackages --arch amd64 pool/main/v/vuio /dev/null > dists/stable/main/binary-amd64/Packages 2>/dev/null || true
    dpkg-scanpackages --arch arm64 pool/main/v/vuio /dev/null > dists/stable/main/binary-arm64/Packages 2>/dev/null || true
elif command -v apt-ftparchive &> /dev/null; then
    apt-ftparchive packages pool/main/v/vuio > dists/stable/main/binary-amd64/Packages 2>/dev/null || true
    cp dists/stable/main/binary-amd64/Packages dists/stable/main/binary-arm64/Packages
else
    echo -e "${RED}Warning: neither dpkg-scanpackages nor apt-ftparchive found. Creating minimal Packages file.${NC}"
    touch dists/stable/main/binary-amd64/Packages
    touch dists/stable/main/binary-arm64/Packages
fi

gzip -9c dists/stable/main/binary-amd64/Packages > dists/stable/main/binary-amd64/Packages.gz 2>/dev/null || true
gzip -9c dists/stable/main/binary-arm64/Packages > dists/stable/main/binary-arm64/Packages.gz 2>/dev/null || true

cat > dists/stable/Release << 'EOF'
Origin: VuIO
Label: VuIO Media Server Repository
Suite: stable
Codename: stable
Architectures: amd64 arm64
Components: main
Description: Official APT repository for VuIO Media Server
EOF

for codename in jammy noble focal bionic bookworm bullseye buster ubuntu debian; do
    if [ ! -e "dists/$codename" ]; then
        ln -s stable "dists/$codename" 2>/dev/null || cp -r dists/stable "dists/$codename"
    fi
done
popd > /dev/null

# -----------------------------------------------------------------------------
# 2. Build RPM Repository Metadata
# -----------------------------------------------------------------------------
echo -e "${YELLOW}Building RPM repository metadata...${NC}"

RPM_DIR="$OUTPUT_DIR/rpm"
pushd "$RPM_DIR" > /dev/null

if command -v createrepo_c &> /dev/null; then
    createrepo_c .
elif command -v createrepo &> /dev/null; then
    createrepo .
else
    echo -e "${RED}Warning: createrepo_c / createrepo not found.${NC}"
fi

cat > vuio.repo << 'EOF'
[vuio]
name=VuIO Media Server Repository
baseurl=https://vuiodev.github.io/vuio/rpm/
enabled=1
gpgcheck=0
repo_gpgcheck=0
EOF

for distro in fedora rhel centos rocky alma; do
    if [ ! -d "$distro" ]; then
        mkdir -p "$distro"
        ln -s ../repodata "$distro/repodata" 2>/dev/null || true
        ln -s ../packages "$distro/packages" 2>/dev/null || true
        ln -s ../vuio.repo "$distro/vuio.repo" 2>/dev/null || true
    fi
done
popd > /dev/null

# -----------------------------------------------------------------------------
# 3. Build Alpine APK Repository Metadata (APKINDEX.tar.gz)
# -----------------------------------------------------------------------------
echo -e "${YELLOW}Building Alpine APK repository metadata...${NC}"

for arch in x86_64 aarch64; do
    ARCH_DIR="$OUTPUT_DIR/alpine/stable/main/$arch"
    if [ -d "$ARCH_DIR" ] && ls "$ARCH_DIR"/*.apk 1>/dev/null 2>&1; then
        pushd "$ARCH_DIR" > /dev/null
        if command -v apk &> /dev/null; then
            apk index -o APKINDEX.tar.gz *.apk 2>/dev/null || true
        else
            rm -f APKINDEX APKINDEX.tar.gz
            for apkfile in *.apk; do
                if [ -f "$apkfile" ]; then
                    # APK files are gzipped tarballs concatenated; extract .PKGINFO from control segment
                    tar -xzf "$apkfile" .PKGINFO 2>/dev/null || true
                    if [ -f .PKGINFO ]; then
                        pkgname=$(grep "^pkgname = " .PKGINFO | cut -d'=' -f2)
                        pkgver=$(grep "^pkgver = " .PKGINFO | cut -d'=' -f2)
                        pkgdesc=$(grep "^pkgdesc = " .PKGINFO | cut -d'=' -f2)
                        url=$(grep "^url = " .PKGINFO | cut -d'=' -f2)
                        size=$(grep "^size = " .PKGINFO | cut -d'=' -f2)
                        bdate=$(grep "^builddate = " .PKGINFO | cut -d'=' -f2)

                        cat >> APKINDEX << EOF
P:${pkgname// /}
V:${pkgver// /}
A:${arch}
S:${size// /}
I:${size// /}
T:${pkgdesc# }
U:${url// /}
L:MIT OR Apache-2.0
t:${bdate// /}

EOF
                        rm -f .PKGINFO
                    fi
                fi
            done
            if [ -f APKINDEX ]; then
                tar -czf APKINDEX.tar.gz APKINDEX
                rm -f APKINDEX
            else
                touch APKINDEX.tar.gz
            fi
        fi
        popd > /dev/null
    fi
done

# Create Alpine version symlinks
ALPINE_BASE="$OUTPUT_DIR/alpine"
for ver in v3.20 v3.21 v3.19 v3.18 edge latest-stable; do
    if [ ! -e "$ALPINE_BASE/$ver" ]; then
        ln -s stable "$ALPINE_BASE/$ver" 2>/dev/null || cp -r "$ALPINE_BASE/stable" "$ALPINE_BASE/$ver"
    fi
done

# -----------------------------------------------------------------------------
# 4. Build Arch Linux Package Repository Metadata (vuio.db.tar.gz)
# -----------------------------------------------------------------------------
echo -e "${YELLOW}Building Arch Linux repository metadata...${NC}"

for arch in x86_64 aarch64; do
    ARCH_DIR="$OUTPUT_DIR/arch/os/$arch"
    if [ -d "$ARCH_DIR" ] && ls "$ARCH_DIR"/*.pkg.tar.zst 1>/dev/null 2>&1; then
        pushd "$ARCH_DIR" > /dev/null
        if command -v repo-add &> /dev/null; then
            repo-add vuio.db.tar.gz *.pkg.tar.zst 2>/dev/null || true
        else
            rm -rf db_build vuio.db vuio.db.tar.gz vuio.files vuio.files.tar.gz
            mkdir -p db_build
            for pkg in *.pkg.tar.zst; do
                if [ -f "$pkg" ]; then
                    # Extract PKGINFO from Arch package
                    mkdir -p pkgtemp
                    zstd -d "$pkg" -o pkgtemp/pkg.tar --quiet 2>/dev/null && \
                        tar -xf pkgtemp/pkg.tar -C pkgtemp .PKGINFO 2>/dev/null || true
                    if [ -f pkgtemp/.PKGINFO ]; then
                        pname=$(grep "^pkgname = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)
                        pver=$(grep "^pkgver = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)
                        pdesc=$(grep "^pkgdesc = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)
                        purl=$(grep "^url = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)
                        psize=$(grep "^size = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)
                        parch=$(grep "^arch = " pkgtemp/.PKGINFO | cut -d'=' -f2 | xargs)

                        ENTRY_DIR="db_build/${pname}-${pver}"
                        mkdir -p "$ENTRY_DIR"
                        cat > "$ENTRY_DIR/desc" << EOF
%FILENAME%
$pkg

%NAME%
$pname

%VERSION%
$pver

%DESC%
$pdesc

%URL%
$purl

%ARCH%
$parch

%CSIZE%
$(du -b "$pkg" | cut -f1)

%ISIZE%
$psize

EOF
                    fi
                    rm -rf pkgtemp
                fi
            done
            if [ -d db_build ] && [ "$(ls db_build)" ]; then
                tar -czf vuio.db.tar.gz -C db_build .
                rm -rf db_build
            else
                rm -rf db_build
                touch vuio.db.tar.gz
            fi
            ln -sf vuio.db.tar.gz vuio.db 2>/dev/null || true
        fi
        popd > /dev/null
    fi
done

# -----------------------------------------------------------------------------
# 5. Generate HTML Landing Page for GitHub Pages
# -----------------------------------------------------------------------------
echo -e "${YELLOW}Generating index.html landing page...${NC}"

cat > "$OUTPUT_DIR/index.html" << 'EOF'
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>VuIO Media Server - Linux Packages & Repositories</title>
    <style>
        :root {
            --bg: #0f172a;
            --card-bg: #1e293b;
            --accent: #38bdf8;
            --accent-hover: #0284c7;
            --text: #f8fafc;
            --text-muted: #94a3b8;
            --code-bg: #090d16;
            --border: #334155;
        }
        body {
            font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
            background-color: var(--bg);
            color: var(--text);
            margin: 0;
            padding: 40px 20px;
            line-height: 1.6;
        }
        .container {
            max-width: 900px;
            margin: 0 auto;
        }
        header {
            text-align: center;
            margin-bottom: 40px;
        }
        h1 {
            font-size: 2.5rem;
            color: var(--accent);
            margin-bottom: 8px;
        }
        p.subtitle {
            color: var(--text-muted);
            font-size: 1.1rem;
        }
        .card {
            background-color: var(--card-bg);
            border: 1px solid var(--border);
            border-radius: 12px;
            padding: 24px;
            margin-bottom: 24px;
            box-shadow: 0 4px 6px -1px rgba(0, 0, 0, 0.1);
        }
        h2 {
            font-size: 1.4rem;
            margin-top: 0;
            color: var(--text);
            border-bottom: 1px solid var(--border);
            padding-bottom: 10px;
        }
        pre {
            background-color: var(--code-bg);
            border: 1px solid var(--border);
            border-radius: 8px;
            padding: 14px;
            overflow-x: auto;
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
            font-size: 0.9rem;
            color: #e2e8f0;
        }
        code {
            font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
        }
        .btn {
            display: inline-block;
            background-color: var(--accent);
            color: #0f172a;
            font-weight: 600;
            padding: 10px 18px;
            border-radius: 6px;
            text-decoration: none;
            margin-right: 10px;
            margin-top: 10px;
            transition: background-color 0.2s;
        }
        .btn:hover {
            background-color: var(--accent-hover);
            color: #fff;
        }
        .badge {
            display: inline-block;
            background: #334155;
            color: #38bdf8;
            padding: 2px 8px;
            border-radius: 4px;
            font-size: 0.8rem;
            font-weight: bold;
        }
        footer {
            text-align: center;
            color: var(--text-muted);
            margin-top: 50px;
            font-size: 0.9rem;
        }
        footer a {
            color: var(--accent);
            text-decoration: none;
        }
    </style>
</head>
<body>
    <div class="container">
        <header>
            <h1>VuIO Media Server</h1>
            <p class="subtitle">Official Linux Package Repositories & Direct Downloads</p>
        </header>

        <div class="card">
            <h2>Ubuntu / Debian (APT Repository) <span class="badge">Debian, Ubuntu, Mint, Pop!_OS</span></h2>
            <p>Add the official APT repository to get automatic updates:</p>
            <pre>echo "deb [trusted=yes] https://vuiodev.github.io/vuio/apt stable main" | sudo tee /etc/apt/sources.list.d/vuio.list
sudo apt update
sudo apt install vuio</pre>
            <p>Or download direct <code>.deb</code> packages:</p>
            <a href="packages/vuio_latest_amd64.deb" class="btn">Download .deb (x86_64)</a>
            <a href="packages/vuio_latest_arm64.deb" class="btn">Download .deb (ARM64)</a>
        </div>

        <div class="card">
            <h2>Fedora / RHEL / CentOS (RPM Repository) <span class="badge">Fedora, RHEL, CentOS, Rocky, Alma</span></h2>
            <p>Add the official DNF / YUM repository:</p>
            <pre>sudo dnf config-manager --add-repo https://vuiodev.github.io/vuio/rpm/vuio.repo
sudo dnf install vuio</pre>
            <p>Or download direct <code>.rpm</code> packages:</p>
            <a href="packages/vuio_latest_x86_64.rpm" class="btn">Download .rpm (x86_64)</a>
            <a href="packages/vuio_latest_aarch64.rpm" class="btn">Download .rpm (ARM64)</a>
        </div>

        <div class="card">
            <h2>Arch Linux (Pacman Repository) <span class="badge">Arch Linux / Manjaro</span></h2>
            <p>Add the official Arch Linux repository to <code>/etc/pacman.conf</code>:</p>
            <pre>[vuio]
SigLevel = Optional TrustAll
Server = https://vuiodev.github.io/vuio/arch/os/$arch

# Then update pacman and install:
sudo pacman -Sy vuio</pre>
            <p>Or download direct <code>.pkg.tar.zst</code> packages:</p>
            <a href="packages/vuio_latest_x86_64.pkg.tar.zst" class="btn">Download .pkg.tar.zst (x86_64)</a>
            <a href="packages/vuio_latest_aarch64.pkg.tar.zst" class="btn">Download .pkg.tar.zst (ARM64)</a>
        </div>

        <div class="card">
            <h2>Alpine Linux (APK Repository) <span class="badge">Alpine Linux / APK</span></h2>
            <p>Add the official Alpine APK repository to <code>/etc/apk/repositories</code>:</p>
            <pre>echo "https://vuiodev.github.io/vuio/alpine/stable/main" | sudo tee -a /etc/apk/repositories
sudo apk update
sudo apk add --allow-untrusted vuio</pre>
            <p>Or download direct <code>.apk</code> packages:</p>
            <a href="packages/vuio_latest_x86_64.apk" class="btn">Download .apk (x86_64)</a>
            <a href="packages/vuio_latest_aarch64.apk" class="btn">Download .apk (ARM64)</a>
        </div>

        <div class="card">
            <h2>Docker Multi-Architecture Image <span class="badge">Docker / Podman</span></h2>
            <p>Run the official container from GitHub Container Registry (supports x86_64 and ARM64):</p>
            <pre>docker run -d \
  --name vuio-server \
  --restart unless-stopped \
  --network host \
  -v /path/to/media:/media:ro \
  -v ./vuio-config:/config \
  ghcr.io/vuiodev/vuio:latest</pre>
        </div>

        <div class="card">
            <h2>Homebrew (macOS & Linux) <span class="badge">Brew</span></h2>
            <pre>brew tap vuiodev/vuio
brew install vuio</pre>
        </div>

        <footer>
            <p>VuIO Media Server &bull; <a href="https://github.com/vuiodev/vuio">GitHub Repository</a> &bull; <a href="https://github.com/vuiodev/vuio/blob/main/install.md">Detailed Installation Guide</a></p>
        </footer>
    </div>
</body>
</html>
EOF

echo -e "${GREEN}✓ Package repository structure generated successfully!${NC}"
