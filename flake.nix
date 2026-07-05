{
  description = "BlazeList — high-performance TODO list (QUIC + WebTransport + WASM)";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    crane.url = "github:ipetkov/crane";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, crane }:
    let
      # Server crate's pname/version, derived from its own Cargo.toml so the
      # flake stays in sync without a manual bump.
      serverManifest = (builtins.fromTOML (builtins.readFile ./server/Cargo.toml)).package;

      # Public PGP key that signs BlazeList release commits. Committed in-tree
      # so the flake can enforce signature verification without pulling the
      # key from a network source.
      releaseSigningKey = ./release-signing-key.asc;

      # Default URL used by `lib.buildFromCommit` when no `url` is given.
      defaultUpstreamUrl = "https://github.com/blazelist/BlazeList.git";

      perSystem = system:
        let
          pkgs = import nixpkgs {
            inherit system;
            overlays = [ rust-overlay.overlays.default ];
          };
          lib = pkgs.lib;

          # Stable Rust + wasm32 target + analyzer/src for dev shell.
          rustToolchain = pkgs.rust-bin.stable.latest.default.override {
            extensions = [ "rust-src" "rust-analyzer" ];
            targets = [ "wasm32-unknown-unknown" ];
          };

          craneLib = (crane.mkLib pkgs).overrideToolchain rustToolchain;

          # wasm-bindgen requires the CLI version to match the crate version
          # exactly. Read the version straight from Cargo.lock so the pin
          # can't drift when the lockfile is bumped — a desync previously
          # broke CI: the mismatched CLI made trunk try to download a matching
          # one into the sandbox's read-only $HOME ("failed creating cache
          # directory"). nixpkgs lacks an attr for every release, so build the
          # CLI via `buildWasmBindgenCli` with hand-pinned FOD hashes. Refresh
          # BOTH hashes when the version changes; a stale hash now fails loudly
          # as `hash mismatch … wasm-bindgen-cli-<ver>` (get the values from
          # `nix build .#blazelist-wasm-dist`).
          cargoLock = builtins.fromTOML (builtins.readFile ./Cargo.lock);
          wasmBindgenVersion =
            (lib.findFirst (p: p.name == "wasm-bindgen")
              (throw "wasm-bindgen not found in Cargo.lock")
              cargoLock.package).version;

          wasmBindgenCli = pkgs.buildWasmBindgenCli rec {
            src = pkgs.fetchCrate {
              pname = "wasm-bindgen-cli";
              version = wasmBindgenVersion;
              hash = "sha256-ymeAEYsr7OnupWYJWjSeVGvq3+s+zxSNkODbzY62rYs=";
            };

            cargoDeps = pkgs.rustPlatform.fetchCargoVendor {
              inherit src;
              inherit (src) pname version;
              hash = "sha256-d7x6gtx5OqEE4MyT6yjYn/qtgjx7GroTpXJewnBV2dU=";
            };
          };

          # Shared crane args. cleanCargoSource trims the source to just the
          # crate metadata + Cargo.lock for the dep-only build, avoiding
          # spurious rebuilds when README/docs change.
          commonArgs = {
            src = craneLib.cleanCargoSource ./.;
            strictDeps = true;
            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ];
          };

          # Native workspace dep cache. Shared by the server build and the
          # workspace tests check. Building deps once and reusing them
          # keeps `nix flake check` fast.
          cargoArtifacts = craneLib.buildDepsOnly (commonArgs // {
            pname = "blazelist-deps";
            version = serverManifest.version;
          });

          # ----------------------------------------------------------------
          # blazelist-server: native release binary.
          #
          # doCheck = false: tests run as a separate workspace check
          # derivation, so a deployment build doesn't pay for the test
          # suite. CI surfaces tests as a parallel job; `nix flake check`
          # still runs them.
          # ----------------------------------------------------------------
          blazelist-server = craneLib.buildPackage (commonArgs // {
            inherit cargoArtifacts;
            pname = "blazelist-server";
            version = serverManifest.version;
            cargoExtraArgs = "-p blazelist-server";
            doCheck = false;

            meta = {
              description = "High-performance TODO list server (QUIC + WebTransport)";
              homepage = "https://github.com/blazelist/BlazeList";
              license = with lib.licenses; [ mit asl20 ];
              mainProgram = "blazelist-server";
              platforms = lib.platforms.linux ++ lib.platforms.darwin;
            };
          });

          # ----------------------------------------------------------------
          # blazelist-wasm-dist: trunk-built dist/ with sw.js post-processed
          # by inject-precache.sh. Output is the dist/ tree, ready to serve.
          # ----------------------------------------------------------------
          #
          # Note: we use the full src (not cleanCargoSource) because Trunk
          # needs index.html, the style/ tree, public/ assets, and the
          # inject-precache.sh script in addition to Cargo metadata.
          blazelist-wasm-dist = craneLib.mkCargoDerivation {
            src = ./.;
            pname = "blazelist-wasm-dist";
            version = (builtins.fromTOML (builtins.readFile ./clients/wasm/Cargo.toml)).package.version;
            cargoArtifacts = null;
            cargoVendorDir = craneLib.vendorCargoDeps { src = ./.; };

            nativeBuildInputs = [
              rustToolchain
              pkgs.trunk
              wasmBindgenCli
              pkgs.binaryen # wasm-opt
              pkgs.pkg-config
            ];

            buildPhaseCargoCommand = ''
              # Trunk's TRUNK_TOOLS_* env vars take version strings, not
              # paths. wasm-bindgen and wasm-opt are on PATH via
              # nativeBuildInputs; tell Trunk to trust the version it finds
              # instead of probing its own expected version.
              export TRUNK_SKIP_VERSION_CHECK=true
              # --offline blocks Trunk from the network for both deps and
              # tools. The sandbox has no network anyway, but passing it
              # explicitly turns a tool-version mismatch into an immediate,
              # legible failure instead of a doomed download into a read-only
              # cache dir.
              trunk build --offline --release --config clients/wasm/Trunk.toml
              # Rewrite sw.js with content-hash CACHE_NAME + URL list.
              sh clients/wasm/inject-precache.sh clients/wasm/dist
            '';

            installPhaseCommand = ''
              mkdir -p $out
              cp -R clients/wasm/dist/. $out/
            '';

            # Don't save crane's cargo artifacts archive into $out — we
            # only want the dist tree.
            doInstallCargoArtifacts = false;
            doCheck = false;
          };

          # ----------------------------------------------------------------
          # Combined output: server bin + WASM dist in one store path.
          # ----------------------------------------------------------------
          blazelist = pkgs.symlinkJoin {
            name = "blazelist-${serverManifest.version}";
            paths = [
              blazelist-server
              (pkgs.runCommand "blazelist-wasm-share" { } ''
                mkdir -p $out/share/blazelist
                cp -R ${blazelist-wasm-dist} $out/share/blazelist/dist
              '')
            ];
            passthru = {
              server = blazelist-server;
              wasm = blazelist-wasm-dist;
              version = serverManifest.version;
            };
            meta = blazelist-server.meta;
          };

          # ----------------------------------------------------------------
          # lib.verifyCommitSignature / lib.buildFromCommit
          #
          # verify defaults to true — refuses to build on an unsigned or
          # wrong-key rev. Pass `verify = false` to opt out (e.g. when
          # building from a fork that doesn't sign its commits).
          # ----------------------------------------------------------------
          verifyCommitSignature = src: rev:
            pkgs.runCommand "blazelist-${builtins.substring 0 8 rev}-verified" {
              nativeBuildInputs = [ pkgs.gnupg pkgs.git ];
              passthru = { inherit rev; };
            } ''
              export GNUPGHOME=$(mktemp -d)
              gpg --batch --quiet --import ${releaseSigningKey}
              cp -R ${src} $out
              chmod -R u+w $out
              cd $out
              echo "Verifying commit signature on $(git rev-parse HEAD)…"
              git verify-commit HEAD
              echo "Signature verified against committed release-signing-key.asc."
              # Strip .git so downstream Nix builds are content-addressable.
              rm -rf .git
            '';

          buildFromCommit =
            { rev
            , hash
            , url ? defaultUpstreamUrl
            , verify ? true
            }:
            let
              raw = pkgs.fetchgit {
                inherit url rev hash;
                leaveDotGit = verify;
                deepClone = false;
                fetchSubmodules = false;
              };
              src = if verify then verifyCommitSignature raw rev else raw;
              suffix = builtins.substring 0 8 rev
                + lib.optionalString (!verify) "-unverified";

              fcCommon = commonArgs // { inherit src; };
              fcArtifacts = craneLib.buildDepsOnly (fcCommon // {
                pname = "blazelist-deps";
                version = serverManifest.version;
                cargoExtraArgs = "-p blazelist-server";
              });

              fcServer = craneLib.buildPackage (fcCommon // {
                pname = "blazelist-server";
                version = serverManifest.version;
                cargoExtraArgs = "-p blazelist-server";
                cargoArtifacts = fcArtifacts;
                # Skip tests here — this codepath builds from a pinned
                # release for deployment; gating on tests would slow
                # every redeploy of a tagged commit.
                doCheck = false;
              });

              fcWasm = craneLib.mkCargoDerivation {
                inherit src;
                pname = "blazelist-wasm-dist";
                cargoArtifacts = null;
                cargoVendorDir = craneLib.vendorCargoDeps { inherit src; };
                nativeBuildInputs = [
                  rustToolchain
                  pkgs.trunk
                  wasmBindgenCli
                  pkgs.binaryen
                  pkgs.pkg-config
                ];
                buildPhaseCargoCommand = ''
                  export TRUNK_SKIP_VERSION_CHECK=true
                  # See blazelist-wasm-dist above for why --offline is passed.
                  trunk build --offline --release --config clients/wasm/Trunk.toml
                  sh clients/wasm/inject-precache.sh clients/wasm/dist
                '';
                installPhaseCommand = ''
                  mkdir -p $out
                  cp -R clients/wasm/dist/. $out/
                '';
                doInstallCargoArtifacts = false;
                doCheck = false;
              };
            in
            pkgs.symlinkJoin {
              name = "blazelist-${suffix}";
              paths = [
                fcServer
                (pkgs.runCommand "wasm-share-${suffix}" { } ''
                  mkdir -p $out/share/blazelist
                  cp -R ${fcWasm} $out/share/blazelist/dist
                '')
              ];
              passthru = {
                server = fcServer;
                wasm = fcWasm;
                inherit rev verify;
              };
            };

        in
        {
          packages = {
            default = blazelist;
            inherit blazelist blazelist-server blazelist-wasm-dist;
          };

          apps.default = {
            type = "app";
            program = "${blazelist-server}/bin/blazelist-server";
          };

          devShells.default = pkgs.mkShell {
            packages = [
              rustToolchain
              pkgs.trunk
              pkgs.just
              pkgs.pkg-config
              pkgs.openssl
              pkgs.gnupg # for release-sign workflow
              pkgs.jq # release-stage: parse `cargo metadata`
              pkgs.rsync # release_prepare.sh: stage the release subset
              pkgs.nodejs_22
              pkgs.chromium
            ];

            # Use the full NixOS Chromium (not chrome-headless-shell) so
            # Playwright can run with --headless=new, which supports
            # WebTransport + serverCertificateHashes.
            PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1";
            PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH = "${pkgs.chromium}/bin/chromium";

            # The nix-built Chromium crashes (SkFontMgr_FontConfigInterface
            # "Not implemented") on systems without /etc/fonts/fonts.conf
            # — i.e. inside Docker containers. Only fall back to a
            # bundled fonts.conf when no system one exists; NixOS hosts
            # keep using their full system fontconfig.
            shellHook = ''
              if [ ! -f /etc/fonts/fonts.conf ] && [ -z "$FONTCONFIG_FILE" ]; then
                export FONTCONFIG_FILE=${pkgs.makeFontsConf { fontDirectories = [ pkgs.dejavu_fonts ]; }}
              fi
            '';
          };

          # Exposed for downstream flakes:
          #   blazelist.lib.${system}.buildFromCommit { rev = ...; hash = ...; }
          #   blazelist.lib.${system}.hardeningSettings  # systemd serviceConfig attrs
          lib = {
            inherit buildFromCommit verifyCommitSignature releaseSigningKey;
            hardeningSettings = import ./nix/hardening-settings.nix;
          };

          checks = {
            fmt = craneLib.cargoFmt {
              pname = "blazelist-workspace";
              version = serverManifest.version;
              src = craneLib.cleanCargoSource ./.;
            };

            # Workspace tests — mirrors `just test` (cargo test --workspace).
            tests = craneLib.cargoTest (commonArgs // {
              inherit cargoArtifacts;
              pname = "blazelist-workspace";
              version = serverManifest.version;
              cargoTestExtraArgs = "--workspace";
            });

            # Clippy is intentionally not wired as a flake check yet —
            # run it locally via `just clippy`. A future change can add
            # `cargoClippy` here alongside any lint cleanup.

            inherit blazelist-server blazelist-wasm-dist blazelist;
          };
        };
    in
    flake-utils.lib.eachDefaultSystem perSystem // {
      # System-independent outputs: the NixOS module.
      nixosModules.blazelist = import ./nix/module.nix { inherit self; };
      nixosModules.default = self.nixosModules.blazelist;
    };
}
