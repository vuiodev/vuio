package ssdp

import (
	"context"
	"fmt"
	"log/slog"
	"net"
	"strings"
	"time"

	"golang.org/x/net/ipv4"
	"vuio-go/platform"
	"vuio-go/state"
)

const (
	ssdpMulticastAddr = "239.255.255.250:1900"
	maxDatagramSize   = 8192
)

// Service handles SSDP discovery.
type Service struct {
	state *state.AppState
}

// New creates a new SSDP service.
func New(state *state.AppState) (*Service, error) {
	return &Service{state: state}, nil
}

// Start begins listening for SSDP messages and sending announcements.
func (s *Service) Start(ctx context.Context) {
	slog.Info("Starting SSDP service")

	addr, err := net.ResolveUDPAddr("udp4", ssdpMulticastAddr)
	if err != nil {
		slog.Error("Failed to resolve SSDP multicast address", "error", err)
		return
	}

	conn, err := net.ListenMulticastUDP("udp4", nil, addr)
	if err != nil {
		slog.Error("Failed to listen on SSDP multicast address", "error", err)
		return
	}
	defer conn.Close()

	if err := conn.SetReadBuffer(maxDatagramSize); err != nil {
		slog.Warn("Failed to set SSDP read buffer size", "error", err)
	}

	// Start listener goroutine
	go s.listen(ctx, conn)

	// Start announcer goroutine
	go s.announce(ctx)

	<-ctx.Done()
	slog.Info("Stopping SSDP service")
}

func (s *Service) listen(ctx context.Context, conn *net.UDPConn) {
	packetConn := ipv4.NewPacketConn(conn)
	buffer := make([]byte, maxDatagramSize)

	for {
		select {
		case <-ctx.Done():
			return
		default:
			_ = packetConn.SetReadDeadline(time.Now().Add(1 * time.Second))
			n, _, src, err := packetConn.ReadFrom(buffer)
			if err != nil {
				if netErr, ok := err.(net.Error); ok && netErr.Timeout() {
					continue
				}
				slog.Error("SSDP read error", "error", err)
				return
			}
			s.handleRequest(buffer[:n], src)
		}
	}
}

func (s *Service) handleRequest(data []byte, src net.Addr) {
	req := string(data)
	if !strings.HasPrefix(req, "M-SEARCH") {
		return
	}

	slog.Debug("Received M-SEARCH request", "from", src.String())

	// Check ST (Search Target) header
	st := getHeader(req, "ST")
	if st == "" {
		return
	}

	if st == "ssdp:all" ||
		st == "upnp:rootdevice" ||
		strings.HasPrefix(st, "urn:schemas-upnp-org:device:MediaServer") {

		go s.sendSearchResponse(src)
	}
}

func (s *Service) sendSearchResponse(dest net.Addr) {
	cfg := s.state.GetConfig()
	serverIP, err := platform.GetPrimaryIP()
	if err != nil {
		slog.Error("Could not get primary IP for SSDP response", "error", err)
		return
	}

	usnRoot := fmt.Sprintf("uuid:%s::upnp:rootdevice", cfg.Server.UUID)
	usnServer := fmt.Sprintf("uuid:%s::urn:schemas-upnp-org:device:MediaServer:1", cfg.Server.UUID)
	usnContent := fmt.Sprintf("uuid:%s::urn:schemas-upnp-org:service:ContentDirectory:1", cfg.Server.UUID)

	responses := []string{
		buildResponse(serverIP, cfg.Server.Port, "upnp:rootdevice", usnRoot),
		buildResponse(serverIP, cfg.Server.Port, "urn:schemas-upnp-org:device:MediaServer:1", usnServer),
		buildResponse(serverIP, cfg.Server.Port, "urn:schemas-upnp-org:service:ContentDirectory:1", usnContent),
	}

	conn, err := net.Dial("udp", dest.String())
	if err != nil {
		slog.Error("Failed to dial for SSDP response", "dest", dest.String(), "error", err)
		return
	}
	defer conn.Close()

	for _, res := range responses {
		_, err := conn.Write([]byte(res))
		if err != nil {
			slog.Warn("Failed to send SSDP response", "dest", dest.String(), "error", err)
		}
		time.Sleep(50 * time.Millisecond) // Stagger responses
	}
	slog.Debug("Sent M-SEARCH response", "to", dest.String())
}

func (s *Service) announce(ctx context.Context) {
	ticker := time.NewTicker(time.Duration(s.state.Config.Network.AnnounceIntervalSeconds) * time.Second)
	defer ticker.Stop()

	// Announce on startup
	s.sendAnnouncements()

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			s.sendAnnouncements()
		}
	}
}

func (s *Service) sendAnnouncements() {
	slog.Info("Sending SSDP NOTIFY announcements")
	cfg := s.state.GetConfig()
	serverIP, err := platform.GetPrimaryIP()
	if err != nil {
		slog.Error("Could not get primary IP for SSDP announcement", "error", err)
		return
	}

	usnRoot := fmt.Sprintf("uuid:%s::upnp:rootdevice", cfg.Server.UUID)
	usnServer := fmt.Sprintf("uuid:%s::urn:schemas-upnp-org:device:MediaServer:1", cfg.Server.UUID)
	usnContent := fmt.Sprintf("uuid:%s::urn:schemas-upnp-org:service:ContentDirectory:1", cfg.Server.UUID)

	announcements := []string{
		buildNotify(serverIP, cfg.Server.Port, "upnp:rootdevice", usnRoot),
		buildNotify(serverIP, cfg.Server.Port, "urn:schemas-upnp-org:device:MediaServer:1", usnServer),
		buildNotify(serverIP, cfg.Server.Port, "urn:schemas-upnp-org:service:ContentDirectory:1", usnContent),
	}

	conn, err := net.Dial("udp", ssdpMulticastAddr)
	if err != nil {
		slog.Error("Failed to dial for SSDP announcement", "error", err)
		return
	}
	defer conn.Close()

	for _, ann := range announcements {
		_, err := conn.Write([]byte(ann))
		if err != nil {
			slog.Warn("Failed to send SSDP announcement", "error", err)
		}
		time.Sleep(100 * time.Millisecond) // Stagger announcements
	}
}

func getHeader(req, header string) string {
	lines := strings.Split(req, "\r\n")
	for _, line := range lines {
		if strings.HasPrefix(strings.ToUpper(line), strings.ToUpper(header)+":") {
			parts := strings.SplitN(line, ":", 2)
			if len(parts) == 2 {
				return strings.TrimSpace(parts[1])
			}
		}
	}
	return ""
}

func buildResponse(ip string, port uint16, st, usn string) string {
	return fmt.Sprintf("HTTP/1.1 200 OK\r\n"+
		"CACHE-CONTROL: max-age=1800\r\n"+
		"EXT:\r\n"+
		"LOCATION: http://%s:%d/description.xml\r\n"+
		"SERVER: VuIO-Go/0.1 UPnP/1.0\r\n"+
		"ST: %s\r\n"+
		"USN: %s\r\n\r\n",
		ip, port, st, usn)
}

func buildNotify(ip string, port uint16, nt, usn string) string {
	return fmt.Sprintf("NOTIFY * HTTP/1.1\r\n"+
		"HOST: 239.255.255.250:1900\r\n"+
		"CACHE-CONTROL: max-age=1800\r\n"+
		"LOCATION: http://%s:%d/description.xml\r\n"+
		"NT: %s\r\n"+
		"NTS: ssdp:alive\r\n"+
		"SERVER: VuIO-Go/0.1 UPnP/1.0\r\n"+
		"USN: %s\r\n\r\n",
		ip, port, nt, usn)
}