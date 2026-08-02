// ── Precache manifest (replaced by inject-precache.sh after trunk build) ─────
const CACHE_NAME = 'blazelist-dev';
const PRECACHE_URLS = ['/', '/index.html'];

// Track whether precache was fully successful (used in activate handler).
let precacheSucceeded = false;

// Hard per-asset timeout for precache fetches so a hanging network (e.g.
// half-working mobile signal) can't leave the SW stuck in "installing"
// forever, which would block `activate` and prevent `clients.claim()`.
const PRECACHE_TIMEOUT_MS = 15000;

function fetchWithTimeout(url, ms) {
    return new Promise((resolve, reject) => {
        const ctrl = typeof AbortController !== 'undefined' ? new AbortController() : null;
        const timer = setTimeout(() => {
            if (ctrl) ctrl.abort();
            reject(new Error(`Timeout after ${ms}ms fetching ${url}`));
        }, ms);
        fetch(url, ctrl ? { signal: ctrl.signal, cache: 'reload' } : { cache: 'reload' })
            .then((r) => { clearTimeout(timer); resolve(r); })
            .catch((err) => { clearTimeout(timer); reject(err); });
    });
}

// ── Install: precache all assets (resilient to partial failures) ─────────────
self.addEventListener('install', (event) => {
    event.waitUntil(
        caches.open(CACHE_NAME).then((cache) =>
            Promise.allSettled(
                PRECACHE_URLS.map((url) =>
                    fetchWithTimeout(url, PRECACHE_TIMEOUT_MS).then((response) => {
                        if (!response || !response.ok) {
                            throw new Error(`Bad response ${response && response.status} for ${url}`);
                        }
                        return cache.put(url, response);
                    }).catch((err) => {
                        console.warn(`[SW] Failed to precache ${url}:`, err);
                        throw err;
                    })
                )
            ).then((results) => {
                precacheSucceeded = results.every((r) => r.status === 'fulfilled');
                if (!precacheSucceeded) {
                    console.warn('[SW] Partial precache — keeping old caches as fallback');
                }
                // Always skip waiting so the new SW activates promptly instead
                // of getting stuck in "waiting" and later activating at an
                // unexpected time (e.g. when the user reopens the app offline).
                self.skipWaiting();
            })
        )
    );
});

// ── Activate: purge stale caches only when precache was fully successful ─────
self.addEventListener('activate', (event) => {
    event.waitUntil(
        caches.keys().then((names) => {
            const stale = names.filter((n) => n !== CACHE_NAME);
            if (stale.length === 0) return;
            // Only purge old caches when precache was fully successful.
            // On partial failure the old caches still contain working assets
            // that caches.match() can find transparently.
            if (precacheSucceeded) {
                return Promise.all(stale.map((n) => caches.delete(n)));
            }
            console.warn('[SW] Keeping stale caches because precache was incomplete:', stale);
        })
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

// ── Fetch: cache-first for navigation + hashed assets, network-first otherwise ──
self.addEventListener('fetch', (event) => {
    const url = new URL(event.request.url);

    // Ignore cross-origin requests (e.g. analytics, external APIs).
    if (url.origin !== self.location.origin) return;

    // Navigation requests: cache-first with stale-while-revalidate.
    //
    // Previously this was network-first with a catch() fallback to cache.
    // That looks correct but breaks catastrophically when the network
    // *hangs* rather than fails: `fetch()` neither resolves nor rejects,
    // so the `.catch()` never fires and the browser waits forever on the
    // navigation. Symptom: Android PWA stuck on the system splash screen
    // (never even reaches the in-HTML "Loading…"), desktop Chrome spins
    // indefinitely. This happens whenever the BlazeList server is
    // unreachable but the OS still has a (dead) connection open — common
    // on mobile with flaky signal, captive portals, VPN transitions, etc.
    //
    // Cache-first is safe because:
    //   - The precache contains `/` and `/index.html` on every install.
    //   - All asset refs inside the cached HTML are content-hashed, so a
    //     stale HTML can only reference assets that are still in the
    //     same cache generation (activate only purges old caches after
    //     a fully successful precache).
    //   - The background revalidate fetch keeps the cache fresh when
    //     online, and a visible update still takes effect on the next
    //     navigation / reload.
    if (event.request.mode === 'navigate') {
        // The shell answers for its own route only. A reverse proxy can mount
        // unrelated services under other paths of this origin, and the
        // fallback chain below would hand their navigations the BlazeList
        // shell instead — leaving the network to serve them keeps those paths
        // intact, and stops the revalidate `cache.put` from filing another
        // service's page under a BlazeList cache key.
        if (url.pathname !== '/' && url.pathname !== '/index.html') return;

        event.respondWith(
            caches.match(event.request)
                .then((r) => r || caches.match('/index.html'))
                .then((r) => r || caches.match('/'))
                .then((cached) => {
                    if (cached) {
                        // Stale-while-revalidate: refresh cache in the
                        // background, but never block the response on it.
                        // Errors (offline, hang, 5xx) are ignored — the
                        // user already has a working page.
                        fetch(event.request).then((response) => {
                            if (response && response.ok) {
                                const clone = response.clone();
                                caches.open(CACHE_NAME).then((c) =>
                                    c.put(event.request, clone)
                                ).catch(() => {});
                            }
                        }).catch(() => {});
                        return cached;
                    }
                    // Nothing cached (first-ever visit while offline, or
                    // cache was wiped). Try the network, fall back to
                    // the inline offline page on failure.
                    return fetch(event.request)
                        .then((response) => {
                            if (response.ok) {
                                const clone = response.clone();
                                caches.open(CACHE_NAME).then((c) =>
                                    c.put(event.request, clone)
                                ).catch(() => {});
                            }
                            return response;
                        })
                        .catch(() => new Response(OFFLINE_PAGE, {
                            headers: { 'Content-Type': 'text/html' },
                        }));
                })
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
                }).catch(() =>
                    new Response('', { status: 503, statusText: 'Offline' })
                );
            })
        );
        return;
    }

    // Everything else: network-first, fall back to cache, then to
    // a synthetic offline response. Returning `undefined` from a
    // fetch handler (e.g. when `caches.match` finds no entry) makes
    // the service worker throw `TypeError: Failed to convert value
    // to 'Response'`, which was showing up in the console on every
    // cert-hash / config fetch while offline.
    event.respondWith(
        fetch(event.request)
            .then((response) => {
                if (response.ok && event.request.method === 'GET') {
                    const clone = response.clone();
                    caches.open(CACHE_NAME).then((c) => c.put(event.request, clone));
                }
                return response;
            })
            .catch(() =>
                caches.match(event.request).then((cached) =>
                    cached || new Response('', { status: 503, statusText: 'Offline' })
                )
            )
    );
});

// Trunk produces content-hashed filenames like base-56da8f4eda224f5.css or
// blazelist-wasm-741409e8f90909ac_bg.wasm. These are immutable by definition.
function isHashedAsset(pathname) {
    if (pathname.startsWith('/snippets/')) return true;
    return /^\/[^/]+-[0-9a-f]{7,}[._][\w.]+$/.test(pathname);
}
