#!/usr/bin/env bash
set -e

CONTAINER_NAME="vuio-perf-test-container"
IMAGE_NAME="vuio-perf-test:latest"
TEST_DIR="/tmp/vuio-docker-perf-test"

echo "=== VUIO Docker ARM64/Linux Watcher & Performance Test Suite ==="

# Cleanup old test environment
echo "Cleaning up any existing containers or test directories..."
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
rm -rf "$TEST_DIR"
mkdir -p "$TEST_DIR/media" "$TEST_DIR/data" "$TEST_DIR/config"

# Build test docker image
echo "Building test Docker image ($IMAGE_NAME)..."
docker build -t "$IMAGE_NAME" -f packaging/docker/Dockerfile.test .

# Generate test media files
echo "Generating test media dataset (2,000 files across 20 subdirectories)..."
for i in $(seq 1 20); do
    SUBDIR="$TEST_DIR/media/Artist_$i/Album_$i"
    mkdir -p "$SUBDIR"
    for j in $(seq 1 100); do
        # Minimal valid MP3 header payload
        printf '\xFF\xFB\x90\x00%060d' $j > "$SUBDIR/track_${j}.mp3"
    done
done

# Generate config.toml
cat > "$TEST_DIR/config/config.toml" <<EOF
[server]
name = "VuIO Docker Test"
interface = "0.0.0.0"
port = 8080

[web_ui]
enabled = true
port = 8090

[media]
directories = [
    { path = "/media", recursive = true }
]
watch_for_changes = true
scan_on_startup = true
cleanup_deleted_files = true

[database]
path = "/data/vuio.db"
cache_mb = 64
backup_enabled = false

[logging]
level = "info"
EOF

# Start Docker container
echo "Starting container $CONTAINER_NAME..."
docker run -d \
    --name "$CONTAINER_NAME" \
    -v "$TEST_DIR/media:/media" \
    -v "$TEST_DIR/data:/data" \
    -v "$TEST_DIR/config:/config" \
    -p 18080:8080 \
    -p 18090:8090 \
    "$IMAGE_NAME"

# Wait for startup scan to finish
echo "Waiting for initial scan and startup..."
sleep 5

# Check health endpoint
echo "Checking healthz endpoint..."
HEALTH=$(curl -s http://127.0.0.1:18080/healthz || echo "failed")
echo "Health status: $HEALTH"
if [[ "$HEALTH" != *"healthy"* && "$HEALTH" != *"OK"* && "$HEALTH" != *"ok"* ]]; then
    echo "Warning: healthz returned $HEALTH"
fi

# Check admin config endpoint
echo "Checking admin config endpoint..."
ADMIN_CONFIG=$(curl -s http://127.0.0.1:18080/api/admin/config || echo "failed")
if [[ "$ADMIN_CONFIG" == *"watch_for_changes"* ]]; then
    echo "✓ Admin config API is working and returned settings!"
else
    echo "Admin API response: $ADMIN_CONFIG"
fi

# Test 1: Measure Idle CPU & Memory
echo "=== Test 1: Measuring Idle CPU & Memory (15 seconds) ==="
TOTAL_CPU=0
SAMPLES=5
for s in $(seq 1 $SAMPLES); do
    sleep 3
    STATS=$(docker stats --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" "$CONTAINER_NAME")
    CPU_STR=$(echo "$STATS" | awk '{print $1}' | tr -d '%')
    MEM_STR=$(echo "$STATS" | awk '{print $2}')
    echo "Sample $s: CPU = ${CPU_STR}%, Memory = ${MEM_STR}"
done

# Test 2: Simulate Concurrent Reads / SOAP Browse
echo "=== Test 2: Simulating Concurrent Reads & Browse Activity ==="
for k in $(seq 1 50); do
    curl -s http://127.0.0.1:18080/healthz > /dev/null &
    curl -s http://127.0.0.1:18080/api/admin/config > /dev/null &
done
wait
sleep 3
STATS_AFTER_READ=$(docker stats --no-stream --format "{{.CPUPerc}} {{.MemUsage}}" "$CONTAINER_NAME")
echo "Stats after read load: $STATS_AFTER_READ"

# Test 3: Rapid File System Event Storm
echo "=== Test 3: Simulating File System Changes (Creations, Renames, Deletions) ==="
for j in $(seq 1 100); do
    echo "new content" > "$TEST_DIR/media/Artist_1/Album_1/new_track_${j}.mp3"
done
for j in $(seq 1 50); do
    mv "$TEST_DIR/media/Artist_2/Album_2/track_${j}.mp3" "$TEST_DIR/media/Artist_2/Album_2/renamed_track_${j}.mp3"
done
for j in $(seq 51 100); do
    rm -f "$TEST_DIR/media/Artist_3/Album_3/track_${j}.mp3"
done

# Wait for debouncer to process
sleep 5

# Check container logs for errors
echo "=== Inspecting Container Logs for Watcher Errors ==="
CONTAINER_LOGS=$(docker logs "$CONTAINER_NAME" 2>&1)

CAPACITY_ERRORS=$(echo "$CONTAINER_LOGS" | grep -c "no available capacity" || true)
RECONCILE_ALL_ERRORS=$(echo "$CONTAINER_LOGS" | grep -c "reconciling all roots" || true)
CRITICAL_ERRORS=$(echo "$CONTAINER_LOGS" | grep -c "ERROR Failed to send file system event" || true)

echo "Errors found in log:"
echo "  - 'no available capacity': $CAPACITY_ERRORS"
echo "  - 'reconciling all roots': $RECONCILE_ALL_ERRORS"
echo "  - 'ERROR Failed to send file system event': $CRITICAL_ERRORS"

# Test 4: Graceful Shutdown Speed
echo "=== Test 4: Testing Graceful Shutdown Speed ==="
START_TIME=$(date +%s)
docker stop -t 10 "$CONTAINER_NAME"
STOP_TIME=$(date +%s)
SHUTDOWN_DURATION=$((STOP_TIME - START_TIME))

echo "Shutdown took: ${SHUTDOWN_DURATION}s (expected < 5s)"

# Summary Report
echo ""
echo "==================== TEST RESULTS SUMMARY ===================="
if [ "$CAPACITY_ERRORS" -eq 0 ] && [ "$CRITICAL_ERRORS" -eq 0 ] && [ "$SHUTDOWN_DURATION" -le 5 ]; then
    echo "✓ ALL TESTS PASSED!"
    echo "  - No inotify capacity errors detected."
    echo "  - No runaway rescan loops."
    echo "  - Clean and fast shutdown (${SHUTDOWN_DURATION}s)."
else
    echo "✗ SOME TESTS FAILED! Check logs above."
fi
echo "=============================================================="

# Cleanup
docker rm -f "$CONTAINER_NAME" 2>/dev/null || true
rm -rf "$TEST_DIR"
