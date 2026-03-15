// ── Precache manifest (replaced by inject-precache.sh after trunk build) ─────
const CACHE_NAME = 'blazelist-dev';
const PRECACHE_URLS = ['/', '/index.html'];

// ── Install: precache all assets ─────────────────────────────────────────────
self.addEventListener('install', (event) => {
    event.waitUntil(
        caches.open(CACHE_NAME).then((cache) => cache.addAll(PRECACHE_URLS))
    );
    self.skipWaiting();
});

// ── Activate: purge stale caches ─────────────────────────────────────────────
self.addEventListener('activate', (event) => {
    event.waitUntil(
        caches.keys().then((names) =>
            Promise.all(
                names.filter((n) => n !== CACHE_NAME).map((n) => caches.delete(n))
            )
        )
    );
    self.clients.claim();
});

// ── Fetch: cache-first for hashed assets, network-first for everything else ──
self.addEventListener('fetch', (event) => {
    // Navigation requests: network-first, fall back to cached /index.html.
    if (event.request.mode === 'navigate') {
        event.respondWith(
            fetch(event.request)
                .then((response) => {
                    if (response.ok) {
                        const clone = response.clone();
                        caches.open(CACHE_NAME).then((c) => c.put(event.request, clone));
                    }
                    return response;
                })
                .catch(() => caches.match('/index.html'))
        );
        return;
    }

    // Hashed static assets: cache-first (content-hashed filenames are immutable).
    if (isHashedAsset(new URL(event.request.url).pathname)) {
        event.respondWith(
            caches.match(event.request).then((cached) => {
                if (cached) return cached;
                return fetch(event.request).then((response) => {
                    if (response.ok) {
                        const clone = response.clone();
                        caches.open(CACHE_NAME).then((c) => c.put(event.request, clone));
                    }
                    return response;
                });
            })
        );
        return;
    }

    // Everything else: network-first, fall back to cache.
    event.respondWith(
        fetch(event.request)
            .then((response) => {
                if (response.ok && event.request.method === 'GET') {
                    const clone = response.clone();
                    caches.open(CACHE_NAME).then((c) => c.put(event.request, clone));
                }
                return response;
            })
            .catch(() => caches.match(event.request))
    );
});

// Trunk produces content-hashed filenames like base-56da8f4eda224f5.css or
// blazelist-wasm-741409e8f90909ac_bg.wasm. These are immutable by definition.
function isHashedAsset(pathname) {
    if (pathname.startsWith('/snippets/')) return true;
    return /^\/[^/]+-[0-9a-f]{7,}[._]\w+$/.test(pathname);
}
