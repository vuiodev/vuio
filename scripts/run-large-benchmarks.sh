#!/bin/bash

# Large Dataset Benchmark Runner
# This script runs the large dataset benchmarks with proper configuration

set -e

echo "=== Large Dataset Benchmark Runner ==="
echo "Warning: These benchmarks will create large datasets and may take hours to complete."
echo "Ensure you have sufficient disk space (5-10 GB) and time available."
echo ""

# Check if user wants to continue
read -p "Do you want to continue? (y/N): " -n 1 -r
echo
if [[ ! $REPLY =~ ^[Yy]$ ]]; then
    echo "Benchmark cancelled."
    exit 0
fi

# Set environment variables for better performance
export RUST_LOG=info
export SQLX_OFFLINE=true

# Create results directory
RESULTS_DIR="benchmark_results_$(date +%Y%m%d_%H%M%S)"
mkdir -p "$RESULTS_DIR"

echo "Results will be saved to: $RESULTS_DIR"
echo ""

# Function to run a benchmark and save results
run_benchmark() {
    local test_name="$1"
    local output_file="$RESULTS_DIR/${test_name}.log"
    
    echo "Running benchmark: $test_name"
    echo "Output file: $output_file"
    
    # Run with release optimizations for realistic performance
    if cargo test --release --test large_dataset_benchmarks "$test_name" -- --ignored --nocapture > "$output_file" 2>&1; then
        echo "✓ $test_name completed successfully"
        
        # Extract key metrics from output
        echo "Key metrics:"
        grep -E "(Total duration|Throughput|Peak memory|Files processed)" "$output_file" | head -5 || true
        echo ""
    else
        echo "✗ $test_name failed - check $output_file for details"
        echo ""
    fi
}

# Run individual benchmarks
echo "Starting large dataset benchmarks..."
echo "Note: Each benchmark may take 30+ minutes to complete."
echo ""

# Benchmark 1: Million file creation
run_benchmark "benchmark_million_file_creation"

# Benchmark 2: Million file streaming  
run_benchmark "benchmark_million_file_streaming"

# Benchmark 3: Database-native cleanup
run_benchmark "benchmark_database_native_cleanup_million_files"

# Benchmark 4: Directory operations
run_benchmark "benchmark_directory_operations_million_files"

# Benchmark 5: Memory bounded operations
run_benchmark "benchmark_memory_bounded_operations"

# Benchmark 6: Database maintenance
run_benchmark "benchmark_database_maintenance_million_files"

echo "=== Benchmark Suite Complete ==="
echo "Results saved in: $RESULTS_DIR"
echo ""

# Generate summary report
SUMMARY_FILE="$RESULTS_DIR/summary.txt"
echo "Large Dataset Benchmark Summary" > "$SUMMARY_FILE"
echo "Generated: $(date)" >> "$SUMMARY_FILE"
echo "System: $(uname -a)" >> "$SUMMARY_FILE"
echo "" >> "$SUMMARY_FILE"

for log_file in "$RESULTS_DIR"/*.log; do
    if [[ -f "$log_file" ]]; then
        benchmark_name=$(basename "$log_file" .log)
        echo "=== $benchmark_name ===" >> "$SUMMARY_FILE"
        
        # Extract key performance metrics
        grep -E "(Total duration|Throughput|Peak memory|Files processed|Database size)" "$log_file" >> "$SUMMARY_FILE" 2>/dev/null || echo "No metrics found" >> "$SUMMARY_FILE"
        echo "" >> "$SUMMARY_FILE"
    fi
done

echo "Summary report generated: $SUMMARY_FILE"
echo ""
echo "To view detailed results:"
echo "  cat $RESULTS_DIR/summary.txt"
echo "  ls $RESULTS_DIR/"