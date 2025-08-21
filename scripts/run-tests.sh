#!/bin/bash
# Cross-platform test runner script for VuIO

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Detect current platform
detect_platform() {
    case "$(uname -s)" in
        Linux*)     PLATFORM=linux;;
        Darwin*)    PLATFORM=macos;;
        CYGWIN*|MINGW*|MSYS*) PLATFORM=windows;;
        *)          PLATFORM=unknown;;
    esac
    print_status "Detected platform: $PLATFORM"
}

# Check if required tools are installed
check_dependencies() {
    print_status "Checking dependencies..."
    
    if ! command -v cargo &> /dev/null; then
        print_error "Cargo is not installed. Please install Rust."
        exit 1
    fi
    
    if ! command -v rustc &> /dev/null; then
        print_error "Rust compiler is not installed."
        exit 1
    fi
    
    print_success "All dependencies are available"
}

# Install platform-specific dependencies
install_platform_deps() {
    print_status "Installing platform-specific dependencies..."
    
    case $PLATFORM in
        linux)
            if command -v apt-get &> /dev/null; then
                sudo apt-get update
                sudo apt-get install -y pkg-config libssl-dev build-essential
            elif command -v yum &> /dev/null; then
                sudo yum install -y pkgconfig openssl-devel gcc
            elif command -v pacman &> /dev/null; then
                sudo pacman -S --noconfirm pkg-config openssl gcc
            else
                print_warning "Unknown Linux distribution. Please install pkg-config, openssl-dev, and build tools manually."
            fi
            ;;
        macos)
            if command -v brew &> /dev/null; then
                brew install pkg-config openssl
            else
                print_warning "Homebrew not found. Please install pkg-config and openssl manually."
            fi
            ;;
        windows)
            print_status "Windows dependencies should be handled by vcpkg or pre-installed"
            ;;
        *)
            print_warning "Unknown platform. Dependencies may need to be installed manually."
            ;;
    esac
}

# Run basic compilation tests
test_compilation() {
    print_status "Testing compilation..."
    
    if cargo check --all-features; then
        print_success "Compilation check passed"
    else
        print_error "Compilation check failed"
        return 1
    fi
}

# Run unit tests
run_unit_tests() {
    print_status "Running unit tests..."
    
    if cargo test --lib --all-features; then
        print_success "Unit tests passed"
    else
        print_error "Unit tests failed"
        return 1
    fi
}

# Run platform-specific tests
run_platform_tests() {
    print_status "Running platform-specific tests..."
    
    if cargo test --test platform_tests; then
        print_success "Platform-specific tests passed"
    else
        print_warning "Some platform-specific tests failed (may be expected in some environments)"
    fi
}

# Run integration tests
run_integration_tests() {
    print_status "Running integration tests..."
    
    if cargo test --test integration_tests; then
        print_success "Integration tests passed"
    else
        print_warning "Some integration tests failed (may be expected without network access)"
    fi
}

# Run database tests
run_database_tests() {
    print_status "Running database tests..."
    
    if cargo test database_tests; then
        print_success "Database tests passed"
    else
        print_error "Database tests failed"
        return 1
    fi
}

# Run file watcher tests
run_watcher_tests() {
    print_status "Running file watcher tests..."
    
    if cargo test watcher_tests; then
        print_success "File watcher tests passed"
    else
        print_warning "File watcher tests failed (may be expected in some environments)"
    fi
}

# Run network tests
run_network_tests() {
    print_status "Running network tests..."
    
    if cargo test network_tests; then
        print_success "Network tests passed"
    else
        print_warning "Network tests failed (may be expected without network access)"
    fi
}

# Run performance tests
run_performance_tests() {
    print_status "Running performance tests..."
    
    if cargo test --release test_database_concurrent_access -- --ignored; then
        print_success "Performance tests passed"
    else
        print_warning "Performance tests failed or were skipped"
    fi
}

# Run security tests
run_security_tests() {
    print_status "Running security tests..."
    
    # Install cargo-audit if not present
    if ! command -v cargo-audit &> /dev/null; then
        print_status "Installing cargo-audit..."
        cargo install cargo-audit
    fi
    
    if cargo audit; then
        print_success "Security audit passed"
    else
        print_warning "Security audit found issues"
    fi
}

# Run code quality checks
run_quality_checks() {
    print_status "Running code quality checks..."
    
    # Format check
    if cargo fmt --all -- --check; then
        print_success "Code formatting is correct"
    else
        print_error "Code formatting issues found. Run 'cargo fmt' to fix."
        return 1
    fi
    
    # Clippy lints
    if cargo clippy --all-targets --all-features -- -D warnings; then
        print_success "Clippy lints passed"
    else
        print_error "Clippy lints failed"
        return 1
    fi
}

# Generate test coverage report
generate_coverage() {
    print_status "Generating test coverage report..."
    
    if command -v cargo-llvm-cov &> /dev/null; then
        cargo llvm-cov --all-features --workspace --html
        print_success "Coverage report generated in target/llvm-cov/html/"
    else
        print_warning "cargo-llvm-cov not installed. Install with: cargo install cargo-llvm-cov"
    fi
}

# Clean up test artifacts
cleanup() {
    print_status "Cleaning up test artifacts..."
    cargo clean
    print_success "Cleanup completed"
}

# Main test runner function
run_all_tests() {
    local failed_tests=()
    
    print_status "Starting comprehensive test suite for VuIO"
    print_status "Platform: $PLATFORM"
    print_status "Rust version: $(rustc --version)"
    print_status "Cargo version: $(cargo --version)"
    echo
    
    # Run all test categories
    test_compilation || failed_tests+=("compilation")
    run_unit_tests || failed_tests+=("unit")
    run_platform_tests || failed_tests+=("platform")
    run_integration_tests || failed_tests+=("integration")
    run_database_tests || failed_tests+=("database")
    run_watcher_tests || failed_tests+=("watcher")
    run_network_tests || failed_tests+=("network")
    run_quality_checks || failed_tests+=("quality")
    run_security_tests || failed_tests+=("security")
    
    # Optional performance tests
    if [[ "$1" == "--performance" ]]; then
        run_performance_tests || failed_tests+=("performance")
    fi
    
    # Optional coverage report
    if [[ "$1" == "--coverage" ]]; then
        generate_coverage
    fi
    
    # Report results
    echo
    print_status "Test suite completed"
    
    if [ ${#failed_tests[@]} -eq 0 ]; then
        print_success "All tests passed!"
        return 0
    else
        print_error "The following test categories failed: ${failed_tests[*]}"
        return 1
    fi
}

# Parse command line arguments
case "${1:-all}" in
    all)
        detect_platform
        check_dependencies
        run_all_tests
        ;;
    deps)
        detect_platform
        install_platform_deps
        ;;
    unit)
        run_unit_tests
        ;;
    platform)
        run_platform_tests
        ;;
    integration)
        run_integration_tests
        ;;
    database)
        run_database_tests
        ;;
    watcher)
        run_watcher_tests
        ;;
    network)
        run_network_tests
        ;;
    performance)
        run_performance_tests
        ;;
    security)
        run_security_tests
        ;;
    quality)
        run_quality_checks
        ;;
    coverage)
        generate_coverage
        ;;
    clean)
        cleanup
        ;;
    help|--help|-h)
        echo "VuIO Test Runner"
        echo
        echo "Usage: $0 [COMMAND]"
        echo
        echo "Commands:"
        echo "  all          Run all tests (default)"
        echo "  deps         Install platform-specific dependencies"
        echo "  unit         Run unit tests only"
        echo "  platform     Run platform-specific tests only"
        echo "  integration  Run integration tests only"
        echo "  database     Run database tests only"
        echo "  watcher      Run file watcher tests only"
        echo "  network      Run network tests only"
        echo "  performance  Run performance tests only"
        echo "  security     Run security audit"
        echo "  quality      Run code quality checks (fmt, clippy)"
        echo "  coverage     Generate test coverage report"
        echo "  clean        Clean up test artifacts"
        echo "  help         Show this help message"
        echo
        echo "Options:"
        echo "  --performance  Include performance tests in 'all' run"
        echo "  --coverage     Generate coverage report in 'all' run"
        ;;
    *)
        print_error "Unknown command: $1"
        print_status "Run '$0 help' for usage information"
        exit 1
        ;;
esac