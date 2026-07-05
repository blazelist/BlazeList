# systemd serviceConfig hardening attrs tuned for the BlazeList server
# binary. Single source of truth: applied internally by the NixOS
# module (when `services.blazelist.hardening.enable = true`, the
# default) and surfaced via the flake's
# `lib.${system}.hardeningSettings` for downstream configs that run
# sibling units with the same threat model.
#
# Safe to inherit verbatim for static-binary network services that:
# - have no JIT (no writable+executable memory mappings),
# - listen only on AF_INET/AF_INET6/AF_UNIX/AF_NETLINK,
# - need only `@system-service` syscalls,
# - require no capabilities, no kernel module loading, no realtime
#   scheduling, no SUID/SGID, no extra namespaces.
#
# A typical reuse is a `miniserve` (or similar) sidecar serving
# user-uploaded assets next to the main server.
#
# DO NOT apply blindly to services that violate any of the above
# (JIT runtimes, raw sockets, kernel-module loaders, etc.).
{
  ProtectSystem = "strict";
  ProtectHome = true;
  PrivateTmp = true;
  PrivateDevices = true;
  ProtectKernelTunables = true;
  ProtectKernelModules = true;
  ProtectKernelLogs = true;
  ProtectControlGroups = true;
  ProtectClock = true;
  ProtectHostname = true;
  NoNewPrivileges = true;
  RestrictRealtime = true;
  RestrictSUIDSGID = true;
  LockPersonality = true;
  # Server is a pre-built native binary with no JIT (Just-In-Time
  # compilation), so it never needs writable+executable memory
  # mappings — free to deny them.
  MemoryDenyWriteExecute = true;
  RestrictAddressFamilies = [ "AF_INET" "AF_INET6" "AF_UNIX" "AF_NETLINK" ];
  CapabilityBoundingSet = "";
  AmbientCapabilities = "";
  RestrictNamespaces = true;
  SystemCallFilter = [ "@system-service" ];
  SystemCallArchitectures = "native";
  RemoveIPC = true;
  UMask = "0077";
}
