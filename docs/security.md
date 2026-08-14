# Security & Authentication Guide

By default, VuIO operates in open/public mode on the local network for frictionless setup. In shared network environments, multi-tenant setups, or remote deployments, administrative authentication can be enabled to secure the web dashboard and management API endpoints.

---

## Enabling Authentication

To enable administrative/management authentication, start VuIO using any of the following methods:

1. **Command Line Flag**:
   ```bash
   vuio --auth /path/to/media
   ```

2. **Environment Variable**:
   ```bash
   export VUIO_AUTH=true
   # or
   export VUIO_MANAGEMENT_ENABLED=true
   ```

3. **Configuration File (`config.toml`)**:
   ```toml
   [management]
   enabled = true
   ```

---

## Authentication Behavior

When authentication is enabled:
- **Web Dashboard**: Navigating to `http://<server-ip>:8080` or `http://<server-ip>:8090` redirects to a secure login page.
- **Session Tokens**: Authenticated sessions receive a secure HTTP cookie valid for the configured TTL (default: 12 hours).
- **REST & Management APIs**: Endpoints requiring admin access (`/api/admin/*`, `/api/playlists/*`, etc.) require either a valid session cookie or an `Authorization: Bearer <token>` header.
- **DLNA / UPnP SOAP Endpoints**: UPnP/DLNA discovery and SOAP endpoints (`/description.xml`, `/ContentDirectory/*`, `/ConnectionManager/*`) remain openly accessible so that smart TVs and media players on the LAN continue to stream without credential prompts.

---

## Managing the Admin Token

### Auto-Generated Token

If authentication is enabled and no explicit token is provided, VuIO automatically generates a cryptographically secure random token on startup and writes it to:

- Native: `./admin.token` (or next to your `config.toml`)
- Linux / Systemd: `/etc/vuio/admin.token`

Keep this file protected:
```bash
chmod 600 admin.token
```

### Pre-Configured Token via Environment Variable

To define your own administrative token (recommended for automated deployments, Docker, and CI/CD):

```bash
export VUIO_ADMIN_TOKEN="your-secure-custom-token-here"
```

### Custom Token File Location

You can configure the token file location in `config.toml`:

```toml
[management]
enabled = true
token_file = "/etc/vuio/admin.token"
session_ttl_hours = 24
```

---

## Network Restrictions (CIDR Filtering)

You can restrict administrative access to specific subnets or IP ranges using the `allowed_networks` parameter in `config.toml`:

```toml
[management]
enabled = true
# Allow only localhost and the 192.168.1.0/24 subnet
allowed_networks = ["127.0.0.1/32", "::1/128", "192.168.1.0/24"]
```

When `allowed_networks` is left empty, VuIO permits loopback and standard RFC1918 private/link-local addresses by default.

---

## Related Documentation

- [Configuration Reference](configuration.md)
- [MCP AI Integration](mcp.md)
- [API Reference](api.md)
