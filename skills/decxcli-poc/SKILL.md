---
name: decxcli-poc
description: Android exploit PoC construction skill. Turns one DECX-supported finding into one buildable PoC app, with optional compile and adb deployment when explicitly requested.
metadata:
  requires:
    bins: ["node", "decx"]
---

# DECX CLI - Android Exploit PoC

## Goal

Turn one `decxcli-app-vulnhunt` or `decxcli-framework-vulnhunt` finding into one buildable `poc-<target>` project.

Current project shape:

- Android side: minimal app template with `ExploitEntry`, `ExploitRegistry`, and `PoCActivity`
- Web side: local `server/` with `index.html`, `scenario.js`, and `server.mjs`
- Bootstrap path: copy templates, replace placeholders, rename package path segments

Default ceiling:

- stop at `build-ready` unless the user explicitly asks for compile
- do not claim `deployed` or `runtime-validated` unless deployment or runtime proof actually happened

## Hard Rules

- Keep exactly one active finding in context.
- Re-verify the finding in DECX before coding.
- Every `decx code` and `decx ard` command must include `-P <port>`.
- If a command is missing, rejected, or uncertain, run the nearest `--help` command before retrying.
- Before final response, close any DECX session used for PoC work with `decx process close <session-name>` or `decx process close --port <port>`.
- If the DECX session is not open, tell the user to run:

```bash
decx process open "<apk-path>" -P <port>
```

- Reuse one `poc-<target>` project per target; add one exploit id per finding.
- Add only the Manifest entries, helper components, permissions, and server assets required for the active exploit.
- Keep `applicationId` under `com.poc.*` and keep `allowBackup="false"`.
- Use hidden-API access only for framework Binder paths that actually need it.
- If re-verification contradicts the report, stop and report the mismatch.

## Required Input

Normalize the active finding into this packet before coding:

```json
{
  "targetApp": "short target name",
  "packageName": "victim package",
  "componentType": "Activity|Service|Provider|Receiver|Intent|WebView|Framework",
  "componentClass": "victim entry class or interface",
  "vulnType": "specific finding type",
  "entryPoint": "externally reachable method or callback",
  "traceSummary": {
    "sourceComponent": "tracked victim entry component or service interface",
    "possibleIssueTypes": ["current issue-type hypotheses or narrowed class"],
    "analyzedAttackChains": ["already traced victim-side chain summaries"],
    "nextCandidateTargets": ["follow-up targets that are not part of this active PoC"]
  },
  "source": "attacker-controlled input",
  "sink": "security-relevant action",
  "callChain": ["minimal verified method path"],
  "bypassConditions": ["exact conditions that make exploitation possible"],
  "impactEvidence": "visible effect the PoC should prove",
  "port": 8080
}
```

Select construction details up front:

```json
{
  "exploitMode": "direct-trigger|interception|returned-handle|hosted-web-content|binder-caller|ui-assisted",
  "validationStory": "button-only|deeplink|intent-url|scenario-page|background-helper|manual-network",
  "supportComponents": ["optional helper activity/service/receiver"],
  "serverNeeds": ["optional trigger links, hosted HTML, bridge logic, result capture"],
  "manifestNeeds": ["only what the exploit needs"],
  "successSignal": "what the PoC should visibly prove"
}
```

Input gate:

- Preferred source: `statically-supported`
- `candidate` findings require explicit exploratory intent from the user
- `rejected` findings should not become PoCs

## Workflow

Track progress with:

```text
PoC Progress
- [ ] Normalize one finding
- [ ] Re-verify target path in DECX
- [ ] Select exploit mode and validation story
- [ ] Create or reuse PoC project
- [ ] Load one matching reference
- [ ] Implement exploit
- [ ] Register exploit and wire support
- [ ] Optional compile
- [ ] Optional deploy and runtime check
- [ ] Close DECX session
```

### 1. Normalize One Finding

Keep only:

- victim package and class
- component type
- exact trigger shape: action, extras, URI, Binder method, deep link, or HTML payload
- current `traceSummary` so the active source component and already analyzed chain do not get lost during handoff
- exact bypass conditions
- visible success signal

### 2. Re-Verify In DECX

Minimum checks:

1. surface exists
2. entry point still matches
3. source is attacker-controlled
4. sink is still reachable
5. no missed non-bypassable guard exists

Suggested commands:

```bash
decx ard exported-components -P <port>
decx ard app-manifest -P <port>
decx code method-source "<full-method-signature>" -P <port>
decx code class-source "<package.Class>" -P <port>
```

Write the result as:

```text
- [PASS/FAIL] Surface exists: ...
- [PASS/FAIL] Entry point matches: ...
- [PASS/FAIL] Source is attacker-controlled: ...
- [PASS/FAIL] Sink is reachable: ...
- [PASS/FAIL] No missed non-bypassable guard: ...
```

### 3. Select The Construction Shape

Exploit modes:

- `direct-trigger`: exported Activity, Service, Receiver, Provider paths
- `interception`: implicit Intent hijack, ordered broadcast, result or grant capture
- `returned-handle`: mutable `PendingIntent`, granted `content://`, Binder or handle reuse
- `hosted-web-content`: WebView, browser handoff, attacker page, JS bridge
- `binder-caller`: AIDL, Messenger, framework Binder method calls
- `ui-assisted`: task hijack, clickjacking, lifecycle or visible choreography

Validation stories:

- `button-only`: local app button is enough
- `deeplink`: browsable URI is the natural trigger
- `intent-url`: browser-clickable `intent://` or custom-scheme trigger
- `scenario-page`: browser page drives the whole chain
- `background-helper`: helper component must stay active outside the foreground activity
- `manual-network`: SSL/MITM or remote-origin setup is required

Selection rules:

- choose the shortest path that proves the verified impact
- model two-stage exploits explicitly as `capture -> trigger`
- do not invent handle acquisition, remote servers, or helper components the finding did not prove
- prefer the local `server/` payload over remote infrastructure when both prove the same thing

### 4. Create Or Reuse The Project

Bootstrap:

```bash
node skill/decxcli-poc/scripts/setup-poc.mjs <target-app>
```

The script:

- copies `assets/poc-template-app/` to `poc-<target>/app/`
- copies `assets/poc-template-server/` to `poc-<target>/server/`
- replaces placeholder package and project names
- renames placeholder package path segments from `targetapp` to the real target name

Reuse rule:

- reuse the same `poc-<target>` project for later findings against the same target
- add a new exploit id instead of creating a new app

### 5. Load One Matching Reference

Load only one primary reference:

| Surface | Reference |
|---|---|
| Activity | `references/poc-app-activity.md` |
| Broadcast / Receiver | `references/poc-app-broadcast.md` |
| Provider | `references/poc-app-provider.md` |
| Service | `references/poc-app-service.md` |
| Intent / grant / handle | `references/poc-app-intent.md` |
| WebView | `references/poc-app-webview.md` |
| Framework Binder / service | `references/poc-framework-service.md` |
| Shared contract | `references/poc-base.md` |

### 6. Implement The Exploit

Place exploit code under:

```text
app/src/main/java/com/poc/<target-app>/exploit/
```

Implementation rules:

- class name must reflect target plus vuln type
- replace every placeholder package, action, URI, extra key, and method name with verified target values
- keep helper logic local unless a Manifest component is actually required
- log a real proof signal, not a theory statement
- if the story is `deeplink`, `intent-url`, or `scenario-page`, keep `PoCActivity` route handling and `server/public/` artifacts aligned
- if the story is `scenario-page`, fill only the sections the chain needs:
  - Trigger Links
  - Hosted Payload
  - JS Bridge Calls
  - Result Recording

### 7. Register And Wire Support

Always:

1. register the exploit in `ExploitRegistry`
2. add only the exact Manifest or helper changes the exploit needs

Common support pieces:

- helper receiver for interception
- helper activity for task hijack or result capture
- helper service for overlays or long-lived capture
- minimal `server/public/` updates for browser-driven flows

### 8. Optional Compile

Only if the user explicitly asks.

Environment check:

```bash
node skill/decxcli-poc/scripts/check-env.mjs
```

Build:

```bash
cd poc-<target-app>/app && timeout 300 ./gradlew assembleDebug --no-daemon
```

If the build fails:

- fix the PoC code
- retry once the blocker is understood
- report the remaining blocker if it still fails

### 9. Optional Deploy And Runtime Check

Only if the user explicitly asks and a device or emulator is available.

Typical commands:

```bash
adb devices
adb install app/build/outputs/apk/debug/app-debug.apk
adb logcat -s PoC:I AndroidRuntime:E
adb uninstall com.poc.<target-app>
```

Runtime proof must name the exact observed effect, for example:

- non-exported activity opened
- protected provider rows returned
- privileged Binder method accepted the call
- victim WebView loaded attacker content and exposed bridge behavior
- browser-clicked trigger launched the PoC helper and reached the verified target path

### 10. Close DECX Session

Always close the DECX session used for re-verification before the final response:

```bash
decx process close --port <port>
```

If the session was opened by name, close it by name instead.

## Final Output Contract

Close with:

- `state`
- `projectPath`
- `activeFinding`
- `exploitMode`
- `validationStory`
- `exploitClass`
- `filesChanged`
- `manifestChanges`
- `deliveryArtifacts`
- `buildStatus`
- `runtimeStatus`
- `remainingManualSteps`

If the PoC stopped before compile or runtime validation, state that explicitly.
