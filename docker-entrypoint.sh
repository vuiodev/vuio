#!/bin/sh
set -e

# Enhanced Docker entrypoint for VuIO with user switching and proper permissions

echo "=== VuIO Docker Container Starting ==="
echo "Container User: $(id)"
echo "PUID: ${PUID:-1000}, PGID: ${PGID:-1000}"

# Function to log with timestamp
log() {
    echo "[$(date '+%Y-%m-%d %H:%M:%S')] $1"
}

# Function to handle errors
error_exit() {
    log "ERROR: $1"
    exit 1
}

# Validate environment variables
validate_env() {
    if [ -z "$VUIO_PORT" ]; then
        error_exit "VUIO_PORT environment variable is required"
    fi
    
    if [ "$VUIO_PORT" -lt 1 ] || [ "$VUIO_PORT" -gt 65535 ]; then
        error_exit "VUIO_PORT must be between 1 and 65535"
    fi
    
    log "Environment validation passed"
}

# Setup user and group with dynamic IDs
setup_user() {
    local target_uid=${PUID:-1000}
    local target_gid=${PGID:-1000}
    
    log "Setting up user with UID=$target_uid, GID=$target_gid"
    
    # Get current IDs
    local current_uid=$(id -u vuio)
    local current_gid=$(id -g vuio)
    
    # Update group if needed
    if [ "$current_gid" != "$target_gid" ]; then
        log "Updating vuio group ID from $current_gid to $target_gid"
        groupmod -g "$target_gid" vuio || error_exit "Failed to update group ID"
    fi
    
    # Update user if needed
    if [ "$current_uid" != "$target_uid" ]; then
        log "Updating vuio user ID from $current_uid to $target_uid"
        usermod -u "$target_uid" vuio || error_exit "Failed to update user ID"
    fi
    
    log "User setup completed"
}

# Setup directories and permissions
setup_directories() {
    log "Setting up directories and permissions"
    
    # Ensure directories exist
    mkdir -p /config /media /app
    
    # Set ownership
    chown -R vuio:vuio /config /media /app
    
    # Set permissions
    chmod 755 /config /media /app
    chmod +x /app/vuio
    
    log "Directory setup completed"
}

# Validate existing configuration
validate_existing_config() {
    local config_file="/config/config.toml"
    
    # Check if file exists and is readable
    if [ ! -f "$config_file" ] || [ ! -r "$config_file" ]; then
        return 1
    fi
    
    # Check for required sections
    if ! grep -q "\[server\]" "$config_file" || 
       ! grep -q "\[network\]" "$config_file" || 
       ! grep -q "\[media\]" "$config_file" || 
       ! grep -q "\[database\]" "$config_file"; then
        log "Configuration file missing required sections"
        return 1
    fi
    
    # Check for required fields
    if ! grep -q "uuid =" "$config_file" || 
       ! grep -q "port =" "$config_file" || 
       ! grep -q "path =" "$config_file"; then
        log "Configuration file missing required fields"
        return 1
    fi
    
    # Basic TOML syntax check - ensure no obvious syntax errors
    if grep -q "^\[.*\]\[" "$config_file"; then
        log "Configuration file has TOML syntax errors"
        return 1
    fi
    
    return 0
}

# Check if environment variables require config updates
should_update_config() {
    local config_file="/config/config.toml"
    
    # Check if critical environment variables differ from config
    local current_port=$(grep '^port' "$config_file" | sed 's/port = \([0-9]*\)/\1/' || echo "")
    local current_name=$(grep '^name' "$config_file" | sed 's/name = "\(.*\)"/\1/' || echo "")
    local current_interface=$(grep '^interface' "$config_file" | sed 's/interface = "\(.*\)"/\1/' || echo "")
    
    [ "${VUIO_PORT:-8080}" != "$current_port" ] || 
    [ "${VUIO_SERVER_NAME:-VuIO}" != "$current_name" ] || 
    [ "${VUIO_BIND_INTERFACE:-0.0.0.0}" != "$current_interface" ]
}

# Update existing configuration with environment variables
update_existing_config() {
    local config_file="/config/config.toml"
    local temp_file="/tmp/config_update.toml"
    
    # Create backup
    cp "$config_file" "${config_file}.backup" || error_exit "Failed to create config backup"
    
    # Update port if different
    if [ "${VUIO_PORT:-8080}" != "$(grep '^port' "$config_file" | sed 's/port = \([0-9]*\)/\1/')" ]; then
        sed "s/^port = .*/port = ${VUIO_PORT:-8080}/" "$config_file" > "$temp_file" && mv "$temp_file" "$config_file"
        log "Updated port to ${VUIO_PORT:-8080}"
    fi
    
    # Update server name if different
    if [ "${VUIO_SERVER_NAME:-VuIO}" != "$(grep '^name' "$config_file" | sed 's/name = "\(.*\)"/\1/')" ]; then
        sed "s/^name = .*/name = \"${VUIO_SERVER_NAME:-VuIO}\"/" "$config_file" > "$temp_file" && mv "$temp_file" "$config_file"
        log "Updated server name to ${VUIO_SERVER_NAME:-VuIO}"
    fi
    
    # Update interface if different
    if [ "${VUIO_BIND_INTERFACE:-0.0.0.0}" != "$(grep '^interface' "$config_file" | sed 's/interface = "\(.*\)"/\1/')" ]; then
        sed "s/^interface = .*/interface = \"${VUIO_BIND_INTERFACE:-0.0.0.0}\"/" "$config_file" > "$temp_file" && mv "$temp_file" "$config_file"
        log "Updated bind interface to ${VUIO_BIND_INTERFACE:-0.0.0.0}"
    fi
    
    # Set proper ownership
    chown vuio:vuio "$config_file"
    chmod 644 "$config_file"
}

# Generate configuration file
generate_config() {
    local config_file="/config/config.toml"
    
    # Check if valid config already exists
    if [ -f "$config_file" ] && validate_existing_config; then
        log "Valid configuration file already exists: $config_file"
        
        # Check if environment variables require config updates
        if should_update_config; then
            log "Updating configuration with new environment variables"
            update_existing_config
        else
            log "Skipping regeneration to preserve user customizations"
        fi
        return 0
    fi
    
    log "Generating configuration file: $config_file"
    
    # Read existing UUID if present
    local uuid=""
    if [ -f "$config_file" ]; then
        uuid=$(grep '^uuid' "$config_file" 2>/dev/null | sed 's/uuid = "\(.*\)"/\1/' || true)
        log "Found existing UUID: ${uuid:-none}"
    fi
    
    # Generate new UUID if none exists
    if [ -z "$uuid" ]; then
        if command -v uuidgen >/dev/null 2>&1; then
            uuid=$(uuidgen)
        elif [ -f /proc/sys/kernel/random/uuid ]; then
            uuid=$(cat /proc/sys/kernel/random/uuid)
        else
            # Last resort: generate pseudo-UUID
            uuid=$(printf "%08x-%04x-%04x-%04x-%012x" \
                $(($(date +%s) % 4294967296)) \
                $((RANDOM % 65536)) \
                $((RANDOM % 65536)) \
                $((RANDOM % 65536)) \
                $((RANDOM % 281474976710656)))
        fi
        log "Generated new UUID: $uuid"
    else
        log "Using existing UUID: $uuid"
    fi
    
    # Validate UUID format
    if ! echo "$uuid" | grep -qE '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'; then
        error_exit "Invalid UUID format: $uuid"
    fi
    
    # Generate configuration with error checking
    cat > "$config_file" <<EOF || error_exit "Failed to write configuration file"
# This file is auto-generated by docker-entrypoint.sh on container start.
# Do not edit this file directly. Use environment variables instead.
# Generated at: $(date -Iseconds)

[server]
port = ${VUIO_PORT:-8080}
interface = "${VUIO_BIND_INTERFACE:-0.0.0.0}"
name = "${VUIO_SERVER_NAME:-VuIO}"
uuid = "$uuid"

[network]
ssdp_port = ${VUIO_SSDP_PORT:-1900}
multicast_ttl = 4
announce_interval_seconds = 300
EOF

    # Handle network interface selection
    local ssdp_interface="${VUIO_SSDP_INTERFACE:-Auto}"
    if [ "$ssdp_interface" = "Auto" ] || [ "$ssdp_interface" = "All" ]; then
        echo "interface_selection = \"$ssdp_interface\"" >> "$config_file"
    else
        echo "interface_selection = \"$ssdp_interface\"" >> "$config_file"
    fi
    
    # Add media and database configuration
    cat >> "$config_file" <<EOF || error_exit "Failed to append configuration"

[media]
scan_on_startup = true
watch_for_changes = true
cleanup_deleted_files = true
supported_extensions = [
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "3gp", "ogv",
    "mp3", "flac", "wav", "aac", "ogg", "wma", "m4a", "opus", "ape",
    "jpg", "jpeg", "png", "gif", "bmp", "webp", "tiff", "svg"
]

[[media.directories]]
path = "${VUIO_MEDIA_DIR:-/media}"
recursive = true

[database]
path = "/config/media.db"
vacuum_on_startup = false
backup_enabled = true
EOF
    
    # Set proper ownership and permissions
    chown vuio:vuio "$config_file"
    chmod 644 "$config_file"
    
    log "Configuration file generated successfully"
    
    # Display configuration (without sensitive data)
    log "Configuration summary:"
    log "  Server port: ${VUIO_PORT:-8080}"
    log "  Server name: ${VUIO_SERVER_NAME:-VuIO}"
    log "  Bind interface: ${VUIO_BIND_INTERFACE:-0.0.0.0}"
    log "  SSDP interface: ${VUIO_SSDP_INTERFACE:-Auto}"
    log "  Media directory: ${VUIO_MEDIA_DIR:-/media}"
}

# Validate generated configuration
validate_config() {
    local config_file="/config/config.toml"
    
    log "Validating configuration file"
    
    if ! validate_existing_config; then
        error_exit "Configuration validation failed"
    fi
    
    log "Configuration validation passed"
}

# Display system information
show_system_info() {
    log "System Information:"
    log "  Container OS: $(cat /etc/os-release | grep PRETTY_NAME | cut -d'=' -f2 | tr -d '"')"
    log "  Architecture: $(uname -m)"
    log "  Kernel: $(uname -r)"
    log "  Available interfaces: $(ls /sys/class/net/ | tr '\n' ' ')"
    
    if [ -d "/media" ]; then
        local media_count=$(find /media -type f 2>/dev/null | wc -l || echo "0")
        log "  Media files found: $media_count"
    fi
}

# Main execution
main() {
    log "Starting VuIO container initialization"
    
    # Run setup steps
    validate_env
    setup_user
    setup_directories
    generate_config
    validate_config
    show_system_info
    
    log "Initialization completed successfully"
    log "Starting VuIO as user vuio with command: $*"
    
    # Switch to vuio user and execute the command
    exec su-exec vuio "$@"
}

# Execute main function with all arguments
main "$@"