---
name: decxcli-poc
description: Android exploit PoC construction skill. Turns a DECX-supported finding into a buildable Android PoC app, with optional compile and adb deployment when explicitly requested.
metadata:
  requires:
    bins: ["node", "decx", "unzip"]
---

# DECX CLI - Android Exploit PoC Construction

## Overview

Use this skill to turn a `decxcli-app-vulnhunt` or `decxcli-framework-vulnhunt` finding into a buildable Android PoC project.

Scope:

- In scope: finding normalization, static re-verification, exploit-mode selection, PoC project setup, exploit-class implementation, Manifest updates, optional compile/deploy
- Out of scope: vulnerability discovery, final risk rating, and unsupported exploit claims
- Vulnerability hunting belongs to `decxcli-app-vulnhunt` and `decxcli-framework-vulnhunt`

Command reference lives in `decxcli`.

## Non-Negotiable Rules

### State Rules

| State | Meaning | Allowed |
|------|---------|---------|
| `finding-normalized` | required fields extracted from the report | Yes |
| `target-verified` | component and path re-verified in DECX | Yes |
| `construction-selected` | exploit mode and support pieces chosen | Yes |
| `code-ready` | PoC project and exploit code prepared | Yes |
| `build-ready` | code appears compile-ready but was not built | Yes |
| `compiled` | `assembleDebug` succeeded | Yes |
| `deployed` | APK installed on a device | Yes |
| `runtime-validated` | exploit effect observed on device/emulator | Yes |

Default ceiling:

- If the user did not ask for compile: stop at `build-ready`
- If the user did not ask for deployment/runtime confirmation: do not claim `deployed` or `runtime-validated`

### Context Rules

- Keep only one active finding in context at a time
- Do not paste the full vuln report into the main thread if only one finding is being implemented
- Load only the one reference file that matches the active component type
- Keep only structured verification notes, not raw source dumps
- References are construction guides, not proof that the final class is compile-safe; generated code must still be self-consistent

### Project Rules

- Create only one PoC app per target app session
- Add one exploit class per finding
- Register every exploit in `ExploitRegistry`
- `applicationId` must stay in the `com.poc.*` namespace
- Keep `allowBackup="false"`
- Add only the permissions, services, activities, and receivers required for the active exploit
- Use `AndroidHiddenApiBypass` only when hidden API access is actually required
- Prefer self-contained PoCs over remote infrastructure when both demonstrate the same effect

### Verification Rules

- Do not code directly from a stale or weak report
- Re-verify the active finding in DECX before building the PoC
- Every `decx code` and `decx ard` command must include `-P <port>`
- If the DECX session is not open, tell the user to run:

```bash
decx process open "<apk-path>" -P <port>
```

- If re-verification contradicts the report, stop and report the mismatch before coding further

## Required Input

Before coding, normalize the active finding into this minimum packet:

```json
{
  "targetApp": "short target name for project directory",
  "packageName": "victim package",
  "componentType": "Activity|Service|Provider|Receiver|Intent|WebView|Framework",
  "componentClass": "victim entry class or interface",
  "vulnType": "specific finding type",
  "entryPoint": "externally reachable method or callback",
  "source": "attacker-controlled input",
  "sink": "security-relevant action",
  "callChain": ["minimal method path"],
  "bypassConditions": ["exact conditions that make exploitation possible"],
  "impactEvidence": "visible effect the PoC should demonstrate",
  "port": 8080
}
```

Construction packet:

```json
{
  "exploitMode": "direct-trigger|interception|returned-handle|hosted-web-content|binder-caller|ui-assisted",
  "supportComponents": ["optional helper activity/service/receiver"],
  "manifestNeeds": ["only what the exploit actually needs"],
  "successSignal": "what the PoC should visibly prove"
}
```

If one of these fields is missing, fill it first from the report or from DECX.

PoC input gate:

- Preferred source: `statically-supported` findings from `decxcli-app-vulnhunt` or `decxcli-framework-vulnhunt`
- `candidate` findings should not become full PoCs unless the user explicitly asks for exploratory probing
- `rejected` findings should not be implemented

## Workflow

Track progress with:

```text
PoC Progress
- [ ] Phase 1: Normalize one finding
- [ ] Phase 2: Re-verify target path in DECX
- [ ] Phase 3: Select exploit mode
- [ ] Phase 4: Create or reuse PoC project
- [ ] Phase 5: Load one component reference
- [ ] Phase 6: Implement exploit class
- [ ] Phase 7: Register exploit and supporting components
- [ ] Phase 8: Optional compile
- [ ] Phase 9: Optional deploy and runtime check
```

### Phase 1 - Normalize One Finding

Goal: reduce the upstream report to one buildable exploit target.

Extract only:

- victim package and class
- component type
- exact exploit trigger
- required extras, actions, URIs, Binder methods, or HTML payload shape
- exact bypass conditions
- visible success signal

Do not carry unrelated findings into the active context.

### Phase 2 - Re-Verify in DECX

Goal: make sure the PoC is built against the real path, not just the report narrative.

Minimum checks:

1. Confirm the component or Binder surface exists
2. Confirm the claimed entry method still matches the report
3. Confirm the source is attacker-controlled
4. Confirm the sink is still reachable
5. Confirm there is no missed non-bypassable guard

Suggested commands:

```bash
decx ard exported-components -P <port>
decx ard app-manifest -P <port>
decx code method-source "<full-method-signature>" -P <port>
decx code class-source "<package.Class>" -P <port>
```

Structured verification output:

```text
- [PASS/FAIL] Surface exists: ...
- [PASS/FAIL] Entry method matches: ...
- [PASS/FAIL] Source is attacker-controlled: ...
- [PASS/FAIL] Sink is reachable: ...
- [PASS/FAIL] No missed non-bypassable guard: ...
```

### Phase 3 - Select Exploit Mode

Choose one exploit mode before writing code.

| Mode | Use for | Typical support |
|------|---------|-----------------|
| `direct-trigger` | exported Activity, Service, Receiver, Provider paths | usually none |
| `interception` | implicit Intent hijack, grant interception, broadcast leak | helper receiver/activity/service |
| `returned-handle` | `PendingIntent`, granted `content://` URI, Binder handle reuse | capture step plus trigger step |
| `hosted-web-content` | WebView bridge, URL bypass, scan-result-driven load | attacker page or local HTML asset |
| `binder-caller` | exported AIDL/Messenger/Framework Binder methods | recreated interface or hidden API access |
| `ui-assisted` | task hijack, clickjacking, lifecycle misuse | helper activity/service and visible UI signal |

Selection rules:

- Prefer the shortest exploit mode that demonstrates the claimed impact
- If two-stage exploitation is required, model it explicitly as `capture -> trigger`
- Do not fake a handle acquisition step that the finding never proved
- Do not fake a remote server when a local asset or visible on-device effect is enough
- If the only realistic validation requires manual setup, keep the code ready and document the manual step instead of over-automating it

### Phase 4 - Create or Reuse the PoC Project

First project creation:

```bash
node skill/decxcli-poc/scripts/setup-poc.mjs <target-app>
```

Expected output:

```text
poc-<target-app>/
```

Project structure:

- `app/src/main/java/com/poc/<target-app>/Exploit.java`
- `app/src/main/java/com/poc/<target-app>/ExploitRegistry.java`
- `app/src/main/java/com/poc/<target-app>/PoCActivity.java`

Reuse rule:

- Reuse the same `poc-<target-app>` project for later findings against the same target
- Add a new exploit class instead of creating a new app

### Phase 5 - Load One Reference

Load only the one reference file matching the active target:

| Component | Reference |
|----------|-----------|
| Activity | `references/poc-app-activity.md` |
| Broadcast / Receiver | `references/poc-app-broadcast.md` |
| Provider | `references/poc-app-provider.md` |
| Service | `references/poc-app-service.md` |
| Intent / grant / mutable handle flows | `references/poc-app-intent.md` |
| WebView | `references/poc-app-webview.md` |
| Framework service | `references/poc-framework-service.md` |
| Base exploit contract | `references/poc-base.md` |

Modern surfaces that must not be skipped:

- mutable `PendingIntent` exposures from notifications, widgets, IPC results, or provider returns
- `ClipData` and URI-grant forwarding
- WebView scan or QR result loaders, `intent://`, custom schemes, message-channel bridges, and file chooser callbacks
- Provider `call()`, `applyBatch()`, `bulkInsert()`, and FileProvider grant chains
- framework Binder paths that require hidden API access

### Phase 6 - Implement the Exploit Class

Create the exploit under:

```text
app/src/main/java/com/poc/<target-app>/exploit/
```

Implementation rules:

- Class name must make the target and vuln type obvious
- Replace every placeholder package, class, action, URI, and extra key with the real target values
- Keep helper logic inside the class unless an existing shared helper already exists in the project
- Reflect the chosen exploit mode in the code shape
- Log a real visible success signal, not a theoretical statement
- For `returned-handle`, separate handle acquisition from handle reuse
- For `hosted-web-content`, prefer a minimal local HTML asset unless the finding specifically depends on a remote origin
- For framework cases, gate hidden-API code to the exact service or method needed

Good success logs:

- `"Launched com.target.InternalAdminActivity through ForwardActivity"`
- `"Read 12 rows from content://.../users"`
- `"PendingIntent send() succeeded with attacker-filled target"`

Bad success logs:

- `"Exploit executed"`
- `"Maybe vulnerable"`
- `"Should lead to privilege escalation"`

### Phase 7 - Register and Wire Supporting Pieces

Always do both:

1. Add the exploit class to `ExploitRegistry`
2. Add any required Manifest declarations for helper components

Common support additions:

- receiver for implicit Intent interception
- activity for task hijack or activity-result capture
- service for overlay or notification observation
- permissions required to trigger the PoC flow

Do not add support components that are unrelated to the active exploit.

### Phase 8 - Optional Compile

Only run build steps if the user explicitly asks for compilation.

Environment check:

```bash
node skill/decxcli-poc/scripts/check-env.mjs
```

If the environment check fails, stop and report the output.

Build command:

```bash
cd poc-<target-app> && timeout 300 ./gradlew assembleDebug --no-daemon
```

If build fails:

- fix the PoC code
- retry
- report the remaining blocker if it still does not compile

### Phase 9 - Optional Deploy and Runtime Check

Only deploy if the user explicitly asks and a device or emulator is available.

Typical commands:

```bash
adb devices
adb install app/build/outputs/apk/debug/app-debug.apk
adb logcat -s PoC:I AndroidRuntime:E
adb uninstall com.poc.<target-app>
```

Runtime validation must name the exact observed effect, for example:

- non-exported activity opened
- protected provider rows returned
- privileged service method accepted the call
- victim WebView loaded attacker-controlled content and exposed bridge behavior

## Final Output Contract

Close with a compact result block containing:

- `state`
- `projectPath`
- `activeFinding`
- `exploitMode`
- `exploitClass`
- `filesChanged`
- `manifestChanges`
- `buildStatus`
- `runtimeStatus`
- `remainingManualSteps`

If the PoC stopped before compile or runtime validation, state that explicitly.
