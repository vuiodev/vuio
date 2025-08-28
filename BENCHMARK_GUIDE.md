# Large Dataset Performance Benchmarks

This guide explains how to run and interpret the large dataset performance benchmarks for the media server database optimizations.

## Overview

The benchmarks are designed to test database performance with 1,000,000+ media files to verify that optimizations work correctly at scale and memory usage remains bounded during large operations.

## Benchmark Files

- `tests/large_dataset_benchmarks.rs` - Main million-file benchmarks
- `tests/benchmark_validation.rs` - Infrastructure validation with smaller datasets
- `tests/database_performance_tests.rs` - Existing 10k-file performance tests
- `scripts/run-large-benchmarks.ps1` - PowerShell script to run all benchmarks
- `scripts/run-large-benchmarks.sh` - Bash script for Unix systems

## System Requirements

### Minimum Requirements
- **Disk Space**: 5-10 GB free space for database and temporary files
- **Memory**: 4-8 GB RAM recommended
- **Time**: 1-3 hours for complete benchmark suite
- **Platform**: Windows, Linux, or macOS

### Recommended Configuration
- **Disk**: SSD for better I/O performance
- **Memory**: 16+ GB RAM for comfortable operation
- **CPU**: Multi-core processor for better compilation and test performance

## Running Benchmarks

### Quick Validation
First, run the validation tests to ensure everything works:

```bash
# Test benchmark infrastructure with small dataset
cargo test --test benchmark_validation -- --nocapture

# Test existing performance suite
cargo test --test database_performance_tests -- --nocapture
```

### Large Dataset Benchmarks

**Warning**: These benchmarks create very large datasets and take significant time.

#### Individual Benchmarks

```bash
# Run specific benchmark (replace benchmark_name)
cargo test --release --test large_dataset_benchmarks benchmark_million_file_creation -- --ignored --nocapture

# Available benchmarks:
# - benchmark_million_file_creation
# - benchmark_million_file_streaming  
# - benchmark_database_native_cleanup_million_files
# - benchmark_directory_operations_million_files
# - benchmark_memory_bounded_operations
# - benchmark_database_maintenance_million_files
```

#### Complete Benchmark Suite

Using PowerShell (Windows):
```powershell
# Run all benchmarks with automated result collection
.\scripts\run-large-benchmarks.ps1

# Force run without confirmation prompt
.\scripts\run-large-benchmarks.ps1 -Force
```

Using Bash (Unix):
```bash
# Make script executable and run
chmod +x scripts/run-large-benchmarks.sh
./scripts/run-large-benchmarks.sh
```

#### Manual Execution

```bash
# Run all large benchmarks manually
cargo test --release --test large_dataset_benchmarks -- --ignored --nocapture
```

## Benchmark Descriptions

### 1. Million File Creation (`benchmark_million_file_creation`)
- **Purpose**: Test database insertion performance at scale
- **Dataset**: 1,000,000 media files with realistic metadata
- **Metrics**: Creation time, throughput, memory usage, database size
- **Expected Time**: 30-60 minutes

### 2. Million File Streaming (`benchmark_million_file_streaming`)
- **Purpose**: Verify streaming performance and memory efficiency
- **Test**: Stream all 1M files using async iterator
- **Metrics**: Streaming time, memory usage, throughput
- **Expected Time**: 2-5 minutes

### 3. Database-Native Cleanup (`benchmark_database_native_cleanup_million_files`)
- **Purpose**: Test optimized cleanup performance
- **Test**: Remove 30% of files using database-native operations
- **Metrics**: Cleanup time, memory efficiency, files removed
- **Expected Time**: 10-30 seconds

### 4. Directory Operations (`benchmark_directory_operations_million_files`)
- **Purpose**: Test directory listing and path queries at scale
- **Test**: Query hierarchical directory structure
- **Metrics**: Query time, result accuracy, memory usage
- **Expected Time**: 5-15 minutes

### 5. Memory Bounded Operations (`benchmark_memory_bounded_operations`)
- **Purpose**: Verify memory usage remains bounded
- **Test**: Monitor memory during streaming and cleanup
- **Metrics**: Peak memory usage, memory growth patterns
- **Expected Time**: 10-20 minutes

### 6. Database Maintenance (`benchmark_database_maintenance_million_files`)
- **Purpose**: Test vacuum and maintenance operations
- **Test**: Delete files and vacuum database
- **Metrics**: Vacuum time, space reclaimed, integrity
- **Expected Time**: 5-15 minutes

## Performance Expectations

### Optimized Performance Targets

| Operation | Target Performance | Memory Usage |
|-----------|-------------------|--------------|
| File Creation | 300-500 files/sec | Bounded |
| Streaming | 8,000-15,000 files/sec | < 500 MB |
| Database Cleanup | 50,000-100,000 files/sec | < 200 MB |
| Directory Listing | < 5 seconds | < 100 MB |
| Path Prefix Query | < 10 seconds | < 100 MB |
| Database Vacuum | < 5 minutes | Bounded |

### Before vs After Optimizations

The benchmarks help measure the impact of database optimizations:

**Before Optimizations:**
- Cleanup: Load entire database into memory (GBs)
- Directory queries: Fetch all descendants with LIKE queries
- Memory usage: Unbounded growth with dataset size

**After Optimizations:**
- Cleanup: Database-native operations with temporary tables
- Directory queries: Efficient SQL with string manipulation
- Memory usage: Bounded regardless of dataset size

## Interpreting Results

### Success Criteria

1. **Completion**: All benchmarks complete without errors
2. **Performance**: Operations complete within expected time limits
3. **Memory**: Memory usage remains bounded (< 1GB for most operations)
4. **Accuracy**: Correct number of files processed/found
5. **Integrity**: Database remains functional after operations

### Warning Signs

- **Memory Growth**: Continuous memory increase during operations
- **Timeouts**: Operations taking much longer than expected
- **Errors**: Database errors or corruption during benchmarks
- **Incorrect Results**: Wrong number of files found/processed

### Result Files

The benchmark scripts create result directories with:
- Individual benchmark logs (`.log` files)
- Summary report (`summary.txt`)
- Performance metrics and timing data
- Error logs if any benchmarks fail

## Troubleshooting

### Common Issues

1. **Insufficient Disk Space**
   - Solution: Ensure 10+ GB free space
   - Check: `df -h` (Unix) or disk properties (Windows)

2. **Memory Limitations**
   - Solution: Close other applications, increase swap
   - Monitor: Task Manager or `htop`

3. **Long Execution Times**
   - Expected: Million-file operations take time
   - Solution: Run with `--release` flag for optimizations

4. **Platform-Specific Issues**
   - Memory measurement may not work on all platforms
   - Path handling differences between Windows/Unix

### Performance Debugging

If benchmarks are slower than expected:

1. **Check System Resources**
   - CPU usage, memory availability, disk I/O
   - Close unnecessary applications

2. **Verify Optimizations**
   - Use `--release` flag for realistic performance
   - Check that database optimizations are enabled

3. **Monitor Progress**
   - Benchmarks print progress updates
   - Check log files for detailed timing

4. **Compare Results**
   - Run smaller validation tests first
   - Compare with expected performance targets

## Integration with Development

### CI/CD Considerations

- **Separate Pipeline**: Run large benchmarks separately from regular tests
- **Resource Requirements**: Ensure CI environment has sufficient resources
- **Time Limits**: Set appropriate timeout values (hours, not minutes)
- **Artifact Collection**: Save benchmark results as build artifacts

### Performance Regression Detection

- **Baseline Establishment**: Run benchmarks on known-good versions
- **Threshold Monitoring**: Alert on significant performance degradation
- **Trend Analysis**: Track performance over time
- **Optimization Validation**: Verify improvements with before/after comparisons

## Contributing

When adding new benchmarks:

1. **Follow Patterns**: Use existing benchmark structure
2. **Add Documentation**: Update this guide and README files
3. **Test Thoroughly**: Validate with smaller datasets first
4. **Consider Resources**: Be mindful of time and space requirements
5. **Include Metrics**: Measure relevant performance indicators

## Support

For issues with benchmarks:

1. **Check Logs**: Review detailed log files in result directories
2. **Validate Setup**: Run smaller validation tests first
3. **System Requirements**: Verify sufficient resources available
4. **Platform Differences**: Consider OS-specific behavior

The benchmarks are designed to be robust and provide clear feedback about database performance at scale.