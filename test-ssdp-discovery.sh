#!/bin/bash

# Test script for SSDP discovery troubleshooting
# This script helps verify if DLNA/SSDP multicast is working properly

echo "=== DLNA/SSDP Discovery Test ==="
echo "Testing multicast connectivity for VuIO DLNA server"
echo ""

# Check if running as root (might affect networking)
if [ "$EUID" -eq 0 ]; then
    echo "⚠️  Running as root - this might affect network detection"
else
    echo "✓ Running as non-root user"
fi
echo ""

# Check network interfaces
echo "📡 Network interfaces:"
if command -v ip >/dev/null 2>&1; then
    ip addr show | grep -E "(^[0-9]+:|inet )" | grep -v "127.0.0.1" | head -10
else
    ifconfig | grep -E "(^[a-z]|inet )" | grep -v "127.0.0.1" | head -10
fi
echo ""

# Check default route
echo "🛣️  Default route:"
if command -v ip >/dev/null 2>&1; then
    ip route show default
else
    route -n | grep "^0.0.0.0"
fi
echo ""

# Test SSDP discovery by sending M-SEARCH request
echo "🔍 Testing SSDP M-SEARCH discovery..."
echo "Sending SSDP discovery request to multicast group..."

# Create M-SEARCH request
MSEARCH_REQUEST="M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\nMX: 3\r\nST: ssdp:all\r\n\r\n"

# Send M-SEARCH and listen for responses
if command -v nc >/dev/null 2>&1; then
    echo "Using netcat to send M-SEARCH request..."
    timeout 5s sh -c "echo -e '$MSEARCH_REQUEST' | nc -u 239.255.255.250 1900" &
    
    # Listen for SSDP responses
    echo "Listening for SSDP responses (5 seconds)..."
    timeout 5s nc -u -l 1900 2>/dev/null | head -20 || echo "No responses received or netcat listen failed"
else
    echo "❌ netcat (nc) not available - cannot test SSDP discovery"
fi
echo ""

# Check if SSDP port is available
echo "🔌 Checking SSDP port availability:"
if command -v ss >/dev/null 2>&1; then
    ss -ulnp | grep ":1900" | head -5 || echo "No processes listening on port 1900"
elif command -v netstat >/dev/null 2>&1; then
    netstat -ulnp 2>/dev/null | grep ":1900" | head -5 || echo "No processes listening on port 1900"
else
    echo "Neither ss nor netstat available - cannot check port 1900"
fi
echo ""

# Check for VuIO container
echo "🐳 Checking VuIO container status:"
if command -v docker >/dev/null 2>&1; then
    docker ps | grep vuio || echo "No VuIO containers running"
    echo ""
    
    # If container is running, check its logs for SSDP messages
    CONTAINER_ID=$(docker ps -q --filter "name=vuio")
    if [ -n "$CONTAINER_ID" ]; then
        echo "📋 Recent VuIO SSDP logs:"
        docker logs --tail 10 "$CONTAINER_ID" 2>/dev/null | grep -E "(SSDP|multicast|discovery)" || echo "No recent SSDP logs found"
    fi
else
    echo "Docker not available"
fi
echo ""

# Check firewall status
echo "🔥 Firewall status:"
if command -v ufw >/dev/null 2>&1; then
    ufw status 2>/dev/null || echo "UFW status unknown"
elif command -v firewall-cmd >/dev/null 2>&1; then
    firewall-cmd --state 2>/dev/null || echo "firewalld status unknown"
elif command -v iptables >/dev/null 2>&1; then
    iptables -L -n 2>/dev/null | head -5 || echo "iptables status unknown"
else
    echo "No common firewall tools detected"
fi
echo ""

echo "=== Test Complete ==="
echo ""
echo "💡 Troubleshooting tips:"
echo "1. Ensure VuIO container is running with 'network_mode: host'"
echo "2. Check that port 1900 or 1902 is not blocked by firewall"
echo "3. Verify your TV is on the same network (192.168.1.x)"
echo "4. Try restarting your TV's network connection"
echo "5. Some routers block multicast - check router settings"
echo ""
echo "🔧 Quick fixes to try:"
echo "1. Restart VuIO container: docker-compose restart"
echo "2. Check TV network settings and refresh DLNA device list"
echo "3. Temporarily disable firewall: sudo ufw disable (re-enable after testing)"
echo "4. Check router multicast/IGMP settings"