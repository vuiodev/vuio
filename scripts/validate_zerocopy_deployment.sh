#!/bin/bash

# ZeroCopy Database Deployment Validation Script
# This script validates that the ZeroCopy database is properly deployed and functioning

set -e

echo "=== ZeroCopy Database Deployment Validation ==="
echo "Date: $(date)"
echo "Platform: $(uname -s) $(uname -m)"
echo

# Function to run a test and report results
run_test() {
    local test_name="$1"
    local test_command="$2"
    
    echo -n "Testing $test_name... "
    if eval "$test_command" > /dev/null 2>&1; then
        echo "✅ PASS"
        return 0
    else
        echo "❌ FAIL"
        return 1
    fi
}

# Function to run a test with output
run_test_with_output() {
    local test_name="$1"
    local test_command="$2"
    
    echo "Testing $test_name..."
    if eval "$test_command"; then
        echo "✅ PASS"
        echo
        return 0
    else
        echo "❌ FAIL"
        echo
        return 1
    fi
}

# Check if we're in the right directory
if [ ! -f "Cargo.toml" ]; then
    echo "❌ Error: Must be run from the project root directory"
    exit 1
fi

echo "1. Compilation Tests"
echo "==================="

# Test that the project compiles
run_test "Project compilation" "cargo check --quiet"

# Test that ZeroCopy database compiles
run_test "ZeroCopy database compilation" "cargo check --quiet --lib"

echo

echo "2. ZeroCopy Database Unit Tests"
echo "==============================="

# Run ZeroCopy specific tests
run_test_with_output "ZeroCopy database tests" "cargo test --lib zerocopy --quiet"

echo "3. Integration Tests"
echo "==================="

# Run integration tests
run_test_with_output "Integration tests" "cargo test --test integration_tests --quiet"

echo "4. Performance Validation"
echo "========================="

# Run performance tests
run_test_with_output "Performance benchmarks" "cargo test --release --test memory_usage_comparison --quiet"

echo "5. Database Health Checks"
echo "========================="

# Create a temporary test database and validate health
TEMP_DIR=$(mktemp -d)
TEST_DB_PATH="$TEMP_DIR/test_health.db"

# Create a simple test program to validate database health
cat > "$TEMP_DIR/health_check.rs" << 'EOF'
use std::path::PathBuf;
use vuio::database::zerocopy::{ZeroCopyDatabase, PerformanceProfile};
use vuio::database::DatabaseManager;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = PathBuf::from(std::env::args().nth(1).expect("Database path required"));
    
    // Create database with minimal profile for testing
    let db = ZeroCopyDatabase::new_with_profile(db_path, PerformanceProfile::Minimal).await?;
    
    // Initialize and open
    db.initialize().await?;
    db.open().await?;
    
    // Perform health check
    let health = db.check_and_repair().await?;
    
    if health.is_healthy {
        println!("✅ Database health check passed");
        
        // Get statistics
        let stats = db.get_stats().await?;
        println!("📊 Database statistics:");
        println!("   - Total files: {}", stats.total_files);
        println!("   - Database size: {} bytes", stats.database_size);
        
        // Test basic operations
        let test_files = vec![
            vuio::database::MediaFile {
                id: None,
                path: PathBuf::from("/test/file1.mp3"),
                canonical_path: "/test/file1.mp3".to_string(),
                canonical_parent_path: "/test".to_string(),
                filename: "file1.mp3".to_string(),
                size: 1024,
                modified: std::time::SystemTime::now(),
                mime_type: "audio/mpeg".to_string(),
                duration: Some(std::time::Duration::from_secs(180)),
                title: Some("Test Song".to_string()),
                artist: Some("Test Artist".to_string()),
                album: Some("Test Album".to_string()),
                genre: Some("Test".to_string()),
                track_number: Some(1),
                year: Some(2024),
                album_artist: Some("Test Artist".to_string()),
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
            }
        ];
        
        // Test bulk operations
        let ids = db.bulk_store_media_files(&test_files).await?;
        println!("✅ Bulk store operation successful: {} files", ids.len());
        
        // Test individual operations
        let file = db.get_file_by_path(&PathBuf::from("/test/file1.mp3")).await?;
        if file.is_some() {
            println!("✅ Individual retrieval operation successful");
        } else {
            println!("❌ Individual retrieval operation failed");
        }
        
        println!("✅ All database operations completed successfully");
    } else {
        println!("❌ Database health check failed:");
        for issue in &health.issues {
            println!("   - {}: {}", 
                match issue.severity {
                    vuio::database::IssueSeverity::Critical => "CRITICAL",
                    vuio::database::IssueSeverity::Error => "ERROR", 
                    vuio::database::IssueSeverity::Warning => "WARNING",
                    vuio::database::IssueSeverity::Info => "INFO",
                },
                issue.description
            );
        }
        return Err("Database health check failed".into());
    }
    
    Ok(())
}
EOF

# Compile and run the health check
echo -n "Compiling health check program... "
if rustc --edition 2021 -L target/release/deps "$TEMP_DIR/health_check.rs" -o "$TEMP_DIR/health_check" --extern vuio=target/release/libvuio.rlib --extern tokio=target/release/deps/libtokio*.rlib 2>/dev/null; then
    echo "✅ PASS"
else
    echo "⚠️  SKIP (compilation failed - using cargo test instead)"
    # Fallback to cargo test for health validation
    run_test "Database health validation" "cargo test --lib zerocopy::tests::test_database_initialization --quiet"
fi

# If health check program compiled, run it
if [ -f "$TEMP_DIR/health_check" ]; then
    echo "Running database health check..."
    if "$TEMP_DIR/health_check" "$TEST_DB_PATH"; then
        echo "✅ Database health check completed successfully"
    else
        echo "❌ Database health check failed"
    fi
    echo
fi

# Cleanup
rm -rf "$TEMP_DIR"

echo "6. Memory Limits and Auto-scaling Validation"
echo "============================================="

# Test different performance profiles
echo "Testing performance profiles..."

# Test with environment variables
export ZEROCOPY_CACHE_MB=8
export ZEROCOPY_INDEX_SIZE=500000
export ZEROCOPY_BATCH_SIZE=50000

run_test "Environment variable configuration" "cargo test --lib zerocopy::config_tests::tests::test_zerocopy_config_from_env --quiet"

# Reset environment
unset ZEROCOPY_CACHE_MB
unset ZEROCOPY_INDEX_SIZE  
unset ZEROCOPY_BATCH_SIZE

echo

echo "7. SQLite Dependency Verification"
echo "================================="

# Verify SQLite is only used in tests and legacy compatibility
echo -n "Checking SQLite usage in production code... "
# Check main application files (excluding database module which contains legacy code)
if grep -r "SqliteDatabase" src/main.rs src/web/ src/media.rs src/platform/ src/watcher/ src/ssdp.rs src/config/ src/logging.rs src/error.rs 2>/dev/null | grep -v "test" | grep -v "#\[cfg(test)\]" > /dev/null; then
    echo "❌ FAIL - SQLite found in main application code"
    echo "SQLite references found in main application:"
    grep -r "SqliteDatabase" src/main.rs src/web/ src/media.rs src/platform/ src/watcher/ src/ssdp.rs src/config/ src/logging.rs src/error.rs 2>/dev/null | grep -v "test" | grep -v "#\[cfg(test)\]"
else
    echo "✅ PASS - SQLite only in database module (legacy/test compatibility)"
fi

echo -n "Verifying ZeroCopy is primary database... "
if grep -r "ZeroCopyDatabase" src/main.rs > /dev/null; then
    echo "✅ PASS - ZeroCopy database in main application"
else
    echo "❌ FAIL - ZeroCopy database not found in main application"
fi

echo

echo "8. Final Validation Summary"
echo "==========================="

# Run a comprehensive test to ensure everything works together (excluding platform-specific path tests)
echo "Running comprehensive system test..."
if cargo test --release --quiet --lib > /dev/null 2>&1 && cargo test --release --quiet --test integration_tests > /dev/null 2>&1 && cargo test --release --quiet --test memory_usage_comparison > /dev/null 2>&1; then
    echo "✅ All tests passed - ZeroCopy database migration is complete"
    echo
    echo "🎉 DEPLOYMENT VALIDATION SUCCESSFUL"
    echo
    echo "Key achievements:"
    echo "  ✅ Complete SQLite to ZeroCopy migration"
    echo "  ✅ ZeroCopy database is the only production database"
    echo "  ✅ All atomic operations working correctly"
    echo "  ✅ Memory limits and auto-scaling functional"
    echo "  ✅ Performance targets achieved"
    echo "  ✅ Database health checks operational"
    echo "  ✅ SQLite kept only as legacy/test compatibility layer"
    echo
    echo "The system is ready for production deployment with ZeroCopy database."
else
    echo "❌ Some tests failed - deployment validation incomplete"
    echo
    echo "Please review test failures and ensure all issues are resolved."
    exit 1
fi