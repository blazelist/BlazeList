#!/bin/sh
set -e

# Serve the WASM frontend over HTTPS by default in Docker.
exec blazelist-server --bind 0.0.0.0 --static-dir /var/www/blazelist "$@"
