# Database Performance Tests

This document describes the database performance tests implemented in `database_performance_tests.rs`.

## Overview

The performance tests verify that the database operations scale properly with large datasets and maintain bounded memory usage. These tests are designed to catch performance regressions and ensure the system can handle real-world media libraries with thousands of files.

## Test Configuration

- **Large Dataset Size**: 10,000 media files
- **Batch Size**: 1,000 files per batch
- **Performance Threshold**: 15 seconds for large operations
- **Fast Operation Threshold**: 1 second for quick operations

## Test Cases

### 1. Large Dataset Creation Performance (`test_large_dataset_creation_performance`)

**Purpose**: Verify that creating and storing large numbers of media files performs within acceptable limits.

**What it tests**:
- Creates 10,000 test media files with varied metadata
- Stores them in the database in batches
- Measures total time and memory usage
- Verifies average time per file is reasonable

**Success criteria**:
- All 10,000 files are created successfully
- Total time is under 15 seconds
- Memory usage remains bounded

### 2. Directory Listing Performance (`test_directory_listing_performance`)

**Purpose**: Ensure directory browsing remains fast even with many files and subdirectories.

**What it tests**:
- Creates hierarchical directory structure (10 directories, 100 files each)
- Tests directory listing at root level (should find subdirectories)
- Tests directory listing at subdirectory level (should find files)
- Measures query time and memory usage

**Success criteria**:
- Directory listing completes in under 1 second
- Correct number of subdirectories and files are found
- Subdirectory listing completes in under 500ms

### 3. Batch Cleanup Performance (`test_batch_cleanup_performance`)

**Purpose**: Verify that cleaning up missing files is efficient and doesn't load entire database into memory.

**What it tests**:
- Creates 10,000 files in database
- Simulates half the files being deleted from disk
- Uses `batch_cleanup_missing_files` to remove database entries
- Measures cleanup time and memory usage

**Success criteria**:
- Approximately 5,000 files are removed
- Cleanup completes in under 15 seconds
- Memory usage remains bounded during operation

### 4. Streaming vs Bulk Memory Usage (`test_streaming_vs_bulk_memory_usage`)

**Purpose**: Compare memory usage between bulk loading and streaming approaches.

**What it tests**:
- Creates 10,000 files in database
- Tests collect method with `collect_all_media_files()` (uses streaming internally)
- Tests streaming with `stream_all_media_files()`
- Compares memory usage and performance

**Success criteria**:
- Both approaches process all 10,000 files
- Both complete within 15 seconds
- Streaming uses less memory than bulk loading (for large datasets)

### 5. Path Prefix Query Performance (`test_path_prefix_query_performance`)

**Purpose**: Ensure path-based queries are efficient for directory operations.

**What it tests**:
- Creates 10,000 files across 5 different path prefixes (2,000 each)
- Tests `get_files_with_path_prefix()` for each prefix
- Tests query with non-existent prefix
- Measures query time and memory usage

**Success criteria**:
- Each prefix query finds approximately 2,000 files
- Each query completes in under 1 second
- Non-existent prefix query completes very quickly (under 100ms)

### 6. Concurrent Operations Performance (`test_concurrent_operations_performance`)

**Purpose**: Verify database can handle multiple concurrent operations efficiently.

**What it tests**:
- Creates 5,000 files in database
- Runs three concurrent tasks:
  - Streaming all files
  - Multiple directory listings
  - Multiple path prefix queries
- Measures total time and memory usage

**Success criteria**:
- All concurrent tasks complete successfully
- Total time is under 15 seconds
- No deadlocks or errors occur

### 7. Database Size and Vacuum Performance (`test_database_size_and_vacuum_performance`)

**Purpose**: Test database maintenance operations and space reclamation.

**What it tests**:
- Creates 10,000 files in database
- Deletes half the files to create fragmentation
- Performs vacuum operation to reclaim space
- Measures database size before and after vacuum

**Success criteria**:
- Vacuum operation completes in under 15 seconds
- Some space is reclaimed (database size decreases)
- Database remains functional after vacuum

## Memory Usage Measurement

The tests include a simplified memory usage measurement using the `ps` command (on Unix systems) or similar approaches. This provides a rough estimate of memory consumption during operations.

**Note**: Memory measurements may not be available on all platforms and should be considered approximate.

## Running the Tests

To run all performance tests:
```bash
cargo test --test database_performance_tests
```

To run a specific test with output:
```bash
cargo test --test database_performance_tests test_large_dataset_creation_performance -- --nocapture
```

## Performance Expectations

Based on the test results, you can expect:

- **File Creation**: ~650-750 microseconds per file
- **Directory Listing**: 1-2 milliseconds for typical directories
- **Batch Cleanup**: ~140-280 milliseconds for 5,000 files
- **Streaming**: ~110-150 milliseconds to process 10,000 files
- **Path Prefix Queries**: 20-30 milliseconds for 2,000 files
- **Database Vacuum**: 45-100 milliseconds for 10,000 file database

## Troubleshooting

If tests fail:

1. **Timeout failures**: The system may be slower than expected. Consider:
   - Running on faster hardware
   - Closing other applications
   - Checking disk I/O performance

2. **Memory measurement failures**: Memory usage measurement may not work on all platforms. The tests will continue with 0 KB reported.

3. **Concurrent operation failures**: May indicate database locking issues or resource contention.

## Integration with CI/CD

These tests are designed to:
- Catch performance regressions in database operations
- Verify memory usage remains bounded
- Ensure the system scales to realistic dataset sizes
- Validate that optimizations actually improve performance

Consider running these tests:
- On every major release
- When database-related code changes
- On different hardware configurations
- With different dataset sizes for stress testing