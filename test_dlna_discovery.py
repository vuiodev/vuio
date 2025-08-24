#!/usr/bin/env python3
"""
Test DLNA discovery to see if OrbStack properly handles multicast
"""

import socket
import time
import threading
from datetime import datetime

def listen_for_ssdp_notify():
    """Listen for SSDP NOTIFY broadcasts that TVs use to discover servers"""
    print("📺 Listening for SSDP NOTIFY broadcasts (how TVs discover DLNA servers)...")
    
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    try:
        # Bind to any available port and join multicast group
        sock.bind(('', 0))  # Use any available port
        local_port = sock.getsockname()[1]
        print(f"Listening on port {local_port}")
        
        # Join SSDP multicast group
        mreq = socket.inet_aton('239.255.255.250') + socket.inet_aton('0.0.0.0')
        sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
        
        sock.settimeout(15)
        print("Waiting 15 seconds for SSDP NOTIFY messages...")
        
        start_time = time.time()
        notify_count = 0
        
        while time.time() - start_time < 15:
            try:
                data, addr = sock.recvfrom(2048)
                message = data.decode('utf-8', errors='ignore')
                
                if 'NOTIFY' in message and 'ssdp:alive' in message:
                    notify_count += 1
                    timestamp = datetime.now().strftime('%H:%M:%S')
                    print(f"\n[{timestamp}] 📡 SSDP NOTIFY #{notify_count} from {addr[0]}:")
                    
                    # Extract key info
                    lines = message.split('\r\n')
                    for line in lines:
                        if line.startswith('LOCATION:'):
                            print(f"  🎯 {line}")
                        elif line.startswith('NT:'):
                            print(f"  📋 {line}")
                        elif line.startswith('SERVER:'):
                            print(f"  🖥️  {line}")
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Error: {e}")
                break
        
        print(f"\n📊 Results after 15 seconds:")
        if notify_count == 0:
            print("❌ No SSDP NOTIFY messages received!")
            print("   This means:")
            print("   - TVs won't automatically discover the DLNA server")
            print("   - OrbStack is likely blocking multicast traffic")
            print("   - You'll need to use native Docker or Linux for TV discovery")
        else:
            print(f"✅ Received {notify_count} NOTIFY messages")
            print("   This means TVs should be able to discover DLNA servers")
                
    except Exception as e:
        print(f"❌ Failed to listen for SSDP NOTIFY: {e}")
    finally:
        sock.close()

def send_msearch_to_trigger_responses():
    """Send M-SEARCH to trigger server responses"""
    time.sleep(2)  # Wait a bit before starting
    
    for i in range(3):
        try:
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.settimeout(1)
            
            msearch_msg = (
                "M-SEARCH * HTTP/1.1\r\n"
                "HOST: 239.255.255.250:1900\r\n"
                "MAN: \"ssdp:discover\"\r\n"
                "ST: ssdp:all\r\n"
                "MX: 3\r\n\r\n"
            )
            
            # Send to multicast group to trigger any DLNA servers
            sock.sendto(msearch_msg.encode(), ('239.255.255.250', 1900))
            print(f"🔍 Sent M-SEARCH #{i+1} to trigger DLNA server responses")
            sock.close()
            
            time.sleep(5)
            
        except Exception as e:
            print(f"Error sending M-SEARCH: {e}")

if __name__ == "__main__":
    print("🧪 DLNA Discovery Test with OrbStack")
    print("=" * 50)
    print("This test checks if DLNA servers can be discovered on your network.")
    print("TVs use SSDP NOTIFY broadcasts to find DLNA servers automatically.\n")
    
    # Start M-SEARCH sender in background
    msearch_thread = threading.Thread(target=send_msearch_to_trigger_responses, daemon=True)
    msearch_thread.start()
    
    # Listen for NOTIFY broadcasts
    listen_for_ssdp_notify()
    
    print("\n🔍 Conclusion:")
    print("If no NOTIFY messages were received, OrbStack doesn't properly")
    print("bridge multicast traffic, making TV discovery impossible.")
    print("\nAlternatives for DLNA with OrbStack:")
    print("1. Use Docker Desktop instead of OrbStack")
    print("2. Run natively on Linux")
    print("3. Use manual IP entry on TV (if supported)")
    print("4. Use a different container runtime that supports host networking")