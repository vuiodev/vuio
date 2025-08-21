# VuIO Server - Windows Setup and Troubleshooting Guide

## Installation

### Option 1: MSI Installer (Recommended)

1. Download the latest `vuio-windows-x64.msi` from the releases page
2. Right-click the MSI file and select "Run as administrator" (if prompted)
3. Follow the installation wizard:
   - Accept the license agreement
   - Choose installation directory (default: `C:\Program Files\VuIO`)
   - Select components to install
   - Click "Install"
4. The installer will automatically:
   - Create Windows Firewall rules for VuIO
   - Add VuIO to the Start Menu
   - Create a desktop shortcut (if selected)

### Option 2: Portable Installation

1. Download `vuio-windows-x64.zip` from the releases page
2. Extract to your preferred directory (e.g., `C:\VuIO`)
3. Run `vuio.exe` from the extracted folder

## Configuration

### Configuration File Location

VuIO stores its configuration in the following location:
```
%APPDATA%\VuIO\config.toml
```

For example: `C:\Users\YourUsername\AppData\Roaming\VuIO\config.toml`

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
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png", "gif"]

[[media.directories]]
path = "C:\\Users\\%USERNAME%\\Videos"
recursive = true

[[media.directories]]
path = "C:\\Users\\%USERNAME%\\Music"
recursive = true

[[media.directories]]
path = "C:\\Users\\%USERNAME%\\Pictures"
recursive = true

[database]
path = "%APPDATA%\\VuIO\\media.db"
vacuum_on_startup = false
backup_enabled = true
```

### Customizing Media Directories

To add or modify monitored directories, edit the `[[media.directories]]` sections:

```toml
[[media.directories]]
path = "D:\\Movies"
recursive = true
extensions = ["mp4", "mkv", "avi"]
exclude_patterns = [".*", "Thumbs.db"]

[[media.directories]]
path = "E:\\Music"
recursive = true
extensions = ["mp3", "flac", "wav"]
```

## Windows Firewall Configuration

### Automatic Configuration (MSI Installer)

The MSI installer automatically creates the following firewall rules:
- **VuIO HTTP Server**: Allows inbound TCP connections on port 8080
- **VuIO SSDP Discovery**: Allows inbound/outbound UDP connections on port 1900

### Manual Firewall Configuration

If you're using the portable version or need to configure manually:

1. Open Windows Defender Firewall with Advanced Security:
   - Press `Win + R`, type `wf.msc`, press Enter
   
2. Create Inbound Rules:
   - Click "Inbound Rules" → "New Rule"
   - Select "Program" → Browse to `vuio.exe`
   - Allow the connection
   - Apply to all profiles (Domain, Private, Public)
   - Name: "VuIO Server"

3. Create Outbound Rules (for SSDP):
   - Click "Outbound Rules" → "New Rule"
   - Select "Port" → UDP → Specific ports: 1900
   - Allow the connection
   - Apply to all profiles
   - Name: "VuIO SSDP"

### Windows Firewall via Command Line

Run Command Prompt as Administrator and execute:

```cmd
netsh advfirewall firewall add rule name="VuIO HTTP" dir=in action=allow protocol=TCP localport=8080
netsh advfirewall firewall add rule name="VuIO SSDP" dir=in action=allow protocol=UDP localport=1900
netsh advfirewall firewall add rule name="VuIO SSDP Out" dir=out action=allow protocol=UDP localport=1900
```

## Troubleshooting

### Common Issues

#### 1. "Port 1900 is already in use"

**Symptoms:**
- VuIO fails to start with port binding error
- SSDP discovery not working

**Solutions:**
1. **Check for conflicting services:**
   ```cmd
   netstat -an | findstr :1900
   ```
   
2. **Stop Windows Media Player Network Sharing Service:**
   ```cmd
   net stop "Windows Media Player Network Sharing Service"
   sc config "WMPNetworkSvc" start= disabled
   ```

3. **Use alternative SSDP port:**
   Edit `config.toml`:
   ```toml
   [network]
   ssdp_port = 8081
   ```

#### 2. "Access Denied" or "Administrator Privileges Required"

**Symptoms:**
- Cannot bind to port 1900
- Permission errors in logs

**Solutions:**
1. **Run as Administrator:**
   - Right-click `vuio.exe` → "Run as administrator"
   
2. **Use non-privileged ports:**
   Edit `config.toml`:
   ```toml
   [server]
   port = 8080
   
   [network]
   ssdp_port = 8081
   ```

3. **Grant network permissions:**
   ```cmd
   netsh http add urlacl url=http://+:8080/ user=Everyone
   ```

#### 3. DLNA Clients Cannot Discover Server

**Symptoms:**
- Server starts successfully but clients can't find it
- No devices appear in DLNA client applications

**Diagnostics:**
1. **Check network connectivity:**
   ```cmd
   ipconfig /all
   ping 239.255.255.250
   ```

2. **Verify multicast is working:**
   ```cmd
   netsh interface ipv4 show joins
   ```

3. **Test SSDP manually:**
   ```cmd
   telnet 239.255.255.250 1900
   ```

**Solutions:**
1. **Disable network isolation:**
   - Open Settings → Network & Internet → Status
   - Click "Network profile" → Set to "Private"

2. **Check Windows Media Player Network Sharing:**
   - Control Panel → Programs → Turn Windows features on/off
   - Ensure "Media Features" → "Windows Media Player" is enabled

3. **Reset network stack:**
   ```cmd
   netsh winsock reset
   netsh int ip reset
   ipconfig /flushdns
   ```
   Restart computer after running these commands.

#### 4. Media Files Not Appearing

**Symptoms:**
- Server starts but no media files are listed
- Partial file listings

**Solutions:**
1. **Check file permissions:**
   - Ensure VuIO has read access to media directories
   - Right-click folder → Properties → Security → Add "Everyone" with Read permissions

2. **Verify file extensions:**
   Check `config.toml` supported extensions match your files:
   ```toml
   [media]
   supported_extensions = ["mp4", "mkv", "avi", "wmv", "mp3", "flac", "wav", "wma"]
   ```

3. **Check database location:**
   Ensure database directory is writable:
   ```toml
   [database]
   path = "%APPDATA%\\VuIO\\media.db"
   ```

4. **Force rescan:**
   Delete the database file and restart VuIO:
   ```cmd
   del "%APPDATA%\VuIO\media.db"
   ```

#### 5. High CPU Usage During Scanning

**Symptoms:**
- VuIO uses excessive CPU during startup
- System becomes unresponsive

**Solutions:**
1. **Disable recursive scanning for large directories:**
   ```toml
   [[media.directories]]
   path = "D:\\LargeMediaFolder"
   recursive = false
   ```

2. **Add exclude patterns:**
   ```toml
   [[media.directories]]
   path = "C:\\Users\\%USERNAME%\\Videos"
   recursive = true
   exclude_patterns = [".*", "Thumbs.db", "*.tmp", "System Volume Information"]
   ```

3. **Disable file watching temporarily:**
   ```toml
   [media]
   watch_for_changes = false
   ```

### Network Troubleshooting

#### Check Network Interfaces

```cmd
# List all network interfaces
ipconfig /all

# Show routing table
route print

# Test multicast connectivity
ping -t 239.255.255.250
```

#### SSDP Debugging

1. **Enable debug logging:**
   Set environment variable:
   ```cmd
   set RUST_LOG=debug
   vuio.exe
   ```

2. **Monitor SSDP traffic:**
   Use Wireshark or built-in tools:
   ```cmd
   netsh trace start capture=yes provider=Microsoft-Windows-TCPIP
   # Run VuIO, then stop trace
   netsh trace stop
   ```

#### UNC Path Issues

For network drives and UNC paths:

1. **Map network drive:**
   ```cmd
   net use Z: \\server\share /persistent:yes
   ```

2. **Use mapped drive in config:**
   ```toml
   [[media.directories]]
   path = "Z:\\Movies"
   recursive = true
   ```

### Performance Optimization

#### Database Optimization

1. **Enable database vacuum:**
   ```toml
   [database]
   vacuum_on_startup = true
   ```

2. **Move database to SSD:**
   ```toml
   [database]
   path = "C:\\VuIO\\media.db"
   ```

#### Network Optimization

1. **Adjust announce interval:**
   ```toml
   [network]
   announce_interval_seconds = 60  # Reduce network traffic
   ```

2. **Limit network interfaces:**
   ```toml
   [network]
   interface_selection = "192.168.1.100"  # Specific interface
   ```

## Advanced Configuration

### Running as Windows Service

1. **Install NSSM (Non-Sucking Service Manager):**
   Download from https://nssm.cc/

2. **Create service:**
   ```cmd
   nssm install VuIO "C:\Program Files\VuIO\vuio.exe"
   nssm set VuIO AppDirectory "C:\Program Files\VuIO"
   nssm set VuIO DisplayName "VuIO Media Server"
   nssm set VuIO Description "Cross-platform DLNA media server"
   nssm set VuIO Start SERVICE_AUTO_START
   ```

3. **Start service:**
   ```cmd
   net start VuIO
   ```

### Registry Configuration

For system-wide configuration, create registry entries:

```reg
Windows Registry Editor Version 5.00

[HKEY_LOCAL_MACHINE\SOFTWARE\VuIO]
"ConfigPath"="C:\\ProgramData\\VuIO\\config.toml"
"DatabasePath"="C:\\ProgramData\\VuIO\\media.db"
"LogLevel"="info"
```

### Group Policy Configuration

For enterprise deployments, configure via Group Policy:

1. Create administrative template (ADMX file)
2. Configure default media directories
3. Set firewall rules automatically
4. Deploy configuration via GPO

## Logging and Diagnostics

### Log File Locations

- **Application logs:** `%APPDATA%\VuIO\logs\vuio.log`
- **Error logs:** `%APPDATA%\VuIO\logs\error.log`
- **Debug logs:** `%APPDATA%\VuIO\logs\debug.log`

### Enable Debug Logging

```cmd
set RUST_LOG=vuio=debug
vuio.exe
```

### System Information Collection

For support requests, collect system information:

```cmd
# System info
systeminfo > system_info.txt

# Network configuration
ipconfig /all > network_config.txt

# Firewall rules
netsh advfirewall firewall show rule name=all > firewall_rules.txt

# Running services
sc query type= service state= all > services.txt

# VuIO logs
copy "%APPDATA%\VuIO\logs\*" support_logs\
```

## Getting Help

If you continue to experience issues:

1. **Check the logs** in `%APPDATA%\VuIO\logs\`
2. **Search existing issues** on GitHub
3. **Create a new issue** with:
   - Windows version (`winver`)
   - VuIO version
   - Configuration file (remove sensitive paths)
   - Relevant log entries
   - Network configuration (`ipconfig /all`)

## Security Considerations

### Windows Defender

VuIO may be flagged by Windows Defender. To whitelist:

1. Open Windows Security
2. Go to Virus & threat protection
3. Click "Manage settings" under Virus & threat protection settings
4. Click "Add or remove exclusions"
5. Add folder exclusion for VuIO installation directory

### Network Security

1. **Use private networks only** - avoid running on public WiFi
2. **Configure firewall rules** to limit access to trusted networks
3. **Regular updates** - keep VuIO updated for security patches
4. **Monitor access logs** for unauthorized access attempts

### File System Security

1. **Limit media directory permissions** - only grant read access
2. **Avoid system directories** - don't monitor Windows system folders
3. **Use dedicated media user** - create a limited user account for VuIO service