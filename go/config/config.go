package config

import (
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"

	"github.com/BurntSushi/toml"
	"github.com/google/uuid"

	"vuio-go/platform"
)

// AppConfig is the main application configuration structure.
type AppConfig struct {
	Server   ServerConfig   `toml:"server"`
	Network  NetworkConfig  `toml:"network"`
	Media    MediaConfig    `toml:"media"`
	Database DatabaseConfig `toml:"database"`
}

// ServerConfig holds server settings.
type ServerConfig struct {
	Port      uint16  `toml:"port"`
	Interface string  `toml:"interface"`
	Name      string  `toml:"name"`
	UUID      string  `toml:"uuid"`
	IP        *string `toml:"ip"`
}

// NetworkConfig holds network settings.
type NetworkConfig struct {
	InterfaceSelection      string `toml:"interface_selection"` // Auto, All, or specific name
	MulticastTTL            uint8  `toml:"multicast_ttl"`
	AnnounceIntervalSeconds uint64 `toml:"announce_interval_seconds"`
}

// MediaConfig holds media library settings.
type MediaConfig struct {
	Directories         []MonitoredDirectoryConfig `toml:"directories"`
	ScanOnStartup       bool                       `toml:"scan_on_startup"`
	WatchForChanges     bool                       `toml:"watch_for_changes"`
	CleanupDeletedFiles bool                       `toml:"cleanup_deleted_files"`
	AutoplayEnabled     bool                       `toml:"autoplay_enabled"`
	SupportedExtensions []string                   `toml:"supported_extensions"`
}

// MonitoredDirectoryConfig holds settings for a single media directory.
type MonitoredDirectoryConfig struct {
	Path            string   `toml:"path"`
	Recursive       bool     `toml:"recursive"`
	Extensions      []string `toml:"extensions,omitempty"`
	ExcludePatterns []string `toml:"exclude_patterns,omitempty"`
}

// DatabaseConfig holds database settings.
type DatabaseConfig struct {
	Path              *string `toml:"path"`
	VacuumOnStartup   bool    `toml:"vacuum_on_startup"`
	BackupEnabled     bool    `toml:"backup_enabled"`
}

// Initialize loads the configuration from a file or creates a default one.
func Initialize(configPath string, args []string) (*AppConfig, error) {
	// For simplicity, we prioritize config file over CLI args if both are present.
	// The Rust version had complex override logic.
	if configPath == "" {
		configPath = platform.GetDefaultConfigFilePath()
		slog.Info("Using default config path", "path", configPath)
	} else {
		slog.Info("Using provided config path", "path", configPath)
	}

	cfg, err := LoadFromFile(configPath)
	if err != nil {
		if os.IsNotExist(err) {
			slog.Info("Configuration file not found, creating a default one.", "path", configPath)
			cfg = Default()
			if err := cfg.SaveToFile(configPath); err != nil {
				return nil, fmt.Errorf("failed to save default config: %w", err)
			}
		} else {
			return nil, fmt.Errorf("failed to load config file: %w", err)
		}
	}

	if err := cfg.Validate(); err != nil {
		return nil, fmt.Errorf("configuration validation failed: %w", err)
	}

	return cfg, nil
}

// Default creates a default configuration based on the current platform.
func Default() *AppConfig {
	plat := platform.GetPlatformConfig()
	hostname, err := os.Hostname()
	if err != nil {
		hostname = "VuIO-Server"
	}

	return &AppConfig{
		Server: ServerConfig{
			Port:      8080,
			Interface: "0.0.0.0",
			Name:      fmt.Sprintf("VuIO Go (%s)", hostname),
			UUID:      uuid.New().String(),
			IP:        nil,
		},
		Network: NetworkConfig{
			InterfaceSelection:      "Auto",
			MulticastTTL:            4,
			AnnounceIntervalSeconds: 30,
		},
		Media: MediaConfig{
			Directories: []MonitoredDirectoryConfig{
				{
					Path:            plat.DefaultMediaDir,
					Recursive:       true,
					ExcludePatterns: plat.DefaultExcludePatterns,
				},
			},
			ScanOnStartup:       true,
			WatchForChanges:     true,
			CleanupDeletedFiles: true,
			AutoplayEnabled:     true,
			SupportedExtensions: plat.DefaultMediaExtensions,
		},
		Database: DatabaseConfig{
			Path:              &plat.DatabasePath,
			VacuumOnStartup:   false,
			BackupEnabled:     true,
		},
	}
}

// LoadFromFile loads configuration from a TOML file.
func LoadFromFile(path string) (*AppConfig, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var cfg AppConfig
	if _, err := toml.Decode(string(data), &cfg); err != nil {
		return nil, err
	}
	return &cfg, nil
}

// SaveToFile saves the configuration to a TOML file.
func (c *AppConfig) SaveToFile(path string) error {
	dir := filepath.Dir(path)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return fmt.Errorf("failed to create config directory: %w", err)
	}
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()
	return toml.NewEncoder(f).Encode(c)
}

// Validate checks the configuration for common errors.
func (c *AppConfig) Validate() error {
	if c.Server.Port == 0 {
		return fmt.Errorf("server port cannot be 0")
	}
	if strings.TrimSpace(c.Server.Name) == "" {
		return fmt.Errorf("server name cannot be empty")
	}
	if _, err := uuid.Parse(c.Server.UUID); err != nil {
		return fmt.Errorf("invalid server UUID: %w", err)
	}
	if len(c.Media.Directories) == 0 {
		slog.Warn("no media directories configured")
	}
	for _, dir := range c.Media.Directories {
		if _, err := os.Stat(dir.Path); os.IsNotExist(err) {
			slog.Warn("media directory does not exist", "path", dir.Path)
		}
	}
	return nil
}

// GetDatabasePath returns the configured database path or the platform default.
func (c *AppConfig) GetDatabasePath() string {
	if c.Database.Path != nil && *c.Database.Path != "" {
		return *c.Database.Path
	}
	return platform.GetPlatformConfig().DatabasePath
}

// GetPrimaryMediaDir returns the first configured media directory.
func (c *AppConfig) GetPrimaryMediaDir() string {
	if len(c.Media.Directories) > 0 {
		return c.Media.Directories[0].Path
	}
	return platform.GetPlatformConfig().DefaultMediaDir
}