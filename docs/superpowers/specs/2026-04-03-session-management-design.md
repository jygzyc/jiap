# Session Management Redesign: Name-Based Sessions

## Problem

Current CLI session management uses PID as the user-facing session identifier (`--pid` flag).
This is unreliable because:

- PIDs get recycled after process exit
- Users must remember/look up PIDs
- `getSessionByPid()` is O(n) file scan
- Same APK re-opened creates confusing PID-based references

## Decision

Replace PID-based session lookup with **filename-based naming**. Users reference sessions by the APK filename (without extension).

## Data Model

```typescript
interface Session {
  hash: string;      // sha256(file)[0:16] — content dedup key
  name: string;      // NEW — APK filename without extension (e.g. "sieve")
  pid: number;       // retained — process management still needs it
  port: number;      // TCP port JIAP server listens on
  path: string;      // absolute path to the APK/DEX/JAR
  startedAt: number; // Unix timestamp (ms) — 30-day expiry
}
```

Storage: `~/.jiap/sessions/<name>.json` (was `<hash>.json`).

## Name Resolution

1. `jiap process open file.apk` → name = `"file"`
2. `jiap process open file.apk --name my-target` → name = `"my-target"` (override)
3. Name collision check:
   - Same name + same hash → reuse session (same APK)
   - Same name + different hash → error: "Session <name> already exists for a different APK. Use --name to specify a different name."
4. Hash dedup check: prevent opening the same APK under different names

## CLI Interface Changes

### `process` commands

```
jiap process open <file> [--name <name>] [--port <port>] [--force]
jiap process close <name>
jiap process close --all
jiap process list        # shows NAME, PORT, PID, PATH columns
jiap process status <name>  # NEW — detailed info for one session
```

### `code` / `ard` commands

```diff
- -p, --pid <pid>     # REMOVED
+ -s, --session <name>  # NEW — reference session by name
  -P, --port <port>     # retained — direct port (escape hatch)
```

- Both `-s` and `-P` are mutually exclusive
- Neither specified: if exactly one alive session exists → auto-use it; multiple → error asking user to specify

## Internal Implementation Changes

### `resolveClient()` (client-helper.ts)

```
opts.session ? lookupByName(opts.session)
: opts.port    ? useDirect(opts.port)
:                autoSelect()   // single alive session → use it
```

- `getSessionByName(name)` → read `~/.jiap/sessions/<name>.json` — O(1)
- Remove `getSessionByPid()` entirely
- `autoSelect()` → list alive sessions, if exactly one return it, else error

### Session CRUD (session.ts)

- `createSession(name, hash, path, pid, port)` → write `<name>.json`
- `readSession(name)` → read by filename — O(1)
- `deleteSession(name)` → delete by filename — O(1)
- `listAllSessions()` / `listAliveSessions()` → scan dir (unchanged)
- Remove `getSessionByPid()`
- `cleanupDead()` → unchanged, still uses `isJadxProcess(pid)` for liveness

### `process open` flow

```
name = opts.name || basename(file, extname(file))

1. existing = readSession(name)
   if existing:
     same hash + alive     → reuse, print "Session <name> already running"
     same hash + dead      → delete old, recreate
     different hash        → error, suggest --name

2. for s in listAliveSessions():
     if s.hash === hash    → error "Already open as session <s.name>"
```

### `process close <name>` flow

```
1. readSession(name)           → O(1)
2. not found                   → error "Session <name> not found"
3. killProcessGroup(session.pid)
4. deleteSession(name)
```

## Migration

Old session files use `<hash>.json` naming. No active migration needed:
- `cleanupDead()` skips files it can't parse
- Old sessions expire after 30 days naturally
- Users can manually `rm ~/.jiap/sessions/*.json` to clean slate

## Test Changes

- `session.test.ts`: update to name-based create/read/delete
- `client.test.ts`: `resolveClient` tests use `--session`
- `config.test.ts`: remove `getSessionByPid` tests
- Remove all `--pid` related test cases

## Known Limitations

- Orphan processes (session file deleted but process alive) — pre-existing issue, not introduced by this change
- `jiap process prune` not included — can be added later if needed
