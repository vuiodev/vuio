# Requirements Document

## Introduction

This feature addresses a critical bug where the VuIO application fails to respect command line arguments for specifying media directories on Windows. The application currently defaults to using `C:\Users\Welcome\Videos` instead of the provided command line argument `"C:\Users\Welcome\Downloads\Video"`, and incorrectly reports that Windows drive letter colons are invalid characters in paths.

## Requirements

### Requirement 1

**User Story:** As a Windows user, I want to specify a custom media directory via command line arguments, so that I can serve media from any directory without modifying configuration files.

#### Acceptance Criteria

1. WHEN I run the application with a media directory argument THEN the system SHALL use that directory instead of platform defaults
2. WHEN I provide a valid Windows path with drive letters THEN the system SHALL accept colons as valid characters in drive letter positions
3. WHEN the provided path exists and is accessible THEN the system SHALL use it as the primary media directory
4. WHEN the provided path does not exist THEN the system SHALL display a clear error message indicating the path was not found

### Requirement 2

**User Story:** As a Windows user, I want proper Windows path validation, so that valid Windows paths with drive letters are not rejected due to incorrect character validation.

#### Acceptance Criteria

1. WHEN validating Windows paths with drive letters THEN the system SHALL allow colons in the drive letter position (e.g., "C:")
2. WHEN validating UNC paths THEN the system SHALL allow colons in network addresses (e.g., "\\\\server:port\\share")
3. WHEN validating other path components THEN the system SHALL reject colons in non-drive-letter positions
4. WHEN path validation fails THEN the system SHALL provide specific error messages indicating which validation rule failed

### Requirement 3

**User Story:** As a user, I want clear error messages when command line arguments are invalid, so that I can understand what went wrong and how to fix it.

#### Acceptance Criteria

1. WHEN command line argument parsing fails THEN the system SHALL display the specific parsing error
2. WHEN a provided media directory doesn't exist THEN the system SHALL show the exact path that was not found
3. WHEN path validation fails THEN the system SHALL explain which validation rule was violated
4. WHEN falling back to defaults THEN the system SHALL log why the command line argument was rejected

### Requirement 4

**User Story:** As a developer, I want robust path validation logic, so that the system correctly handles all valid Windows path formats without false positives.

#### Acceptance Criteria

1. WHEN checking for invalid characters THEN the system SHALL exclude drive letter positions from colon validation
2. WHEN checking for invalid characters THEN the system SHALL exclude UNC path components from colon validation  
3. WHEN normalizing paths THEN the system SHALL preserve valid Windows path structures
4. WHEN comparing paths THEN the system SHALL handle case-insensitive Windows path matching correctly

### Requirement 5

**User Story:** As a user, I want the application to prioritize command line arguments over configuration file settings, so that I can override defaults without modifying files.

#### Acceptance Criteria

1. WHEN both command line arguments and configuration files are present THEN command line arguments SHALL take precedence
2. WHEN command line arguments are provided THEN the system SHALL not fall back to configuration file media directories
3. WHEN command line arguments are invalid THEN the system SHALL report the error and exit rather than silently using defaults
4. WHEN no command line arguments are provided THEN the system SHALL use configuration file or platform defaults as normal