#!/usr/bin/env python3
import socket
import time

def test_ssdp():
    """Test SSDP discovery by sending M-SEARCH to container"""
    sock = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    
    msearch_msg = (
        "M-SEARCH * HTTP/1.1\r\n"
        "HOST: 127.0.0.1:1902\r\n"
        "MAN: \"ssdp:discover\"\r\n"
        "ST: upnp:rootdevice\r\n"
        "MX: 3\r\n\r\n"
    )
    
    try:
        # Send to localhost (Docker port forwarding)
        sock.sendto(msearch_msg.encode(), ('127.0.0.1', 1902))
        print("Sent M-SEARCH to localhost:1902 (Docker port forwarding)")
        
        # Listen for response
        sock.settimeout(5)
        try:
            data, addr = sock.recvfrom(2048)
            response = data.decode('utf-8', errors='ignore')
            print(f"\n✅ SUCCESS! Response from {addr}:")
            print(response)
            
            # Check for correct LOCATION
            if 'LOCATION:' in response:
                for line in response.split('\r\n'):
                    if 'LOCATION:' in line:
                        if '192.168.1.126' in line:
                            print(f"✅ LOCATION uses host IP: {line}")
                        else:
                            print(f"⚠️  LOCATION IP: {line}")
                        
        except socket.timeout:
            print("❌ No response received within 5 seconds")
            
    except Exception as e:
        print(f"❌ Error: {e}")
    finally:
        sock.close()

if __name__ == "__main__":
    test_ssdp()