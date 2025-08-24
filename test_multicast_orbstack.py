#!/usr/bin/env python3
"""
Test multicast functionality with OrbStack to demonstrate the limitations
"""

import socket
import struct
import time
import threading
from datetime import datetime

def test_multicast_send():
    """Test sending multicast from host to see if container receives it"""
    print("🔍 Testing multicast send from host...")
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    # Set TTL for multicast
    ttl = struct.pack('b', 1)
    sock.setsockopt(socket.IPPROTO_IP, socket.IP_MULTICAST_TTL, ttl)
    
    message = f"MULTICAST TEST from host at {datetime.now()}"
    
    try:
        # Send to SSDP multicast group
        sock.sendto(message.encode(), ('239.255.255.250', 1900))
        print(f"✅ Sent multicast message: {message}")
    except Exception as e:
        print(f"❌ Failed to send multicast: {e}")
    finally:
        sock.close()

def test_multicast_listen():
    """Test listening for multicast on host"""
    print("👂 Testing multicast listen on host...")
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        # Bind to multicast group
        sock.bind(('', 1900))
        
        # Join multicast group
        mreq = socket.inet_aton('239.255.255.250') + socket.inet_aton('0.0.0.0')
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
        
        sock.settimeout(5)
        print("Listening for multicast traffic for 5 seconds...")
        
        start_time = time.time()
        while time.time() - start_time < 5:
            try:
                data, addr = sock.recvfrom(1024)
                message = data.decode('utf-8', errors='ignore')
                print(f"📡 Received from {addr}: {message[:100]}...")
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Error: {e}")
                break
                
    except Exception as e:
        print(f"❌ Failed to listen for multicast: {e}")
    finally:
        sock.close()

def test_ssdp_notify_listen():
    """Listen specifically for SSDP NOTIFY messages"""
    print("📺 Testing SSDP NOTIFY listening (like a TV would)...")
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        sock.bind(('', 1900))
        
        # Join SSDP multicast group
        mreq = socket.inet_aton('239.255.255.250') + socket.inet_aton('0.0.0.0')
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
        
        sock.settimeout(10)
        print("Listening for SSDP NOTIFY messages for 10 seconds...")
        print("(This is how TVs discover DLNA servers)")
        
        start_time = time.time()
        notify_count = 0
        
        while time.time() - start_time < 10:
            try:
                data, addr = sock.recvfrom(2048)
                message = data.decode('utf-8', errors='ignore')
                
                if message.startswith('NOTIFY'):
                    notify_count += 1
                    print(f"📺 NOTIFY #{notify_count} from {addr[0]}:")
                    print("=" * 50)
                    print(message)
                    print("=" * 50)
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Error: {e}")
                break
        
        if notify_count == 0:
            print("❌ No SSDP NOTIFY messages received!")
            print("   This means TVs won't discover the DLNA server.")
        else:
            print(f"✅ Received {notify_count} NOTIFY messages")
                
    except Exception as e:
        print(f"❌ Failed to listen for SSDP NOTIFY: {e}")
    finally:
        sock.close()

def test_network_interfaces():
    """Show network interface information"""
    print("🌐 Network interface information:")
    
    import subprocess
    try:
        # Show host interfaces
        result = subprocess.run(['ifconfig'], capture_output=True, text=True)
        lines = result.stdout.split('\n')
        
        current_interface = None
        for line in lines:
            if line and not line.startswith(' ') and not line.startswith('\t'):
                if ':' in line:
                    current_interface = line.split(':')[0]
                    print(f"\n📡 Interface: {current_interface}")
            elif 'inet ' in line and '127.0.0.1' not in line:
                ip = line.strip().split()[1]
                print(f"   IP: {ip}")
                
    except Exception as e:
        print(f"❌ Failed to get network info: {e}")

if __name__ == "__main__":
    print("🧪 OrbStack DLNA/Multicast Compatibility Test")
    print("=" * 60)
    
    # Show network info
    test_network_interfaces()
    print()
    
    # Test multicast functionality
    test_multicast_send()
    print()
    
    # Start listening in background
    listen_thread = threading.Thread(target=test_multicast_listen, daemon=True)
    listen_thread.start()
    
    time.sleep(1)
    
    # Test SSDP NOTIFY listening (most important for TV discovery)
    test_ssdp_notify_listen()
    
    print("\n🔍 Analysis:")
    print("- If you see NOTIFY messages, OrbStack multicast works")
    print("- If you don't see NOTIFY messages, OrbStack blocks multicast")
    print("- TVs need to receive NOTIFY messages to discover DLNA servers")