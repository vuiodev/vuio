# Database Performance Tests

This document describes the database performance tests implemented in `database_performance_tests.rs` and the large dataset benchmarks in `large_dataset_benchmarks.rs`.

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

## Large Dataset Benchmarks

The `large_dataset_benchmarks.rs` file contains comprehensive benchmarks for testing with 1,000,000+ media files. These benchmarks are designed to:

- **Verify scalability**: Ensure database operations work correctly with million-file datasets
- **Measure optimization impact**: Compare performance before and after database optimizations
- **Validate memory bounds**: Confirm memory usage remains bounded during large operations
- **Test real-world scenarios**: Simulate actual usage patterns with large media libraries

### Large Dataset Test Cases

1. **Million File Creation Benchmark** (`benchmark_million_file_creation`)
   - Creates 1,000,000 media files with realistic metadata
   - Measures creation time and memory usage
   - Verifies database integrity with large datasets

2. **Million File Streaming Benchmark** (`benchmark_million_file_streaming`)
   - Tests streaming performance with 1,000,000 files
   - Monitors memory usage during streaming
   - Validates that streaming remains memory-bounded

3. **Database-Native Cleanup Benchmark** (`benchmark_database_native_cleanup_million_files`)
   - Tests optimized cleanup with 1,000,000 files
   - Compares performance of database-native vs. application-level cleanup
   - Measures memory efficiency of large cleanup operations

4. **Directory Operations Benchmark** (`benchmark_directory_operations_million_files`)
   - Tests directory listing and path prefix queries with million files
   - Validates performance of hierarchical directory structures
   - Ensures query times remain reasonable at scale

5. **Memory Bounded Operations Benchmark** (`benchmark_memory_bounded_operations`)
   - Verifies memory usage remains bounded during large operations
   - Tests streaming and cleanup memory efficiency
   - Validates that operations don't cause memory leaks

6. **Database Maintenance Benchmark** (`benchmark_database_maintenance_million_files`)
   - Tests vacuum and maintenance operations with million files
   - Measures space reclamation efficiency
   - Validates database integrity after maintenance

### Running Large Dataset Benchmarks

**Warning**: These benchmarks create very large datasets and may take significant time and disk space.

```bash
# Run all large dataset benchmarks (requires --ignored flag)
cargo test --test large_dataset_benchmarks -- --ignored --nocapture

# Run a specific benchmark
cargo test --test large_dataset_benchmarks benchmark_million_file_creation -- --ignored --nocapture

# Run with release optimizations for more realistic performance
cargo test --release --test large_dataset_benchmarks -- --ignored --nocapture
```

### Performance Expectations (Million Files)

Based on optimized implementations:

- **File Creation**: ~300-500 files/second (30-50 minutes for 1M files)
- **Streaming**: ~8,000-15,000 files/second (1-2 minutes for 1M files)
- **Database-Native Cleanup**: ~50,000-100,000 files/second (10-20 seconds for 1M files)
- **Directory Listing**: 1-5 seconds for directories with 10k+ files
- **Path Prefix Queries**: 2-10 seconds for 10k+ matching files
- **Database Vacuum**: 2-5 minutes for 1M file database

### System Requirements

For million-file benchmarks:
- **Disk Space**: 5-10 GB for database and temporary files
- **Memory**: 4-8 GB RAM recommended
- **Time**: 1-3 hours for complete benchmark suite
- **Platform**: Unix systems preferred for memory measurement

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

**Note**: Large dataset benchmarks should be run separately from regular CI due to their resource requirements and execution time.