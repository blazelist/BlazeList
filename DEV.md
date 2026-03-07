# BlazeList — Developer Guide

This document covers the local development workflow for BlazeList.

## Prerequisites

- [Rust](https://rustup.rs/) (stable toolchain)
- [just](https://github.com/casey/just) (command runner)
- [Trunk](https://trunkrs.dev/) (WASM build tool) — install with `cargo install trunk`
- `wasm32-unknown-unknown` target — install with `rustup target add wasm32-unknown-unknown`

## Quick Start

```bash
just dev
```

This single command will:

1. Clean any existing database files
2. Build and start the BlazeList server
3. Wait for the server to be ready
4. Run the dev seeder to populate test data (1000 cards, 15 tags)
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
just seed
```

Runs the dev seeder against the running server. Generates deterministic test data (default: seed=42, 1000 cards, 15 tags).

### WASM Client

```bash
just wasm-serve          # Live-reload dev server
just wasm-build          # Build (dev mode)
just wasm-build-release  # Build (release mode)
just wasm-check          # Compile check only
just wasm-clippy         # Run clippy lints
```

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

The `just dev` command runs `clean` automatically before starting.

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
