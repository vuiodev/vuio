//go:build linux

package platform

import (
	"os"
	"path/filepath"
)

// GetPlatformConfig returns the configuration defaults for Linux.
func GetPlatformConfig() *Config {
	homeDir, _ := os.UserHomeDir()
	videosDir := filepath.Join(homeDir, "Videos")
	configDir := filepath.Join(homeDir, ".config", "vuio-go")
	return &Config{
		DefaultMediaDir: videosDir,
		ConfigDir:       configDir,
		DatabasePath:    filepath.Join(configDir, "media.db"),
		LogPath:         filepath.Join(configDir, "vuio.log"),
		DefaultExcludePatterns: []string{
			".*", "lost+found", "*.tmp",
		},
		DefaultMediaExtensions: commonMediaExtensions(),
	}
}

// GetDefaultConfigFilePath returns the default config file path for Linux.
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