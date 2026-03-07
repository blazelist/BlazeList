# Set the shell interpreter for all just commands to bash with specific options
# -c: Execute the command string that follows
# -u: Treat unset variables as an error during parameter expansion
set shell := ["bash", "-cu"]

# Disable echoing recipe lines before executing.
set quiet

# List all available recipes
default:
    @just --list

# Constants
WASM_TARGET := "wasm32-unknown-unknown"
WASM_DIR := "clients/wasm"
DB_FILES := "blazelist.db blazelist.db-shm blazelist.db-wal"

# Dev port bases — each service gets its own range.
# Pass offset=N to run multiple environments side-by-side
# (variable overrides must come right after `just`):
#   just offset=1 dev
#
# Port layout (base + offset):
#   QUIC          47200 + offset
#   WebTransport  47400 + offset
#   HTTP cert     47600 + offset
#   Trunk         47800 + offset
QUIC_PORT_BASE := "47200"
WT_PORT_BASE := "47400"
HTTP_PORT_BASE := "47600"
TRUNK_PORT_BASE := "47800"

# Port offset for running multiple dev environments simultaneously.
# Override on the command line (must be right after `just`): just offset=1 dev
offset := "0"

# Bind address for all listeners.
# Override to expose on all interfaces: just bind=0.0.0.0 dev
bind := "127.0.0.1"

# Whether to clean the database before starting. Override: just clean_db=false dev
clean_db := "true"

# Whether to seed the database after starting. Override: just seed=false dev
seed := "true"

# ==============================
# Composite workflows
# ==============================

# Build everything without running (server + WASM client)
dev-build:
    cargo build -p blazelist-server
    trunk build --config {{WASM_DIR}}/Trunk.toml

# Start server, seed data, and serve WASM client (Ctrl+C to stop)
# Usage: just dev                       — full (server + WASM)
#        just offset=1 dev              — second dev environment
#        just clean_db=false dev        — keep existing database
#        just seed=false dev            — skip seeding
dev:
    #!/usr/bin/env bash
    QUIC_PORT=$(({{QUIC_PORT_BASE}} + {{offset}}))
    WT_PORT=$(({{WT_PORT_BASE}} + {{offset}}))
    HTTP_PORT=$(({{HTTP_PORT_BASE}} + {{offset}}))
    TRUNK_PORT=$(({{TRUNK_PORT_BASE}} + {{offset}}))

    if [ "{{clean_db}}" = "true" ]; then
        just clean
    fi

    echo "Starting BlazeList server (offset={{offset}})..."
    cargo run -p blazelist-server -- \
        --quic-port "$QUIC_PORT" \
        --wt-port "$WT_PORT" \
        --http-port "$HTTP_PORT" \
        --bind "{{bind}}" &
    SERVER_PID=$!

    # Ensure server is stopped on exit (Ctrl+C or error).
    trap "kill $SERVER_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null" EXIT

    just _wait-for-server "$QUIC_PORT"

    if [ "{{seed}}" = "true" ]; then
        echo "Running dev seeder..."
        just offset={{offset}} seed
    fi

    echo ""
    echo "Dev environment ready (offset={{offset}})."
    echo "  QUIC:          {{bind}}:$QUIC_PORT"
    echo "  WebTransport:  {{bind}}:$WT_PORT"
    echo "  HTTP cert:     {{bind}}:$HTTP_PORT"
    echo "  Trunk:         http://{{bind}}:$TRUNK_PORT"
    trunk serve --config {{WASM_DIR}}/Trunk.toml --port "$TRUNK_PORT" --address "{{bind}}" &
    TRUNK_PID=$!

    trap "kill $TRUNK_PID 2>/dev/null; kill $SERVER_PID 2>/dev/null; wait $TRUNK_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null" EXIT

    echo "Open http://{{bind}}:$TRUNK_PORT in your browser."
    echo "Press Ctrl+C to stop."
    wait $SERVER_PID

# Build WASM client and start server with HTTPS for LAN/Tailscale access
# Usage: just bind=0.0.0.0 dev-lan
dev-lan:
    #!/usr/bin/env bash
    QUIC_PORT=$(({{QUIC_PORT_BASE}} + {{offset}}))
    WT_PORT=$(({{WT_PORT_BASE}} + {{offset}}))
    HTTP_PORT=$(({{HTTP_PORT_BASE}} + {{offset}}))
    HTTPS_PORT=$(({{TRUNK_PORT_BASE}} + {{offset}}))

    if [ "{{clean_db}}" = "true" ]; then
        just clean
    fi

    echo "Building WASM client..."
    trunk build --config {{WASM_DIR}}/Trunk.toml

    echo "Starting BlazeList server with HTTPS (offset={{offset}})..."
    cargo run -p blazelist-server -- \
        --quic-port "$QUIC_PORT" \
        --wt-port "$WT_PORT" \
        --http-port "$HTTP_PORT" \
        --https-port "$HTTPS_PORT" \
        --static-dir "{{WASM_DIR}}/dist" \
        --bind "{{bind}}" &
    SERVER_PID=$!

    trap "kill $SERVER_PID 2>/dev/null; wait $SERVER_PID 2>/dev/null" EXIT

    just _wait-for-server "$QUIC_PORT"

    if [ "{{seed}}" = "true" ]; then
        echo "Running dev seeder..."
        just offset={{offset}} seed
    fi

    echo ""
    echo "LAN dev environment ready (offset={{offset}})."
    echo "  QUIC:          {{bind}}:$QUIC_PORT"
    echo "  WebTransport:  {{bind}}:$WT_PORT"
    echo "  HTTP cert:     {{bind}}:$HTTP_PORT"
    echo "  HTTPS:         https://{{bind}}:$HTTPS_PORT"
    echo ""
    echo "Open https://$(hostname):$HTTPS_PORT from a LAN device."
    echo "Press Ctrl+C to stop."
    wait $SERVER_PID

# ==============================
# Individual commands
# ==============================

# Start the BlazeList server
server:
    #!/usr/bin/env bash
    QUIC_PORT=$(({{QUIC_PORT_BASE}} + {{offset}}))
    WT_PORT=$(({{WT_PORT_BASE}} + {{offset}}))
    HTTP_PORT=$(({{HTTP_PORT_BASE}} + {{offset}}))
    cargo run -p blazelist-server -- \
        --quic-port "$QUIC_PORT" \
        --wt-port "$WT_PORT" \
        --http-port "$HTTP_PORT" \
        --bind "{{bind}}"

# Run the dev seeder with defaults
seed:
    #!/usr/bin/env bash
    QUIC_PORT=$(({{QUIC_PORT_BASE}} + {{offset}}))
    cargo run -p blazelist-dev-seeder -- --server "127.0.0.1:$QUIC_PORT"

# ==============================
# WASM client
# ==============================

# Check that the WASM client compiles
wasm-check:
    cargo check -p blazelist-wasm --target {{WASM_TARGET}}

# Build the WASM client with Trunk (dev mode)
wasm-build:
    trunk build --config {{WASM_DIR}}/Trunk.toml

# Build the WASM client with Trunk (release mode)
wasm-build-release:
    trunk build --release --config {{WASM_DIR}}/Trunk.toml

# Serve the WASM client with Trunk (live-reload dev server)
wasm-serve:
    #!/usr/bin/env bash
    TRUNK_PORT=$(({{TRUNK_PORT_BASE}} + {{offset}}))
    trunk serve --config {{WASM_DIR}}/Trunk.toml --port "$TRUNK_PORT" --address "{{bind}}"

# Run clippy on the WASM client
wasm-clippy:
    cargo clippy -p blazelist-wasm --target {{WASM_TARGET}}

# Wait for the server to be ready (up to 30 seconds). QUIC uses UDP, so check with ss.
_wait-for-server port:
    #!/usr/bin/env bash
    for i in $(seq 1 300); do
        if ss -ulnp 2>/dev/null | grep -q ":{{port}} "; then
            break
        fi
        sleep 0.1
    done

# Remove database files
clean:
    #!/usr/bin/env bash
    for f in {{DB_FILES}}; do
        [ -f "$f" ] && rm "$f" && echo "Removed $f"
    done
    echo "Clean complete."

# Run all workspace tests
test:
    cargo test --workspace

# Run all benchmarks
bench:
    cargo bench

# Run benchmarks for a specific crate (e.g., just bench-crate blazelist-server)
bench-crate crate:
    cargo bench -p {{crate}}

# Build all crates in the workspace
build:
    cargo build

# Check all crates (fast compile check without codegen)
check:
    cargo check --workspace
    cargo check -p blazelist-wasm --target {{WASM_TARGET}}

# Run clippy lints (workspace + WASM target)
clippy:
    cargo clippy --workspace
    cargo clippy -p blazelist-wasm --target {{WASM_TARGET}}

# Check formatting
fmt-check:
    cargo fmt --check

# Format code
fmt:
    cargo fmt

# ==============================
# Aliases
# ==============================

alias c := check
alias d := dev
alias t := test
