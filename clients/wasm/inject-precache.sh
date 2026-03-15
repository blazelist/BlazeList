#!/bin/sh
set -eu

DIST="${1:?Usage: inject-precache.sh <dist-dir>}"

# Collect every file in dist except sw.js itself, as URL paths.
URLS=$(find "$DIST" -type f ! -name 'sw.js' | sort | while read -r f; do
    printf "'/%s', " "$(echo "$f" | sed "s|^${DIST}/||")"
done)

# Add the root URL (/) which maps to index.html via the server.
URLS="'/', ${URLS%, }"

# Compute a short version hash from the sorted filename listing.
VERSION=$(find "$DIST" -type f ! -name 'sw.js' | sort | sha256sum | cut -c1-8)

# Replace sentinel lines in the built sw.js.
sed -i \
    -e "s|^const CACHE_NAME = .*|const CACHE_NAME = 'blazelist-${VERSION}';|" \
    -e "s|^const PRECACHE_URLS = .*|const PRECACHE_URLS = [${URLS}];|" \
    "$DIST/sw.js"

echo "inject-precache: blazelist-${VERSION} ($(echo "$URLS" | tr -cd ',' | wc -c | tr -d ' ') URLs)"
