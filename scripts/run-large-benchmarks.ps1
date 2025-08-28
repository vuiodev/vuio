# Large Dataset Benchmark Runner (PowerShell)
# This script runs the large dataset benchmarks with proper configuration

param(
    [switch]$Force = $false
)

Write-Host "=== Large Dataset Benchmark Runner ===" -ForegroundColor Cyan
Write-Host "Warning: These benchmarks will create large datasets and may take hours to complete."
Write-Host "Ensure you have sufficient disk space (5-10 GB) and time available."
Write-Host ""

# Check if user wants to continue
if (-not $Force) {
    $response = Read-Host "Do you want to continue? (y/N)"
    if ($response -notmatch "^[Yy]$") {
        Write-Host "Benchmark cancelled." -ForegroundColor Yellow
        exit 0
    }
}

# Set environment variables for better performance
$env:RUST_LOG = "info"
$env:SQLX_OFFLINE = "true"

# Create results directory
$timestamp = Get-Date -Format "yyyyMMdd_HHmmss"
$resultsDir = "benchmark_results_$timestamp"
New-Item -ItemType Directory -Path $resultsDir -Force | Out-Null

Write-Host "Results will be saved to: $resultsDir" -ForegroundColor Green
Write-Host ""

# Function to run a benchmark and save results
function Run-Benchmark {
    param(
        [string]$TestName,
        [string]$ResultsDir
    )
    
    $outputFile = Join-Path $ResultsDir "$TestName.log"
    
    Write-Host "Running benchmark: $TestName" -ForegroundColor Yellow
    Write-Host "Output file: $outputFile"
    
    try {
        # Run with release optimizations for realistic performance
        $output = & cargo test --release --test large_dataset_benchmarks $TestName -- --ignored --nocapture 2>&1
        $output | Out-File -FilePath $outputFile -Encoding UTF8
        
        if ($LASTEXITCODE -eq 0) {
            Write-Host "✓ $TestName completed successfully" -ForegroundColor Green
            
            # Extract key metrics from output
            Write-Host "Key metrics:"
            $metrics = $output | Select-String -Pattern "(Total duration|Throughput|Peak memory|Files processed)" | Select-Object -First 5
            $metrics | ForEach-Object { Write-Host "  $($_.Line)" }
            Write-Host ""
        } else {
            Write-Host "✗ $TestName failed - check $outputFile for details" -ForegroundColor Red
            Write-Host ""
        }
    }
    catch {
        Write-Host "✗ $TestName failed with exception: $($_.Exception.Message)" -ForegroundColor Red
        $_.Exception.Message | Out-File -FilePath $outputFile -Encoding UTF8
        Write-Host ""
    }
}

# Run individual benchmarks
Write-Host "Starting large dataset benchmarks..." -ForegroundColor Cyan
Write-Host "Note: Each benchmark may take 30+ minutes to complete."
Write-Host ""

# Benchmark 1: Million file creation
Run-Benchmark -TestName "benchmark_million_file_creation" -ResultsDir $resultsDir

# Benchmark 2: Million file streaming  
Run-Benchmark -TestName "benchmark_million_file_streaming" -ResultsDir $resultsDir

# Benchmark 3: Database-native cleanup
Run-Benchmark -TestName "benchmark_database_native_cleanup_million_files" -ResultsDir $resultsDir

# Benchmark 4: Directory operations
Run-Benchmark -TestName "benchmark_directory_operations_million_files" -ResultsDir $resultsDir

# Benchmark 5: Memory bounded operations
Run-Benchmark -TestName "benchmark_memory_bounded_operations" -ResultsDir $resultsDir

# Benchmark 6: Database maintenance
Run-Benchmark -TestName "benchmark_database_maintenance_million_files" -ResultsDir $resultsDir

Write-Host "=== Benchmark Suite Complete ===" -ForegroundColor Cyan
Write-Host "Results saved in: $resultsDir" -ForegroundColor Green
Write-Host ""

# Generate summary report
$summaryFile = Join-Path $resultsDir "summary.txt"
$summary = @()
$summary += "Large Dataset Benchmark Summary"
$summary += "Generated: $(Get-Date)"
$summary += "System: $env:COMPUTERNAME - $env:OS"
$summary += ""

Get-ChildItem -Path $resultsDir -Filter "*.log" | ForEach-Object {
    $benchmarkName = $_.BaseName
    $summary += "=== $benchmarkName ==="
    
    # Extract key performance metrics
    $content = Get-Content $_.FullName -ErrorAction SilentlyContinue
    $metrics = $content | Select-String -Pattern "(Total duration|Throughput|Peak memory|Files processed|Database size)"
    
    if ($metrics) {
        $metrics | ForEach-Object { $summary += $_.Line }
    } else {
        $summary += "No metrics found"
    }
    $summary += ""
}

$summary | Out-File -FilePath $summaryFile -Encoding UTF8

Write-Host "Summary report generated: $summaryFile" -ForegroundColor Green
Write-Host ""
Write-Host "To view detailed results:"
Write-Host "  Get-Content $summaryFile"
Write-Host "  Get-ChildItem $resultsDir"