# Implementation Plan

- [x] 1. Create platform detection and abstraction foundation
  - Create new module `src/platform/mod.rs` with platform detection capabilities
  - Implement `PlatformInfo` struct with OS detection, version, and capabilities
  - Add conditional compilation flags for Windows, macOS, and Linux specific code
  - _Requirements: 1.3, 5.4, 7.2_

- [ ] 2. Implement cross-platform network manager
- [x] 2.1 Create network abstraction trait and base implementation
  - Define `NetworkManager` trait in `src/platform/network.rs`
  - Create `NetworkInterface` and `SsdpSocket` data structures
  - Implement base network interface detection logic
  - _Requirements: 3.1, 3.2, 5.3_

- [x] 2.2 Implement Windows-specific network manager
  - Create `src/platform/network/windows.rs` with Windows networking implementation
  - Handle privileged port binding with automatic fallback to alternative ports
  - Implement Windows firewall detection and user guidance
  - Add SO_REUSEADDR socket options for Windows compatibility
  - _Requirements: 1.1, 1.2, 1.3, 3.5_

- [x] 2.3 Implement macOS-specific network manager
  - Create `src/platform/network/macos.rs` with macOS networking implementation
  - Implement proper multicast interface selection for macOS
  - Handle macOS network interface enumeration and prioritization
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 2.4 Implement Linux-specific network manager
  - Create `src/platform/network/linux.rs` with Linux networking implementation
  - Handle multiple network namespaces and interface binding
  - Implement Linux-specific multicast group joining
  - _Requirements: 3.1, 3.2, 3.3_

- [x] 3. Refactor SSDP service to use platform abstraction
- [x] 3.1 Update SSDP service to use NetworkManager trait
  - Modify `src/ssdp.rs` to use the new `NetworkManager` abstraction
  - Replace direct socket binding with platform-aware socket creation
  - Implement graceful fallback when multicast fails
  - _Requirements: 1.1, 1.4, 3.3, 3.5_

- [x] 3.2 Add comprehensive SSDP error handling and recovery
  - Implement automatic port fallback when 1900 is unavailable
  - Add retry logic for multicast group joining failures
  - Create platform-specific error messages and troubleshooting guidance
  - _Requirements: 1.2, 1.4, 5.1, 5.3_

- [x] 4. Create database management system
- [x] 4.1 Implement SQLite database manager
  - Create `src/database/mod.rs` with `DatabaseManager` trait and SQLite implementation
  - Design database schema for media files with metadata storage
  - Implement CRUD operations for media file records
  - Add database initialization and migration support
  - _Requirements: 8.1, 8.2, 8.5_

- [x] 4.2 Add database error handling and recovery
  - Implement automatic database corruption detection and recovery
  - Add database backup and restore functionality
  - Create graceful fallback when database operations fail
  - _Requirements: 8.5, 8.6_

- [x] 5. Create file system watcher for real-time monitoring
- [x] 5.1 Implement cross-platform file system watcher
  - Create `src/watcher/mod.rs` with `FileSystemWatcher` trait
  - Use notify crate for cross-platform file system event monitoring
  - Implement event filtering for media file types only
  - Add debouncing to prevent excessive events during file operations
  - _Requirements: 8.3, 8.4_

- [x] 5.2 Integrate file watcher with database updates
  - Connect file system events to database operations
  - Implement automatic media file addition when new files are detected
  - Add automatic database cleanup when files are deleted
  - Handle file rename and move operations properly
  - _Requirements: 8.3, 8.4_

- [x] 6. Create configuration management system
- [x] 6.1 Implement configuration file handling
  - Create `src/config/mod.rs` with TOML-based configuration system
  - Define configuration schema for monitored directories and network settings
  - Implement configuration file loading with sensible defaults
  - Add configuration validation and error handling
  - _Requirements: 9.1, 9.2, 9.4_

- [x] 6.2 Add runtime configuration reloading
  - Implement configuration file watching for runtime updates
  - Add hot-reloading of monitored directories without restart
  - Update network interface selection when configuration changes
  - _Requirements: 9.3, 9.5_

- [x] 7. Create cross-platform file system manager
- [x] 7.1 Implement file system abstraction layer
  - Create `src/platform/filesystem.rs` with `FileSystemManager` trait
  - Implement cross-platform path normalization and validation
  - Add file permission checking and error handling
  - _Requirements: 4.1, 4.2, 4.3_

- [x] 7.2 Add Windows-specific file system handling
  - Handle Windows drive letters and UNC paths properly
  - Implement case-insensitive file matching for Windows NTFS
  - Add Windows file permission and access control integration
  - _Requirements: 4.1, 4.2, 4.4, 4.5_

- [x] 7.3 Update media scanning to use file system manager
  - Modify `src/media.rs` to use the new `FileSystemManager` abstraction
  - Replace full directory scans with incremental database-driven updates
  - Implement platform-specific file encoding handling
  - _Requirements: 4.1, 4.3, 4.4, 5.2, 8.6_

- [x] 8. Create cross-platform configuration management
- [x] 8.1 Implement platform-aware configuration system
  - Create `src/platform/config.rs` with `PlatformConfig` struct
  - Define platform-specific default directories (config, cache, logs, database)
  - Implement automatic detection of common media directories per platform
  - _Requirements: 6.4, 7.1, 9.1_

- [x] 8.2 Update main configuration to use platform defaults
  - Modify existing configuration system to use platform-aware defaults
  - Replace hardcoded paths with platform-appropriate directories
  - Add configuration validation for each platform
  - _Requirements: 6.1, 6.2, 6.3, 6.4, 9.2_

- [x] 9. Enhance error handling and logging
- [x] 9.1 Create platform-specific error types
  - Define `PlatformError` enum in `src/platform/error.rs`
  - Implement Windows, macOS, and Linux specific error variants
  - Add database and configuration error types
  - Add error recovery strategies and user guidance
  - _Requirements: 5.1, 5.2, 5.3_

- [x] 9.2 Update application error handling
  - Modify `src/error.rs` to integrate platform-specific errors
  - Add detailed error messages with platform-specific troubleshooting
  - Implement error recovery and fallback mechanisms for database and file watching
  - _Requirements: 5.1, 5.2, 5.5_

- [x] 9.3 Enhance logging with platform diagnostics
  - Add platform-specific diagnostic information to log messages
  - Include network interface details and system configuration in debug logs
  - Create platform-specific startup diagnostic checks
  - Add database and file watcher status logging
  - _Requirements: 5.1, 5.3, 5.4, 5.5_

- [x] 10. Create cross-platform build system
- [x] 10.1 Create PowerShell build script for Windows
  - Write `build.ps1` equivalent to existing `build.sh` bash script
  - Handle Windows-specific cross-compilation requirements
  - Add automatic detection of required Windows build tools
  - _Requirements: 2.1, 2.2_

- [x] 10.2 Enhance Cargo configuration for cross-compilation
  - Update `.cargo/config.toml` with comprehensive cross-platform settings
  - Add Windows MSVC and GNU toolchain configurations
  - Configure proper linking flags for each target platform
  - Add SQLite static linking configuration for all platforms
  - _Requirements: 2.2, 2.3_

- [x] 10.3 Create platform-specific package generation
  - Implement MSI installer generation for Windows using WiX or similar
  - Create macOS PKG installer or Homebrew formula configuration
  - Add DEB and RPM package generation for Linux distributions
  - Include default configuration files in all package formats
  - _Requirements: 2.3, 6.1, 6.2, 6.3_

- [x] 11. Add comprehensive cross-platform testing
- [x] 11.1 Create platform-specific unit tests
  - Write Windows-specific tests for network manager and file system handling
  - Create macOS-specific tests for multicast and interface detection
  - Implement Linux-specific tests for network namespaces and permissions
  - Add database and file watcher unit tests for each platform
  - _Requirements: 7.2, 7.4_

- [x] 11.2 Add integration tests for cross-platform functionality
  - Create end-to-end DLNA discovery tests for each platform
  - Test file serving with platform-specific file systems and permissions
  - Validate error handling and recovery mechanisms across platforms
  - Test database operations and file watching integration
  - Test configuration loading and hot-reloading functionality
  - _Requirements: 7.2, 7.4_

- [x] 11.3 Set up continuous integration for all platforms
  - Configure GitHub Actions matrix for Windows, macOS, and Linux builds
  - Add automated testing on virtual machines for each target platform
  - Implement cross-compilation validation and artifact generation
  - Add database migration and file watching tests to CI pipeline
  - _Requirements: 2.4, 7.4_

- [x] 12. Create platform-specific documentation and guides
- [x] 12.1 Write Windows setup and troubleshooting guide
  - Document Windows installation process and common issues
  - Create firewall configuration guide for Windows users
  - Add troubleshooting section for Windows-specific networking problems
  - Document configuration file location and format for Windows
  - _Requirements: 6.1, 6.4, 7.5, 9.1_

- [x] 12.2 Create macOS installation and configuration guide
  - Document macOS installation via PKG installer or Homebrew
  - Add macOS-specific permission and security configuration
  - Create troubleshooting guide for macOS networking issues
  - Document configuration file management on macOS
  - _Requirements: 6.2, 6.4, 7.5, 9.1_

- [x] 12.3 Write Linux distribution-specific guides
  - Create installation guides for major Linux distributions (Ubuntu, CentOS, etc.)
  - Document SELinux and AppArmor configuration requirements
  - Add systemd service configuration examples
  - Document configuration file locations and permissions for Linux
  - _Requirements: 6.3, 6.4, 7.5, 8.3, 9.1_

- [x] 13. Update main application to integrate all platform features
- [x] 13.1 Refactor main.rs to use platform abstraction
  - Update `src/main.rs` to initialize platform-specific managers
  - Replace direct networking calls with platform abstraction layer
  - Add comprehensive startup diagnostics and error reporting
  - Initialize database manager and file system watcher
  - _Requirements: 1.1, 3.1, 5.4, 6.4, 8.1, 8.3_

- [x] 13.2 Add runtime platform detection and adaptation
  - Implement automatic platform detection and configuration at startup
  - Add dynamic adaptation to network changes and interface availability
  - Create graceful degradation when platform features are unavailable
  - Implement configuration hot-reloading and directory monitoring updates
  - _Requirements: 3.3, 3.4, 5.4, 6.4, 9.3_

- [x] 13.3 Integrate security and permissions handling
  - Add platform-specific security checks and permission requests
  - Implement proper integration with Windows UAC, macOS permissions, and Linux capabilities
  - Create secure defaults and privilege minimization for each platform
  - _Requirements: 10.1, 10.2, 10.3, 10.4, 10.5_

- [x] 13.4 Implement startup sequence with database and file watching
  - Create proper initialization order: config → database → file watcher → network
  - Add incremental media scanning on startup using database state
  - Start file system monitoring for all configured directories
  - Implement graceful shutdown with proper cleanup of all resources
  - _Requirements: 8.1, 8.2, 8.6, 9.1, 9.3_