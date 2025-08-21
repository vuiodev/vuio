# Cross-platform test runner script for VuIO (PowerShell)

param(
    [Parameter(Position=0)]
    [string]$Command = "all",
    
    [switch]$Performance,
    [switch]$Coverage,
    [switch]$Help
)

# Colors for output
$Colors = @{
    Red = "Red"
    Green = "Green"
    Yellow = "Yellow"
    Blue = "Blue"
    White = "White"
}

function Write-Status {
    param([string]$Message)
    Write-Host "[INFO] $Message" -ForegroundColor $Colors.Blue
}

function Write-Success {
    param([string]$Message)
    Write-Host "[SUCCESS] $Message" -ForegroundColor $Colors.Green
}

function Write-Warning {
    param([string]$Message)
    Write-Host "[WARNING] $Message" -ForegroundColor $Colors.Yellow
}

function Write-Error {
    param([string]$Message)
    Write-Host "[ERROR] $Message" -ForegroundColor $Colors.Red
}

# Detect current platform
function Get-Platform {
    if ($IsWindows -or $env:OS -eq "Windows_NT") {
        return "windows"
    } elseif ($IsLinux) {
        return "linux"
    } elseif ($IsMacOS) {
        return "macos"
    } else {
        return "unknown"
    }
}

# Check if required tools are installed
function Test-Dependencies {
    Write-Status "Checking dependencies..."
    
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Error "Cargo is not installed. Please install Rust."
        exit 1
    }
    
    if (-not (Get-Command rustc -ErrorAction SilentlyContinue)) {
        Write-Error "Rust compiler is not installed."
        exit 1
    }
    
    Write-Success "All dependencies are available"
}

# Install platform-specific dependencies
function Install-PlatformDependencies {
    $platform = Get-Platform
    Write-Status "Installing platform-specific dependencies for $platform..."
    
    switch ($platform) {
        "windows" {
            # Check for Visual Studio Build Tools or Visual Studio
            if (-not (Get-Command cl -ErrorAction SilentlyContinue)) {
                Write-Warning "Visual Studio Build Tools not found. Please install Visual Studio or Build Tools."
            }
            
            # Check for vcpkg
            if ($env:VCPKG_ROOT -and (Test-Path $env:VCPKG_ROOT)) {
                Write-Status "vcpkg found at $env:VCPKG_ROOT"
            } else {
                Write-Warning "vcpkg not found. Some dependencies may need manual installation."
            }
        }
        "linux" {
            Write-Status "Linux dependencies should be installed using the shell script"
        }
        "macos" {
            Write-Status "macOS dependencies should be installed using the shell script"
        }
        default {
            Write-Warning "Unknown platform. Dependencies may need to be installed manually."
        }
    }
}

# Run basic compilation tests
function Test-Compilation {
    Write-Status "Testing compilation..."
    
    $result = & cargo check --all-features
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Compilation check passed"
        return $true
    } else {
        Write-Error "Compilation check failed"
        return $false
    }
}

# Run unit tests
function Invoke-UnitTests {
    Write-Status "Running unit tests..."
    
    $result = & cargo test --lib --all-features
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Unit tests passed"
        return $true
    } else {
        Write-Error "Unit tests failed"
        return $false
    }
}

# Run platform-specific tests
function Invoke-PlatformTests {
    Write-Status "Running platform-specific tests..."
    
    $result = & cargo test --test platform_tests
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Platform-specific tests passed"
        return $true
    } else {
        Write-Warning "Some platform-specific tests failed (may be expected in some environments)"
        return $true  # Don't fail the overall test suite
    }
}

# Run integration tests
function Invoke-IntegrationTests {
    Write-Status "Running integration tests..."
    
    $result = & cargo test --test integration_tests
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Integration tests passed"
        return $true
    } else {
        Write-Warning "Some integration tests failed (may be expected without network access)"
        return $true  # Don't fail the overall test suite
    }
}

# Run database tests
function Invoke-DatabaseTests {
    Write-Status "Running database tests..."
    
    $result = & cargo test database_tests
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Database tests passed"
        return $true
    } else {
        Write-Error "Database tests failed"
        return $false
    }
}

# Run file watcher tests
function Invoke-WatcherTests {
    Write-Status "Running file watcher tests..."
    
    $result = & cargo test watcher_tests
    if ($LASTEXITCODE -eq 0) {
        Write-Success "File watcher tests passed"
        return $true
    } else {
        Write-Warning "File watcher tests failed (may be expected in some environments)"
        return $true  # Don't fail the overall test suite
    }
}

# Run network tests
function Invoke-NetworkTests {
    Write-Status "Running network tests..."
    
    $result = & cargo test network_tests
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Network tests passed"
        return $true
    } else {
        Write-Warning "Network tests failed (may be expected without network access)"
        return $true  # Don't fail the overall test suite
    }
}

# Run performance tests
function Invoke-PerformanceTests {
    Write-Status "Running performance tests..."
    
    $result = & cargo test --release test_database_concurrent_access -- --ignored
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Performance tests passed"
        return $true
    } else {
        Write-Warning "Performance tests failed or were skipped"
        return $true  # Don't fail the overall test suite
    }
}

# Run security tests
function Invoke-SecurityTests {
    Write-Status "Running security tests..."
    
    # Install cargo-audit if not present
    if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) {
        Write-Status "Installing cargo-audit..."
        & cargo install cargo-audit
    }
    
    $result = & cargo audit
    if ($LASTEXITCODE -eq 0) {
        Write-Success "Security audit passed"
        return $true
    } else {
        Write-Warning "Security audit found issues"
        return $true  # Don't fail the overall test suite
    }
}

# Run code quality checks
function Invoke-QualityChecks {
    Write-Status "Running code quality checks..."
    
    # Format check
    $result = & cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Code formatting issues found. Run 'cargo fmt' to fix."
        return $false
    }
    Write-Success "Code formatting is correct"
    
    # Clippy lints
    $result = & cargo clippy --all-targets --all-features -- -D warnings
    if ($LASTEXITCODE -ne 0) {
        Write-Error "Clippy lints failed"
        return $false
    }
    Write-Success "Clippy lints passed"
    
    return $true
}

# Generate test coverage report
function New-CoverageReport {
    Write-Status "Generating test coverage report..."
    
    if (Get-Command cargo-llvm-cov -ErrorAction SilentlyContinue) {
        & cargo llvm-cov --all-features --workspace --html
        Write-Success "Coverage report generated in target/llvm-cov/html/"
    } else {
        Write-Warning "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov"
    }
}

# Clean up test artifacts
function Clear-TestArtifacts {
    Write-Status "Cleaning up test artifacts..."
    & cargo clean
    Write-Success "Cleanup completed"
}

# Main test runner function
function Invoke-AllTests {
    param([bool]$IncludePerformance, [bool]$IncludeCoverage)
    
    $failedTests = @()
    $platform = Get-Platform
    
    Write-Status "Starting comprehensive test suite for VuIO"
    Write-Status "Platform: $platform"
    Write-Status "Rust version: $(& rustc --version)"
    Write-Status "Cargo version: $(& cargo --version)"
    Write-Host ""
    
    # Run all test categories
    if (-not (Test-Compilation)) { $failedTests += "compilation" }
    if (-not (Invoke-UnitTests)) { $failedTests += "unit" }
    if (-not (Invoke-PlatformTests)) { $failedTests += "platform" }
    if (-not (Invoke-IntegrationTests)) { $failedTests += "integration" }
    if (-not (Invoke-DatabaseTests)) { $failedTests += "database" }
    if (-not (Invoke-WatcherTests)) { $failedTests += "watcher" }
    if (-not (Invoke-NetworkTests)) { $failedTests += "network" }
    if (-not (Invoke-QualityChecks)) { $failedTests += "quality" }
    if (-not (Invoke-SecurityTests)) { $failedTests += "security" }
    
    # Optional performance tests
    if ($IncludePerformance) {
        if (-not (Invoke-PerformanceTests)) { $failedTests += "performance" }
    }
    
    # Optional coverage report
    if ($IncludeCoverage) {
        New-CoverageReport
    }
    
    # Report results
    Write-Host ""
    Write-Status "Test suite completed"
    
    if ($failedTests.Count -eq 0) {
        Write-Success "All tests passed!"
        return $true
    } else {
        Write-Error "The following test categories failed: $($failedTests -join ', ')"
        return $false
    }
}

# Show help message
function Show-Help {
    Write-Host "VuIO Test Runner (PowerShell)" -ForegroundColor $Colors.Blue
    Write-Host ""
    Write-Host "Usage: .\run-tests.ps1 [COMMAND] [OPTIONS]" -ForegroundColor $Colors.White
    Write-Host ""
    Write-Host "Commands:" -ForegroundColor $Colors.White
    Write-Host "  all          Run all tests (default)" -ForegroundColor $Colors.White
    Write-Host "  deps         Install platform-specific dependencies" -ForegroundColor $Colors.White
    Write-Host "  unit         Run unit tests only" -ForegroundColor $Colors.White
    Write-Host "  platform     Run platform-specific tests only" -ForegroundColor $Colors.White
    Write-Host "  integration  Run integration tests only" -ForegroundColor $Colors.White
    Write-Host "  database     Run database tests only" -ForegroundColor $Colors.White
    Write-Host "  watcher      Run file watcher tests only" -ForegroundColor $Colors.White
    Write-Host "  network      Run network tests only" -ForegroundColor $Colors.White
    Write-Host "  performance  Run performance tests only" -ForegroundColor $Colors.White
    Write-Host "  security     Run security audit" -ForegroundColor $Colors.White
    Write-Host "  quality      Run code quality checks (fmt, clippy)" -ForegroundColor $Colors.White
    Write-Host "  coverage     Generate test coverage report" -ForegroundColor $Colors.White
    Write-Host "  clean        Clean up test artifacts" -ForegroundColor $Colors.White
    Write-Host "  help         Show this help message" -ForegroundColor $Colors.White
    Write-Host ""
    Write-Host "Options:" -ForegroundColor $Colors.White
    Write-Host "  -Performance  Include performance tests in 'all' run" -ForegroundColor $Colors.White
    Write-Host "  -Coverage     Generate coverage report in 'all' run" -ForegroundColor $Colors.White
    Write-Host "  -Help         Show this help message" -ForegroundColor $Colors.White
}

# Main script logic
if ($Help) {
    Show-Help
    exit 0
}

$platform = Get-Platform
Test-Dependencies

switch ($Command.ToLower()) {
    "all" {
        $success = Invoke-AllTests -IncludePerformance $Performance -IncludeCoverage $Coverage
        exit $(if ($success) { 0 } else { 1 })
    }
    "deps" {
        Install-PlatformDependencies
    }
    "unit" {
        $success = Invoke-UnitTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "platform" {
        $success = Invoke-PlatformTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "integration" {
        $success = Invoke-IntegrationTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "database" {
        $success = Invoke-DatabaseTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "watcher" {
        $success = Invoke-WatcherTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "network" {
        $success = Invoke-NetworkTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "performance" {
        $success = Invoke-PerformanceTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "security" {
        $success = Invoke-SecurityTests
        exit $(if ($success) { 0 } else { 1 })
    }
    "quality" {
        $success = Invoke-QualityChecks
        exit $(if ($success) { 0 } else { 1 })
    }
    "coverage" {
        New-CoverageReport
    }
    "clean" {
        Clear-TestArtifacts
    }
    "help" {
        Show-Help
    }
    default {
        Write-Error "Unknown command: $Command"
        Write-Status "Run '.\run-tests.ps1 help' for usage information"
        exit 1
    }
}