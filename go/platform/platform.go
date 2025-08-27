package platform

import (
	"fmt"
	"log/slog"
	"net"
	"os"
	"runtime"
)

// OS represents the operating system type.
type OS string

const (
	Windows OS = "windows"
	Darwin  OS = "darwin" // macOS
	Linux   OS = "linux"
	BSD     OS = "bsd"
)

// Platform holds detected information about the current platform.
type Platform struct {
	OS   OS
	Arch string
}

// Detect returns information about the current platform.
func Detect() (*Platform, error) {
	var osType OS
	switch runtime.GOOS {
	case "windows":
		osType = Windows
	case "darwin":
		osType = Darwin
	case "linux":
		osType = Linux
	case "freebsd", "openbsd", "netbsd", "dragonfly":
		osType = BSD
	default:
		return nil, fmt.Errorf("unsupported operating system: %s", runtime.GOOS)
	}
	return &Platform{
		OS:   osType,
		Arch: runtime.GOARCH,
	}, nil
}

// Config holds platform-specific configuration defaults.
type Config struct {
	DefaultMediaDir      string
	ConfigDir            string
	DatabasePath         string
	LogPath              string
	DefaultExcludePatterns []string
	DefaultMediaExtensions []string
}

// Interface represents a network interface.
type Interface struct {
	Name             string
	IP               net.IP
	IsLoopback       bool
	IsUp             bool
	SupportsMulticast bool
}

// GetInterfaces returns a list of suitable network interfaces.
func GetInterfaces() ([]Interface, error) {
	ifaces, err := net.Interfaces()
	if err != nil {
		return nil, err
	}

	var result []Interface
	for _, i := range ifaces {
		addrs, err := i.Addrs()
		if err != nil {
			continue
		}
		for _, addr := range addrs {
			var ip net.IP
			switch v := addr.(type) {
			case *net.IPNet:
				ip = v.IP
			case *net.IPAddr:
				ip = v.IP
			}
			if ip == nil || ip.IsLoopback() {
				continue
			}
			// We only care about IPv4 for now for simplicity
			if ip.To4() == nil {
				continue
			}

			result = append(result, Interface{
				Name:             i.Name,
				IP:               ip,
				IsLoopback:       (i.Flags & net.FlagLoopback) != 0,
				IsUp:             (i.Flags & net.FlagUp) != 0,
				SupportsMulticast: (i.Flags & net.FlagMulticast) != 0,
			})
			// Only take the first valid IPv4 address for an interface
			break
		}
	}
	return result, nil
}

// GetPrimaryIP returns the most likely primary IP address for the server.
func GetPrimaryIP() (string, error) {
	ifaces, err := GetInterfaces()
	if err != nil {
		return "", err
	}

	for _, i := range ifaces {
		if i.IsUp && !i.IsLoopback {
			return i.IP.String(), nil
		}
	}

	// Fallback for docker or other restricted environments
	if hostIP := os.Getenv("VUIO_IP"); hostIP != "" {
		slog.Warn("No suitable network interface found, using VUIO_IP from environment", "ip", hostIP)
		return hostIP, nil
	}

	return "", fmt.Errorf("no suitable network interface found")
}