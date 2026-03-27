// ── Precache manifest (replaced by inject-precache.sh after trunk build) ─────
const CACHE_NAME = 'blazelist-dev';
const PRECACHE_URLS = ['/', '/index.html'];

// ── Install: precache all assets (resilient to partial failures) ─────────────
self.addEventListener('install', (event) => {
    event.waitUntil(
        caches.open(CACHE_NAME).then((cache) =>
            Promise.allSettled(
                PRECACHE_URLS.map((url) =>
                    cache.add(url).catch((err) => {
                        console.warn(`[SW] Failed to precache ${url}:`, err);
                        throw err;
                    })
                )
            ).then((results) => {
                if (results.every((r) => r.status === 'fulfilled')) {
                    self.skipWaiting();
                }
            })
        )
    );
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

// ── Offline fallback page ────────────────────────────────────────────────────
const OFFLINE_PAGE = `<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>BlazeList — Offline</title>
    <style>
        body { margin:0; display:flex; flex-direction:column; align-items:center;
               justify-content:center; height:100vh; background:#0a0a0d;
               color:#888890; font-family:system-ui,sans-serif; text-align:center;
               padding:1rem; }
        h1 { font-size:1.2rem; font-weight:600; color:#d0d0d4; margin-bottom:0.5rem; }
        p { font-size:0.85rem; max-width:28rem; line-height:1.5; }
        button { margin-top:1rem; padding:0.5rem 1.5rem; border:1px solid #444;
                 border-radius:4px; background:#1a1a2e; color:#d0d0d4;
                 font-size:0.85rem; cursor:pointer; }
        button:hover { background:#252540; }
    </style>
</head>
<body>
    <h1>BlazeList</h1>
    <p>You appear to be offline and the app hasn\u2019t been fully cached yet.
       Please ensure your BlazeList server is reachable and load the app once to enable offline access.</p>
    <button onclick="location.reload()">Retry</button>
</body>
</html>`;

// ── Fetch: cache-first for hashed assets, network-first for everything else ──
self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);

    // Ignore cross-origin requests (e.g. analytics, external APIs).
    if (url.origin !== self.location.origin) return;

    // Navigation requests: network-first, fall back to cached page or offline page.
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
                .catch(() =>
                    caches.match(event.request)
                        .then((r) => r || caches.match('/index.html'))
                        .then((r) => r || caches.match('/'))
                        .then((r) => r || new Response(OFFLINE_PAGE, {
                            headers: { 'Content-Type': 'text/html' },
                        }))
                )
        );
        return;
    }

    // Hashed static assets: cache-first (content-hashed filenames are immutable).
    if (isHashedAsset(url.pathname)) {
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
    return /^\/[^/]+-[0-9a-f]{7,}[._][\w.]+$/.test(pathname);
}
