{ config, lib, pkgs, ... }:

with lib;

let
  cfg = config.services.vuio;
  vuioPkg = if cfg.package != null then cfg.package else pkgs.callPackage ./vuio.nix {};
in {
  options.services.vuio = {
    enable = mkEnableOption "VuIO DLNA Media Server";

    package = mkOption {
      type = types.nullOr types.package;
      default = null;
      description = "The vuio package to use. Defaults to compiling from local release derivation.";
    };

    port = mkOption {
      type = types.port;
      default = 8080;
      description = "Port the DLNA/web dashboard will listen on.";
    };

    interface = mkOption {
      type = types.str;
      default = "0.0.0.0";
      description = "Network interface address to bind to.";
    };

    friendlyName = mkOption {
      type = types.str;
      default = "Vuio";
      description = "Friendly DLNA server name.";
    };

    mediaDirectories = mkOption {
      type = types.listOf types.str;
      default = [ "/var/lib/vuio/media" ];
      description = "List of folders monitored for media files.";
    };

    configFile = mkOption {
      type = types.nullOr types.path;
      default = null;
      description = "Optional path to a custom vuio.toml configuration file. If not provided, one will be generated from module options.";
    };
  };

  config = mkIf cfg.enable {
    # Generate default configuration if custom is not provided
    environment.etc."vuio/vuio.toml" = mkIf (cfg.configFile == null) {
      text = ''
        [server]
        port = ${toString cfg.port}
        interface = "${cfg.interface}"
        name = "${cfg.friendlyName}"

        [network]
        ssdp_port = 1900
        interface_selection = "auto"
        multicast_ttl = 4
        announce_interval_seconds = 30

        [media]
        scan_on_startup = true
        watch_for_changes = true
        supported_extensions = ["mp4", "mkv", "avi", "ts", "m2ts", "mp3", "flac", "wav", "jpg", "png", "gif"]

        ${concatMapStrings (dir: ''
          [[media.directories]]
          path = "${dir}"
          recursive = true
        '') cfg.mediaDirectories}

        [database]
        vacuum_on_startup = false
        backup_enabled = true
      '';
      mode = "0640";
      user = "vuio";
      group = "vuio";
    };

    # Systemd service
    systemd.services.vuio = {
      description = "VuIO DLNA Media Server";
      after = [ "network.target" ];
      wantedBy = [ "multi-user.target" ];
      
      serviceConfig = {
        ExecStart = "${vuioPkg}/bin/vuio --config ${if cfg.configFile != null then cfg.configFile else "/etc/vuio/vuio.toml"}";
        User = "vuio";
        Group = "vuio";
        Restart = "always";
        RestartSec = 5;
        StateDirectory = "vuio";
        LogsDirectory = "vuio";
        
        # Hardening
        NoNewPrivileges = true;
        PrivateTmp = true;
        ProtectSystem = "strict";
        ProtectHome = true;
        ProtectKernelTunables = true;
        ProtectKernelModules = true;
        ProtectControlGroups = true;
        ReadWritePaths = [ "/var/log/vuio" "/var/lib/vuio" ];
      };
    };

    # System users/groups
    users.users.vuio = {
      isSystemUser = true;
      group = "vuio";
      description = "VuIO Media Server service user";
      home = "/var/lib/vuio";
      createHome = true;
    };

    users.groups.vuio = {};
  };
}
