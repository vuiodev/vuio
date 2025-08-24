#!/bin/bash

# SSDP announcement script for macOS Docker
HOST_IP="192.168.1.126"
PORT="8080"
UUID="b50a761a-01e9-4698-9738-271260324a69"

# Send SSDP NOTIFY messages
send_ssdp() {
    local NT="$1"
    local USN="$2"
    
    SSDP_MSG="NOTIFY * HTTP/1.1\r\n"
    SSDP_MSG+="HOST: 239.255.255.250:1900\r\n"
    SSDP_MSG+="CACHE-CONTROL: max-age=1800\r\n"
    SSDP_MSG+="LOCATION: http://${HOST_IP}:${PORT}/device.xml\r\n"
    SSDP_MSG+="NT: ${NT}\r\n"
    SSDP_MSG+="NTS: ssdp:alive\r\n"
    SSDP_MSG+="USN: ${USN}\r\n"
    SSDP_MSG+="SERVER: VuIO/1.0 UPnP/1.0\r\n"
    SSDP_MSG+="\r\n"
    
    echo -e "$SSDP_MSG" | nc -u 239.255.255.250 1900
}

# Send announcements
send_ssdp "upnp:rootdevice" "uuid:${UUID}::upnp:rootdevice"
send_ssdp "urn:schemas-upnp-org:device:MediaServer:1" "uuid:${UUID}::urn:schemas-upnp-org:device:MediaServer:1"
send_ssdp "urn:schemas-upnp-org:service:ContentDirectory:1" "uuid:${UUID}::urn:schemas-upnp-org:service:ContentDirectory:1"

echo "SSDP announcements sent for ${HOST_IP}:${PORT}"