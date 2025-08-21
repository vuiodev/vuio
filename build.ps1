#!/usr/bin/env pwsh

# This script automates the cross-compilation of the Rust project for various targets.
# It builds for Linux, Windows, and macOS on both amd64 and arm64 architectures.
# PowerShell equivalent of build.sh for Windows compatibility.

param(
    [string[]]$Targets = @(),
    [string]$BuildDir = "builds",
    [switch]$Help,
    [switch]$CheckTools
)

# Exit on any error
$ErrorActionPreference = "Stop"

# --- Help Function ---
function Show-Help {
    Write-Host "--- Build Script for Cross-Platform Compilation ---" -ForegroundColor Green
    Write-Host ""
    Write-Host "Usage: .\build.ps1 [OPTIONS]"
    Write-Host ""
    Write-Host "Options:"
    Write-Host "  -Targets <array>     Specify custom targets to build (default: all supported)"
    Write-Host "  -BuildDir <string>   Output directory for binaries (default: 'builds')"
    Write-Host "  -CheckTools          Check for required build tools and exit"
    Write-Host "  -Help               Show this help message"
    Write-Host ""
    Write-Host "Examples:"
    Write-Host "  .\build.ps1                                    # Build all targets"
    Write-Host "  .\build.ps1 -Targets @('x86_64-pc-windows-msvc')  # Build Windows MSVC only"
    Write-Host "  .\build.ps1 -CheckTools                        # Check build tools"
    Write-Host ""
}

if ($Help) {
    Show-Help
    exit 0
}

# --- Configuration ---
try {
    # Automatically get the package name from Cargo.toml
    $CargoToml = Get-Content "Cargo.toml" -ErrorAction Stop
    $PackageNameLine = $CargoToml | Where-Object { $_ -match '^name\s*=' } | Select-Object -First 1
    if (-not $PackageNameLine) {
        throw "Could not find package name in Cargo.toml"
    }
    $PackageName = ($PackageNameLine -split '"')[1]
    Write-Host "Package name: $PackageName" -ForegroundColor Green
} catch {
    Write-Error "Failed to read package name from Cargo.toml: $_"
    exit 1
}

# --- Target Definitions ---
# Default targets if none specified
$DefaultTargets = @(
    @{ Triple = "x86_64-unknown-linux-gnu"; Suffix = "linux-amd64" },
    @{ Triple = "aarch64-unknown-linux-gnu"; Suffix = "linux-arm64" },
    @{ Triple = "x86_64-pc-windows-gnu"; Suffix = "windows-amd64" },
    @{ Triple = "x86_64-pc-windows-msvc"; Suffix = "windows-amd64-msvc" },
    @{ Triple = "aarch64-pc-windows-gnu"; Suffix = "windows-arm64" },
    @{ Triple = "aarch64-pc-windows-msvc"; Suffix = "windows-arm64-msvc" },
    @{ Triple = "x86_64-apple-darwin"; Suffix = "macos-amd64" },
    @{ Triple = "aarch64-apple-darwin"; Suffix = "macos-arm64" }
)

# Convert custom targets to target objects if provided
if ($Targets.Count -gt 0) {
    $TargetObjects = @()
    foreach ($target in $Targets) {
        $suffix = $target -replace "x86_64-unknown-linux-gnu", "linux-amd64" `
                          -replace "aarch64-unknown-linux-gnu", "linux-arm64" `
                          -replace "x86_64-pc-windows-gnu", "windows-amd64" `
                          -replace "x86_64-pc-windows-msvc", "windows-amd64-msvc" `
                          -replace "aarch64-pc-windows-gnu", "windows-arm64" `
                          -replace "aarch64-pc-windows-msvc", "windows-arm64-msvc" `
                          -replace "x86_64-apple-darwin", "macos-amd64" `
                          -replace "aarch64-apple-darwin", "macos-arm64"
        $TargetObjects += @{ Triple = $target; Suffix = $suffix }
    }
} else {
    $TargetObjects = $DefaultTargets
}

# --- Tool Detection Functions ---
function Test-Command {
    param([string]$Command)
    try {
        Get-Command $Command -ErrorAction Stop | Out-Null
        return $true
    } catch {
        return $false
    }
}

function Test-BuildTools {
    Write-Host "--- Checking Build Tools ---" -ForegroundColor Yellow
    
    $tools = @{
        "rustc" = "Rust compiler"
        "cargo" = "Cargo build tool"
        "rustup" = "Rust toolchain manager"
    }
    
    $missing = @()
    
    foreach ($tool in $tools.Keys) {
        if (Test-Command $tool) {
            Write-Host "✓ $($tools[$tool]) ($tool) found" -ForegroundColor Green
        } else {
            Write-Host "✗ $($tools[$tool]) ($tool) not found" -ForegroundColor Red
            $missing += $tool
        }
    }
    
    # Check for Windows-specific tools
    if ($env:OS -eq "Windows_NT") {
        $windowsTools = @{
            "cl.exe" = "MSVC Compiler (for MSVC targets)"
            "link.exe" = "MSVC Linker (for MSVC targets)"
        }
        
        foreach ($tool in $windowsTools.Keys) {
            if (Test-Command $tool) {
                Write-Host "✓ $($windowsTools[$tool]) found" -ForegroundColor Green
            } else {
                Write-Host "⚠ $($windowsTools[$tool]) not found (optional for GNU targets)" -ForegroundColor Yellow
            }
        }
    }
    
    if ($missing.Count -gt 0) {
        Write-Host ""
        Write-Host "Missing required tools: $($missing -join ', ')" -ForegroundColor Red
        Write-Host "Please install Rust from https://rustup.rs/" -ForegroundColor Yellow
        return $false
    }
    
    Write-Host ""
    Write-Host "All required build tools are available!" -ForegroundColor Green
    return $true
}

if ($CheckTools) {
    Test-BuildTools
    exit 0
}

# --- Prerequisite Information ---
Write-Host "--- Build Script for $PackageName ---" -ForegroundColor Green
Write-Host ""
Write-Host "This script will attempt to cross-compile for multiple targets."
Write-Host "Please ensure you have the necessary linkers and toolchains installed."
Write-Host ""

if ($env:OS -eq "Windows_NT") {
    Write-Host "On Windows, you might need:" -ForegroundColor Yellow
    Write-Host "  - Visual Studio Build Tools (for MSVC targets)"
    Write-Host "  - MinGW-w64 (for GNU targets)"
    Write-Host "  - LLVM/Clang (alternative linker)"
    Write-Host ""
} else {
    Write-Host "On macOS with Homebrew, you might need:" -ForegroundColor Yellow
    Write-Host "  brew install x86_64-unknown-linux-gnu aarch64-unknown-linux-gnu mingw-w64"
    Write-Host ""
    Write-Host "On a Debian-based Linux, you might need:" -ForegroundColor Yellow
    Write-Host "  sudo apt-get install -y gcc-x86-64-linux-gnu gcc-aarch64-linux-gnu gcc-mingw-w64"
    Write-Host ""
}

Write-Host "You also MUST configure Cargo to use these linkers. Create or edit '.cargo/config.toml' in your project root." -ForegroundColor Yellow
Write-Host "For a simpler cross-compilation experience, consider using the 'cross' tool:" -ForegroundColor Yellow
Write-Host "  cargo install cross"
Write-Host "  # Then run: cross build --release --target <target-triple>"
Write-Host "----------------------------------------------------"
Write-Host ""

# Check build tools
if (-not (Test-BuildTools)) {
    Write-Error "Required build tools are missing. Use -CheckTools for details."
    exit 1
}

# --- Build Process ---
Write-Host "Creating build directory: $BuildDir" -ForegroundColor Green
if (-not (Test-Path $BuildDir)) {
    New-Item -ItemType Directory -Path $BuildDir -Force | Out-Null
}

$successfulBuilds = @()
$failedBuilds = @()

foreach ($targetObj in $TargetObjects) {
    $target = $targetObj.Triple
    $suffix = $targetObj.Suffix
    
    $filename = "$PackageName-$suffix"
    $sourceBinaryName = $PackageName
    
    # Windows targets have a .exe extension
    if ($target -match "-windows-") {
        $filename += ".exe"
        $sourceBinaryName += ".exe"
    }
    
    $sourcePath = "target/$target/release/$sourceBinaryName"
    $destPath = "$BuildDir/$filename"
    
    Write-Host "--- Building for $target ---" -ForegroundColor Cyan
    
    try {
        # Install the Rust target via rustup if it's not already installed
        Write-Host "Checking for target toolchain $target..."
        rustup target add $target
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to add target $target"
        }
        
        # Build the project for the specified target in release mode
        Write-Host "Running cargo build for $target..."
        cargo build --release --target $target
        if ($LASTEXITCODE -ne 0) {
            throw "Cargo build failed for target $target"
        }
        
        # Copy the compiled binary to the build directory with a clean name
        Write-Host "Copying binary to $destPath"
        if (-not (Test-Path $sourcePath)) {
            throw "Source binary not found at $sourcePath"
        }
        
        Copy-Item $sourcePath $destPath -Force
        Write-Host "Build for $target complete." -ForegroundColor Green
        $successfulBuilds += $target
        
    } catch {
        Write-Host "Build for $target failed: $_" -ForegroundColor Red
        $failedBuilds += $target
    }
    
    Write-Host ""
}

# --- Summary ---
Write-Host "--- Build Summary ---" -ForegroundColor Green
Write-Host "Successful builds ($($successfulBuilds.Count)): $($successfulBuilds -join ', ')" -ForegroundColor Green

if ($failedBuilds.Count -gt 0) {
    Write-Host "Failed builds ($($failedBuilds.Count)): $($failedBuilds -join ', ')" -ForegroundColor Red
}

if ($successfulBuilds.Count -gt 0) {
    Write-Host ""
    Write-Host "Final binaries are located in the '$BuildDir' directory:" -ForegroundColor Green
    Get-ChildItem $BuildDir | Format-Table Name, Length, LastWriteTime -AutoSize
} else {
    Write-Error "No builds were successful!"
    exit 1
}

Write-Host "--- All builds completed! ---" -ForegroundColor Green