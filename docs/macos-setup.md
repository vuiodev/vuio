# VuIO Server - macOS Setup and Configuration Guide

## Installation

### Option 1: Homebrew (Recommended)

1. **Install Homebrew** (if not already installed):
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **Add VuIO tap and install**:
   ```bash
   brew tap vuio/tap
   brew install vuio
   ```

3. **Start the service**:
   ```bash
   brew services start vuio
   ```

### Option 2: PKG Installer

1. Download the latest `vuio-macos-universal.pkg` from the releases page
2. Double-click the PKG file to start the installer
3. Follow the installation wizard:
   - Enter administrator password when prompted
   - Choose installation location (default: `/Applications/VuIO`)
   - Complete the installation
4. The installer will:
   - Install VuIO to `/Applications/VuIO/`
   - Create a launch daemon for automatic startup
   - Set up proper permissions and security attributes

### Option 3: Manual Installation

1. Download `vuio-macos-universal.tar.gz` from the releases page
2. Extract to your preferred location:
   ```bash
   tar -xzf vuio-macos-universal.tar.gz
   sudo mv vuio /usr/local/bin/
   sudo chmod +x /usr/local/bin/vuio
   ```

## Configuration

### Configuration File Location

VuIO stores its configuration in the following location:
```
~/Library/Application Support/VuIO/config.toml
```

### Default Configuration

On first run, VuIO creates a default configuration file:

```toml
[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "auto-generated-uuid"

[network]
ssdp_port = 1900
interface_selection = "Auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "mov", "m4v", "mp3", "m4a", "flac", "wav", "aiff", "jpg", "jpeg", "png", "gif", "heic"]

[[media.directories]]
path = "~/Movies"
recursive = true

[[media.directories]]
path = "~/Music"
recursive = true

[[media.directories]]
path = "~/Pictures"
recursive = true

[database]
path = "~/Library/Application Support/VuIO/media.db"
vacuum_on_startup = false
backup_enabled = true
```

### Customizing Media Directories

To add or modify monitored directories, edit the `[[media.directories]]` sections:

```toml
[[media.directories]]
path = "/Volumes/External Drive/Movies"
recursive = true
extensions = ["mp4", "mkv", "mov", "m4v"]
exclude_patterns = [".*", ".DS_Store", "Thumbs.db"]

[[media.directories]]
path = "~/Downloads/Media"
recursive = false
extensions = ["mp4", "mp3"]
```

## macOS Permissions and Security

### Privacy Permissions

VuIO requires several permissions on macOS:

#### Full Disk Access (macOS 10.14+)

1. Open **System Preferences** → **Security & Privacy** → **Privacy**
2. Click the lock icon and enter your password
3. Select **Full Disk Access** from the left sidebar
4. Click the **+** button and add VuIO:
   - If installed via Homebrew: `/usr/local/bin/vuio`
   - If installed via PKG: `/Applications/VuIO/vuio`
   - If running from custom location: navigate to your VuIO binary

#### Network Access

1. When first starting VuIO, macOS will prompt for network access
2. Click **Allow** to permit incoming network connections
3. If you missed the prompt, go to **System Preferences** → **Security & Privacy** → **Firewall** → **Firewall Options**
4. Ensure VuIO is listed and set to **Allow incoming connections**

#### File System Access

For accessing external drives and network volumes:

1. **System Preferences** → **Security & Privacy** → **Privacy**
2. Select **Files and Folders** from the left sidebar
3. Ensure VuIO has access to:
   - Downloads Folder
   - Documents Folder
   - Desktop Folder
   - Removable Volumes (for external drives)
   - Network Volumes (for network shares)

### Gatekeeper and Code Signing

If you encounter "VuIO cannot be opened because it is from an unidentified developer":

#### Option 1: Allow in Security Preferences
1. Try to run VuIO
2. Go to **System Preferences** → **Security & Privacy** → **General**
3. Click **Allow Anyway** next to the VuIO message

#### Option 2: Override Gatekeeper (Advanced)
```bash
sudo xattr -rd com.apple.quarantine /path/to/vuio
```

#### Option 3: Disable Gatekeeper Temporarily (Not Recommended)
```bash
sudo spctl --master-disable
# Run VuIO, then re-enable:
sudo spctl --master-enable
```

## Running VuIO

### Manual Execution

```bash
# Run in foreground
vuio

# Run in background
nohup vuio > ~/Library/Logs/vuio.log 2>&1 &

# Run with debug logging
RUST_LOG=debug vuio
```

### Launch Daemon (Automatic Startup)

Create a launch daemon for automatic startup:

1. **Create launch daemon plist**:
   ```bash
   sudo nano /Library/LaunchDaemons/com.vuio.server.plist
   ```

2. **Add the following content**:
   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.vuio.server</string>
       <key>ProgramArguments</key>
       <array>
           <string>/usr/local/bin/vuio</string>
       </array>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
       <key>StandardOutPath</key>
       <string>/var/log/vuio.log</string>
       <key>StandardErrorPath</key>
       <string>/var/log/vuio.error.log</string>
       <key>WorkingDirectory</key>
       <string>/usr/local/bin</string>
       <key>UserName</key>
       <string>_vuio</string>
   </dict>
   </plist>
   ```

3. **Set permissions and load**:
   ```bash
   sudo chown root:wheel /Library/LaunchDaemons/com.vuio.server.plist
   sudo chmod 644 /Library/LaunchDaemons/com.vuio.server.plist
   sudo launchctl load /Library/LaunchDaemons/com.vuio.server.plist
   ```

### User Launch Agent (User-specific)

For user-specific startup (recommended for desktop use):

1. **Create user launch agent**:
   ```bash
   mkdir -p ~/Library/LaunchAgents
   nano ~/Library/LaunchAgents/com.vuio.server.plist
   ```

2. **Add the following content**:
   ```xml
   <?xml version="1.0" encoding="UTF-8"?>
   <!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
   <plist version="1.0">
   <dict>
       <key>Label</key>
       <string>com.vuio.server</string>
       <key>ProgramArguments</key>
       <array>
           <string>/usr/local/bin/vuio</string>
       </array>
       <key>RunAtLoad</key>
       <true/>
       <key>KeepAlive</key>
       <true/>
       <key>StandardOutPath</key>
       <string>/Users/$(whoami)/Library/Logs/vuio.log</string>
       <key>StandardErrorPath</key>
       <string>/Users/$(whoami)/Library/Logs/vuio.error.log</string>
   </dict>
   </plist>
   ```

3. **Load the launch agent**:
   ```bash
   launchctl load ~/Library/LaunchAgents/com.vuio.server.plist
   ```

## Troubleshooting

### Common Issues

#### 1. "Permission denied" or "Operation not permitted"

**Symptoms:**
- Cannot access media directories
- Database creation fails
- Network binding errors

**Solutions:**
1. **Grant Full Disk Access** (see Permissions section above)
2. **Check file permissions**:
   ```bash
   ls -la ~/Library/Application\ Support/VuIO/
   chmod 755 ~/Library/Application\ Support/VuIO/
   ```
3. **Run with proper user permissions**:
   ```bash
   sudo chown -R $(whoami):staff ~/Library/Application\ Support/VuIO/
   ```

#### 2. Network Discovery Issues

**Symptoms:**
- DLNA clients cannot find the server
- SSDP multicast not working

**Diagnostics:**
```bash
# Check network interfaces
ifconfig

# Test multicast connectivity
ping 239.255.255.250

# Check if port is available
lsof -i :1900
netstat -an | grep 1900
```

**Solutions:**
1. **Check macOS Firewall**:
   - System Preferences → Security & Privacy → Firewall
   - Ensure firewall is either disabled or VuIO is allowed

2. **Verify network interface selection**:
   ```toml
   [network]
   interface_selection = "en0"  # Specify your primary interface
   ```

3. **Check for conflicting services**:
   ```bash
   sudo lsof -i :1900
   # If iTunes or other services are using port 1900:
   sudo launchctl unload /System/Library/LaunchDaemons/com.apple.AirPlayXPCHelper.plist
   ```

#### 3. External Drive Access Issues

**Symptoms:**
- Cannot scan external drives
- Permission errors for mounted volumes

**Solutions:**
1. **Grant Removable Volumes access** (see Permissions section)
2. **Check mount permissions**:
   ```bash
   ls -la /Volumes/
   # Ensure your user has read access to the external drive
   ```
3. **Use full paths in configuration**:
   ```toml
   [[media.directories]]
   path = "/Volumes/My External Drive/Movies"
   recursive = true
   ```

#### 4. High CPU Usage on Apple Silicon Macs

**Symptoms:**
- Excessive CPU usage during file scanning
- System becomes unresponsive

**Solutions:**
1. **Use native Apple Silicon binary** if available
2. **Limit concurrent operations**:
   ```toml
   [media]
   scan_on_startup = false  # Disable initial scan
   watch_for_changes = true  # Use file watching instead
   ```
3. **Exclude system directories**:
   ```toml
   [[media.directories]]
   path = "~/Movies"
   recursive = true
   exclude_patterns = [".*", ".DS_Store", ".Spotlight-V100", ".Trashes", ".fseventsd"]
   ```

#### 5. Rosetta 2 Issues (Apple Silicon)

If running x86_64 binary on Apple Silicon:

1. **Install Rosetta 2**:
   ```bash
   softwareupdate --install-rosetta
   ```

2. **Verify architecture**:
   ```bash
   file /usr/local/bin/vuio
   # Should show: Mach-O 64-bit executable arm64 (for native)
   # Or: Mach-O 64-bit executable x86_64 (for Intel/Rosetta)
   ```

### Network Troubleshooting

#### Interface Detection

```bash
# List all network interfaces
networksetup -listallhardwareports

# Get interface details
ifconfig en0

# Check routing table
netstat -rn

# Test multicast
ping -c 3 239.255.255.250
```

#### Firewall Configuration

```bash
# Check firewall status
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --getglobalstate

# Add VuIO to firewall exceptions
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --add /usr/local/bin/vuio
sudo /usr/libexec/ApplicationFirewall/socketfilterfw --unblockapp /usr/local/bin/vuio
```

#### Port Binding Issues

```bash
# Check what's using port 1900
sudo lsof -i :1900

# Kill conflicting processes (be careful!)
sudo kill -9 <PID>

# Use alternative port
# Edit config.toml:
[network]
ssdp_port = 8081
```

### File System Issues

#### Case Sensitivity

macOS uses case-insensitive file systems by default, but this can cause issues:

```bash
# Check file system case sensitivity
diskutil info / | grep "Case-sensitive"

# If you need case-sensitive behavior, create a case-sensitive volume:
hdiutil create -size 100g -fs "Case-sensitive HFS+" -volname "MediaDrive" ~/MediaDrive.dmg
hdiutil attach ~/MediaDrive.dmg
```

#### File Permissions

```bash
# Fix permissions for VuIO directories
sudo chown -R $(whoami):staff ~/Library/Application\ Support/VuIO/
chmod -R 755 ~/Library/Application\ Support/VuIO/

# For media directories
chmod -R 755 ~/Movies ~/Music ~/Pictures
```

#### Extended Attributes

macOS adds extended attributes that can interfere:

```bash
# Remove quarantine attributes
xattr -d com.apple.quarantine /path/to/media/files/*

# Remove all extended attributes (use with caution)
xattr -c /path/to/media/files/*
```

## Advanced Configuration

### Network Interface Selection

For systems with multiple network interfaces:

```toml
[network]
interface_selection = "en0"  # Ethernet
# or
interface_selection = "en1"  # WiFi
# or
interface_selection = "Auto"  # Automatic selection
```

### Performance Tuning

#### Database Optimization

```toml
[database]
path = "~/Library/Application Support/VuIO/media.db"
vacuum_on_startup = true
backup_enabled = true
```

#### File Watching Optimization

```toml
[media]
watch_for_changes = true
scan_on_startup = false  # Rely on database and file watching

[[media.directories]]
path = "~/Movies"
recursive = true
exclude_patterns = [
    ".*",           # Hidden files
    ".DS_Store",    # macOS metadata
    ".Spotlight-V100", # Spotlight index
    ".Trashes",     # Trash folders
    ".fseventsd",   # File system events
    "*.tmp",        # Temporary files
    "*.part"        # Partial downloads
]
```

### Security Hardening

#### Create Dedicated User

```bash
# Create dedicated user for VuIO
sudo dscl . -create /Users/_vuio
sudo dscl . -create /Users/_vuio UserShell /usr/bin/false
sudo dscl . -create /Users/_vuio RealName "VuIO Server"
sudo dscl . -create /Users/_vuio UniqueID 501
sudo dscl . -create /Users/_vuio PrimaryGroupID 20
sudo dscl . -create /Users/_vuio NFSHomeDirectory /var/empty
```

#### Sandboxing

For enhanced security, run VuIO in a sandbox:

```bash
# Create sandbox profile
cat > ~/vuio.sb << 'EOF'
(version 1)
(deny default)
(allow process-exec (literal "/usr/local/bin/vuio"))
(allow file-read* (subpath "/Users/$(whoami)/Movies"))
(allow file-read* (subpath "/Users/$(whoami)/Music"))
(allow file-read* (subpath "/Users/$(whoami)/Pictures"))
(allow file-write* (subpath "/Users/$(whoami)/Library/Application Support/VuIO"))
(allow network-inbound (local tcp "*:8080"))
(allow network-outbound (remote udp "*:1900"))
EOF

# Run with sandbox
sandbox-exec -f ~/vuio.sb vuio
```

## Logging and Diagnostics

### Log File Locations

- **Application logs:** `~/Library/Logs/vuio.log`
- **System logs:** `/var/log/vuio.log` (if running as daemon)
- **Launch daemon logs:** Check with `sudo launchctl list | grep vuio`

### Enable Debug Logging

```bash
# Set environment variable
export RUST_LOG=vuio=debug
vuio

# Or for launch daemon, edit the plist:
<key>EnvironmentVariables</key>
<dict>
    <key>RUST_LOG</key>
    <string>vuio=debug</string>
</dict>
```

### System Information Collection

For support requests:

```bash
# System information
system_profiler SPSoftwareDataType > system_info.txt
system_profiler SPNetworkDataType > network_info.txt

# VuIO configuration
cp ~/Library/Application\ Support/VuIO/config.toml config_backup.toml

# Network configuration
ifconfig > network_config.txt
netstat -rn > routing_table.txt

# Permissions check
ls -la ~/Library/Application\ Support/VuIO/ > permissions.txt
```

## Uninstallation

### Homebrew Installation

```bash
brew services stop vuio
brew uninstall vuio
brew untap vuio/tap
```

### PKG Installation

```bash
# Stop and remove launch daemon
sudo launchctl unload /Library/LaunchDaemons/com.vuio.server.plist
sudo rm /Library/LaunchDaemons/com.vuio.server.plist

# Remove application
sudo rm -rf /Applications/VuIO

# Remove user data (optional)
rm -rf ~/Library/Application\ Support/VuIO
rm -rf ~/Library/Logs/vuio*
```

### Manual Installation

```bash
# Stop any running instances
pkill vuio

# Remove binary
sudo rm /usr/local/bin/vuio

# Remove launch agent
launchctl unload ~/Library/LaunchAgents/com.vuio.server.plist
rm ~/Library/LaunchAgents/com.vuio.server.plist

# Remove user data
rm -rf ~/Library/Application\ Support/VuIO
```

## Getting Help

If you continue to experience issues:

1. **Check the logs** in `~/Library/Logs/` or `~/Library/Application Support/VuIO/logs/`
2. **Verify permissions** in System Preferences → Security & Privacy
3. **Test network connectivity** with the diagnostic commands above
4. **Search existing issues** on GitHub
5. **Create a new issue** with:
   - macOS version (`sw_vers`)
   - VuIO version
   - Configuration file (remove sensitive paths)
   - Relevant log entries
   - Network configuration (`ifconfig`)
   - Permission settings screenshots

## macOS-Specific Tips

### Optimizing for Different macOS Versions

#### macOS Big Sur (11.0) and later
- Enhanced privacy controls require explicit permission grants
- Use the new privacy APIs for better integration

#### macOS Monterey (12.0) and later
- AirPlay improvements may conflict with DLNA
- Consider disabling AirPlay if experiencing issues

#### Apple Silicon Macs
- Use native ARM64 binaries when available
- Monitor CPU usage and thermal throttling
- Consider power management settings for always-on operation

### Integration with macOS Features

#### Spotlight Integration
VuIO can integrate with Spotlight for faster media discovery:

```toml
[media]
use_spotlight_metadata = true  # Use Spotlight metadata when available
```

#### Time Machine Exclusions
Exclude VuIO data from Time Machine backups:

```bash
tmutil addexclusion ~/Library/Application\ Support/VuIO/media.db
tmutil addexclusion ~/Library/Logs/vuio.log
```