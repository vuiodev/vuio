package database

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"time"

	"github.com/jmoiron/sqlx"
	_ "github.com/mattn/go-sqlite3"
)

// MediaFile represents a media file in the database.
type MediaFile struct {
	ID           int64          `db:"id"`
	Path         string         `db:"path"`
	ParentPath   string         `db:"parent_path"`
	Filename     string         `db:"filename"`
	Size         int64          `db:"size"`
	Modified     time.Time      `db:"modified"`
	MimeType     string         `db:"mime_type"`
	Duration     sql.NullInt64  `db:"duration"` // in milliseconds
	Title        sql.NullString `db:"title"`
	Artist       sql.NullString `db:"artist"`
	Album        sql.NullString `db:"album"`
	Genre        sql.NullString `db:"genre"`
	TrackNumber  sql.NullInt32  `db:"track_number"`
	Year         sql.NullInt32  `db:"year"`
	AlbumArtist  sql.NullString `db:"album_artist"`
	CreatedAt    time.Time      `db:"created_at"`
	UpdatedAt    time.Time      `db:"updated_at"`
}

// MediaDirectory represents a subdirectory in the media library.
type MediaDirectory struct {
	Path string
	Name string
}

// Manager defines the interface for database operations.
type Manager interface {
	Initialize() error
	StoreMediaFile(file *MediaFile) (int64, error)
	GetFileByID(id int64) (*MediaFile, error)
	GetFileByPath(path string) (*MediaFile, error)
	GetFilesInDirectory(dirPath string) ([]MediaFile, error)
	RemoveMediaFile(path string) (bool, error)
	UpdateMediaFile(file *MediaFile) error
	GetDirectoryListing(parentPath, mediaTypeFilter string) ([]MediaDirectory, []MediaFile, error)
	GetAllPaths() ([]string, error)
	CleanupMissingFiles(existingPaths []string) (int, error)
	Close() error
	// ... other methods from Rust trait
}

// SqliteDatabase is the SQLite implementation of the Manager interface.
type SqliteDatabase struct {
	db *sqlx.DB
}

// NewSqliteDatabase creates a new SQLite database connection.
func NewSqliteDatabase(dbPath string) (*SqliteDatabase, error) {
	dir := filepath.Dir(dbPath)
	if err := os.MkdirAll(dir, 0755); err != nil {
		return nil, fmt.Errorf("failed to create database directory: %w", err)
	}

	db, err := sqlx.Connect("sqlite3", dbPath+"?_journal=WAL")
	if err != nil {
		return nil, err
	}

	return &SqliteDatabase{db: db}, nil
}

// Close closes the database connection.
func (s *SqliteDatabase) Close() error {
	return s.db.Close()
}

// Initialize creates the database schema.
func (s *SqliteDatabase) Initialize() error {
	schema := `
	CREATE TABLE IF NOT EXISTS media_files (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		path TEXT UNIQUE NOT NULL,
		parent_path TEXT NOT NULL,
		filename TEXT NOT NULL,
		size INTEGER NOT NULL,
		modified DATETIME NOT NULL,
		mime_type TEXT NOT NULL,
		duration INTEGER,
		title TEXT,
		artist TEXT,
		album TEXT,
		genre TEXT,
		track_number INTEGER,
		year INTEGER,
		album_artist TEXT,
		created_at DATETIME NOT NULL,
		updated_at DATETIME NOT NULL
	);
	CREATE INDEX IF NOT EXISTS idx_media_files_path ON media_files(path);
	CREATE INDEX IF NOT EXISTS idx_media_files_parent_path ON media_files(parent_path);
	`
	_, err := s.db.Exec(schema)
	return err
}

// StoreMediaFile adds a new media file to the database.
func (s *SqliteDatabase) StoreMediaFile(file *MediaFile) (int64, error) {
	query := `INSERT INTO media_files 
	(path, parent_path, filename, size, modified, mime_type, duration, title, artist, album, genre, track_number, year, album_artist, created_at, updated_at)
	VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)`

	now := time.Now()
	file.CreatedAt = now
	file.UpdatedAt = now

	res, err := s.db.Exec(query, file.Path, filepath.Dir(file.Path), file.Filename, file.Size, file.Modified, file.MimeType, file.Duration, file.Title, file.Artist, file.Album, file.Genre, file.TrackNumber, file.Year, file.AlbumArtist, file.CreatedAt, file.UpdatedAt)
	if err != nil {
		return 0, err
	}
	return res.LastInsertId()
}

// GetFileByID retrieves a media file by its ID.
func (s *SqliteDatabase) GetFileByID(id int64) (*MediaFile, error) {
	var file MediaFile
	err := s.db.Get(&file, "SELECT * FROM media_files WHERE id = ?", id)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return &file, err
}

// GetFileByPath retrieves a media file by its path.
func (s *SqliteDatabase) GetFileByPath(path string) (*MediaFile, error) {
	var file MediaFile
	err := s.db.Get(&file, "SELECT * FROM media_files WHERE path = ?", path)
	if err == sql.ErrNoRows {
		return nil, nil
	}
	return &file, err
}

// GetFilesInDirectory retrieves all media files in a given directory path.
func (s *SqliteDatabase) GetFilesInDirectory(dirPath string) ([]MediaFile, error) {
	var files []MediaFile
	err := s.db.Select(&files, "SELECT * FROM media_files WHERE parent_path = ? ORDER BY filename", dirPath)
	return files, err
}

// RemoveMediaFile removes a media file from the database by its path.
func (s *SqliteDatabase) RemoveMediaFile(path string) (bool, error) {
	res, err := s.db.Exec("DELETE FROM media_files WHERE path = ?", path)
	if err != nil {
		return false, err
	}
	rowsAffected, err := res.RowsAffected()
	return rowsAffected > 0, err
}

// UpdateMediaFile updates an existing media file record.
func (s *SqliteDatabase) UpdateMediaFile(file *MediaFile) error {
	query := `UPDATE media_files SET 
		parent_path = ?, filename = ?, size = ?, modified = ?, mime_type = ?, 
		duration = ?, title = ?, artist = ?, album = ?, genre = ?, 
		track_number = ?, year = ?, album_artist = ?, updated_at = ?
		WHERE id = ?`
	file.UpdatedAt = time.Now()
	_, err := s.db.Exec(query,
		filepath.Dir(file.Path), file.Filename, file.Size, file.Modified, file.MimeType,
		file.Duration, file.Title, file.Artist, file.Album, file.Genre,
		file.TrackNumber, file.Year, file.AlbumArtist, file.UpdatedAt,
		file.ID)
	return err
}

// GetDirectoryListing retrieves subdirectories and files for a given path.
func (s *SqliteDatabase) GetDirectoryListing(parentPath, mediaTypeFilter string) ([]MediaDirectory, []MediaFile, error) {
	var files []MediaFile
	query := "SELECT * FROM media_files WHERE parent_path = ? AND mime_type LIKE ? ORDER BY filename"
	err := s.db.Select(&files, query, parentPath, mediaTypeFilter+"%")
	if err != nil {
		return nil, nil, err
	}

	// This is a simplified way to get subdirectories.
	// A more efficient way would be a dedicated table or more complex query.
	var subdirs []MediaDirectory
	dirMap := make(map[string]bool)
	query = "SELECT DISTINCT parent_path FROM media_files WHERE parent_path LIKE ? AND parent_path != ?"
	rows, err := s.db.Query(query, parentPath+string(filepath.Separator)+"%", parentPath)
	if err != nil {
		return nil, nil, err
	}
	defer rows.Close()

	for rows.Next() {
		var path string
		if err := rows.Scan(&path); err != nil {
			continue
		}
		rel, err := filepath.Rel(parentPath, path)
		if err != nil {
			continue
		}
		firstComponent := strings.Split(rel, string(filepath.Separator))[0]
		if _, exists := dirMap[firstComponent]; !exists {
			dirMap[firstComponent] = true
			subdirs = append(subdirs, MediaDirectory{
				Name: firstComponent,
				Path: filepath.Join(parentPath, firstComponent),
			})
		}
	}

	return subdirs, files, nil
}

// GetAllPaths returns all file paths from the database.
func (s *SqliteDatabase) GetAllPaths() ([]string, error) {
	var paths []string
	err := s.db.Select(&paths, "SELECT path FROM media_files")
	return paths, err
}

// CleanupMissingFiles removes records for files that no longer exist.
func (s *SqliteDatabase) CleanupMissingFiles(existingPaths []string) (int, error) {
	// This is inefficient for large libraries. A better approach would be to
	// use temporary tables, but this mirrors the Rust implementation's logic.
	allDbPaths, err := s.GetAllPaths()
	if err != nil {
		return 0, err
	}

	existingSet := make(map[string]bool, len(existingPaths))
	for _, p := range existingPaths {
		existingSet[p] = true
	}

	var toDelete []string
	for _, dbPath := range allDbPaths {
		if !existingSet[dbPath] {
			toDelete = append(toDelete, dbPath)
		}
	}

	if len(toDelete) == 0 {
		return 0, nil
	}

	query, args, err := sqlx.In("DELETE FROM media_files WHERE path IN (?)", toDelete)
	if err != nil {
		return 0, err
	}
	query = s.db.Rebind(query)
	res, err := s.db.Exec(query, args...)
	if err != nil {
		return 0, err
	}
	rowsAffected, err := res.RowsAffected()
	return int(rowsAffected), err
}