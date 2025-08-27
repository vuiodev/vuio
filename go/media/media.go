package media

import (
	"database/sql"
	"fmt"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"time"

	"vuio-go/config"
	"vuio-go/database"
)

// Scanner handles scanning media directories.
type Scanner struct {
	db database.Manager
}

// NewScanner creates a new media scanner.
func NewScanner(db database.Manager) *Scanner {
	return &Scanner{db: db}
}

// ScanAllDirectories scans all directories configured in AppConfig.
func (s *Scanner) ScanAllDirectories(cfg *config.AppConfig) error {
	slog.Info("Starting media scan for all configured directories")
	for _, dir := range cfg.Media.Directories {
		slog.Info("Scanning directory", "path", dir.Path)
		if err := s.ScanDirectory(&dir); err != nil {
			slog.Error("Failed to scan directory", "path", dir.Path, "error", err)
			// Continue to next directory
		}
	}

	if cfg.Media.CleanupDeletedFiles {
		slog.Info("Cleaning up deleted files from database...")
		if err := s.cleanup(); err != nil {
			slog.Error("Failed to clean up deleted files", "error", err)
		}
	}

	slog.Info("Media scan finished")
	return nil
}

// ScanDirectory scans a single directory configuration.
func (s *Scanner) ScanDirectory(dirConfig *config.MonitoredDirectoryConfig) error {
	return filepath.Walk(dirConfig.Path, func(path string, info os.FileInfo, err error) error {
		if err != nil {
			return err
		}
		if info.IsDir() {
			if !dirConfig.Recursive && path != dirConfig.Path {
				return filepath.SkipDir
			}
			return nil // Continue walking
		}

		// Check if it's a media file
		if !isMediaFile(path) {
			return nil
		}

		// Sync file with database
		return s.SyncFile(path, info)
	})
}

// SyncFile checks a single file against the database and adds/updates it if necessary.
func (s *Scanner) SyncFile(path string, info os.FileInfo) error {
	existing, err := s.db.GetFileByPath(path)
	if err != nil {
		return fmt.Errorf("error getting file from db: %w", err)
	}

	if existing != nil {
		// File exists, check if it needs an update
		if info.ModTime().After(existing.Modified) || info.Size() != existing.Size {
			slog.Debug("Updating existing file in database", "path", path)
			mf := buildMediaFile(path, info)
			mf.ID = existing.ID
			mf.CreatedAt = existing.CreatedAt
			return s.db.UpdateMediaFile(mf)
		}
	} else {
		// New file
		slog.Debug("Adding new file to database", "path", path)
		mf := buildMediaFile(path, info)
		_, err := s.db.StoreMediaFile(mf)
		return err
	}
	return nil
}

// cleanup removes files from the database that no longer exist on disk.
func (s *Scanner) cleanup() error {
	allDbPaths, err := s.db.GetAllPaths()
	if err != nil {
		return err
	}

	var toDelete []string
	for _, path := range allDbPaths {
		if _, err := os.Stat(path); os.IsNotExist(err) {
			toDelete = append(toDelete, path)
		}
	}

	if len(toDelete) > 0 {
		slog.Info("Found deleted files to remove from database", "count", len(toDelete))
		for _, path := range toDelete {
			if _, err := s.db.RemoveMediaFile(path); err != nil {
				slog.Error("Failed to remove deleted file from db", "path", path, "error", err)
			}
		}
	}

	return nil
}

// buildMediaFile creates a MediaFile struct from file info.
func buildMediaFile(path string, info os.FileInfo) *database.MediaFile {
	return &database.MediaFile{
		Path:       path,
		ParentPath: filepath.Dir(path),
		Filename:   info.Name(),
		Size:       info.Size(),
		Modified:   info.ModTime(),
		MimeType:   getMimeType(path),
		Title:      sql.NullString{String: strings.TrimSuffix(info.Name(), filepath.Ext(info.Name())), Valid: true},
		CreatedAt:  time.Now(),
		UpdatedAt:  time.Now(),
	}
}

func isMediaFile(path string) bool {
	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".mp4", ".mkv", ".avi", ".mov", ".wmv", ".flv", ".webm", ".m4v", ".3gp",
		".mp3", ".flac", ".wav", ".aac", ".ogg", ".wma", ".m4a",
		".jpg", ".jpeg", ".png", ".gif", ".bmp", ".webp":
		return true
	default:
		return false
	}
}

func getMimeType(path string) string {
	ext := strings.ToLower(filepath.Ext(path))
	switch ext {
	case ".mp4", ".m4a", ".m4v":
		return "video/mp4"
	case ".mkv":
		return "video/x-matroska"
	case ".avi":
		return "video/x-msvideo"
	case ".mov":
		return "video/quicktime"
	case ".wmv":
		return "video/x-ms-wmv"
	case ".flv":
		return "video/x-flv"
	case ".webm":
		return "video/webm"
	case ".mp3":
		return "audio/mpeg"
	case ".flac":
		return "audio/flac"
	case ".wav":
		return "audio/wav"
	case ".aac":
		return "audio/aac"
	case ".ogg":
		return "audio/ogg"
	case ".wma":
		return "audio/x-ms-wma"
	case ".jpg", ".jpeg":
		return "image/jpeg"
	case ".png":
		return "image/png"
	case ".gif":
		return "image/gif"
	case ".bmp":
		return "image/bmp"
	case ".webp":
		return "image/webp"
	default:
		return "application/octet-stream"
	}
}
