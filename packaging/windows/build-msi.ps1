#!/usr/bin/env pwsh

# Build MSI installer for VuIO on Windows
# Requires WiX Toolset v3 or v4 to be installed

param(
    [string]$BinaryPath = "..\..\target\x86_64-pc-windows-msvc\release\vuio.exe",
    [string]$OutputDir = "..\..\builds",
    [string]$Version = "0.1.0",
    [switch]$Help
)

$ErrorActionPreference = "Stop"

function Show-Help {
    Write-Host "--- MSI Installer Build Script ---" -ForegroundColor Green
    Write-Host ""
    Write-Host "Usage: .\build-msi.ps1 [OPTIONS]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -BinaryPath <string>  Path to the compiled vuio.exe (default: target\x86_64-pc-windows-msvc\release\vuio.exe)"
    Write-Host "  -OutputDir <string>   Output directory for MSI file (default: builds)"
    Write-Host "  -Version <string>     Version number for the installer (default: 0.1.0)"
    Write-Host "  -Help                Show this help message"
    Write-Host ""
    Write-Host "Prerequisites:"
    Write-Host "  - WiX Toolset v3 or v4 must be installed"
    Write-Host "  - Visual Studio Build Tools (for candle.exe and light.exe)"
    Write-Host ""
}

if ($Help) {
    Show-Help
    exit 0
}

# Check for WiX tools
function Test-WixTools {
    $wixTools = @("candle.exe", "light.exe")
    $missing = @()
    
    foreach ($tool in $wixTools) {
        try {
            Get-Command $tool -ErrorAction Stop | Out-Null
            Write-Host "✓ $tool found" -ForegroundColor Green
        } catch {
            Write-Host "✗ $tool not found" -ForegroundColor Red
            $missing += $tool
        }
    }
    
    if ($missing.Count -gt 0) {
        Write-Host ""
        Write-Host "Missing WiX tools: $($missing -join ', ')" -ForegroundColor Red
        Write-Host "Please install WiX Toolset from https://wixtoolset.org/" -ForegroundColor Yellow
        Write-Host "Or install via Chocolatey: choco install wixtoolset" -ForegroundColor Yellow
        return $false
    }
    
    return $true
}

# Check prerequisites
Write-Host "--- Checking Prerequisites ---" -ForegroundColor Yellow

if (-not (Test-WixTools)) {
    exit 1
}

if (-not (Test-Path $BinaryPath)) {
    Write-Error "Binary not found at: $BinaryPath"
    Write-Host "Please build the project first or specify correct path with -BinaryPath" -ForegroundColor Yellow
    exit 1
}

Write-Host "✓ Binary found at: $BinaryPath" -ForegroundColor Green

# Create necessary directories and files
Write-Host ""
Write-Host "--- Preparing Build Environment ---" -ForegroundColor Yellow

$tempDir = "temp"
if (Test-Path $tempDir) {
    Remove-Item $tempDir -Recurse -Force
}
New-Item -ItemType Directory -Path $tempDir -Force | Out-Null

# Create config directory and default config
$configDir = "$tempDir\config"
New-Item -ItemType Directory -Path $configDir -Force | Out-Null

# Create default configuration file
$defaultConfig = @"
# VuIO Server Configuration
# This is the default configuration file for VuIO

[server]
port = 8080
interface = "0.0.0.0"
name = "VuIO Server"
uuid = "12345678-1234-1234-1234-123456789012"

[network]
ssdp_port = 1900
interface_selection = "auto"
multicast_ttl = 4
announce_interval_seconds = 30

[media]
scan_on_startup = true
watch_for_changes = true
supported_extensions = ["mp4", "mkv", "avi", "mp3", "flac", "wav", "jpg", "png", "gif"]

[[media.directories]]
path = "C:\\Users\\Public\\Videos"
recursive = true

[[media.directories]]
path = "C:\\Users\\Public\\Music"
recursive = true

[[media.directories]]
path = "C:\\Users\\Public\\Pictures"
recursive = true

[database]
vacuum_on_startup = false
backup_enabled = true
"@

$defaultConfig | Out-File -FilePath "$configDir\default.toml" -Encoding UTF8

# Copy binary to temp directory
Copy-Item $BinaryPath "$tempDir\vuio.exe"

# Create a simple RTF license file
$licenseRtf = @"
{\rtf1\ansi\deff0 {\fonttbl {\f0 Times New Roman;}}
\f0\fs24 VuIO Server License Agreement\par
\par
This software is provided as-is under the MIT License.\par
\par
Permission is hereby granted, free of charge, to any person obtaining a copy of this software and associated documentation files (the "Software"), to deal in the Software without restriction, including without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the following conditions:\par
\par
The above copyright notice and this permission notice shall be included in all copies or substantial portions of the Software.\par
\par
THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.\par
}
"@

$licenseRtf | Out-File -FilePath "$tempDir\LICENSE.rtf" -Encoding ASCII

Write-Host "✓ Build environment prepared" -ForegroundColor Green

# Build MSI
Write-Host ""
Write-Host "--- Building MSI Installer ---" -ForegroundColor Yellow

$wixObjFile = "vuio.wixobj"
$msiFile = "vuio-$Version-x64.msi"

try {
    # Compile WiX source
    Write-Host "Compiling WiX source..."
    $candleArgs = @(
        "vuio.wxs",
        "-dSourceDir=$tempDir",
        "-dVersion=$Version",
        "-out", $wixObjFile
    )
    
    & candle.exe @candleArgs
    if ($LASTEXITCODE -ne 0) {
        throw "candle.exe failed with exit code $LASTEXITCODE"
    }
    
    # Link MSI
    Write-Host "Linking MSI installer..."
    $lightArgs = @(
        $wixObjFile,
        "-ext", "WixUIExtension",
        "-ext", "WixFirewallExtension",
        "-out", $msiFile
    )
    
    & light.exe @lightArgs
    if ($LASTEXITCODE -ne 0) {
        throw "light.exe failed with exit code $LASTEXITCODE"
    }
    
    # Move MSI to output directory
    if (-not (Test-Path $OutputDir)) {
        New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
    }
    
    $finalMsiPath = "$OutputDir\$msiFile"
    Move-Item $msiFile $finalMsiPath -Force
    
    Write-Host "✓ MSI installer created successfully: $finalMsiPath" -ForegroundColor Green
    
    # Show file info
    $msiInfo = Get-Item $finalMsiPath
    Write-Host ""
    Write-Host "Installer Details:" -ForegroundColor Cyan
    Write-Host "  File: $($msiInfo.Name)"
    Write-Host "  Size: $([math]::Round($msiInfo.Length / 1MB, 2)) MB"
    Write-Host "  Path: $($msiInfo.FullName)"
    
} catch {
    Write-Error "Failed to build MSI installer: $_"
    exit 1
} finally {
    # Cleanup
    Write-Host ""
    Write-Host "--- Cleaning Up ---" -ForegroundColor Yellow
    
    if (Test-Path $wixObjFile) {
        Remove-Item $wixObjFile -Force
    }
    
    if (Test-Path $tempDir) {
        Remove-Item $tempDir -Recurse -Force
    }
    
    Write-Host "✓ Cleanup completed" -ForegroundColor Green
}

Write-Host ""
Write-Host "--- MSI Build Complete ---" -ForegroundColor Green