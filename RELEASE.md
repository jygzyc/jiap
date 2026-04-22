# DECX v3.0.1

DECX v3.0.1 adds request-level timeout handling, a bounded route thread pool, thread-safe cache counters, and extracted shared warmup utilities.

### Changes

- Server: bounded thread pool for route handling with configurable request timeout (default 120s, via `decx.requestTimeoutMs` system property). Long-running requests return 504 `REQUEST_TIMEOUT`.

- Server: new `REQUEST_TIMEOUT` error type in `DecxError` (504, `REQUEST_TIMEOUT`).

- Cache: fixed `ConcurrentModificationException` by using write lock in `get()`; replaced `Long` counters with `AtomicLong` for thread safety; increased `CACHE_MAX_SIZE` from 7 to 15.

- Warmup: extracted class selection and decompilation warmup logic into shared `WarmupUtils`, used by both plugin and server entry points.

- Plugin: `cleanupOnError()` now ensures MCP temp files are cleaned up even if sidecar stop fails.
