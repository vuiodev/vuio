#!/usr/bin/env python3
"""
Test script to listen for SSDP NOTIFY broadcasts from the DLNA server.
This simulates how a TV would discover the server automatically.
"""

import socket
import time
import threading
from datetime import datetime

def listen_for_notify():
    """Listen for SSDP NOTIFY broadcasts on the multicast group"""
    # Create socket for multicast listening
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    sock.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    
    # Bind to the SSDP multicast address and port
    sock.bind(('', 1900))
    
    # Join the multicast group
    mreq = socket.inet_aton('239.255.255.250') + socket.inet_aton('0.0.0.0')
    sock.setsockopt(socket.IPPROTO_IP, socket.IP_ADD_MEMBERSHIP, mreq)
    
    print(f"[{datetime.now().strftime('%H:%M:%S')}] Listening for SSDP NOTIFY broadcasts on 239.255.255.250:1900...")
    print("This simulates how a TV discovers DLNA servers automatically.")
    print("Press Ctrl+C to stop.\n")
    
    try:
        while True:
            try:
                data, addr = sock.recvfrom(2048)
                message = data.decode('utf-8', errors='ignore')
                
                # Only show NOTIFY messages (not M-SEARCH)
                if message.startswith('NOTIFY'):
                    timestamp = datetime.now().strftime('%H:%M:%S')
                    print(f"[{timestamp}] 📺 NOTIFY received from {addr[0]}:{addr[1]}")
                    print("=" * 60)
                    print(message)
                    print("=" * 60)
                    print()
                    
                    # Parse key information
                    lines = message.split('\r\n')
                    for line in lines:
                        if line.startswith('LOCATION:'):
                            print(f"🎯 Server Location: {line}")
                        elif line.startswith('NT:'):
                            print(f"📋 Notification Type: {line}")
                        elif line.startswith('USN:'):
                            print(f"🆔 Unique Service Name: {line}")
                    print()
                    
            except socket.timeout:
                continue
            except Exception as e:
                print(f"Error receiving data: {e}")
                
    except KeyboardInterrupt:
        print(f"\n[{datetime.now().strftime('%H:%M:%S')}] Stopping NOTIFY listener...")
    finally:
        sock.close()

def send_msearch_periodically():
    """Send M-SEARCH requests periodically to trigger responses"""
    time.sleep(2)  # Wait a bit before starting
    
    while True:
        try:
            # Send M-SEARCH to trigger server responses
            sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
            sock.settimeout(1)
            
            msearch_msg = (
                "M-SEARCH * HTTP/1.1\r\n"
                "HOST: 239.255.255.250:1900\r\n"
                "MAN: \"ssdp:discover\"\r\n"
                "ST: ssdp:all\r\n"
                "MX: 3\r\n\r\n"
            )
            
            # Send to container's SSDP port
            sock.sendto(msearch_msg.encode(), ('192.168.139.2', 1902))
            print(f"[{datetime.now().strftime('%H:%M:%S')}] 🔍 Sent M-SEARCH to trigger server response")
            sock.close()
            
            time.sleep(30)  # Send every 30 seconds
            
        except Exception as e:
            print(f"Error sending M-SEARCH: {e}")
            time.sleep(30)

if __name__ == "__main__":
    # Start M-SEARCH sender in background
    msearch_thread = threading.Thread(target=send_msearch_periodically, daemon=True)
    msearch_thread.start()
    
    # Listen for NOTIFY broadcasts
    listen_for_notify()