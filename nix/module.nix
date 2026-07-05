{ self }:
{ config, lib, pkgs, ... }:

let
  cfg = config.services.blazelist;

  # Convert a lowerCamelCase attr name into BLAZELIST_DEFAULT_SCREAMING_SNAKE.
  # e.g. autoSyncIntervalMs -> BLAZELIST_DEFAULT_AUTO_SYNC_INTERVAL_MS
  envName = name:
    let
      chars = lib.stringToCharacters name;
      go = acc: c:
        if c >= "A" && c <= "Z"
        then acc + "_" + c
        else acc + (lib.toUpper c);
      screaming = lib.foldl' go "" chars;
    in
    "BLAZELIST_DEFAULT_" + screaming;

  # Coerce a settingsDefaults value (bool/int/str/float) to a string.
  toEnv = v:
    if builtins.isBool v then (if v then "true" else "false")
    else toString v;

  settingsEnv =
    lib.mapAttrs' (n: v: lib.nameValuePair (envName n) (toEnv v)) cfg.settingsDefaults;

  cli = lib.escapeShellArgs ([
    "--quic-port" (toString cfg.ports.quic)
    "--wt-port" (toString cfg.ports.wt)
    "--http-port" (toString cfg.ports.http)
    "--https-port" (toString cfg.ports.https)
    "--bind" cfg.bind
    "--static-dir" cfg.staticDir
    "--db" cfg.dbPath
  ] ++ cfg.extraArgs);

  hardeningSettings = lib.optionalAttrs cfg.hardening.enable
    (import ./hardening-settings.nix);

in
{
  options.services.blazelist = {
    enable = lib.mkEnableOption "the BlazeList server";

    package = lib.mkOption {
      type = lib.types.package;
      default = self.packages.${pkgs.stdenv.hostPlatform.system}.blazelist;
      defaultText = lib.literalExpression
        "blazelist.packages.\${pkgs.stdenv.hostPlatform.system}.blazelist";
      description = ''
        The BlazeList package. Defaults to the flake's combined `blazelist`
        package (server binary + WASM dist under `share/blazelist/dist`).
        Override with `lib.buildFromCommit` to pin a specific rev with
        signature verification.
      '';
    };

    staticDir = lib.mkOption {
      type = lib.types.str;
      default = "${cfg.package}/share/blazelist/dist";
      defaultText = lib.literalExpression
        ''"''${cfg.package}/share/blazelist/dist"'';
      description = ''
        Directory served as the WASM client. Defaults to the dist tree
        inside the combined package; override to point at a separately
        built bundle.
      '';
    };

    user = lib.mkOption {
      type = with lib.types; either str ints.unsigned;
      default = "blazelist";
      description = "User name or UID to run as. Not created by the module.";
    };

    group = lib.mkOption {
      type = with lib.types; either str ints.unsigned;
      default = "blazelist";
      description = "Group name or GID to run as. Not created by the module.";
    };

    bind = lib.mkOption {
      type = lib.types.str;
      default = "127.0.0.1";
      description = "Bind address for all listeners.";
    };

    ports = {
      quic = lib.mkOption {
        type = lib.types.port;
        default = 47200;
        description = "QUIC (native clients) UDP port.";
      };
      wt = lib.mkOption {
        type = lib.types.port;
        default = 47400;
        description = "WebTransport (browser clients) UDP port.";
      };
      http = lib.mkOption {
        type = lib.types.port;
        default = 47600;
        description = "HTTP cert-hash endpoint TCP port.";
      };
      https = lib.mkOption {
        type = lib.types.port;
        default = 47800;
        description = "HTTPS (web UI + API) TCP port.";
      };
    };

    dbPath = lib.mkOption {
      type = lib.types.str;
      default = "/var/lib/blazelist/blazelist.db";
      description = ''
        Path to the SQLite database file. The parent directory is added
        to `ReadWritePaths`; create and own it yourself (e.g. via tmpfiles
        or a ZFS dataset module).
      '';
    };

    allowIrreversibleAutomaticUpgradeMigration = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Sets `BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION=true`,
        permitting on-startup migrations that cannot be rolled back. Pair
        with ZFS snapshots if you care about recovery.
      '';
    };

    sqliteCheckpointInterval = lib.mkOption {
      type = lib.types.ints.positive;
      default = 300;
      description = ''
        Sets `BLAZELIST_SQLITE_CHECKPOINT_INTERVAL` (seconds). Controls
        how often the server checkpoints the SQLite WAL.
      '';
    };

    settingsDefaults = lib.mkOption {
      type = lib.types.attrsOf (lib.types.oneOf [
        lib.types.bool
        lib.types.int
        lib.types.float
        lib.types.str
      ]);
      default = { };
      example = lib.literalExpression ''
        {
          autoSync = true;
          autoSyncIntervalMs = 30000;
          uiDensity = "compact";
          swipeUndoTimeoutMs = 4000;
        }
      '';
      description = ''
        Server-side defaults for client settings, surfaced via /config.
        Keys are lowerCamelCase and converted to `BLAZELIST_DEFAULT_*`
        screaming-snake env vars at unit-launch time. See the client's
        settings module for the full list of supported keys.
      '';
    };

    extraEnvironment = lib.mkOption {
      type = lib.types.attrsOf lib.types.str;
      default = { };
      description = "Extra environment variables for the service. Escape hatch.";
    };

    extraArgs = lib.mkOption {
      type = lib.types.listOf lib.types.str;
      default = [ ];
      description = "Extra CLI arguments appended to `ExecStart`.";
    };

    hardening.enable = lib.mkOption {
      type = lib.types.bool;
      default = true;
      description = ''
        Apply the systemd hardening block (ProtectSystem=strict,
        capability dropping, SystemCallFilter, MemoryDenyWriteExecute,
        etc.). Disable only for development VMs.
      '';
    };

    openFirewall = lib.mkOption {
      type = lib.types.bool;
      default = false;
      description = ''
        Open the configured ports in the NixOS firewall. Off by default —
        many deployments expose BlazeList through a reverse proxy or a
        private network interface and don't want the raw ports open on
        the public firewall.
      '';
    };
  };

  config = lib.mkIf cfg.enable {
    systemd.services.blazelist = {
      description = "BlazeList server";
      wantedBy = [ "multi-user.target" ];
      after = [ "network-online.target" ];
      wants = [ "network-online.target" ];

      environment = lib.mkMerge [
        (lib.optionalAttrs cfg.allowIrreversibleAutomaticUpgradeMigration {
          BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION = "true";
        })
        { BLAZELIST_SQLITE_CHECKPOINT_INTERVAL = toString cfg.sqliteCheckpointInterval; }
        settingsEnv
        cfg.extraEnvironment
      ];

      serviceConfig = {
        User = toString cfg.user;
        Group = toString cfg.group;

        ExecStart = "${cfg.package}/bin/blazelist-server ${cli}";

        Restart = lib.mkDefault "always";
        RestartSec = lib.mkDefault 5;

        ReadWritePaths = [ (builtins.dirOf cfg.dbPath) ];

        RuntimeDirectory = "blazelist";
        WorkingDirectory = "/run/blazelist";

        LimitNOFILE = 65536;
      } // hardeningSettings;
    };

    networking.firewall = lib.mkIf cfg.openFirewall {
      allowedTCPPorts = [ cfg.ports.http cfg.ports.https ];
      allowedUDPPorts = [ cfg.ports.quic cfg.ports.wt ];
    };
  };
}
