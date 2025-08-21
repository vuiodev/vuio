# VuIO Packaging

This directory contains scripts and configurations for building platform-specific packages for VuIO Server.

## Supported Package Formats

### Windows
- **MSI Installer**: Windows Installer package with service integration
- **Requirements**: WiX Toolset v3 or v4, PowerShell
- **Features**: 
  - Windows Service registration
  - Firewall rule configuration
  - Start Menu shortcuts
  - Automatic configuration file creation

### macOS
- **PKG Installer**: macOS installer package with LaunchDaemon integration
- **Requirements**: Xcode Command Line Tools (pkgbuild, productbuild)
- **Features**:
  - LaunchDaemon service configuration
  - System user creation
  - Proper permission handling
  - Signed package support

### Linux
- **DEB Package**: Debian/Ubuntu package with systemd integration
- **RPM Package**: RedHat/SUSE package with systemd integration
- **Requirements**: dpkg-deb (for DEB), rpmbuild (for RPM)
- **Features**:
  - Systemd service integration
  - User and group creation
  - SELinux/AppArmor compatibility
  - Proper file permissions

## Quick Start

### Build All Packages
```bash
# Build packages for current platform
./build-packages.sh

# Build packages with specific version
./build-packages.sh 1.0.0

# Build packages with custom output directory
./build-packages.sh 1.0.0 /path/to/output
```

### Build Platform-Specific Packages
```bash
# Build only Windows packages
./build-packages.sh --windows

# Build only macOS packages
./build-packages.sh --macos

# Build only Linux packages
./build-packages.sh --linux
```

### Build Individual Package Types
```bash
# Windows MSI
cd windows
pwsh build-msi.ps1

# macOS PKG
cd macos
./build-pkg.sh

# Linux DEB
cd linux
./build-deb.sh

# Linux RPM
cd linux
./build-rpm.sh
```

## Prerequisites

### Windows
1. **WiX Toolset**: Download from https://wixtoolset.org/
   ```powershell
   # Or install via Chocolatey
   choco install wixtoolset
   ```

2. **PowerShell**: Windows PowerShell 5.1+ or PowerShell Core 7+

3. **Visual Studio Build Tools** (optional, for MSVC targets)

### macOS
1. **Xcode Command Line Tools**:
   ```bash
   xcode-select --install
   ```

2. **Developer ID Certificate** (optional, for signed packages):
   - Obtain from Apple Developer Program
   - Install in Keychain Access

### Linux
1. **For DEB packages**:
   ```bash
   # Debian/Ubuntu
   sudo apt-get install dpkg-dev fakeroot
   
   # RHEL/CentOS/Fedora
   sudo dnf install dpkg-dev fakeroot
   ```

2. **For RPM packages**:
   ```bash
   # RHEL/CentOS/Fedora
   sudo dnf install rpm-build
   
   # Debian/Ubuntu
   sudo apt-get install rpm
   
   # SUSE
   sudo zypper install rpm-build
   ```

## Directory Structure

```
packaging/
├── README.md                 # This file
├── build-packages.sh         # Master build script
├── windows/
│   ├── build-msi.ps1        # MSI builder script
│   └── vuio.wxs         # WiX installer definition
├── macos/
│   └── build-pkg.sh         # PKG builder script
└── linux/
    ├── build-deb.sh         # DEB builder script
    └── build-rpm.sh         # RPM builder script
```

## Package Details

### Windows MSI Features
- **Service Integration**: Installs as Windows Service
- **Firewall Rules**: Automatically configures Windows Firewall
- **Start Menu**: Creates Start Menu shortcuts
- **Configuration**: Includes default configuration file
- **Uninstall**: Clean removal with service cleanup

### macOS PKG Features
- **LaunchDaemon**: System service integration
- **User Management**: Creates dedicated service user
- **Permissions**: Proper macOS permission handling
- **Configuration**: Default configuration in `/usr/local/etc/vuio/`
- **Logs**: Centralized logging in `/usr/local/var/log/vuio/`

### Linux DEB/RPM Features
- **Systemd Integration**: Native systemd service
- **User Management**: Creates `vuio` system user
- **Security**: Hardened systemd service configuration
- **Configuration**: System configuration in `/etc/vuio/`
- **Logs**: Journal integration with fallback to `/var/log/vuio/`

## Configuration Files

All packages include default configuration files with platform-appropriate defaults:

### Windows Default Paths
- **Config**: `%PROGRAMFILES%\VuIO\config\vuio.toml`
- **Logs**: `%PROGRAMFILES%\VuIO\logs\`
- **Media**: `%PUBLIC%\Videos`, `%PUBLIC%\Music`, `%PUBLIC%\Pictures`

### macOS Default Paths
- **Config**: `/usr/local/etc/vuio/vuio.toml`
- **Logs**: `/usr/local/var/log/vuio/`
- **Media**: `/Users/Shared/Movies`, `/Users/Shared/Music`, `/Users/Shared/Pictures`

### Linux Default Paths
- **Config**: `/etc/vuio/vuio.toml`
- **Logs**: `/var/log/vuio/` or `journalctl -u vuio`
- **Media**: `/home/media/Videos`, `/home/media/Music`, `/home/media/Pictures`

## Service Management

### Windows
```cmd
# Start service
net start VuIO

# Stop service
net stop VuIO

# Service status
sc query VuIO
```

### macOS
```bash
# Start service
sudo launchctl start com.vuio.server

# Stop service
sudo launchctl stop com.vuio.server

# Service status
sudo launchctl list | grep vuio
```

### Linux
```bash
# Start service
sudo systemctl start vuio

# Stop service
sudo systemctl stop vuio

# Enable auto-start
sudo systemctl enable vuio

# Service status
sudo systemctl status vuio

# View logs
journalctl -u vuio -f
```

## Troubleshooting

### Windows
- **WiX not found**: Install WiX Toolset and ensure it's in PATH
- **PowerShell execution policy**: Run `Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser`
- **Admin privileges**: Some operations may require administrator privileges

### macOS
- **Command line tools**: Install with `xcode-select --install`
- **Permissions**: Some operations require sudo privileges
- **Signing**: For distribution, sign packages with Developer ID certificate

### Linux
- **Missing tools**: Install required packages for your distribution
- **Permissions**: Use fakeroot for DEB building if not running as root
- **Dependencies**: Ensure all runtime dependencies are available

## Customization

### Modifying Default Configuration
Edit the configuration templates in each platform's build script:
- Windows: `build-msi.ps1` (search for "defaultConfig")
- macOS: `build-pkg.sh` (search for "vuio.toml")
- Linux: `build-deb.sh` and `build-rpm.sh` (search for "vuio.toml")

### Adding Custom Files
Modify the package structure in each build script to include additional files:
- Documentation
- Additional configuration files
- Helper scripts
- Icons or resources

### Changing Installation Paths
Update the installation paths in:
- Windows: `vuio.wxs` (Directory elements)
- macOS: `build-pkg.sh` (INSTALL_LOCATION variable)
- Linux: `build-deb.sh` and `build-rpm.sh` (directory creation sections)

## Security Considerations

All packages implement security best practices:
- **Dedicated Users**: Services run as dedicated system users
- **Minimal Privileges**: Services run with minimal required privileges
- **Secure Defaults**: Configuration files have appropriate permissions
- **Network Security**: Firewall rules are configured appropriately
- **Sandboxing**: Services are sandboxed where supported (systemd, macOS)

## Contributing

When adding new packaging features:
1. Test on the target platform
2. Follow platform conventions
3. Update this README
4. Ensure security best practices
5. Test installation and removal procedures