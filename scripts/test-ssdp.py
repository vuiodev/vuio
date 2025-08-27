#!/usr/bin/env python3
"""
Simple SSDP test script for macOS DLNA troubleshooting
Tests multicast sending and receiving on the local network
"""

import socket
import struct
import time
import threading
import sys
from datetime import datetime

SSDP_ADDR = "239.255.255.250"
SSDP_PORT = 1900

def get_local_ip():
    """Get the local IP address"""
    try:
        # Connect to a remote address to determine local IP
        s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        s.connect(("8.8.8.8", 80))
        local_ip = s.getsockname()[0]
        s.close()
        return local_ip
    except:
        return "127.0.0.1"

def send_msearch_and_listen():
    """Send M-SEARCH and listen for responses on the same socket"""
    local_ip = get_local_ip()
    responses = []
    
    # Create M-SEARCH message
    msearch_msg = (
        "M-SEARCH * HTTP/1.1\r\n"
        f"HOST: {SSDP_ADDR}:{SSDP_PORT}\r\n"
        "MAN: \"ssdp:discover\"\r\n"
        "ST: upnp:rootdevice\r\n"
        "MX: 3\r\n"
        "\r\n"
    )
    
    print(f"🔍 Sending M-SEARCH from {local_ip}...")
    print(f"📡 Target: {SSDP_ADDR}:{SSDP_PORT}")
    
    try:
        # Create socket
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        # Set TTL for multicast
        ttl = struct.pack('b', 4)
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, ttl)
        
        # Bind to local interface with a specific port for responses
        sock.bind((local_ip, 0))
        local_port = sock.getsockname()[1]
        print(f"📡 Bound to {local_ip}:{local_port} for responses")
        
        # Send M-SEARCH
        sock.sendto(msearch_msg.encode(), (SSDP_ADDR, SSDP_PORT))
        print("✅ M-SEARCH sent successfully")
        
        # Listen for responses
        print(f"👂 Listening for responses for 10 seconds...")
        sock.settimeout(1.0)
        
        start_time = time.time()
        while time.time() - start_time < 10:
            try:
                data, addr = sock.recvfrom(2048)
                response = data.decode('utf-8', errors='ignore')
                
                if "HTTP/1.1 200 OK" in response and "LOCATION:" in response:
                    print(f"📨 Response from {addr[0]}:")
                    
                    # Extract key information
                    lines = response.split('\r\n')
                    for line in lines:
                        if line.startswith(('LOCATION:', 'SERVER:', 'ST:', 'USN:')):
                            print(f"   {line}")
                    
                    responses.append((addr, response))
                    print()
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"⚠️  Error receiving data: {e}")
        
        sock.close()
        return responses
        
    except Exception as e:
        print(f"❌ Failed to send M-SEARCH: {e}")
        return []

def listen_for_responses(duration=10):
    """Listen for SSDP responses"""
    local_ip = get_local_ip()
    responses = []
    
    print(f"👂 Listening for responses on {local_ip} for {duration} seconds...")
    
    try:
        # Create socket for receiving responses (not on port 1900)
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        # Bind to a random available port to receive unicast responses
        sock.bind((local_ip, 0))
        bound_port = sock.getsockname()[1]
        print(f"📡 Listening on {local_ip}:{bound_port} for unicast responses...")
        sock.settimeout(1.0)  # 1 second timeout for non-blocking
        
        start_time = time.time()
        
        while time.time() - start_time < duration:
            try:
                data, addr = sock.recvfrom(2048)
                response = data.decode('utf-8', errors='ignore')
                
                if "HTTP/1.1 200 OK" in response and "LOCATION:" in response:
                    print(f"📨 Response from {addr[0]}:")
                    
                    # Extract key information
                    lines = response.split('\r\n')
                    for line in lines:
                        if line.startswith(('LOCATION:', 'SERVER:', 'ST:', 'USN:')):
                            print(f"   {line}")
                    
                    responses.append((addr, response))
                    print()
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"⚠️  Error receiving data: {e}")
        
        sock.close()
        
    except Exception as e:
        print(f"❌ Failed to listen for responses: {e}")
    
    return responses

def test_multicast_join():
    """Test if we can join the SSDP multicast group"""
    local_ip = get_local_ip()
    
    print(f"🔗 Testing multicast join on {local_ip}...")
    
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
        sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        
        # Try to bind to SSDP port (may fail without sudo)
        try:
            sock.bind(('', SSDP_PORT))
            print(f"✅ Successfully bound to port {SSDP_PORT}")
            can_bind_1900 = True
        except:
            print(f"⚠️  Cannot bind to port {SSDP_PORT} (may need sudo)")
            sock.bind(('', 0))  # Bind to any available port
            can_bind_1900 = False
        
        # Join multicast group
        mreq = struct.pack("4s4s", 
                          socket.inet_aton(SSDP_ADDR),
                          socket.inet_aton(local_ip))
        
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
        print(f"✅ Successfully joined multicast group {SSDP_ADDR}")
        
        sock.close()
        return can_bind_1900
        
    except Exception as e:
        print(f"❌ Failed to join multicast group: {e}")
        return False

def check_network_interfaces():
    """Check network interface configuration"""
    import subprocess
    
    print("🌐 Checking network interfaces...")
    
    try:
        # Get interface information
        result = subprocess.run(['ifconfig'], capture_output=True, text=True)
        
        interfaces = {}
        current_iface = None
        
        for line in result.stdout.split('\n'):
            if line and not line.startswith('\t') and ':' in line:
                # New interface
                current_iface = line.split(':')[0]
                interfaces[current_iface] = {'up': False, 'multicast': False, 'ip': None}
                
                if 'UP' in line:
                    interfaces[current_iface]['up'] = True
                if 'MULTICAST' in line:
                    interfaces[current_iface]['multicast'] = True
                    
            elif line.strip().startswith('inet ') and current_iface:
                # IP address
                parts = line.strip().split()
                if len(parts) >= 2 and not '127.0.0.1' in parts[1]:
                    interfaces[current_iface]['ip'] = parts[1]
        
        # Show relevant interfaces
        for name, info in interfaces.items():
            if name.startswith('lo'):
                continue  # Skip loopback
                
            status = "🟢" if info['up'] else "🔴"
            multicast = "📡" if info['multicast'] else "❌"
            ip = info['ip'] or "no IP"
            
            print(f"  {status} {name}: {ip} {multicast}")
            
            if info['up'] and info['multicast'] and info['ip']:
                print(f"    ✅ Good for DLNA")
            elif not info['up']:
                print(f"    ⚠️  Interface is down")
            elif not info['multicast']:
                print(f"    ⚠️  No multicast support")
            elif not info['ip']:
                print(f"    ⚠️  No IP address")
        
    except Exception as e:
        print(f"❌ Failed to check interfaces: {e}")

def main():
    print("🧪 SSDP/DLNA Network Test for macOS")
    print("=" * 40)
    print(f"⏰ {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print()
    
    # Check network interfaces
    check_network_interfaces()
    print()
    
    # Test multicast capabilities
    can_bind_1900 = test_multicast_join()
    print()
    
    # Send M-SEARCH and listen for responses
    print()
    responses = send_msearch_and_listen()
    
    if responses:
        print(f"🎉 Found {len(responses)} DLNA server(s)!")
    else:
        print("😞 No DLNA servers responded")
        print()
        print("💡 Troubleshooting tips:")
        print("   • Make sure your DLNA server is running")
        print("   • Check that server and client are on same network")
        print("   • Verify firewall settings allow multicast")
        print("   • Try running this script with sudo")
        if not can_bind_1900:
            print("   • Port 1900 binding failed - server may need sudo")
    
    print()
    print("🔧 To fix common issues:")
    print("   1. Run: ./scripts/fix-macos-dlna.sh")
    print("   2. Check macOS firewall settings")
    print("   3. Ensure TV and Mac are on same network")
    print("   4. Try running DLNA server with sudo")

if __name__ == "__main__":
    main()