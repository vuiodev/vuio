//go:build darwin

package platform

import (
	"os"
	"path/filepath"
)

// GetPlatformConfig returns the configuration defaults for macOS.
func GetPlatformConfig() *Config {
	homeDir, _ := os.UserHomeDir()
	moviesDir := filepath.Join(homeDir, "Movies")
	configDir := filepath.Join(homeDir, ".config", "vuio-go")
	return &Config{
		DefaultMediaDir: moviesDir,
		ConfigDir:       configDir,
		DatabasePath:    filepath.Join(configDir, "media.db"),
		LogPath:         filepath.Join(configDir, "vuio.log"),
		DefaultExcludePatterns: []string{
			".*", ".DS_Store", ".AppleDouble", "*.tmp",
		},
		DefaultMediaExtensions: commonMediaExtensions(),
	}
}

// GetDefaultConfigFilePath returns the default config file path for macOS.
func GetDefaultConfigFilePath() string {
	return filepath.Join(GetPlatformConfig().ConfigDir, "config.toml")
}

func commonMediaExtensions() []string {
	return []string{
		"mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "3gp",
		"mp3", "flac", "wav", "aac", "ogg", "wma", "m4a",
		"jpg", "jpeg", "png", "gif", "bmp", "webp",
	}
}