# Requirements Document

## Introduction

This feature addresses critical architectural and performance issues identified in the media server codebase through comprehensive analysis. The primary focus is on fixing path normalization inconsistencies that cause DLNA browsing failures, eliminating code duplication, removing unused code, and resolving performance bottlenecks that prevent the application from scaling to large media libraries.

## Requirements

### Requirement 1: Path Normalization Standardization

**User Story:** As a user with a large media library, I want consistent file path handling so that all my media files are discoverable and browsable through DLNA clients regardless of the operating system or path format variations.

#### Acceptance Criteria

1. WHEN a media file is scanned THEN the system SHALL store all paths in a canonical format (lowercase, absolute paths with forward slashes)
2. WHEN a DLNA browse request is processed THEN the system SHALL normalize the ObjectID-derived path using the same canonicalization logic as the scanner
3. WHEN paths are queried from the database THEN the system SHALL return results that match the normalized query path exactly
4. IF a path contains symbolic links or relative components THEN the system SHALL resolve them to absolute paths before normalization
5. WHEN the application runs on Windows THEN paths like "C:\Music" and "c:/music/" SHALL be treated as identical after normalization

### Requirement 2: Database Query Optimization

**User Story:** As a user with thousands of media files, I want fast directory browsing and efficient file management so that the DLNA server remains responsive even with large libraries.

#### Acceptance Criteria

1. WHEN requesting a directory listing THEN the system SHALL use efficient queries that fetch only direct children, not all descendants
2. WHEN cleaning up missing files THEN the system SHALL process deletions in batches without loading the entire database into memory
3. WHEN a directory is deleted THEN the system SHALL efficiently find and remove all contained files using path prefix queries
4. WHEN querying subdirectories THEN the system SHALL use a two-query approach: one for files and one optimized query for subdirectories
5. WHEN the media library exceeds 5000 files THEN all database operations SHALL continue to perform efficiently

### Requirement 3: Code Duplication Elimination

**User Story:** As a developer maintaining this codebase, I want consolidated, non-duplicated code so that bug fixes and improvements only need to be made in one place.

#### Acceptance Criteria

1. WHEN SSDP services are implemented THEN there SHALL be only one unified implementation instead of three near-identical versions
2. WHEN network IP detection is needed THEN there SHALL be a single implementation shared across modules
3. WHEN platform-specific behavior is required THEN it SHALL be abstracted through the existing trait system
4. WHEN socket configuration differs by platform THEN the differences SHALL be handled within the unified service implementation

### Requirement 4: Unused Code Removal

**User Story:** As a developer working on this codebase, I want clean, maintainable code without dead code paths so that I can focus on active functionality and reduce cognitive overhead.

#### Acceptance Criteria

1. WHEN the codebase is analyzed THEN all unused files like `watcher/integration.rs` SHALL be removed
2. WHEN error handling is implemented THEN unused error recovery traits and functions SHALL be removed
3. WHEN platform-specific functions are defined THEN unused underscore-prefixed functions SHALL be removed
4. WHEN configuration watching is implemented THEN abandoned or duplicate config watcher services SHALL be removed

### Requirement 5: Performance Bottleneck Resolution

**User Story:** As a user with a large media collection, I want the server to start quickly and handle file operations efficiently so that I don't experience delays or memory issues.

#### Acceptance Criteria

1. WHEN file deletion events occur THEN the system SHALL NOT load the entire database into memory
2. WHEN argument parsing happens THEN it SHALL occur only once during startup, not multiple times
3. WHEN configuration changes are detected THEN the system SHALL use proper file watching instead of manual polling
4. WHEN media scanning occurs THEN recursive directory scanning SHALL minimize database queries through batching
5. WHEN metadata extraction is performed THEN synchronous I/O operations SHALL be wrapped in spawn_blocking to prevent blocking the async runtime
6. WHEN database queries use path parameters THEN they SHALL use canonical paths consistently to avoid case-sensitivity issues on different filesystems

### Requirement 6: Configuration and Validation Improvements

**User Story:** As a user setting up the media server, I want flexible configuration validation so that the server can start even when some media directories are temporarily unavailable.

#### Acceptance Criteria

1. WHEN a configured media directory is temporarily unavailable THEN the system SHALL log a warning but continue startup
2. WHEN file exclusion patterns are specified THEN the system SHALL support more powerful globbing patterns beyond simple extensions
3. WHEN configuration files change THEN the system SHALL detect and reload them automatically using proper debouncing
4. WHEN dependency management is configured THEN redundant dependencies SHALL be removed from Cargo.toml

### Requirement 7: Handler Refactoring

**User Story:** As a developer maintaining the web handlers, I want modular, focused functions so that DLNA browse logic is easier to understand and modify.

#### Acceptance Criteria

1. WHEN DLNA browse requests are handled THEN the monolithic handler SHALL be broken into specialized functions by content type
2. WHEN ObjectID processing occurs THEN path normalization SHALL be consistently applied before database queries
3. WHEN different browse types are requested THEN each SHALL have its own dedicated handler function
4. WHEN server IP information is needed THEN it SHALL be retrieved from application state, not re-detected

### Requirement 8: Critical Database Performance Optimization

**User Story:** As a user with a large media library (100,000+ files), I want database operations to be performed efficiently within the database engine so that file cleanup and directory browsing remain fast and don't consume excessive memory.

#### Acceptance Criteria

1. WHEN batch_cleanup_missing_files is executed THEN the system SHALL perform cleanup entirely within the database using temporary tables or IN clauses instead of loading all paths into Rust memory
2. WHEN get_direct_subdirectories or get_filtered_direct_subdirectories is called THEN the system SHALL use pure SQL with string manipulation functions (SUBSTR, INSTR) to find immediate children instead of fetching all descendants with LIKE queries
3. WHEN playlist import operations are performed THEN the system SHALL use batch queries and transactions to avoid N+1 query problems instead of individual database calls per playlist entry
4. WHEN large directory structures are processed THEN memory usage SHALL remain bounded regardless of the number of files
5. WHEN database operations are performed on libraries with 100,000+ files THEN response times SHALL remain under 1 second for typical operations

### Requirement 9: Configuration Robustness

**User Story:** As a system administrator, I want configuration file generation to be robust and maintainable so that future library updates don't break comment injection or formatting.

#### Acceptance Criteria

1. WHEN configuration files are generated with comments THEN the system SHALL use a library that preserves comments and formatting (like toml_edit) instead of manual string manipulation
2. WHEN TOML serialization order changes in future library versions THEN configuration generation SHALL continue to work correctly
3. WHEN configuration templates are used THEN comments SHALL be preserved through a template-based approach rather than post-processing injection
4. WHEN configuration files are saved THEN the format SHALL be consistent and maintainable across library updates