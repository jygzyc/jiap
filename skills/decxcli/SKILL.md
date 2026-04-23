---
name: decxcli
description: General Android analysis skill for DECX CLI. Use for APK/DEX/JAR opening, source lookup, cross-reference tracing, manifest inspection, resource inspection, and session management.
metadata:
  requires:
    bins: ["decx"]
---

# DECX CLI - General Analysis

## Overview

Use this skill for general Android reverse-engineering and code navigation with DECX.

Scope:

- In scope: open APK/DEX/JAR files, inspect manifest and resources, navigate source, inspect xrefs, trace inheritance and interfaces, manage DECX sessions
- Out of scope: exploitability rating, final vulnerability triage, and PoC construction
- App vulnerability hunting belongs to `decxcli-app-vulnhunt`
- Framework vulnerability hunting belongs to `decxcli-framework-vulnhunt`
- PoC construction belongs to `decxcli-poc`

## Non-Negotiable Rules

### Command Rules

- Every session-backed `decx code` command and every session-backed `decx ard` command must include `-P <port>`
- adb-backed `decx ard` commands such as `system-services` and `perm-info` do not use `-P <port>`; they use `--serial` / `--adb-path` when needed
- Quote package names, class names, method signatures, file paths, and resource paths
- Method signatures must use the full format: `"package.Class.method(paramType1,paramType2):returnType"`
- Never use `...` in signatures
- If a command is missing, rejected, or uncertain, run the nearest `--help` command before retrying

### Session Rules

- Open a session before analysis
- Reuse the same session for related work instead of reopening the same target repeatedly
- Do not close the session automatically if downstream work is likely to continue in `decxcli-app-vulnhunt`, `decxcli-framework-vulnhunt`, or `decxcli-poc`
- If you opened a session only for a one-off lookup and no follow-up work is needed, close it before finishing
- `process close` can close by name or `--port <port>`; `process list` does not take `-P <port>`

### Output Rules

- Prefer targeted commands over broad search
- Keep only the code and metadata needed for the active question
- Do not paste long raw source dumps unless the user explicitly asks for them
- Treat `search-class` and `search-method` as expensive discovery tools, not default navigation tools

## Core Workflow

Track progress with:

```text
DECX Progress
- [ ] Phase 1: Confirm environment or active session
- [ ] Phase 2: Open or reuse target session
- [ ] Phase 3: Identify target class, method, component, or resource
- [ ] Phase 4: Read only the minimal source or metadata needed
- [ ] Phase 5: Follow xrefs or inheritance if needed
- [ ] Phase 6: Close session if no downstream work remains
```

### Phase 1 - Confirm Environment

Use these commands when you need to confirm DECX health:

```bash
decx process check -P <port>
decx process status -P <port>
decx process list
```

Use `process check` when you are unsure whether the local DECX runtime is ready.

### Phase 2 - Open or Reuse a Session

Open a file:

```bash
decx process open "<file-or-url>" -P <port>
```

Useful options:

```text
-P, --port <port>     server port
-n, --name <name>     explicit session name
--force               reopen even if DECX detects an existing conflicting session
```

Notes:

- `<file-or-url>` can be a local path or an HTTP/HTTPS URL
- URL targets are downloaded into DECX temporary storage and cached
- Use explicit `--name` when you want a stable session identifier across repeated analysis turns

Session conflict behavior:

- same name + same hash + alive process: DECX reuses the session
- same name + different hash: DECX errors unless `--force` or a new `--name` is used
- different name + same hash: DECX errors unless `--force` is used

### Phase 3 - Pick the Right Surface

Use `ard` first when the question is about Android structure:

```bash
decx ard app-manifest -P <port>
decx ard exported-components -P <port>
decx ard app-deeplinks -P <port>
decx ard app-receivers -P <port>
decx ard get-aidl -P <port>
decx ard strings -P <port>
decx ard system-services --serial <serial> --grep <keyword>
decx ard perm-info "<permission>" --serial <serial>
```

Use `code` first when the question is about implementation details:

```bash
decx code class-context "<class>" -P <port>
decx code class-source "<class>" -P <port>
decx code method-source "<signature>" -P <port>
decx code method-context "<signature>" -P <port>
decx code method-cfg "<signature>" -P <port>
decx code xref-method "<signature>" -P <port>
decx code xref-class "<class>" -P <port>
decx code xref-field "<field>" -P <port>
decx code implement "<interface>" -P <port>
decx code subclass "<class>" -P <port>
```

### Phase 4 - Read Minimal Source

Default navigation order:

1. `class-context`
2. `class-source` or `method-source`
3. `xref-*`
4. `implement` or `subclass`
5. `search-*` only if the target entry is still unknown

Prefer:

- `method-context` when you need callers and callees in one call
- `method-source` when you need the full body
- `class-context` when you need a quick method/field overview
- `class-source` when you need surrounding context
- `xref-method` when you want only callers (no callees needed)
- `xref-field` when you want reads and writes
- `implement` for interfaces
- `subclass` for base classes or framework callbacks

### Phase 5 - Use Search Sparingly

Search commands:

```bash
decx code search-global "<keyword>" --limit <n> -P <port>
decx code search-class "<class>" "<keyword>" --limit <n> -P <port>
decx code search-method "<name>" -P <port>
```

Use them only when:

- the real target class is unknown
- the real method name is uncertain
- a keyword is the only stable starting point

Do not fan out into bulk repeated searches if `class-source` + `xref-*` can answer the question.

## Command Surface

### `process`

| Command | Purpose |
|--------|---------|
| `decx process check -P <port>` | Check DECX environment and local runtime readiness |
| `decx process open "<file>" -P <port>` | Open a target for analysis |
| `decx process close "[name]"` | Close one session |
| `decx process close -a` | Close all sessions |
| `decx process list` | List active sessions |
| `decx process status "[name]" -P <port>` | Check active server or session status |

### `code`

| Command | Purpose |
|--------|---------|
| `decx code classes -P <port>` | List classes (`--limit`, `--include-package`, `--exclude-package`, `--no-regex`) |
| `decx code class-context "<class>" -P <port>` | Show class metadata (fields and methods) |
| `decx code class-source "<class>" -P <port>` | Show class source (`--limit`) |
| `decx code class-source "<class>" --smali -P <port>` | Show class smali |
| `decx code method-source "<signature>" -P <port>` | Show method source |
| `decx code method-source "<signature>" --smali -P <port>` | Show method smali |
| `decx code method-context "<signature>" -P <port>` | Show method signature, callers, and callees |
| `decx code method-cfg "<signature>" -P <port>` | Show method control flow graph as DOT source |
| `decx code xref-method "<signature>" -P <port>` | Show method callers |
| `decx code xref-class "<class>" -P <port>` | Show class references |
| `decx code xref-field "<field>" -P <port>` | Show field reads and writes |
| `decx code implement "<interface>" -P <port>` | List interface implementations |
| `decx code subclass "<class>" -P <port>` | List subclasses |
| `decx code search-global "<keyword>" --limit <n> -P <port>` | Search all class bodies (`--limit`, `--include-package`, `--exclude-package`, `--no-regex`, `--case-sensitive`) |
| `decx code search-class "<class>" "<keyword>" --limit <n> -P <port>` | Grep one class (`--no-regex`, `--case-sensitive`) |
| `decx code search-method "<name>" -P <port>` | Search method names |

### `ard`

| Command | Purpose |
|--------|---------|
| `decx ard app-manifest -P <port>` | Read `AndroidManifest.xml` |
| `decx ard main-activity -P <port>` | Show main activity |
| `decx ard app-application -P <port>` | Show application class |
| `decx ard exported-components -P <port>` | List exported components (`--type`, `--exclude-type`, `--no-regex`) |
| `decx ard app-deeplinks -P <port>` | List deep links |
| `decx ard app-receivers -P <port>` | List dynamic receivers (`--limit`, `--include-package`, `--exclude-package`, `--no-regex`) |
| `decx ard get-aidl -P <port>` | List AIDL interfaces (`--limit`, `--include-package`, `--exclude-package`, `--no-regex`) |
| `decx ard system-service-impl "<interface>" -P <port>` | Resolve framework service implementation |
| `decx ard system-services --serial <serial> [--grep <keyword>]` | List live Binder/system services as structured JSON |
| `decx ard perm-info "<permission>" --serial <serial>` | Resolve one permission into a structured JSON object |
| `decx ard all-resources -P <port>` | List resource file names (`--include`, `--no-regex`) |
| `decx ard resource-file "<res>" -P <port>` | Read one resource file |
| `decx ard strings -P <port>` | Read `strings.xml` |

adb-backed `ard` output notes:

- `system-services` returns JSON with:
  - `total`
  - `services[]`
  - per service: `index`, `name`, `interfaces`
- `perm-info` returns one JSON object with fields like:
  - `permission`
  - `package`
  - `label`
  - `description`
  - `protectionLevel`
- Do not treat these commands as raw shell text lookups; consume the parsed JSON fields directly
- Use `--grep` on `system-services` to narrow the runtime surface before choosing an interface for `system-service-impl`

### `self`

| Command | Purpose |
|--------|---------|
| `decx self install` | Install `decx-server.jar` |
| `decx self install -p` | Install prerelease server |
| `decx self update` | Update CLI and server |
| `decx self update -p` | Update with prerelease server |

## Signature and Identifier Formats

Method signature:

```text
package.Class.methodName(paramType1,paramType2):returnType
```

Example:

```text
"com.example.MainActivity.onCreate(android.os.Bundle):void"
```

Field identifier:

```text
"package.Class.fieldName :type"
```

Resource path:

```text
"res/xml/file_paths.xml"
```

## Common Analysis Patterns

### Understand App Structure

```bash
decx ard app-manifest -P <port>
decx ard exported-components -P <port>
decx ard app-deeplinks -P <port>
decx code classes -P <port>
decx ard system-services --serial <serial> --grep activity
```

### Trace a Specific Feature

```bash
decx code search-method "login" -P <port>
decx code class-source "com.example.AuthManager" --limit 120 -P <port>
decx code xref-method "com.example.AuthManager.login(java.lang.String,java.lang.String):boolean" -P <port>
decx code xref-field "com.example.AuthManager.mToken" -P <port>
```

### Analyze Inheritance or Interfaces

```bash
decx code subclass "com.example.BaseActivity" -P <port>
decx code implement "com.example.MyInterface" -P <port>
decx ard get-aidl -P <port>
decx ard system-services --serial <serial> --grep permission
decx ard perm-info "android.permission.DUMP" --serial <serial>
```

### Inspect Resources

```bash
decx ard all-resources --include "res/xml" -P <port>
decx ard resource-file "res/xml/file_paths.xml" -P <port>
decx ard strings -P <port>
```

## Session Closure

Close one session:

```bash
decx process close "<name>"
```

Close all sessions:

```bash
decx process close -a
```

If the user is likely to continue into vulnerability hunting or PoC work, keep the session alive and state which session name and port should be reused.
