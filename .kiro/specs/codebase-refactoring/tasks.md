# Implementation Plan

## Phase 1: Path Normalization Foundation

- [x] 1. Create PathNormalizer trait and core implementation
  - Define PathNormalizer trait with to_canonical, from_canonical, and normalize_for_query methods
  - Implement WindowsPathNormalizer with lowercase, forward-slash canonical format
  - Create unit tests for path normalization edge cases (mixed case, different separators, UNC paths)
  - _Requirements: 1.1, 1.2, 1.4_

- [x] 2. Integrate path normalization into FileSystemManager
  - Modify platform/filesystem/mod.rs to use PathNormalizer in normalize_path method
  - Update WindowsFileSystemManager to apply canonical normalization
  - Ensure canonicalize_path resolves symbolic links before normalization
  - _Requirements: 1.1, 1.4_

- [x] 3. Update MediaScanner to use canonical paths
  - Modify MediaScanner::scan_directory_recursive to normalize paths before database storage
  - Update MediaFile struct creation to use canonical path format
  - Apply normalization to both path and parent_path fields
  - _Requirements: 1.1, 1.2_

- [x] 4. Modify database schema for canonical path storage
  - Create database migration to add canonical_path and canonical_parent_path columns
  - Update SqliteDatabase::store_media_file to store canonical paths
  - Add database indexes for efficient path-based queries
  - _Requirements: 1.1, 1.3_

- [x] 5. Update web handlers to use normalized paths
  - Modify content_directory_control to normalize ObjectID-derived paths before database queries
  - Apply same normalization logic used in scanning to browse request handling
  - Update path construction from ObjectID to use canonical format
  - _Requirements: 1.2, 1.3_

## Phase 2: Database Query Optimization

- [x] 6. Implement efficient directory listing queries
  - Replace get_directory_listing LIKE query with direct parent_path matching
  - Create get_direct_subdirectories method using two-query approach
  - Write unit tests to verify only direct children are returned, not all descendants
  - _Requirements: 2.1, 2.4_

- [x] 7. Create batch file cleanup operations
  - Implement batch_cleanup_missing_files using HashSet difference logic
  - Process file deletions in batches to avoid SQL parameter limits
  - Add transaction support for atomic cleanup operations
  - _Requirements: 2.2, 2.5_

- [x] 8. Add path prefix query support
  - Implement get_files_with_path_prefix for efficient directory deletion handling
  - Create database index on canonical_path for prefix queries
  - Update file deletion logic in main.rs to use prefix queries instead of loading entire database
  - _Requirements: 2.3, 2.5_

- [x] 9. Replace get_all_media_files with streaming interface
  - Implement stream_all_media_files using async streams
  - Remove hardcoded LIMIT from get_all_media_files and mark as deprecated
  - Update code that uses get_all_media_files to use streaming or paginated queries
  - _Requirements: 2.5_

## Phase 3: Code Consolidation and Cleanup

- [x] 10. Remove unused watcher integration code
  - Delete src/watcher/integration.rs file entirely
  - Remove WatcherDatabaseIntegration module declaration from src/watcher/mod.rs
  - Clean up any imports or references to the deleted integration code
  - _Requirements: 4.1_

- [x] 11. Consolidate SSDP service implementations
  - Create UnifiedSsdpService struct with single implementation
  - Implement SsdpPlatformAdapter trait for platform-specific behavior
  - Create WindowsSsdpAdapter, DockerSsdpAdapter, and UnixSsdpAdapter implementations
  - _Requirements: 3.1, 3.3_

- [x] 12. Replace duplicate SSDP implementations
  - Remove ssdp_service_docker, ssdp_service_windows, and native implementations from ssdp.rs
  - Update main.rs to use single run_ssdp_service function with unified implementation
  - Migrate platform-specific socket configuration to adapter pattern
  - _Requirements: 3.1, 3.4_

- [x] 13. Remove duplicate IP detection code
  - Remove get_server_ip function from web/xml.rs
  - Update XML generation functions to receive server IP from AppState
  - Ensure single source of truth for server IP in platform_info
  - _Requirements: 3.2_

- [x] 14. Clean up unused error handling and platform code
  - Remove ErrorRecovery trait and retry_with_backoff function from error.rs
  - Delete unused underscore-prefixed functions from platform modules (linux.rs, macos.rs, bsd.rs)
  - Remove handle_platform_feature_unavailable function from main.rs
  - _Requirements: 4.2, 4.3_

- [x] 15. Remove unused config watcher service
  - Delete src/config/watcher.rs file entirely (ConfigWatcherService and ConfigChangeRegistry)
  - Remove module declaration and any references to the abandoned config watcher system
  - Clean up imports related to the unused trait-based config change system
  - _Requirements: 4.1_

## Phase 4: Architecture Improvements

- [x] 16. Simplify argument parsing and startup sequence
  - Refactor main.rs to parse arguments once and apply as config overrides
  - Remove redundant parse_early_args function and duplicate argument processing
  - Consolidate config loading and argument application into single flow
  - _Requirements: 5.2_

- [x] 17. Implement proper configuration file watching
  - Update ConfigManager to use notify-debouncer-full correctly without manual debouncing
  - Remove check_and_reload_configuration function and manual polling from main.rs
  - Implement automatic config reload with proper debouncing
  - _Requirements: 5.3, 6.3_

- [x] 18. Improve configuration validation flexibility
  - Change validate_monitored_directory to log warnings instead of errors for missing paths
  - Implement ValidationMode enum (Strict, Warn, Skip) for media directory validation
  - Update configuration loading to continue startup when directories are temporarily unavailable
  - _Requirements: 6.1_

- [x] 19. Modularize content directory handlers
  - Break down content_directory_control into specialized handler functions
  - Create handle_video_browse, handle_music_browse, handle_artist_browse, handle_album_browse methods
  - Ensure consistent path normalization across all specialized handlers
  - _Requirements: 7.1, 7.2, 7.3_

- [ ] 20. Enhance configuration pattern matching
  - Replace simple matches_pattern with proper globbing library (wildmatch or similar)
  - Update should_exclude_file to support advanced patterns like *.~tmp, temp_*
  - Add unit tests for enhanced pattern matching capabilities
  - _Requirements: 6.2_

- [ ] 21. Clean up dependency management
  - Remove direct dependency on notify from Cargo.toml (included in notify-debouncer-full)
  - Review and remove any other redundant dependencies
  - Update dependency versions if needed for compatibility
  - _Requirements: 6.4_

- [x] 22. Optimize media scanner recursive scanning
  - Fix N+1 query problem in scan_directory_recursive by performing single database query at start
  - Pass collection of known files down through recursive calls to avoid repeated database hits
  - Implement batch processing for large directory structures
  - _Requirements: 5.1_

## Integration and Testing Tasks

- [x] 23. Create comprehensive path normalization tests





  - Write integration tests for Windows path variations (C:\, c:/, \\?\C:\)
  - Test symbolic links and junction point resolution
  - Verify Unicode character handling in paths
  - _Requirements: 1.1, 1.2, 1.5_

- [ ] 24. Add database performance tests
  - Create test database with 10,000+ media files
  - Benchmark directory listing performance before and after optimization
  - Verify memory usage remains bounded during large operations
  - _Requirements: 2.1, 2.2, 2.5_

- [ ] 25. Implement end-to-end DLNA browse testing
  - Create integration tests that scan media, store in database, and query via ObjectID
  - Verify path normalization consistency across scan-store-query cycle
  - Test various ObjectID formats and path constructions
  - _Requirements: 1.2, 1.3, 7.2_

- [ ] 26. Add regression tests for critical scenarios
  - Test mixed case paths on Windows
  - Test network paths and UNC formats
  - Test very long path names and special characters
  - _Requirements: 1.5, 2.1_

- [ ] 27. Create migration and upgrade tests
  - Test database migration from old path format to canonical format
  - Verify existing media libraries continue to work after upgrade
  - Test rollback scenarios if migration fails
  - _Requirements: 1.1, 1.3_