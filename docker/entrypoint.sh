#!/bin/sh
set -e

# Serve the WASM frontend over HTTPS by default in Docker.
# The blazelist-server handles everything — nginx is not needed.
exec blazelist-server --bind 0.0.0.0 --static-dir /var/www/blazelist "$@"
