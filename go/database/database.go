package database

import (
	"database/sql"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"time"

	"github.com/jmoiron/sqlx" // Corrected import path
	_ "github.com/mattn/go-sqlite3"
)

// MediaFile represents a media file in the database.
type MediaFile struct {
	ID          int64          `db:"id"`
	Path        string         `db:"path"`
	ParentPath  string         `db:"parent_path"`
	Filename    string         `db:"filename"`
	Size        int64          `db:"size"`
	Modified    time.Time      `db:"modified"`
	MimeType    string         `db:"mime_type"`
	Duration    sql.NullInt64  `db:"duration"` // in milliseconds
	Title       sql.NullString `db:"title"`
	Artist      sql.NullString `db:"artist"`
	Album       sql.NullString `db:"album"`
	Genre       sql.NullString `db:"genre"`
	TrackNumber sql.NullInt32  `db:"track_number"`
	Year        sql.NullInt32  `db:"year"`
	AlbumArtist sql.NullString `db:"album_artist"`
	CreatedAt   time.Time      `db:"created_at"`
	UpdatedAt   time.Time      `db:"updated_at"`
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
	filesQuery := "SELECT * FROM media_files WHERE parent_path = ? AND mime_type LIKE ? ORDER BY filename"
	err := s.db.Select(&files, filesQuery, parentPath, mediaTypeFilter+"%")
	if err != nil {
		return nil, nil, fmt.Errorf("failed to get files: %w", err)
	}

	var subdirs []MediaDirectory
	var subdirsQuery string
	var queryArgs []interface{}

	// Normalize parentPath for consistent SQL LIKE patterns.
	// Treat "" and "/" as the canonical root.
	normalizedParentPath := parentPath
	if normalizedParentPath == string(filepath.Separator) {
		normalizedParentPath = ""
	}

	if normalizedParentPath == "" {
		// For the root, we want parent_paths that are top-level directories.
		// These are paths that do not contain the path separator.
		// e.g., "Music", "Videos", not "Music/Albums"
		subdirsQuery = `
			SELECT DISTINCT parent_path AS immediate_subdir_name
			FROM media_files
			WHERE parent_path != '' AND parent_path IS NOT NULL
			  AND INSTR(parent_path, ?) = 0 -- No separator in the path itself
			ORDER BY immediate_subdir_name;
		`
		queryArgs = []interface{}{string(filepath.Separator)}
	} else {
		// For a non-root path, we want the immediate subdirectories.
		// e.g., for parentPath="/music", we want "album1" from "/music/album1" or "/music/album1/song.mp3"
		// The parent_path in the DB for a file in "/music/album1" is "/music/album1".
		// We need to extract the component immediately following `normalizedParentPath + separator`.
		prefixWithSeparator := normalizedParentPath + string(filepath.Separator)
		subdirsQuery = `
			SELECT DISTINCT
				CASE
					WHEN INSTR(SUBSTR(parent_path, LENGTH(?) + 1), ?) > 0 THEN
						SUBSTR(parent_path, LENGTH(?) + 1, INSTR(SUBSTR(parent_path, LENGTH(?) + 1), ?) - 1)
					ELSE
						SUBSTR(parent_path, LENGTH(?) + 1)
				END AS immediate_subdir_name
			FROM media_files
			WHERE parent_path LIKE ? || '%' AND parent_path != ?
			ORDER BY immediate_subdir_name;
		`
		// Corrected queryArgs: The original code had 7 arguments for 8 placeholders.
		queryArgs = []interface{}{
			prefixWithSeparator,        // 1st '?' in LENGTH(?) + 1 (first SUBSTR)
			string(filepath.Separator), // 1st '?' in INSTR(..., ?)
			prefixWithSeparator,        // 2nd '?' in LENGTH(?) + 1 (THEN clause, first SUBSTR)
			prefixWithSeparator,        // 3rd '?' in LENGTH(?) + 1 (THEN clause, second SUBSTR)
			string(filepath.Separator), // 2nd '?' in INSTR(..., ?) (THEN clause)
			prefixWithSeparator,        // 4th '?' in LENGTH(?) + 1 (ELSE clause, SUBSTR)
			prefixWithSeparator,        // 1st '?' in LIKE ? || '%'
			normalizedParentPath,       // 1st '?' in parent_path != ?
		}
	}

	rows, err := s.db.Query(subdirsQuery, queryArgs...)
	if err != nil {
		return nil, nil, fmt.Errorf("failed to get subdirectories: %w", err)
	}
	defer rows.Close()

	for rows.Next() {
		var name string
		if err := rows.Scan(&name); err != nil {
			fmt.Fprintf(os.Stderr, "Error scanning subdirectory name: %v\n", err)
			continue
		}
		if name == "" {
			continue
		}
		subdirs = append(subdirs, MediaDirectory{
			Name: name,
			Path: filepath.Join(parentPath, name), // Use original parentPath for joining
		})
	}

	if err = rows.Err(); err != nil {
		return nil, nil, fmt.Errorf("error iterating subdirectory rows: %w", err)
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
// This implementation uses a temporary table for efficiency with large datasets.
func (s *SqliteDatabase) CleanupMissingFiles(existingPaths []string) (int, error) {
	if len(existingPaths) == 0 {
		// If there are no existing paths, it means all files in the DB are missing.
		res, err := s.db.Exec("DELETE FROM media_files")
		if err != nil {
			return 0, fmt.Errorf("failed to delete all media files: %w", err)
		}
		rowsAffected, err := res.RowsAffected()
		return int(rowsAffected), err
	}

	tx, err := s.db.Beginx()
	if err != nil {
		return 0, fmt.Errorf("failed to begin transaction for cleanup: %w", err)
	}
	defer tx.Rollback() // Rollback on error or if commit fails

	// 1. Create a temporary table for existing paths
	_, err = tx.Exec(`CREATE TEMPORARY TABLE IF NOT EXISTS existing_paths (path TEXT PRIMARY KEY)`)
	if err != nil {
		return 0, fmt.Errorf("failed to create temporary table: %w", err)
	}

	// 2. Insert existing paths into the temporary table in batches
	const batchSize = 1000
	for i := 0; i < len(existingPaths); i += batchSize {
		end := i + batchSize
		if end > len(existingPaths) {
			end = len(existingPaths)
		}
		batch := existingPaths[i:end]

		valueStrings := make([]string, 0, len(batch))
		valueArgs := make([]interface{}, 0, len(batch))
		for _, path := range batch {
			valueStrings = append(valueStrings, "(?)")
			valueArgs = append(valueArgs, path)
		}

		insertQuery := fmt.Sprintf("INSERT INTO existing_paths (path) VALUES %s", strings.Join(valueStrings, ","))
		_, err = tx.Exec(insertQuery, valueArgs...)
		if err != nil {
			return 0, fmt.Errorf("failed to insert batch into temporary table: %w", err)
		}
	}

	// 3. Delete from media_files where path is NOT IN existing_paths
	res, err := tx.Exec(`DELETE FROM media_files WHERE path NOT IN (SELECT path FROM existing_paths)`)
	if err != nil {
		return 0, fmt.Errorf("failed to delete missing media files: %w", err)
	}

	// 4. Drop the temporary table (optional, as it's temporary and session-scoped, but good practice)
	_, err = tx.Exec(`DROP TABLE IF EXISTS existing_paths`)
	if err != nil {
		fmt.Fprintf(os.Stderr, "Warning: failed to drop temporary table: %v\n", err)
	}

	err = tx.Commit()
	if err != nil {
		return 0, fmt.Errorf("failed to commit cleanup transaction: %w", err)
	}

	rowsAffected, err := res.RowsAffected()
	return int(rowsAffected), err
}
