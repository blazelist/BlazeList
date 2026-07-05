# BlazeList — Developer Guide

This document covers the local development workflow for BlazeList.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [just](https://github.com/casey/just) (command runner)
- [Trunk](https://trunkrs.dev/) (WASM build tool) — install with `cargo install trunk`
- `wasm32-unknown-unknown` target — install with `rustup target add wasm32-unknown-unknown`

Or get the whole toolchain via the bundled [Nix](https://nixos.org/) flake — see [Nix](#nix) below.

## Quick Start

```bash
just dev
```

This single command will:

1. Clean any existing database files
2. Build and start the BlazeList server
3. Wait for the server to be ready
4. Run the dev seeder to populate test data (medium preset: 400 cards, 18 tags)
5. Start the Trunk dev server with live reload for the WASM client

Once running, open `http://127.0.0.1:47800` in your browser.

Press `Ctrl+C` to stop everything.

## Port Layout

Each service has its own port:

| Service | Default Port |
|---|---|
| QUIC | `47200` |
| WebTransport | `47400` |
| HTTP cert hash | `47600` |
| Trunk (WASM client) | `47800` |

## Running Multiple Environments

Use the `offset` parameter to run multiple dev environments side-by-side on the same machine. Each offset shifts all ports by the given number:

```bash
# First environment (default ports)
just dev

# Second environment (ports shifted by 1: 47201, 47401, 47601, 47801)
just offset=1 dev

# Third environment (ports shifted by 2: 47202, 47402, 47602, 47802)
just offset=2 dev
```

> [!TIP]
> This is useful when testing multiple branches simultaneously — spin up each branch with a different offset and they won't conflict.

The offset variable must come right after `just`:

```bash
just offset=1 dev        # ✅ correct
just dev offset=1        # ❌ won't work
```

## LAN / Tailscale Development

To build the WASM client and serve it over HTTPS (for access from other devices on your network):

```bash
just bind=0.0.0.0 dev-lan
```

This builds the WASM client with Trunk, then starts the server with `--static-dir` pointing to the built WASM assets. Access from other devices via `https://<hostname>:47800`.

## Individual Commands

### Server Only

```bash
just server
```

Starts only the BlazeList server (no WASM client, no seeding).

### Dev Seeder

```bash
just seed                # Medium preset (default): 400 cards, 18 tags
just seed-sm             # Small preset: 120 cards, 8 tags
just seed-lg             # Large preset: 1200 cards, 50 tags
just preset=large seed   # Same as seed-lg
just preset=small dev    # Start dev environment with small dataset
```

Runs the dev seeder against the running server. Three size presets are available:

| Preset | Cards | Tags | Use case |
|---|---|---|---|
| `small` | 120 | 8 | Fast iteration, quick smoke tests |
| `medium` | 400 | 18 | Everyday development (default) |
| `large` | 1200 | 50 | Stress testing, full dataset |

The `--cards` and `--tags` CLI flags can override preset values for custom sizes.

### WASM Client

```bash
just wasm-serve          # Live-reload dev server
just wasm-build          # Build (dev mode)
just wasm-build-release  # Build (release mode)
just wasm-check          # Compile check only
just wasm-clippy         # Run clippy lints
```

> **WASM shadow stack:** the `wasm32-unknown-unknown` profile sets an 8 MiB
> shadow stack via `-z stack-size` in [`.cargo/config.toml`](.cargo/config.toml).
> The reactive view tree renders through a stack-heavy, by-value `into_owned`
> recursion (the `AppState` context is a large `#[derive(Copy)]` struct captured
> into many view closures), and rust-lld's default 1 MiB stack overflows in
> debug builds — surfacing as an opaque `RuntimeError: memory access out of
> bounds` deep in `tachys`. Don't lower it without re-checking the detail-panel
> render.

## Build and Quality

```bash
just build               # Build all crates
just check               # Fast compile check (workspace + WASM)
just clippy              # Run clippy lints (workspace + WASM)
just fmt                 # Format code
just fmt-check           # Check formatting
```

## Testing

```bash
just test                # Run all workspace tests
just bench               # Run all benchmarks
just bench-crate <name>  # Run benchmarks for a specific crate
```

## Database Management

```bash
just clean               # Remove local database files (blazelist.db, .db-shm, .db-wal)
```

The `just dev` command runs `clean` automatically before starting. To reuse an
existing database (skip both the clean and the seeder), pass `--keep`:

```bash
just dev --keep          # shorthand for clean_db=false seed=false
just offset=1 dev --keep
```

## Bind Address

By default, all services bind to `127.0.0.1` (localhost only). To expose on all interfaces:

```bash
just bind=0.0.0.0 dev
```

## Aliases

| Alias | Command |
|---|---|
| `just c` | `just check` |
| `just d` | `just dev` |
| `just t` | `just test` |

## Nix

The flake provides the full local toolchain plus deployment artifacts.

```bash
nix develop             # Drop into the dev shell (rust + wasm32 + trunk + just)
nix build .#default     # Build server binary + WASM dist into ./result
nix build .#blazelist-server      # Just the server binary
nix build .#blazelist-wasm-dist   # Just the WASM client (post-processed sw.js)
nix run                 # Start the server with the default flags
nix flake check         # Eval-check the flake outputs
```

`inject-precache.sh` runs automatically inside the WASM derivation — not a
manual step under Nix.

For deploying via the bundled NixOS module (`services.blazelist`), see
[DOCS.md → NixOS](DOCS.md#nixos).

### Pinned, signature-verified builds

`lib.buildFromCommit` builds a specific upstream commit and, by default,
verifies its GPG signature against the committed `release-signing-key.asc`
before building — it refuses an unsigned or wrong-key commit:

```nix
blazelist.lib.${system}.buildFromCommit {
  rev = "<commit sha>";
  hash = "<fetchgit hash>";
  # verify = false;   # opt out (e.g. a fork that doesn't sign its commits)
}
```

Verification runs `git verify-commit` inside the build sandbox, so a successful
build is proof the pinned commit carries a valid release signature.
